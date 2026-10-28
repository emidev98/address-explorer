//! Settings service: load and persist the user's preferences — the theme
//! palette and appearance mode, and the default fiat currency prices are
//! displayed in.

use crate::data::repositories::settings as settings_repository;
use crate::data::{Database, DatabaseError};
use crate::domain::price::{normalize_symbol, PriceError};
use crate::domain::theme::ThemePreference;

/// Failures of the settings service: database access and invalid preference
/// values rejected at the domain boundary.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Price(#[from] PriceError),
}

/// Read the persisted theme preference, with defaults for missing values.
pub fn load_theme(database: &Database) -> Result<ThemePreference, DatabaseError> {
    settings_repository::load(database)
}

/// Persist the selected theme preference as one logical change.
pub fn save_theme(database: &Database, preference: &ThemePreference) -> Result<(), DatabaseError> {
    settings_repository::save(database, preference)
}

/// Read the persisted default fiat currency, USD when never configured.
pub fn load_default_fiat_currency(database: &Database) -> Result<String, DatabaseError> {
    settings_repository::load_default_fiat_currency(database)
}

/// Validate and persist the default fiat currency, returning the normalized
/// symbol that was stored.
pub fn save_default_fiat_currency(
    database: &Database,
    symbol: &str,
) -> Result<String, SettingsError> {
    let symbol = normalize_symbol(symbol)?;
    settings_repository::save_default_fiat_currency(database, &symbol)?;
    Ok(symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fiat_currency_defaults_to_usd() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "USD");
    }

    #[test]
    fn save_default_fiat_currency_normalizes_and_persists() {
        let database = Database::open_in_memory().unwrap();
        let saved = save_default_fiat_currency(&database, " eur ").unwrap();
        assert_eq!(saved, "EUR");
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "EUR");
    }

    #[test]
    fn save_default_fiat_currency_rejects_invalid_symbols() {
        let database = Database::open_in_memory().unwrap();
        for invalid in ["", "us", "U$D"] {
            assert!(matches!(
                save_default_fiat_currency(&database, invalid),
                Err(SettingsError::Price(PriceError::InvalidSymbol(_)))
            ));
        }
        // A rejected value must not disturb the stored preference.
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "USD");
    }
}
