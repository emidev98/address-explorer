//! SQL access to the `setting` key/value table, holding the theme preference,
//! the default fiat currency, and interface preferences.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::data::{Database, DatabaseError};
use crate::domain::exchange::OnChainLinkScoringSettings;
use crate::domain::interface_settings::{InterfaceSettings, NavigationPosition};
use crate::domain::price::USD_SYMBOL;
use crate::domain::theme::{ThemeMode, ThemePalette, ThemePreference};

const PALETTE_KEY: &str = "theme";
const MODE_KEY: &str = "mode";
const DEFAULT_FIAT_CURRENCY_KEY: &str = "default_fiat_currency";
const CONFIRM_DESTRUCTIVE_KEY: &str = "confirm_destructive_actions";
const ON_CHAIN_SCORING_KEY: &str = "on_chain_link_scoring";
const SHOW_DATE_RANGE_SUPPORT_KEY: &str = "show_date_range_support";
const NAVIGATION_POSITION_KEY: &str = "navigation_position";
const SIDE_NAV_COLLAPSED_KEY: &str = "side_nav_collapsed";
/// Key used by the earlier Flow-only helper preference. It is read as a
/// fallback so an existing preference survives the generic rename; saves
/// always write [`SHOW_DATE_RANGE_SUPPORT_KEY`].
const LEGACY_SHOW_FLOW_DATE_RANGE_HELPER_KEY: &str = "show_flow_date_range_helper";

/// Load the stored theme, falling back to defaults for missing or
/// unrecognized values.
pub fn load(database: &Database) -> Result<ThemePreference, DatabaseError> {
    let connection = database.connection()?;

    let palette = read_setting(&connection, PALETTE_KEY)?
        .and_then(|value| ThemePalette::from_value(&value))
        .unwrap_or_default();
    let mode = read_setting(&connection, MODE_KEY)?
        .and_then(|value| ThemeMode::from_value(&value))
        .unwrap_or_default();

    Ok(ThemePreference { palette, mode })
}

/// Store the palette and mode as one logical change.
pub fn save(database: &Database, preference: &ThemePreference) -> Result<(), DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    write_setting(&transaction, PALETTE_KEY, preference.palette.as_str())?;
    write_setting(&transaction, MODE_KEY, preference.mode.as_str())?;

    transaction.commit()?;
    Ok(())
}

/// Load the default fiat currency, falling back to USD when it was never
/// configured. The stored value is returned verbatim; symbol validation is a
/// service concern.
pub fn load_default_fiat_currency(database: &Database) -> Result<String, DatabaseError> {
    let connection = database.connection()?;
    Ok(read_setting(&connection, DEFAULT_FIAT_CURRENCY_KEY)?
        .unwrap_or_else(|| USD_SYMBOL.to_string()))
}

/// Store the default fiat currency.
pub fn save_default_fiat_currency(database: &Database, symbol: &str) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    write_setting(&connection, DEFAULT_FIAT_CURRENCY_KEY, symbol)
}

/// Load the on-chain matching weights and tolerances.
pub fn load_on_chain_link_scoring(
    database: &Database,
) -> Result<OnChainLinkScoringSettings, DatabaseError> {
    let connection = database.connection()?;
    Ok(read_setting(&connection, ON_CHAIN_SCORING_KEY)?
        .and_then(|value| serde_json::from_str::<OnChainLinkScoringSettings>(&value).ok())
        .unwrap_or_default()
        .normalized())
}

pub fn save_confirm_destructive_actions(
    database: &Database,
    enabled: bool,
) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    write_setting(
        &connection,
        CONFIRM_DESTRUCTIVE_KEY,
        if enabled { "true" } else { "false" },
    )
}

pub fn save_on_chain_link_scoring(
    database: &Database,
    settings: &OnChainLinkScoringSettings,
) -> Result<OnChainLinkScoringSettings, DatabaseError> {
    let settings = settings.normalized();
    let value =
        serde_json::to_string(&settings).map_err(|error| DatabaseError::new(error.to_string()))?;
    let connection = database.connection()?;
    write_setting(&connection, ON_CHAIN_SCORING_KEY, &value)?;
    Ok(settings)
}

/// Load interface preferences, falling back to their defaults for missing or
/// malformed values (supporting text hidden, automatic navigation).
pub fn load_interface_settings(database: &Database) -> Result<InterfaceSettings, DatabaseError> {
    let connection = database.connection()?;
    let show_date_range_support = match read_setting(&connection, SHOW_DATE_RANGE_SUPPORT_KEY)? {
        Some(value) => value.parse::<bool>().ok().unwrap_or(false),
        None => read_setting(&connection, LEGACY_SHOW_FLOW_DATE_RANGE_HELPER_KEY)?
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false),
    };
    let navigation_position = read_setting(&connection, NAVIGATION_POSITION_KEY)?
        .and_then(|value| NavigationPosition::from_value(&value))
        .unwrap_or_default();
    let side_nav_collapsed = read_setting(&connection, SIDE_NAV_COLLAPSED_KEY)?
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(false);
    let confirm_destructive_actions = read_setting(&connection, CONFIRM_DESTRUCTIVE_KEY)?
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(true);
    Ok(InterfaceSettings {
        show_date_range_support,
        navigation_position,
        side_nav_collapsed,
        confirm_destructive_actions,
    })
}

/// Persist the date-range support-text visibility.
pub fn save_show_date_range_support(
    database: &Database,
    enabled: bool,
) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    write_setting(
        &connection,
        SHOW_DATE_RANGE_SUPPORT_KEY,
        if enabled { "true" } else { "false" },
    )
}

/// Persist the primary-navigation placement.
pub fn save_navigation_position(
    database: &Database,
    position: NavigationPosition,
) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    write_setting(&connection, NAVIGATION_POSITION_KEY, position.as_str())
}

/// Persist whether the side panel is compressed into an icon rail.
pub fn save_side_nav_collapsed(database: &Database, collapsed: bool) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    write_setting(
        &connection,
        SIDE_NAV_COLLAPSED_KEY,
        if collapsed { "true" } else { "false" },
    )
}

fn read_setting(connection: &Connection, key: &str) -> Result<Option<String>, DatabaseError> {
    connection
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(Into::into)
}

fn write_setting(connection: &Connection, key: &str, value: &str) -> Result<(), DatabaseError> {
    connection.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::theme::{ThemeMode, ThemePalette};

    #[test]
    fn missing_settings_use_defaults() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(load(&database).unwrap(), ThemePreference::default());
    }

    #[test]
    fn theme_preference_round_trips() {
        let database = Database::open_in_memory().unwrap();
        let saved = ThemePreference {
            palette: ThemePalette::Forest,
            mode: ThemeMode::Dark,
        };

        save(&database, &saved).unwrap();

        assert_eq!(load(&database).unwrap(), saved);
    }

    #[test]
    fn saving_again_updates_the_existing_rows() {
        let database = Database::open_in_memory().unwrap();
        let first = ThemePreference {
            palette: ThemePalette::Ocean,
            mode: ThemeMode::Light,
        };
        let second = ThemePreference {
            palette: ThemePalette::Orange,
            mode: ThemeMode::Dark,
        };

        save(&database, &first).unwrap();
        save(&database, &second).unwrap();

        let count: i64 = database
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM setting", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(load(&database).unwrap(), second);
    }

    #[test]
    fn default_fiat_currency_falls_back_to_usd_and_round_trips() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "USD");

        save_default_fiat_currency(&database, "EUR").unwrap();
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "EUR");

        // Saving again updates the existing row instead of adding a new one.
        save_default_fiat_currency(&database, "RON").unwrap();
        let count: i64 = database
            .connection()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM setting WHERE key = 'default_fiat_currency'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(load_default_fiat_currency(&database).unwrap(), "RON");
    }

    #[test]
    fn on_chain_link_scoring_round_trips_and_normalizes() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(
            load_on_chain_link_scoring(&database).unwrap(),
            OnChainLinkScoringSettings::default()
        );

        let scoring = OnChainLinkScoringSettings {
            hash_weight: -1.0,
            amount_tolerance_sats: 0.0,
            ..OnChainLinkScoringSettings::default()
        };
        let saved = save_on_chain_link_scoring(&database, &scoring).unwrap();
        assert_eq!(saved.hash_weight, 0.0);
        assert_eq!(saved.amount_tolerance_sats, 5_000.0);
        assert_eq!(load_on_chain_link_scoring(&database).unwrap(), saved);
    }

    #[test]
    fn invalid_on_chain_link_scoring_falls_back_to_defaults() {
        let database = Database::open_in_memory().unwrap();
        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value) VALUES ('on_chain_link_scoring', 'not-json')",
                [],
            )
            .unwrap();
        assert_eq!(
            load_on_chain_link_scoring(&database).unwrap(),
            OnChainLinkScoringSettings::default()
        );
    }
    #[test]
    fn interface_settings_default_to_hidden_and_round_trip() {
        let database = Database::open_in_memory().unwrap();
        let defaults = load_interface_settings(&database).unwrap();
        assert!(!defaults.show_date_range_support);
        assert_eq!(defaults.navigation_position, NavigationPosition::Auto);
        assert!(!defaults.side_nav_collapsed);

        save_show_date_range_support(&database, true).unwrap();
        assert!(
            load_interface_settings(&database)
                .unwrap()
                .show_date_range_support
        );

        save_show_date_range_support(&database, false).unwrap();
        assert!(
            !load_interface_settings(&database)
                .unwrap()
                .show_date_range_support
        );
    }

    #[test]
    fn navigation_position_round_trips_and_rejects_unknown_values() {
        let database = Database::open_in_memory().unwrap();

        for position in NavigationPosition::ALL {
            save_navigation_position(&database, position).unwrap();
            assert_eq!(
                load_interface_settings(&database)
                    .unwrap()
                    .navigation_position,
                position
            );
        }

        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value)
                 VALUES ('navigation_position', 'diagonal')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .unwrap();
        assert_eq!(
            load_interface_settings(&database)
                .unwrap()
                .navigation_position,
            NavigationPosition::Auto
        );
    }

    #[test]
    fn side_nav_collapsed_round_trips_and_invalid_values_expand() {
        let database = Database::open_in_memory().unwrap();

        save_side_nav_collapsed(&database, true).unwrap();
        assert!(
            load_interface_settings(&database)
                .unwrap()
                .side_nav_collapsed
        );

        save_side_nav_collapsed(&database, false).unwrap();
        assert!(
            !load_interface_settings(&database)
                .unwrap()
                .side_nav_collapsed
        );

        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value)
                 VALUES ('side_nav_collapsed', 'sideways')
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )
            .unwrap();
        assert!(
            !load_interface_settings(&database)
                .unwrap()
                .side_nav_collapsed
        );
    }

    #[test]
    fn legacy_flow_helper_setting_is_read_when_generic_setting_is_missing() {
        let database = Database::open_in_memory().unwrap();
        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value)
                 VALUES ('show_flow_date_range_helper', 'true')",
                [],
            )
            .unwrap();

        assert!(
            load_interface_settings(&database)
                .unwrap()
                .show_date_range_support
        );

        // Saving the generic preference takes precedence over the legacy key.
        save_show_date_range_support(&database, false).unwrap();
        assert!(
            !load_interface_settings(&database)
                .unwrap()
                .show_date_range_support
        );
    }

    #[test]
    fn invalid_interface_setting_falls_back_to_hidden() {
        let database = Database::open_in_memory().unwrap();
        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value)
                 VALUES ('show_date_range_support', 'not-a-bool')",
                [],
            )
            .unwrap();

        assert!(
            !load_interface_settings(&database)
                .unwrap()
                .show_date_range_support
        );
    }

    #[test]
    fn removed_stored_values_fall_back_to_defaults() {
        for palette in ["orchid", "graphite-lime", "sunset", "ledger"] {
            let database = Database::open_in_memory().unwrap();
            database
                .connection()
                .unwrap()
                .execute(
                    "INSERT INTO setting (key, value) VALUES ('theme', ?1), ('mode', 'system')",
                    [palette],
                )
                .unwrap();

            assert_eq!(load(&database).unwrap(), ThemePreference::default());
        }
    }

    #[test]
    fn invalid_stored_values_fall_back_to_defaults() {
        let database = Database::open_in_memory().unwrap();
        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value)
                 VALUES ('theme', 'unknown'), ('mode', 'unknown')",
                [],
            )
            .unwrap();

        assert_eq!(load(&database).unwrap(), ThemePreference::default());
    }
}
