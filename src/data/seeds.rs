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
//! The bundled exchange address collection in
//! `assets/exchange_samples/exchange_addresses.csv` becomes exchange-scoped
//! rows of the `addresses` table. It is embedded with `include_str!` so
//! packaged builds do not need a runtime file lookup, and a conflict on the
//! address primary key is ignored so an existing address keeps the user's
//! description and classification.
//!
//! Seed migrations run only for on-disk databases (`open_path`,
//! `open_default`); see `run_migrations` in `migrations.rs`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};

use super::pricing_format;
use super::DatabaseError;
use crate::data::repositories::exchanges as exchange_repository;
use crate::domain::address::validate_bitcoin_address;
use crate::domain::exchange::csv::parse_csv_text;

/// Quote symbol of the imported price history.
pub const BTC_USD_SYMBOL: &str = "USD";

const PACKED_RELATIVE_PATH: &str = "assets/pricing/btc_usd.bin";
const PACKED_ENV_VAR: &str = "ADDRESS_EXPLORER_PRICING_FILE";

/// Bundled public exchange wallets, imported by the
/// `0013_exchange_addresses` seed migration. See
/// `assets/exchange_samples/README.md` for attribution sources.
const EXCHANGE_ADDRESSES_CSV: &str =
    include_str!("../../assets/exchange_samples/exchange_addresses.csv");

/// Full researched exchange-wallet catalog, imported by the
/// `0014_exchange_addresses` seed migration. Every row is tagged by exchange
/// name and existing addresses keep their user-edited description and owner.
const EXCHANGE_ADDRESS_CATALOG_SQL: &str = include_str!("migrations/0014_exchange_addresses.sql");

/// Number of exchange wallets the catalog is expected to cover. Keep in sync
/// with the SQL file; the seed fails loudly when a catalog name is missing.
const EXCHANGE_ADDRESS_CATALOG_SIZE: i64 = 296;

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

/// Data migration `0013_exchange_addresses` entry point.
///
/// Stores the public exchange wallets from the bundled address collection as
/// exchange-scoped `addresses` rows. Existing addresses are left untouched so
/// a re-run never overwrites a user-edited description or classification.
pub fn exchange_addresses(connection: &Connection) -> Result<(), DatabaseError> {
    let document = parse_csv_text(EXCHANGE_ADDRESSES_CSV).map_err(DatabaseError::new)?;
    for header in ["exchange", "address", "description"] {
        if !document.headers.iter().any(|value| value == header) {
            return Err(DatabaseError::new(format!(
                "exchange address collection is missing the `{header}` column"
            )));
        }
    }
    if document.rows.is_empty() {
        return Err(DatabaseError::new(
            "exchange address collection contains no rows",
        ));
    }

    let exchange_ids = exchange_type_ids(connection)?;
    let mut statement = connection.prepare(
        "INSERT INTO addresses (
            address, description, exchange_id,
            chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
            chain_spent_txo_sum, chain_tx_count,
            mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
            mempool_spent_txo_sum, mempool_tx_count
         ) VALUES (?1, ?2, ?3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
         ON CONFLICT(address) DO NOTHING",
    )?;

    for row in &document.rows {
        let exchange = required_field(row, "exchange")?;
        let exchange_id = *exchange_ids.get(exchange).ok_or_else(|| {
            DatabaseError::new(format!(
                "exchange address collection references an unknown exchange: {exchange}"
            ))
        })?;
        let address = validate_bitcoin_address(required_field(row, "address")?)
            .map_err(DatabaseError::new)?;
        let description = required_field(row, "description")?;
        statement.execute(params![address, description, exchange_id])?;
    }
    Ok(())
}

/// Data migration `0014_exchange_addresses` entry point.
///
/// Runs the checked-in SQL catalog. The SQL is idempotent: it joins the
/// exchange catalog by name and skips addresses that are already stored, so
/// user-edited descriptions and classifications are preserved.
pub fn exchange_address_catalog(connection: &Connection) -> Result<(), DatabaseError> {
    connection.execute_batch(EXCHANGE_ADDRESS_CATALOG_SQL)?;
    let stored: i64 = connection.query_row(
        "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
        [],
        |row| row.get(0),
    )?;
    if stored < EXCHANGE_ADDRESS_CATALOG_SIZE {
        return Err(DatabaseError::new(format!(
            "exchange address catalog stored {stored} rows, expected at least {EXCHANGE_ADDRESS_CATALOG_SIZE}"
        )));
    }

    // Adding addresses can change the alphabetically first address that holds
    // a provider's stored ledger balance. Refresh every provider's anchor
    // inside the same migration transaction so the invariant from 0012 stays
    // true for the whole catalog, not only the originally seeded providers.
    let exchange_ids = exchange_type_ids(connection)?;
    for id in exchange_ids.values() {
        exchange_repository::refresh_provider_balance(connection, *id)?;
    }
    Ok(())
}

/// Map of the exchange catalog names to their `exchanges_types` ids.
fn exchange_type_ids(connection: &Connection) -> Result<BTreeMap<String, i64>, DatabaseError> {
    let mut statement = connection.prepare("SELECT id, name FROM exchanges_types")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get(1)?)))?;
    let mut ids = BTreeMap::new();
    for row in rows {
        let (id, name) = row?;
        ids.insert(name, id);
    }
    Ok(ids)
}

/// Required, non-empty field of one address-collection row.
fn required_field<'a>(
    row: &'a BTreeMap<String, String>,
    field: &str,
) -> Result<&'a str, DatabaseError> {
    row.get(field)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            DatabaseError::new(format!(
                "exchange address collection row is missing `{field}`"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::migrations;
    use crate::data::Database;
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

    #[test]
    fn exchange_address_seed_stores_exchange_scoped_rows_idempotently() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        // A re-run must never duplicate or overwrite the stored rows.
        exchange_addresses(&connection).unwrap();
        exchange_addresses(&connection).unwrap();

        let mut statement = connection
            .prepare(
                "SELECT a.address, a.description, e.name
                 FROM addresses a
                 JOIN exchanges_types e ON e.id = a.exchange_id",
            )
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(rows.len(), 8);
        for (address, description, _) in &rows {
            assert_eq!(
                validate_bitcoin_address(address).unwrap(),
                *address,
                "the seed must only store canonical mainnet addresses"
            );
            assert!(description
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()));
        }
        assert_eq!(
            rows.iter().filter(|(_, _, name)| name == "Binance").count(),
            4
        );
        assert_eq!(
            rows.iter()
                .filter(|(_, _, name)| name == "Crypto.com")
                .count(),
            2
        );
        assert_eq!(
            rows.iter().filter(|(_, _, name)| name == "Revolut").count(),
            2
        );
    }

    #[test]
    fn exchange_address_catalog_imports_verified_wallets_idempotently() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        // The historical 0013 seed runs first; the user then labels one of
        // those rows. The 0014 catalog must not overwrite either choice.
        exchange_addresses(&connection).unwrap();
        connection
            .execute(
                "UPDATE addresses SET description = 'Wallet 1'
                 WHERE address = '34xp4vRoCGJym3xR7yCVPFHoCNxv4Twseo'",
                [],
            )
            .unwrap();

        exchange_address_catalog(&connection).unwrap();
        exchange_address_catalog(&connection).unwrap();

        let counts = ["Binance", "Crypto.com", "Revolut"]
            .into_iter()
            .map(|name| {
                connection
                    .query_row(
                        "SELECT COUNT(*) FROM addresses a
                         JOIN exchanges_types e ON e.id = a.exchange_id
                         WHERE e.name = ?1",
                        [name],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(counts, [70, 7, 5]);

        let total: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(total, 296);

        let description: String = connection
            .query_row(
                "SELECT description FROM addresses
                 WHERE address = '34xp4vRoCGJym3xR7yCVPFHoCNxv4Twseo'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(description, "Wallet 1");

        let mut statement = connection
            .prepare(
                "SELECT address, description FROM addresses
                 WHERE exchange_id IS NOT NULL",
            )
            .unwrap();
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        for (address, description) in rows {
            assert_eq!(validate_bitcoin_address(&address).unwrap(), address);
            assert!(description
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()));
        }
    }

    #[test]
    fn exchange_address_catalog_moves_the_provider_balance_to_the_new_anchor() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO exchange_accounts
                    (name, provider_id, created_at, last_modified_at)
                 VALUES ('binance_btc', 1, 1, 1)",
                [],
            )
            .unwrap();
        let account_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO exchange_transactions (
                    exchange_id, fingerprint, timestamp, tx_type, amount_sats,
                    currency, raw_data, match_state, created_at, updated_at
                 ) VALUES (?1, 'catalog-anchor', 1, 'buy', 100000000,
                    'BTC', '{}', 'unmatched', 1, 1)",
                [account_id],
            )
            .unwrap();

        exchange_addresses(&connection).unwrap();
        exchange_address_catalog(&connection).unwrap();

        let (address, balance): (String, i64) = connection
            .query_row(
                "SELECT address, exchange_balance_sats FROM addresses
                 WHERE exchange_id = 1 AND exchange_balance_sats <> 0",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(address, "12VuUfQHTGqWDvBzm8TBad1mZBm4hjGEzN");
        assert_eq!(balance, 100_000_000);
    }
}
