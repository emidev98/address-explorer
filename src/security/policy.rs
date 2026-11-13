//! Active security policy.
//!
//! Before a database is unlocked the compiled defaults are active. Unlocking
//! replaces them with the values stored in the active database. Saving from
//! General Settings writes the database row and swaps the in-memory policy in
//! the same operation.

use crate::data::repositories::settings as settings_repository;
use crate::data::{Database, DatabaseError};
use crate::domain::security_settings::{SecuritySettings, SecuritySettingsError};
use std::sync::{Arc, LazyLock, RwLock};
use tokio::sync::Semaphore;

/// Whether this build is the standalone network server, where the inactivity
/// timeout is capped at one hour.
#[allow(unexpected_cfgs, dead_code)]
pub const fn is_network_server() -> bool {
    cfg!(all(
        feature = "server",
        not(target_arch = "wasm32"),
        not(any(feature = "desktop", feature = "mobile"))
    ))
}

static CURRENT: LazyLock<RwLock<SecuritySettings>> =
    LazyLock::new(|| RwLock::new(SecuritySettings::default()));
static ARGON2_GATE: LazyLock<RwLock<Arc<Semaphore>>> = LazyLock::new(|| {
    let permits = SecuritySettings::default().max_concurrent_argon2 as usize;
    RwLock::new(Arc::new(Semaphore::new(permits)))
});

/// The policy that currently applies to this process.
pub fn current() -> SecuritySettings {
    read().clone()
}

/// Replace the active policy and rebuild the Argon2 concurrency gate.
pub fn replace(settings: SecuritySettings) {
    let permits = settings.max_concurrent_argon2.max(1) as usize;
    if let Ok(mut gate) = ARGON2_GATE.write() {
        *gate = Arc::new(Semaphore::new(permits));
    }
    if let Ok(mut current) = CURRENT.write() {
        *current = settings;
    }
}

/// Reset the in-memory policy to the recommended defaults. Called when the
/// vault locks so unauthenticated requests always use the safe baseline.
pub fn reset_to_defaults() {
    replace(SecuritySettings::default());
}

/// Load the policy from an unlocked database, seeding defaults when the row is
/// missing or malformed. The normalized values are written back so General
/// Settings always has a concrete row to edit.
pub fn load_from_database(database: &Database) -> Result<SecuritySettings, DatabaseError> {
    let settings = settings_repository::load_security_settings(database)?.unwrap_or_default();
    settings_repository::save_security_settings(database, &settings)?;
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(parent) = database.path().and_then(std::path::Path::parent) {
        crate::security::files::cleanup_stale_temp_files(parent, settings.stale_temp_cleanup_hours);
    }
    replace(settings.clone());
    Ok(settings)
}

/// Validate, persist, and activate a policy saved from General Settings.
pub fn save_to_database(
    database: &Database,
    settings: &SecuritySettings,
) -> Result<SecuritySettings, SecuritySettingsError> {
    settings.validate()?;
    settings_repository::save_security_settings(database, settings)
        .map_err(|error| SecuritySettingsError::Message(error.to_string()))?;
    replace(settings.clone());
    Ok(settings.clone())
}

/// Snapshot of the Argon2 concurrency gate. Operations hold a clone of the
/// semaphore for their lifetime, so changing the policy cannot deadlock an
/// operation that already started.
#[allow(dead_code)]
pub fn argon2_gate() -> Arc<Semaphore> {
    match ARGON2_GATE.read() {
        Ok(gate) => Arc::clone(&gate),
        Err(poisoned) => Arc::clone(&poisoned.into_inner()),
    }
}

fn read() -> std::sync::RwLockReadGuard<'static, SecuritySettings> {
    match CURRENT.read() {
        Ok(current) => current,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn saving_validates_and_reloads() {
        let _guard = TEST_LOCK.lock().expect("policy test lock"); // allow-unwrap: test setup.
        let database = Database::open_in_memory().expect("database"); // allow-unwrap: test setup.
        let initial = load_from_database(&database).expect("load"); // allow-unwrap: test setup.
        assert_eq!(initial, SecuritySettings::default());

        let mut next = initial;
        next.password_min_length = 16;
        next.max_concurrent_argon2 = 2;
        let saved = save_to_database(&database, &next).expect("save"); // allow-unwrap: test setup.
        assert_eq!(saved.password_min_length, 16);
        assert_eq!(current().max_concurrent_argon2, 2);

        reset_to_defaults();
        assert_eq!(current(), SecuritySettings::default());
    }

    #[test]
    fn unsafe_values_are_rejected_when_saving() {
        let _guard = TEST_LOCK.lock().expect("policy test lock"); // allow-unwrap: test setup.
        let database = Database::open_in_memory().expect("database"); // allow-unwrap: test setup.
        let settings = SecuritySettings {
            password_min_length: 4,
            ..SecuritySettings::default()
        };
        assert!(save_to_database(&database, &settings).is_err());
    }
}
