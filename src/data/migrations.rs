//! Ordered schema migrations and the data-migration runner.
//!
//! Schema changes are versioned with SQLite's `PRAGMA user_version`, updated
//! in the same transaction as the schema change. New migrations MUST be
//! appended to [`MIGRATIONS`]; a migration that has shipped MUST NOT be
//! edited in place (Section 6.5 of CONTRIBUTING.md).

use rusqlite::{Connection, Transaction};

use super::seeds;
use super::DatabaseError;

/// A data migration entry: a function applied only to on-disk databases.
type MigrationSeed = fn(&Connection) -> Result<(), DatabaseError>;

/// One ordered migration step.
///
/// Schema migrations run on every database, including the in-memory
/// databases used by tests. Seed migrations import reference datasets from
/// files on disk, so they run only for on-disk databases; on in-memory
/// databases the version still advances past them, keeping the linear
/// `user_version` sequence identical on both kinds of databases.
pub(crate) enum Migration {
    Schema(&'static str),
    Seed(MigrationSeed),
}

/// All migrations, schema and data, in one linear order. The `user_version`
/// of an entry is its 1-based position. New migrations MUST be appended; a
/// migration that has shipped MUST NOT be edited in place, and entries MUST
/// NOT be reordered: shipped databases track progress through a single
/// `user_version` counter, so any reordering or insertion would skip or
/// re-run migrations (Section 6.5 of CONTRIBUTING.md).
///
/// `0009` re-runs the same import as `0008` after the source switched from
/// the kline CSV export to the packed pricing file (`assets/pricing/README.md`).
/// The import is idempotent (`INSERT OR IGNORE`), so on fresh databases it is
/// a no-op, while databases that applied `0008` from the CSV export pick up
/// the 2013-2017 daily closes the CSV did not cover.
pub(crate) const MIGRATIONS: &[(&str, Migration)] = &[
    (
        "0001_hello_world",
        Migration::Schema(include_str!("migrations/0001_hello_world.sql")),
    ),
    (
        "0002_addresses",
        Migration::Schema(include_str!("migrations/0002_addresses.sql")),
    ),
    (
        "0003_settings",
        Migration::Schema(include_str!("migrations/0003_settings.sql")),
    ),
    (
        "0004_transactions",
        Migration::Schema(include_str!("migrations/0004_transactions.sql")),
    ),
    (
        "0005_exchange_wallet_types",
        Migration::Schema(include_str!("migrations/0005_exchange_wallet_types.sql")),
    ),
    (
        "0006_address_owner",
        Migration::Schema(include_str!("migrations/0006_address_owner.sql")),
    ),
    (
        "0007_historical_prices",
        Migration::Schema(include_str!("migrations/0007_historical_prices.sql")),
    ),
    (
        "0008_historical_prices_import",
        Migration::Seed(seeds::historical_prices),
    ),
    (
        "0009_historical_prices_packed_backfill",
        Migration::Seed(seeds::historical_prices),
    ),
    (
        "0010_tx_index_rate_limited",
        Migration::Schema(include_str!("migrations/0010_tx_index_rate_limited.sql")),
    ),
    (
        "0011_historical_prices_close_time_index",
        Migration::Schema(include_str!(
            "migrations/0011_historical_prices_close_time_index.sql"
        )),
    ),
    (
        "0012_exchanges",
        Migration::Schema(include_str!("migrations/0012_exchanges.sql")),
    ),
    (
        "0013_exchange_addresses",
        Migration::Seed(seeds::exchange_addresses),
    ),
    (
        "0014_exchange_addresses",
        Migration::Seed(seeds::exchange_address_catalog),
    ),
];

/// Apply pending migrations in order. `include_seeds` is set for on-disk
/// databases; in-memory test databases skip the seed imports but still
/// advance `user_version` past them.
pub(crate) fn run_migrations(
    connection: &mut Connection,
    include_seeds: bool,
) -> Result<(), DatabaseError> {
    let current_version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DatabaseError::from)?;

    for (index, (_name, migration)) in MIGRATIONS.iter().enumerate() {
        let version = (index + 1) as i32;
        if current_version >= version {
            continue;
        }

        let transaction: Transaction<'_> = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::from)?;
        match migration {
            Migration::Schema(sql) => transaction
                .execute_batch(sql)
                .map_err(DatabaseError::from)?,
            Migration::Seed(seed) => {
                if include_seeds {
                    seed(&transaction)?;
                }
            }
        }
        transaction
            .pragma_update(None, "user_version", version)
            .map_err(DatabaseError::from)?;
        transaction.commit().map_err(DatabaseError::from)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::Database;

    /// SQL of a schema migration; the upgrade tests only apply schema
    /// entries, never the on-disk seed imports.
    fn schema_sql(entry: &Migration) -> &'static str {
        match entry {
            Migration::Schema(sql) => sql,
            Migration::Seed(_) => panic!("expected a schema migration"),
        }
    }

    /// Apply the schema entries `..through` (exclusive) in the raw
    /// connection and stamp the matching `user_version`.
    fn apply_schema_through(connection: &mut Connection, through: usize) {
        for (_, migration) in &MIGRATIONS[..through] {
            connection.execute_batch(schema_sql(migration)).unwrap();
        }
        connection
            .pragma_update(None, "user_version", through as i32)
            .unwrap();
    }

    #[test]
    fn hello_world_migration_runs() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM hello_world", [], |row| row.get(0))
            .unwrap();
        assert!(count >= 1);
    }

    #[test]
    fn historical_prices_migration_creates_table() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('historical_prices') ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(columns, ["close", "close_time", "symbol"]);
    }

    #[test]
    fn setting_table_migration_runs() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'setting'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
    }

    #[test]
    fn setting_migration_upgrades_version_two_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_schema_through(&mut connection, 2);

        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let setting_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM setting", [], |row| row.get(0))
            .unwrap();
        assert_eq!(setting_rows, 0);

        // Upgrading must not disturb data created by earlier migrations.
        let hello_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM hello_world", [], |row| row.get(0))
            .unwrap();
        assert_eq!(hello_rows, 1);
    }

    #[test]
    fn transaction_migration_creates_tables() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        for table in ["transactions", "transaction_inputs", "transaction_outputs"] {
            let table_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(table_count, 1, "missing table {table}");
        }
    }

    #[test]
    fn transaction_migration_upgrades_version_three_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_schema_through(&mut connection, 3);
        connection
            .execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count
                 ) VALUES ('bc1qexisting', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let address_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_rows, 1);
    }

    #[test]
    fn exchange_and_wallet_type_migration_seeds_reference_data() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let exchange_names: Vec<String> = connection
            .prepare("SELECT name FROM exchanges_types ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            exchange_names,
            [
                "Binance",
                "Coinbase",
                "Kraken",
                "OKX",
                "Bybit",
                "MEXC",
                "Bitget",
                "Gate",
                "KuCoin",
                "Crypto.com",
                "Revolut",
                "Upbit",
                "HTX",
                "Gemini",
                "Bitfinex",
                "Robinhood",
            ]
        );

        let wallet_names: Vec<String> = connection
            .prepare("SELECT name FROM wallets_types ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            wallet_names,
            [
                "Ledger",
                "Trezor",
                "MetaMask",
                "Trust Wallet",
                "Exodus",
                "Phantom",
                "Coinbase Wallet",
                "SafePal",
                "Tangem",
                "Bitget Wallet",
                "OKX Wallet",
                "Base Wallet",
                "Sparrow Wallet",
                "Electrum",
                "BlueWallet",
                "Muun",
                "Wasabi Wallet",
                "Blockchain.com Wallet",
            ]
        );
    }

    #[test]
    fn exchange_and_wallet_type_migration_upgrades_version_four_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_schema_through(&mut connection, 4);
        connection
            .execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count
                 ) VALUES ('bc1qexisting', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, false).unwrap();
        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let address_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_rows, 1);

        let exchange_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM exchanges_types", [], |row| row.get(0))
            .unwrap();
        assert_eq!(exchange_rows, 16);

        let wallet_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM wallets_types", [], |row| row.get(0))
            .unwrap();
        assert_eq!(wallet_rows, 18);
    }

    #[test]
    fn address_owner_migration_extends_and_keeps_addresses() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_schema_through(&mut connection, 5);
        connection
            .execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count
                 ) VALUES ('bc1qexisting', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let columns: Vec<String> = connection
            .prepare(
                "SELECT name FROM pragma_table_info('addresses') WHERE name IN
                 ('description', 'exchange_id', 'wallet_id') ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(columns, ["description", "exchange_id", "wallet_id"]);

        // Data written before the migration survives the schema change.
        let address_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_rows, 1);
    }

    #[test]
    fn address_owner_columns_enforce_reference_integrity() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let insert = |address: &str, exchange_id: Option<i64>, wallet_id: Option<i64>| {
            connection.execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count, description, exchange_id, wallet_id
                ) VALUES (?1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, NULL, ?2, ?3)",
                rusqlite::params![address, exchange_id, wallet_id],
            )
        };

        // Unknown owner ids must be rejected by the foreign keys.
        assert!(insert("bc1qexchange", Some(999), None).is_err());
        assert!(insert("bc1qwallet", None, Some(999)).is_err());

        insert("bc1qexchange", Some(1), None).unwrap();
        insert("bc1qwallet", None, Some(1)).unwrap();
        insert("bc1qplain", None, None).unwrap();

        // An address may not belong to an exchange and a wallet at once.
        assert!(insert("bc1qboth", Some(1), Some(1)).is_err());

        let address_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_rows, 3);
    }

    #[test]
    fn rate_limited_migration_extends_version_nine_databases() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_schema_through(&mut connection, 7);
        connection
            .execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count
                 ) VALUES ('bc1qexisting', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                [],
            )
            .unwrap();
        // On-disk databases applied both seed imports after the schema
        // migrations, so they sit at version 9. Skipping the seed versions
        // must not skip the schema migration that follows them.
        connection.pragma_update(None, "user_version", 9).unwrap();

        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let rate_limited: i64 = connection
            .query_row(
                "SELECT tx_index_rate_limited FROM addresses WHERE address = 'bc1qexisting'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rate_limited, 0);
    }

    #[test]
    fn exchange_migration_creates_tables_and_address_balance_column() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        for table in [
            "exchange_accounts",
            "exchange_transactions",
            "fiat_exchange_rates",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {table}");
        }

        connection
            .execute(
                "INSERT INTO addresses (
                    address, chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count, mempool_funded_txo_count,
                    mempool_funded_txo_sum, mempool_spent_txo_count, mempool_spent_txo_sum,
                    mempool_tx_count
                 ) VALUES ('bc1qexchangebalance', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)",
                [],
            )
            .unwrap();
        let balance: i64 = connection
            .query_row(
                "SELECT exchange_balance_sats FROM addresses WHERE address = 'bc1qexchangebalance'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(balance, 0);
    }

    #[test]
    fn exchange_migration_upgrades_the_previous_database_without_losing_addresses() {
        let mut connection = Connection::open_in_memory().unwrap();
        for (_, migration) in &MIGRATIONS[..11] {
            if let Migration::Schema(sql) = migration {
                connection.execute_batch(sql).unwrap();
            }
        }
        connection.pragma_update(None, "user_version", 11).unwrap();
        connection
            .execute(
                "INSERT INTO addresses (
                    address, chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count, mempool_funded_txo_count,
                    mempool_funded_txo_sum, mempool_spent_txo_count, mempool_spent_txo_sum,
                    mempool_tx_count
                 ) VALUES ('bc1qexisting12', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, false).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);
        let balance: i64 = connection
            .query_row(
                "SELECT exchange_balance_sats FROM addresses WHERE address = 'bc1qexisting12'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(balance, 0);
        let address_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_count, 1);
    }
    #[test]
    fn exchange_addresses_seed_upgrades_a_version_twelve_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        for (_, migration) in &MIGRATIONS[..12] {
            if let Migration::Schema(sql) = migration {
                connection.execute_batch(sql).unwrap();
            }
        }
        connection.pragma_update(None, "user_version", 12).unwrap();
        connection
            .execute(
                "INSERT INTO addresses (
                    address, chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count, mempool_funded_txo_count,
                    mempool_funded_txo_sum, mempool_spent_txo_count, mempool_spent_txo_sum,
                    mempool_tx_count, description
                 ) VALUES ('bc1qexisting13', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 'My wallet')",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, true).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        // The user's own row survives untouched.
        let description: String = connection
            .query_row(
                "SELECT description FROM addresses WHERE address = 'bc1qexisting13'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(description, "My wallet");

        // Both seed migrations run: the initial 0013 subset and the complete
        // 0014 catalog, without duplicating any address.
        let exchange_addresses: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exchange_addresses, 296);
    }

    #[test]
    fn exchange_address_catalog_upgrades_a_version_thirteen_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        for (_, migration) in &MIGRATIONS[..12] {
            if let Migration::Schema(sql) = migration {
                connection.execute_batch(sql).unwrap();
            }
        }
        // Reproduce the on-disk state after 0013: eight seeded wallets plus a
        // user-owned address.
        seeds::exchange_addresses(&connection).unwrap();
        connection.pragma_update(None, "user_version", 13).unwrap();
        connection
            .execute(
                "INSERT INTO addresses (
                    address, chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count, mempool_funded_txo_count,
                    mempool_funded_txo_sum, mempool_spent_txo_count, mempool_spent_txo_sum,
                    mempool_tx_count, description
                 ) VALUES ('bc1quserowned14', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 'User wallet')",
                [],
            )
            .unwrap();

        run_migrations(&mut connection, true).unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let exchange_addresses: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exchange_addresses, 296);

        let user_description: String = connection
            .query_row(
                "SELECT description FROM addresses WHERE address = 'bc1quserowned14'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(user_description, "User wallet");
    }

    #[test]
    fn in_memory_databases_advance_past_the_seed_versions() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i32);

        let columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('addresses') ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(columns.contains(&"tx_index_rate_limited".to_string()));
    }
}
