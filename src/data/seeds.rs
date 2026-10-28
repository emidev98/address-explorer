//! Data migrations that bulk-load reference datasets into SQLite.
//!
//! The packed BTC price history in `assets/pricing/btc_usd.bin`
//! becomes the `USD` rows of `historical_prices`. The file layout is
//! specified by `assets/pricing/README.md` and decoded by [`pricing_format`];
//! the import rules are:
//!
//! - records that do not decode are skipped by the codec;
//! - duplicate close times keep the first occurrence in file order, enforced
//!   with `INSERT OR IGNORE`, which also makes a re-run of the import a
//!   no-op (see the `0009` backfill entry in `migrations.rs`).
//!
//! Seed migrations run only for on-disk databases (`open_path`,
//! `open_default`); see `run_seed_migrations` in `mod.rs`.

use rusqlite::{params, Connection};
use std::fs;
use std::path::{Path, PathBuf};

use super::pricing_format;
use super::DatabaseError;

/// Quote symbol of the imported price history.
pub const BTC_USD_SYMBOL: &str = "USD";

const PACKED_RELATIVE_PATH: &str = "assets/pricing/btc_usd.bin";
const PACKED_ENV_VAR: &str = "ADDRESS_EXPLORER_PRICING_FILE";

/// Data migration `0008_historical_prices_import` entry point.
///
/// Fails loudly when the packed pricing file cannot be located or yields no
/// importable rows, because a migration MUST NOT silently produce an empty
/// reference table.
pub fn historical_prices(connection: &Connection) -> Result<(), DatabaseError> {
    let path = packed_pricing_path()?;
    let imported = import_packed_prices(connection, &path)?;
    if imported == 0 {
        return Err(DatabaseError::new(format!(
            "historical price import produced no rows from {}",
            path.display()
        )));
    }
    Ok(())
}

/// Resolve the packed pricing file location: the
/// `ADDRESS_EXPLORER_PRICING_FILE` environment variable takes precedence,
/// then the workspace checkout and the directory next to the executable (for
/// packaged builds).
fn packed_pricing_path() -> Result<PathBuf, DatabaseError> {
    if let Ok(path) = std::env::var(PACKED_ENV_VAR) {
        if path.trim().is_empty() {
            return Err(DatabaseError::new(format!(
                "{PACKED_ENV_VAR} is set but empty"
            )));
        }
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(DatabaseError::new(format!(
            "{PACKED_ENV_VAR} points at a missing file: {}",
            path.display()
        )));
    }

    let mut candidates = vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PACKED_RELATIVE_PATH)];
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            candidates.push(directory.join(PACKED_RELATIVE_PATH));
        }
    }
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| {
            let searched: Vec<String> = candidates
                .iter()
                .map(|path| path.display().to_string())
                .collect();
            DatabaseError::new(format!(
                "packed pricing file not found; searched: {}; set {PACKED_ENV_VAR}",
                searched.join(", ")
            ))
        })
}

/// Parse the packed pricing file at `path` and insert one row per decoded
/// record into `historical_prices`. Returns the number of rows attempted
/// (including rows rejected by the primary key, which keeps the first
/// occurrence).
pub fn import_packed_prices(connection: &Connection, path: &Path) -> Result<usize, DatabaseError> {
    let contents = fs::read_to_string(path).map_err(DatabaseError::from)?;
    let records = pricing_format::parse_records(&contents);
    let mut statement = connection.prepare(
        "INSERT OR IGNORE INTO historical_prices (close_time, close, symbol)
         VALUES (?1, ?2, ?3)",
    )?;

    let mut imported = 0usize;
    for record in &records {
        statement.execute(params![
            record.close_time,
            record.close_cents as f64 / 100.0,
            BTC_USD_SYMBOL
        ])?;
        imported += 1;
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::domain::civil_time::epoch_seconds;

    struct TempPricingFile(PathBuf);

    impl TempPricingFile {
        fn write(contents: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "address-explorer-seed-test-{}-{unique}.bin",
                std::process::id(),
                unique = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_nanos())
                    .unwrap_or(0)
            ));
            fs::write(&path, contents).unwrap();
            Self(path)
        }
    }

    impl Drop for TempPricingFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn fresh_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        let schema = match &migrations::MIGRATIONS[6].1 {
            migrations::Migration::Schema(sql) => sql,
            _ => panic!("0007_historical_prices must stay a schema migration"),
        };
        connection.execute_batch(schema).unwrap();
        connection
    }

    #[test]
    fn import_writes_close_time_close_and_usd_symbol() {
        // 2018-01-01 00:14:59 UTC close of $13,556.15 in packed form.
        let packed = TempPricingFile::write("201801010014591355615,");
        let connection = fresh_connection();

        assert_eq!(import_packed_prices(&connection, &packed.0).unwrap(), 1);

        let (close_time, close, symbol): (i64, f64, String) = connection
            .query_row(
                "SELECT close_time, close, symbol FROM historical_prices",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(close_time, epoch_seconds(2018, 1, 1, 0, 14, 59));
        assert_eq!(close, 13556.15);
        assert_eq!(symbol, BTC_USD_SYMBOL);
    }

    #[test]
    fn import_keeps_the_first_row_of_a_duplicate_close_time() {
        // Mis-stamped duplicates in the source export appear *after* the
        // genuine rows; the first occurrence must win.
        let packed = TempPricingFile::write("20180707061459659290,2018070706145910811555,");
        let connection = fresh_connection();

        assert_eq!(import_packed_prices(&connection, &packed.0).unwrap(), 2);

        let close: f64 = connection
            .query_row(
                "SELECT close FROM historical_prices WHERE symbol = 'USD'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(close, 6592.9);
    }

    #[test]
    fn import_skips_malformed_records() {
        let packed = TempPricingFile::write(
            "201801010014591355615,not-a-record,201813010014591355615,20180101001459,",
        );
        let connection = fresh_connection();

        assert_eq!(import_packed_prices(&connection, &packed.0).unwrap(), 1);

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn import_fails_for_a_missing_file() {
        let connection = fresh_connection();
        assert!(import_packed_prices(
            &connection,
            Path::new("/nonexistent/definitely-missing.bin")
        )
        .is_err());
    }

    #[test]
    fn full_migration_imports_the_workspace_pricing_file() {
        let connection = fresh_connection();
        historical_prices(&connection).unwrap();

        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM historical_prices WHERE symbol = 'USD'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 290_000, "unexpectedly few rows imported: {count}");

        // The packed file starts with the SRK daily closes, so the earliest
        // row predates the 15-minute export that used to be the source.
        let first: i64 = connection
            .query_row("SELECT MIN(close_time) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(first, epoch_seconds(2013, 4, 29, 23, 59, 59));

        // The 15-minute export follows the daily closes; its first candle is
        // still present with its original close.
        let close: f64 = connection
            .query_row(
                "SELECT close FROM historical_prices WHERE close_time = ?1",
                [epoch_seconds(2018, 1, 1, 0, 14, 59)],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(close, 13556.15);
    }
}
