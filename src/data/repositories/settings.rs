//! SQL access to the `setting` key/value table, holding the theme preference
//! and the default fiat currency.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::data::{Database, DatabaseError};
use crate::domain::price::USD_SYMBOL;
use crate::domain::theme::{ThemeMode, ThemePalette, ThemePreference};

const PALETTE_KEY: &str = "theme";
const MODE_KEY: &str = "mode";
const DEFAULT_FIAT_CURRENCY_KEY: &str = "default_fiat_currency";

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
            palette: ThemePalette::Sunset,
            mode: ThemeMode::System,
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
