//! Settings service: load and persist the user's preferences — the theme
//! palette and appearance mode, the default fiat currency prices are displayed
//! in, exchange safety/scoring settings, interface hints, and the primary
//! navigation placement.
use crate::data::repositories::settings as settings_repository;
use crate::data::{Database, DatabaseError};
use crate::domain::exchange::OnChainLinkScoringSettings;
use crate::domain::interface_settings::{InterfaceSettings, NavigationPosition};
use crate::domain::price::{normalize_default_symbol, normalize_symbol, PriceError};
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
///
/// The stored value is normalized here because the setting is displayed and
/// passed to price services by several screens; a manually edited lowercase
/// code must not leak into labels or currency formatting.
pub fn load_default_fiat_currency(database: &Database) -> Result<String, DatabaseError> {
    let symbol = settings_repository::load_default_fiat_currency(database)?;
    Ok(normalize_default_symbol(&symbol))
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
pub fn load_on_chain_link_scoring(
    database: &Database,
) -> Result<OnChainLinkScoringSettings, DatabaseError> {
    settings_repository::load_on_chain_link_scoring(database)
}
pub fn save_confirm_destructive_actions(
    database: &Database,
    enabled: bool,
) -> Result<(), DatabaseError> {
    settings_repository::save_confirm_destructive_actions(database, enabled)
}
pub fn save_on_chain_link_scoring(
    database: &Database,
    settings: &OnChainLinkScoringSettings,
) -> Result<OnChainLinkScoringSettings, DatabaseError> {
    settings_repository::save_on_chain_link_scoring(database, settings)
}
pub fn load_interface_settings(database: &Database) -> Result<InterfaceSettings, DatabaseError> {
    settings_repository::load_interface_settings(database)
}
pub fn save_show_date_range_support(
    database: &Database,
    enabled: bool,
) -> Result<(), DatabaseError> {
    settings_repository::save_show_date_range_support(database, enabled)
}
pub fn save_navigation_position(
    database: &Database,
    position: NavigationPosition,
) -> Result<(), DatabaseError> {
    settings_repository::save_navigation_position(database, position)
}
pub fn save_side_nav_collapsed(database: &Database, collapsed: bool) -> Result<(), DatabaseError> {
    settings_repository::save_side_nav_collapsed(database, collapsed)
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
    fn load_default_fiat_currency_normalizes_legacy_values() {
        let database = Database::open_in_memory().unwrap();
        settings_repository::save_default_fiat_currency(&database, " eur ").unwrap();
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "EUR");
        settings_repository::save_default_fiat_currency(&database, "  ").unwrap();
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
            ),);
        }
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "USD");
    }
}
