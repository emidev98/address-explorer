//! Native/server encryption vault.
//!
//! The vault owns only the Argon2id-derived SQLCipher key and the public salt
//! while a database is unlocked. The password is never stored. Locking drops
//! the key, so every later database open fails until the password is entered
//! again.

use super::database::open_connection_with_key;
use super::encryption::{
    derive_key, generate_salt, is_plaintext_sqlite, salt_from_header, EncryptionError, VaultKey,
    SALT_LEN, VAULT_LOCKED_MESSAGE,
};
use super::repositories::settings as settings_repository;
use super::{Database, DatabaseError};
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};
use rusqlite::{Connection, MAIN_DB};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, RwLock};
use std::time::{Duration, Instant};

static VAULT: LazyLock<RwLock<Option<Vault>>> = LazyLock::new(|| RwLock::new(None));
static INIT: Mutex<()> = Mutex::new(());
static TRANSITION: Mutex<()> = Mutex::new(());
static IDLE_STARTED: AtomicBool = AtomicBool::new(false);
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct Vault {
    path: PathBuf,
    mode: VaultMode,
    prompt_seen: bool,
    idle_timeout: IdleTimeout,
    last_activity: Instant,
    active_operations: usize,
}

enum VaultMode {
    Plaintext,
    Locked { salt: [u8; SALT_LEN] },
    Unlocked { key: VaultKey, salt: [u8; SALT_LEN] },
}

impl Vault {
    fn configured(&self) -> bool {
        !matches!(self.mode, VaultMode::Plaintext)
    }

    fn locked(&self) -> bool {
        matches!(self.mode, VaultMode::Locked { .. })
    }

    fn status(&self) -> EncryptionStatus {
        EncryptionStatus {
            configured: self.configured(),
            locked: self.locked(),
            idle_timeout: self.idle_timeout,
            prompt_seen: self.prompt_seen,
        }
    }
}

/// One long-running database operation that must finish before an idle lock.
pub struct OperationGuard;

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut guard) = VAULT.write() {
            if let Some(vault) = guard.as_mut() {
                vault.active_operations = vault.active_operations.saturating_sub(1);
            }
        }
    }
}

/// Initialize the native/server vault once.
pub fn init() -> Result<(), EncryptionError> {
    let _init = INIT.lock().map_err(|_| {
        EncryptionError::Message("the encryption initialization lock was poisoned".to_string())
    })?;
    if VAULT
        .read()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?
        .is_some()
    {
        return Ok(());
    }

    let path = Database::default_path().map_err(EncryptionError::from)?;
    let mut vault = Vault {
        path,
        mode: VaultMode::Plaintext,
        prompt_seen: false,
        idle_timeout: IdleTimeout::default(),
        last_activity: Instant::now(),
        active_operations: 0,
    };

    if vault.path.is_file() {
        let header = read_header(&vault.path)?;
        if !is_plaintext_sqlite(&header) && !header.is_empty() {
            // A truncated or damaged encrypted file must stay locked so the
            // destructive reset path remains the only way forward.
            vault.mode = VaultMode::Locked {
                salt: salt_from_header(&header).unwrap_or([0u8; SALT_LEN]),
            };
            vault.prompt_seen = true;
        } else {
            let path = vault.path.clone();
            load_plaintext_settings(&path, &mut vault)?;
        }
    } else {
        let path = vault.path.clone();
        load_plaintext_settings(&path, &mut vault)?;
    }

    *VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))? =
        Some(vault);
    start_idle_thread();
    Ok(())
}

/// Return the encryption state of the active backend.
pub fn status() -> Result<EncryptionStatus, EncryptionError> {
    init()?;
    let vault = VAULT
        .read()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    vault
        .as_ref()
        .map(Vault::status)
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))
}

/// Open the active database for one request. Locked vaults fail closed.
pub fn open_database() -> Result<Database, DatabaseError> {
    let _transition = TRANSITION
        .lock()
        .map_err(|_| DatabaseError::new("the vault transition lock was poisoned"))?;
    init().map_err(|error| DatabaseError::new(error.to_string()))?;
    let _ = touch();
    let vault = VAULT
        .read()
        .map_err(|_| DatabaseError::new("the vault lock was poisoned"))?;
    let Some(vault) = vault.as_ref() else {
        return Err(DatabaseError::new("the vault is not initialized"));
    };
    match &vault.mode {
        VaultMode::Plaintext => Database::open_path(&vault.path),
        VaultMode::Unlocked { key, salt } => Database::open_path_with_key(&vault.path, key, salt),
        VaultMode::Locked { .. } => Err(DatabaseError::new(VAULT_LOCKED_MESSAGE)),
    }
}

/// Enable encryption for a plaintext database and keep it unlocked.
pub fn setup(password: &str) -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let _operation = begin_operation()?;
    if password.is_empty() {
        return Err(EncryptionError::Message(
            "the password must not be empty".to_string(),
        ));
    }
    let (path, timeout, had_plaintext) = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_ref()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        if vault.configured() {
            return Err(EncryptionError::AlreadyConfigured);
        }
        (vault.path.clone(), vault.idle_timeout, vault.path.is_file())
    };

    let salt = generate_salt()?;
    let key = derive_key(password, &salt)?;
    if had_plaintext {
        convert_plaintext_database(&path, &key, &salt)?;
    } else {
        Database::open_path_with_key(&path, &key, &salt).map_err(EncryptionError::from)?;
    }

    // The file is encrypted and validated at this point. Switch the vault
    // before writing preferences so a settings failure cannot misclassify the
    // encrypted file as plaintext.
    {
        let mut vault = VAULT
            .write()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_mut()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        vault.mode = VaultMode::Unlocked {
            key: key.clone(),
            salt,
        };
        vault.prompt_seen = true;
        vault.idle_timeout = timeout;
        vault.last_activity = Instant::now();
    }

    let database =
        Database::open_path_with_key(&path, &key, &salt).map_err(EncryptionError::from)?;
    settings_repository::save_encryption_prompt_seen(&database, true)
        .map_err(EncryptionError::from)?;
    settings_repository::save_encryption_idle_timeout(&database, timeout)
        .map_err(EncryptionError::from)?;
    drop(database);

    status()
}

/// Unlock an encrypted database and retain only the derived key.
pub fn unlock(password: &str) -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    if password.is_empty() {
        return Err(EncryptionError::WrongPassword);
    }
    let (path, salt) = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_ref()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        match vault.mode {
            VaultMode::Plaintext => return Err(EncryptionError::NotConfigured),
            VaultMode::Unlocked { .. } => return Ok(vault.status()),
            VaultMode::Locked { salt } => (vault.path.clone(), salt),
        }
    };

    let key = derive_key(password, &salt)?;
    let database = Database::open_path_with_key(&path, &key, &salt)
        .map_err(|_| EncryptionError::WrongPassword)?;
    let timeout = settings_repository::load_encryption_idle_timeout(&database)
        .map_err(|_| EncryptionError::WrongPassword)?;
    let prompt_seen = settings_repository::load_encryption_prompt_seen(&database)
        .map_err(|_| EncryptionError::WrongPassword)?;
    drop(database);

    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.mode = VaultMode::Unlocked { key, salt };
    vault.idle_timeout = timeout;
    vault.prompt_seen = prompt_seen;
    vault.last_activity = Instant::now();
    Ok(vault.status())
}

/// Discard the in-memory key and lock an encrypted database.
pub fn lock() -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    if vault.active_operations > 0 {
        return Err(EncryptionError::Message(
            "a database operation is still running; try again when it finishes".to_string(),
        ));
    }
    if let VaultMode::Unlocked { salt, .. } = &vault.mode {
        let salt = *salt;
        vault.mode = VaultMode::Locked { salt };
    }
    vault.active_operations = 0;
    Ok(vault.status())
}

/// Extend the idle timer after user activity or a database request.
pub fn touch() -> Result<(), EncryptionError> {
    init()?;
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let Some(vault) = vault.as_mut() else {
        return Ok(());
    };
    if matches!(vault.mode, VaultMode::Unlocked { .. }) {
        vault.last_activity = Instant::now();
    }
    Ok(())
}

/// Register one long operation so the idle watchdog cannot lock mid-write.
pub fn begin_operation() -> Result<OperationGuard, EncryptionError> {
    init()?;
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.active_operations = vault.active_operations.saturating_add(1);
    Ok(OperationGuard)
}

/// Change the password by re-encrypting the database with a new raw key.
pub fn change_password(
    current_password: &str,
    new_password: &str,
) -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let _operation = begin_operation()?;
    if current_password.is_empty() || new_password.is_empty() {
        return Err(EncryptionError::Message(
            "the password must not be empty".to_string(),
        ));
    }
    let (path, current_key, current_salt) = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_ref()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        match &vault.mode {
            VaultMode::Locked { .. } => return Err(EncryptionError::Locked),
            VaultMode::Plaintext => return Err(EncryptionError::NotConfigured),
            VaultMode::Unlocked { key, salt } => (vault.path.clone(), key.clone(), *salt),
        }
    };

    let candidate = derive_key(current_password, &current_salt)?;
    if !current_key.matches(&candidate) {
        return Err(EncryptionError::WrongPassword);
    }
    let new_salt = generate_salt()?;
    let new_key = derive_key(new_password, &new_salt)?;

    let safety = temporary_path(&path, "rekey");
    fs::copy(&path, &safety).map_err(EncryptionError::from)?;

    let result = (|| -> Result<(), EncryptionError> {
        let connection = open_connection_with_key(&path, &current_key, &current_salt)
            .map_err(EncryptionError::from)?;
        let spec = zeroize::Zeroizing::new(new_key.key_spec(&new_salt));
        connection
            .pragma_update(None, "rekey", spec.as_str())
            .map_err(EncryptionError::from)?;
        drop(connection);

        let database = Database::open_path_with_key(&path, &new_key, &new_salt).map_err(|_| {
            EncryptionError::Message("the password change could not be verified".to_string())
        })?;
        let connection = database.connection().map_err(EncryptionError::from)?;
        super::backup::validate_current_schema(&connection).map_err(EncryptionError::from)?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path_sidecar(&path, "-wal"));
        let _ = fs::remove_file(path_sidecar(&path, "-shm"));
        let _ = fs::remove_file(path_sidecar(&path, "-journal"));
        if let Err(restore_error) = fs::copy(&safety, &path) {
            return Err(EncryptionError::Message(format!(
                "{error}; the previous database could not be restored: {restore_error}"
            )));
        }
        let _ = fs::remove_file(&safety);
        return Err(error);
    }
    let _ = fs::remove_file(&safety);

    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.mode = VaultMode::Unlocked {
        key: new_key,
        salt: new_salt,
    };
    vault.last_activity = Instant::now();
    Ok(vault.status())
}

/// Persist the idle-lock preference and apply it immediately.
pub fn save_idle_timeout(timeout: IdleTimeout) -> Result<EncryptionStatus, EncryptionError> {
    init()?;
    let database = open_database().map_err(|_| EncryptionError::Locked)?;
    settings_repository::save_encryption_idle_timeout(&database, timeout)
        .map_err(EncryptionError::from)?;
    drop(database);

    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.idle_timeout = timeout;
    let unlocked_salt = match &vault.mode {
        VaultMode::Unlocked { salt, .. } => Some(*salt),
        _ => None,
    };
    if let (Some(salt), Some(seconds)) = (unlocked_salt, timeout.as_secs()) {
        if vault.last_activity.elapsed() >= Duration::from_secs(seconds) {
            vault.mode = VaultMode::Locked { salt };
            vault.active_operations = 0;
        }
    }
    Ok(vault.status())
}

/// Import a plaintext SQLite backup into the active database.
///
/// The live file is replaced only after the imported database has passed
/// schema, integrity, and foreign-key validation. An encrypted vault wraps the
/// validated plaintext in a new SQLCipher file with the existing key.
pub fn import_plaintext(bytes: &[u8]) -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let _operation = begin_operation()?;
    if bytes.is_empty() {
        return Err(EncryptionError::Message(
            "the selected SQLite file is empty".to_string(),
        ));
    }
    if !super::encryption::is_plaintext_sqlite(bytes) {
        return Err(EncryptionError::Message(
            "the selected file is not a plaintext SQLite database".to_string(),
        ));
    }
    let (path, key, salt, configured) = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_ref()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        match &vault.mode {
            VaultMode::Locked { .. } => return Err(EncryptionError::Locked),
            VaultMode::Plaintext => (vault.path.clone(), None, None, false),
            VaultMode::Unlocked { key, salt } => {
                (vault.path.clone(), Some(key.clone()), Some(*salt), true)
            }
        }
    };

    let mut connection = Connection::open_in_memory().map_err(EncryptionError::from)?;
    connection
        .deserialize_read_exact(MAIN_DB, Cursor::new(bytes), bytes.len(), false)
        .map_err(EncryptionError::from)?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(EncryptionError::from)?;
    super::backup::precheck_connection(&connection).map_err(EncryptionError::from)?;
    super::migrations::run_migrations(&mut connection, true).map_err(EncryptionError::from)?;
    super::backup::integrity_check(&connection).map_err(EncryptionError::from)?;
    super::backup::validate_current_schema(&connection).map_err(EncryptionError::from)?;
    super::backup::foreign_key_check(&connection).map_err(EncryptionError::from)?;
    let imported = Database::from_connection(connection);
    let timeout = settings_repository::load_encryption_idle_timeout(&imported)
        .map_err(EncryptionError::from)?;
    let prompt_seen = settings_repository::load_encryption_prompt_seen(&imported)
        .map_err(EncryptionError::from)?;

    if configured {
        let (key, salt) = match (key, salt) {
            (Some(key), Some(salt)) => (key, salt),
            _ => return Err(EncryptionError::Locked),
        };
        let encrypted = temporary_path(&path, "import-enc");
        {
            let source = imported.connection().map_err(EncryptionError::from)?;
            let result = export_encrypted_copy(&source, &encrypted, &key, &salt);
            drop(source);
            result?;
        }
        drop(imported);
        validate_encrypted_file(&encrypted, &key, &salt)?;
        replace_file(&encrypted, &path)?;
        let database =
            Database::open_path_with_key(&path, &key, &salt).map_err(EncryptionError::from)?;
        settings_repository::save_encryption_prompt_seen(&database, true)
            .map_err(EncryptionError::from)?;
        settings_repository::save_encryption_idle_timeout(&database, timeout)
            .map_err(EncryptionError::from)?;
        drop(database);
        let mut vault = VAULT
            .write()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_mut()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        vault.idle_timeout = timeout;
        vault.prompt_seen = true;
        vault.last_activity = Instant::now();
        return Ok(vault.status());
    }

    let plaintext = temporary_path(&path, "import-plain");
    {
        let source = imported.connection().map_err(EncryptionError::from)?;
        let data = source.serialize(MAIN_DB).map_err(EncryptionError::from)?;
        if let Err(error) = fs::write(&plaintext, &*data) {
            let _ = fs::remove_file(&plaintext);
            return Err(EncryptionError::from(error));
        }
    }
    drop(imported);
    replace_file(&plaintext, &path)?;
    let database = Database::open_path(&path).map_err(EncryptionError::from)?;
    settings_repository::save_encryption_idle_timeout(&database, timeout)
        .map_err(EncryptionError::from)?;
    drop(database);
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.idle_timeout = timeout;
    vault.prompt_seen = prompt_seen;
    vault.last_activity = Instant::now();
    Ok(vault.status())
}

/// Replace the active database with a fresh migrated and seeded database,
/// preserving encryption and the current password when enabled.
pub fn reset_to_defaults() -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let _operation = begin_operation()?;
    let (path, key, salt, configured) = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_ref()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        match &vault.mode {
            VaultMode::Locked { .. } => return Err(EncryptionError::Locked),
            VaultMode::Plaintext => (vault.path.clone(), None, None, false),
            VaultMode::Unlocked { key, salt } => {
                (vault.path.clone(), Some(key.clone()), Some(*salt), true)
            }
        }
    };

    if configured {
        let (key, salt) = match (key, salt) {
            (Some(key), Some(salt)) => (key, salt),
            _ => return Err(EncryptionError::Locked),
        };
        let mut connection = Connection::open_in_memory().map_err(EncryptionError::from)?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(EncryptionError::from)?;
        super::migrations::run_migrations(&mut connection, true).map_err(EncryptionError::from)?;
        super::backup::validate_current_schema(&connection).map_err(EncryptionError::from)?;
        super::backup::foreign_key_check(&connection).map_err(EncryptionError::from)?;
        let encrypted = temporary_path(&path, "reset-enc");
        {
            let result = export_encrypted_copy(&connection, &encrypted, &key, &salt);
            drop(connection);
            result?;
        }
        validate_encrypted_file(&encrypted, &key, &salt)?;
        replace_file(&encrypted, &path)?;
        let database =
            Database::open_path_with_key(&path, &key, &salt).map_err(EncryptionError::from)?;
        settings_repository::save_encryption_prompt_seen(&database, true)
            .map_err(EncryptionError::from)?;
        settings_repository::save_encryption_idle_timeout(&database, IdleTimeout::default())
            .map_err(EncryptionError::from)?;
        drop(database);
        let mut vault = VAULT
            .write()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_mut()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        vault.idle_timeout = IdleTimeout::default();
        vault.prompt_seen = true;
        vault.last_activity = Instant::now();
        return Ok(vault.status());
    }

    let fresh = temporary_path(&path, "reset-plain");
    {
        let database = Database::open_path(&fresh).map_err(EncryptionError::from)?;
        let connection = database.connection().map_err(EncryptionError::from)?;
        super::backup::validate_current_schema(&connection).map_err(EncryptionError::from)?;
    }
    replace_file(&fresh, &path)?;
    let database = Database::open_path(&path).map_err(EncryptionError::from)?;
    settings_repository::save_encryption_prompt_seen(&database, false)
        .map_err(EncryptionError::from)?;
    settings_repository::save_encryption_idle_timeout(&database, IdleTimeout::default())
        .map_err(EncryptionError::from)?;
    drop(database);
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.idle_timeout = IdleTimeout::default();
    vault.prompt_seen = false;
    vault.last_activity = Instant::now();
    Ok(vault.status())
}

/// Record the user's answer to the first-run encryption prompt.
pub fn dismiss_prompt() -> Result<EncryptionStatus, EncryptionError> {
    init()?;
    let database = open_database().map_err(|_| EncryptionError::Locked)?;
    settings_repository::save_encryption_prompt_seen(&database, true)
        .map_err(EncryptionError::from)?;
    drop(database);
    let mut vault = VAULT
        .write()
        .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
    let vault = vault
        .as_mut()
        .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
    vault.prompt_seen = true;
    Ok(vault.status())
}

/// Erase an encrypted database and create a fresh plaintext one.
///
/// This is the only lost-password path. It never keeps a recovery copy or
/// attempts to decrypt anything.
pub fn reset() -> Result<EncryptionStatus, EncryptionError> {
    let _transition = TRANSITION.lock().map_err(|_| {
        EncryptionError::Message("the vault transition lock was poisoned".to_string())
    })?;
    init()?;
    let _operation = begin_operation()?;
    let path = {
        let vault = VAULT
            .read()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        vault
            .as_ref()
            .map(|vault| vault.path.clone())
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?
    };
    remove_database_files(&path)?;
    let database = Database::open_path(&path).map_err(EncryptionError::from)?;
    {
        let mut vault = VAULT
            .write()
            .map_err(|_| EncryptionError::Message("the vault lock was poisoned".to_string()))?;
        let vault = vault
            .as_mut()
            .ok_or_else(|| EncryptionError::Message("the vault is not initialized".to_string()))?;
        vault.mode = VaultMode::Plaintext;
        vault.prompt_seen = false;
        vault.idle_timeout = IdleTimeout::default();
        vault.last_activity = Instant::now();
        vault.active_operations = 0;
    }
    settings_repository::save_encryption_prompt_seen(&database, false)
        .map_err(EncryptionError::from)?;
    settings_repository::save_encryption_idle_timeout(&database, IdleTimeout::default())
        .map_err(EncryptionError::from)?;
    drop(database);
    status()
}

fn start_idle_thread() {
    if IDLE_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("address-explorer-vault".to_string())
        .spawn(|| loop {
            std::thread::sleep(Duration::from_secs(1));
            idle_tick();
        });
}

fn idle_tick() {
    let Ok(mut vault) = VAULT.write() else {
        return;
    };
    let Some(vault) = vault.as_mut() else {
        return;
    };
    if vault.active_operations > 0 {
        return;
    }
    let VaultMode::Unlocked { salt, .. } = &vault.mode else {
        return;
    };
    let Some(seconds) = vault.idle_timeout.as_secs() else {
        return;
    };
    if vault.last_activity.elapsed() >= Duration::from_secs(seconds) {
        let salt = *salt;
        vault.mode = VaultMode::Locked { salt };
        vault.active_operations = 0;
    }
}

fn read_header(path: &Path) -> Result<Vec<u8>, EncryptionError> {
    let mut file = fs::File::open(path).map_err(EncryptionError::from)?;
    let mut header = vec![0u8; SALT_LEN];
    let read = file.read(&mut header).map_err(EncryptionError::from)?;
    header.truncate(read);
    Ok(header)
}

fn load_plaintext_settings(path: &Path, vault: &mut Vault) -> Result<(), EncryptionError> {
    let database = Database::open_path(path).map_err(EncryptionError::from)?;
    vault.prompt_seen = settings_repository::load_encryption_prompt_seen(&database)
        .map_err(EncryptionError::from)?;
    vault.idle_timeout = settings_repository::load_encryption_idle_timeout(&database)
        .map_err(EncryptionError::from)?;
    Ok(())
}

fn convert_plaintext_database(
    path: &Path,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), EncryptionError> {
    let source = Database::open_path(path).map_err(EncryptionError::from)?;
    let temporary = temporary_path(path, "encrypt");
    let _ = fs::remove_file(&temporary);
    {
        let source_connection = source.connection().map_err(EncryptionError::from)?;
        export_encrypted_copy(&source_connection, &temporary, key, salt)?;
    }
    drop(source);
    validate_encrypted_file(&temporary, key, salt)?;
    replace_file(&temporary, path)?;
    Ok(())
}

/// Copy an in-memory/file plaintext connection into a new SQLCipher file.
///
/// SQLCipher's `sqlcipher_export` creates the target schema and data while
/// writing through the codec; no plaintext temporary database is needed.
fn export_encrypted_copy(
    source: &Connection,
    destination: &Path,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), EncryptionError> {
    let _ = fs::remove_file(destination);
    let escaped = destination.to_string_lossy().replace('\'', "''");
    let spec = zeroize::Zeroizing::new(key.key_spec(salt));
    let version: i32 = source
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(EncryptionError::from)?;
    let sql = zeroize::Zeroizing::new(format!(
        "ATTACH DATABASE '{}' AS encrypted KEY \"{}\";",
        escaped,
        spec.as_str()
    ));
    source.execute_batch(&sql).map_err(EncryptionError::from)?;
    let result = (|| -> Result<(), EncryptionError> {
        source
            .execute_batch("SELECT sqlcipher_export('encrypted');")
            .map_err(EncryptionError::from)?;
        source
            .pragma_update(Some("encrypted"), "user_version", version)
            .map_err(EncryptionError::from)?;
        source
            .execute_batch("DETACH DATABASE encrypted;")
            .map_err(EncryptionError::from)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = source.execute_batch("DETACH DATABASE encrypted;");
    }
    result
}

fn validate_encrypted_file(
    path: &Path,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), EncryptionError> {
    let database = Database::open_path_with_key(path, key, salt).map_err(EncryptionError::from)?;
    let connection = database.connection().map_err(EncryptionError::from)?;
    super::backup::integrity_check(&connection).map_err(EncryptionError::from)?;
    super::backup::validate_current_schema(&connection).map_err(EncryptionError::from)?;
    super::backup::foreign_key_check(&connection).map_err(EncryptionError::from)?;
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), EncryptionError> {
    #[cfg(windows)]
    {
        if destination.exists() {
            fs::remove_file(destination).map_err(EncryptionError::from)?;
        }
    }
    fs::rename(source, destination).map_err(EncryptionError::from)?;
    remove_sidecars(source);
    remove_sidecars(destination);
    Ok(())
}

fn remove_database_files(path: &Path) -> Result<(), EncryptionError> {
    if path.exists() {
        fs::remove_file(path).map_err(EncryptionError::from)?;
    }
    remove_sidecars(path);
    Ok(())
}

fn remove_sidecars(path: &Path) {
    for suffix in ["-wal", "-shm", "-journal"] {
        let _ = fs::remove_file(path_sidecar(path, suffix));
    }
}

fn path_sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn temporary_path(path: &Path, tag: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(
        ".address-explorer-{tag}-{}-{count}.sqlite",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use rusqlite::Connection;

    fn test_path(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "address-explorer-vault-test-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap(); // allow-unwrap: test directory.
        directory.join("app.db")
    }

    #[test]
    fn full_vault_lifecycle_never_falls_back_to_plaintext() {
        let directory = std::env::temp_dir().join(format!(
            "address-explorer-vault-lifecycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap(); // allow-unwrap: test directory.
        let path = directory.join("app.db");
        let _path_guard = super::super::database::DEFAULT_PATH_TEST_LOCK
            .lock()
            .unwrap(); // allow-unwrap: test setup.
        let previous_path = std::env::var_os("DATABASE_PATH");
        std::env::set_var("DATABASE_PATH", &path);
        init().unwrap(); // allow-unwrap: test setup.
        assert!(!status().unwrap().configured); // allow-unwrap: test setup.
        let configured = setup("first password").unwrap(); // allow-unwrap: test setup.
        assert!(configured.configured);
        assert!(!configured.locked);
        assert_eq!(configured.idle_timeout, IdleTimeout::OneHour);
        assert!(setup("another password").is_err());
        let encrypted_header = fs::read(&path).unwrap(); // allow-unwrap: test setup.
        assert!(import_plaintext(&encrypted_header).is_err());

        {
            let database = open_database().unwrap(); // allow-unwrap: test setup.
            assert!(settings_repository::save_encryption_prompt_seen(&database, true).is_ok());
        }
        let backup = super::super::backup::export_to_bytes(&open_database().unwrap()).unwrap(); // allow-unwrap: test setup.
        assert!(super::super::encryption::is_plaintext_sqlite(&backup));
        assert!(!backup
            .windows(b"first password".len())
            .any(|window| window == b"first password"));
        let encrypted_bytes = fs::read(&path).unwrap(); // allow-unwrap: test setup.
        assert!(!encrypted_bytes
            .windows(b"first password".len())
            .any(|window| window == b"first password"));
        lock().unwrap(); // allow-unwrap: test setup.
        assert!(open_database().is_err());
        assert!(unlock("wrong password").is_err());
        assert!(!open_database().is_ok());
        unlock("first password").unwrap(); // allow-unwrap: test setup.
        assert!(open_database().is_ok());
        change_password("first password", "second password").unwrap(); // allow-unwrap: test setup.
        lock().unwrap(); // allow-unwrap: test setup.
        assert!(unlock("first password").is_err());
        unlock("second password").unwrap(); // allow-unwrap: test setup.
        import_plaintext(&backup).unwrap(); // allow-unwrap: test setup.
        assert!(open_database().is_ok());
        reset_to_defaults().unwrap(); // allow-unwrap: test setup.
        assert!(status().unwrap().configured); // allow-unwrap: test setup.
        reset().unwrap(); // allow-unwrap: test setup.
        let reset_status = status().unwrap(); // allow-unwrap: test setup.
        assert!(!reset_status.configured);
        assert!(!reset_status.prompt_seen);
        if let Some(previous) = previous_path {
            std::env::set_var("DATABASE_PATH", previous);
        } else {
            std::env::remove_var("DATABASE_PATH");
        }
        let _ = fs::remove_dir_all(&directory);
    }

    #[test]
    fn plaintext_conversion_and_reopen_with_key() {
        let path = test_path("conversion");
        {
            let mut connection = Connection::open(&path).unwrap(); // allow-unwrap: test setup.
            migrations::run_migrations(&mut connection, false).unwrap(); // allow-unwrap: test setup.
            connection
                .execute(
                    "INSERT INTO setting (key, value) VALUES ('theme', 'forest')",
                    [],
                )
                .unwrap(); // allow-unwrap: test setup.
        }
        let source = Database::open_path(&path).unwrap(); // allow-unwrap: test setup.
        assert!(!source.path().unwrap().to_string_lossy().is_empty()); // allow-unwrap: test setup.
        drop(source);

        let salt = [9u8; SALT_LEN];
        let key = VaultKey::new([5u8; 32]);
        convert_plaintext_database(&path, &key, &salt).unwrap(); // allow-unwrap: test setup.

        let bytes = fs::read(&path).unwrap(); // allow-unwrap: test setup.
        assert!(!is_plaintext_sqlite(&bytes));
        assert_eq!(salt_from_header(&bytes).unwrap(), salt); // allow-unwrap: test setup.
        let database = Database::open_path_with_key(&path, &key, &salt).unwrap(); // allow-unwrap: test setup.
        let value: String = database
            .connection()
            .unwrap() // allow-unwrap: test setup.
            .query_row("SELECT value FROM setting WHERE key = 'theme'", [], |row| {
                row.get(0)
            })
            .unwrap(); // allow-unwrap: test setup.
        assert_eq!(value, "forest");
        let _ = fs::remove_dir_all(path.parent().unwrap()); // allow-unwrap: test setup.
    }
}
