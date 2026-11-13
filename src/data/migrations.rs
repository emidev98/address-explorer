//! Ordered schema and data migrations and the migration runner.
//!
//! Migrations are consolidated per table: `migrations/<ordinal>_<table>.sql`
//! creates one table with its final columns, indexes, and triggers, and
//! `migrations/<ordinal>_<table>_data.sql` inserts the reference rows owned by
//! that table. There is exactly one schema file and at most one data file per
//! table, so a fresh database never replays the historical `ALTER TABLE`
//! sequence. Migration order is the order of [`MIGRATIONS`].
//!
//! Every file name starts with the four-digit ordinal the entry occupies in
//! [`MIGRATIONS`], beginning at `0001`, so the highest-numbered file is always
//! the latest migration. The ordinal is also the `user_version` the migration
//! stamps. Rust seed migrations consume their ordinal; `0019_addresses_data`
//! also reads the checked-in `0019_addresses_data.sql` catalog.
//!
//! Progress is versioned with SQLite's `PRAGMA user_version`. Schema and SQL
//! data migrations run on every database, including the in-memory databases
//! used by tests. Seed migrations import reference datasets from files or the
//! browser-precached asset, so they run for persistent databases (native on-disk
//! and browser-local); on in-memory databases the version still advances past
//! them, keeping the linear `user_version` sequence identical on both kinds of
//! databases.
//!
//! New migrations MUST be appended with a new ordinal and name; a migration
//! that has shipped MUST NOT be edited in place, and entries MUST NOT be
//! reordered, because a shipped database tracks progress through a single
//! `user_version` counter.
use super::seeds;
use super::DatabaseError;
use rusqlite::{Connection, Transaction};
/// A data migration entry: a function applied only to on-disk databases.
type MigrationSeed = fn(&Connection) -> Result<(), DatabaseError>;
/// One ordered migration step.
pub(crate) enum Migration {
    /// A checked-in SQL file, applied with `execute_batch`.
    Sql(&'static str),
    /// A Rust seed importing an on-disk reference dataset.
    Seed(MigrationSeed),
}
/// All migrations in one linear order, consolidated per table. The
/// `user_version` of an entry is its 1-based position in this list, and every
/// name repeats that position as a four-digit prefix starting at `0001`, so
/// the highest-numbered file is always the latest migration.
///
/// Data files are named `<ordinal>_<table>_data`; they insert the reference
/// rows of the table created by `<ordinal>_<table>` and are idempotent, so a
/// re-run never duplicates or overwrites user edits.
///
/// The prefix of every entry MUST equal its position; the tests enforce it.
pub(crate) const MIGRATIONS: &[(&str, Migration)] = &[
    (
        "0001_setting",
        Migration::Sql(include_str!("migrations/0001_setting.sql")),
    ),
    (
        "0002_exchange_categories",
        Migration::Sql(include_str!("migrations/0002_exchange_categories.sql")),
    ),
    (
        "0003_exchange_categories_data",
        Migration::Sql(include_str!("migrations/0003_exchange_categories_data.sql")),
    ),
    (
        "0004_wallet_categories",
        Migration::Sql(include_str!("migrations/0004_wallet_categories.sql")),
    ),
    (
        "0005_wallet_categories_data",
        Migration::Sql(include_str!("migrations/0005_wallet_categories_data.sql")),
    ),
    (
        "0006_exchanges_types",
        Migration::Sql(include_str!("migrations/0006_exchanges_types.sql")),
    ),
    (
        "0007_exchanges_types_data",
        Migration::Sql(include_str!("migrations/0007_exchanges_types_data.sql")),
    ),
    (
        "0008_wallets_types",
        Migration::Sql(include_str!("migrations/0008_wallets_types.sql")),
    ),
    (
        "0009_wallets_types_data",
        Migration::Sql(include_str!("migrations/0009_wallets_types_data.sql")),
    ),
    (
        "0010_addresses",
        Migration::Sql(include_str!("migrations/0010_addresses.sql")),
    ),
    (
        "0011_transactions",
        Migration::Sql(include_str!("migrations/0011_transactions.sql")),
    ),
    (
        "0012_transaction_inputs",
        Migration::Sql(include_str!("migrations/0012_transaction_inputs.sql")),
    ),
    (
        "0013_transaction_outputs",
        Migration::Sql(include_str!("migrations/0013_transaction_outputs.sql")),
    ),
    (
        "0014_historical_prices",
        Migration::Sql(include_str!("migrations/0014_historical_prices.sql")),
    ),
    (
        "0015_historical_prices_data",
        Migration::Seed(seeds::historical_prices),
    ),
    (
        "0016_exchange_accounts",
        Migration::Sql(include_str!("migrations/0016_exchange_accounts.sql")),
    ),
    (
        "0017_exchange_transactions",
        Migration::Sql(include_str!("migrations/0017_exchange_transactions.sql")),
    ),
    (
        "0018_fiat_exchange_rates",
        Migration::Sql(include_str!("migrations/0018_fiat_exchange_rates.sql")),
    ),
    ("0019_addresses_data", Migration::Seed(seeds::addresses)),
    (
        "0020_provider_brand_colors",
        Migration::Sql(include_str!("migrations/0020_provider_brand_colors.sql")),
    ),
    (
        "0021_flow_indexes",
        Migration::Sql(include_str!("migrations/0021_flow_indexes.sql")),
    ),
    (
        "0022_exchange_transactions_btc_fee_sats",
        Migration::Sql(include_str!(
            "migrations/0022_exchange_transactions_btc_fee_sats.sql"
        )),
    ),
];
/// `user_version` of a database with every migration applied.
pub(crate) const CURRENT_SCHEMA_VERSION: i32 = MIGRATIONS.len() as i32;
/// Logical name of a migration, without its `00NN_` version prefix.
#[cfg(test)]
pub(crate) fn logical_name(name: &str) -> &str {
    name.split_once('_').map_or(name, |(_, rest)| rest)
}
/// SQL file of a table's schema migration, without its `00NN_` prefix. Used by
/// tests and seed tests that build one table in isolation.
#[cfg(test)]
pub(crate) fn schema_sql(table: &str) -> Option<&'static str> {
    MIGRATIONS
        .iter()
        .find_map(|(name, migration)| match migration {
            Migration::Sql(sql) if logical_name(name) == table => Some(*sql),
            _ => None,
        })
}
/// Build the canonical schema for an older or current backup version.
///
/// Only SQL migrations participate: they create the tables, indexes, and
/// triggers a legitimate backup at that version contains. Seed migrations read
/// bundled files and do not change the schema, so they are skipped.
pub(crate) fn expected_schema(version: i32) -> Result<Connection, DatabaseError> {
    if version <= 0 || version > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::new(format!(
            "unsupported backup schema version {version}"
        )));
    }
    let connection = Connection::open_in_memory().map_err(DatabaseError::from)?;
    for (index, (_name, migration)) in MIGRATIONS.iter().enumerate() {
        if index as i32 >= version {
            break;
        }
        if let Migration::Sql(sql) = migration {
            connection.execute_batch(sql).map_err(DatabaseError::from)?;
        }
    }
    Ok(connection)
}

/// Apply pending migrations in order.
///
/// `include_seeds` is set for persistent databases (native on-disk and
/// browser-local); in-memory test databases skip the seed imports but still
/// advance `user_version` past them.
pub(crate) fn run_migrations(
    connection: &mut Connection,
    include_seeds: bool,
) -> Result<(), DatabaseError> {
    let current_version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DatabaseError::from)?;
    if current_version > CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::new(format!(
            "database schema version {current_version} is newer than this build supports \
             ({CURRENT_SCHEMA_VERSION})",
        )));
    }
    for (index, (_name, migration)) in MIGRATIONS.iter().enumerate() {
        let version = index as i32 + 1;
        if current_version >= version {
            continue;
        }
        let transaction: Transaction<'_> = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::from)?;
        match migration {
            Migration::Sql(sql) => transaction
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
    /// SQL of a named migration; the upgrade tests only apply SQL entries,
    /// never the on-disk seed imports.
    fn sql_of(table: &str) -> &'static str {
        schema_sql(table).unwrap_or_else(|| panic!("missing SQL migration for {table}"))
    }
    /// Apply every SQL entry (skipping the Rust seeds) in order, without
    /// touching `user_version`. Used to simulate a fully migrated database.
    fn apply_sql(connection: &Connection) {
        for (_, migration) in MIGRATIONS {
            if let Migration::Sql(sql) = migration {
                connection.execute_batch(sql).unwrap();
            }
        }
    }
    /// Apply the SQL entries up to and including `version`.
    fn apply_sql_through(connection: &Connection, version: i32) {
        for (index, (_, migration)) in MIGRATIONS.iter().enumerate() {
            if index as i32 + 1 > version {
                break;
            }
            if let Migration::Sql(sql) = migration {
                connection.execute_batch(sql).unwrap();
            }
        }
    }
    /// Stamp the version of a database built by [`apply_sql`].
    fn stamp(connection: &Connection, version: i32) {
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
    }
    fn user_version(connection: &Connection) -> i32 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }
    fn table_names(connection: &Connection) -> Vec<String> {
        connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    fn column_names(connection: &Connection, table: &str) -> Vec<String> {
        connection
            .prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
            .unwrap()
            .query_map([table], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
    #[test]
    fn fresh_database_applies_every_migration() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        assert_eq!(user_version(&connection), CURRENT_SCHEMA_VERSION);
        let tables = table_names(&connection);
        for table in [
            "addresses",
            "exchange_accounts",
            "exchange_categories",
            "exchange_transactions",
            "exchanges_types",
            "fiat_exchange_rates",
            "historical_prices",
            "setting",
            "transaction_inputs",
            "transaction_outputs",
            "transactions",
            "wallet_categories",
            "wallets_types",
        ] {
            assert!(
                tables.iter().any(|name| name == table),
                "missing table {table}"
            );
        }
        assert!(!tables.iter().any(|name| name == "hello_world"));
    }
    #[test]
    fn migration_names_are_numbered_from_one() {
        let mut names = std::collections::BTreeSet::new();
        for (index, (name, _)) in MIGRATIONS.iter().enumerate() {
            assert!(names.insert(*name), "duplicate migration name {name}");
            let position = index as i32 + 1;
            let prefix = name.split_once('_').map(|(prefix, _)| prefix).unwrap();
            assert_eq!(prefix.len(), 4, "migration {name} needs a 00NN_ prefix");
            assert_eq!(
                prefix.parse::<i32>().ok(),
                Some(position),
                "migration {name} must be prefixed with its position {position:04}",
            );
            let logical = logical_name(name);
            if let Some(table) = logical.strip_suffix("_data") {
                assert!(
                    MIGRATIONS
                        .iter()
                        .any(|(other, _)| logical_name(other) == table),
                    "migration {name} does not name a consolidated table migration",
                );
            }
        }
        assert_eq!(names.len(), MIGRATIONS.len());
        assert_eq!(MIGRATIONS[0].0, "0001_setting");
        assert_eq!(
            MIGRATIONS[MIGRATIONS.len() - 1].0,
            "0022_exchange_transactions_btc_fee_sats",
        );
        assert_eq!(CURRENT_SCHEMA_VERSION, MIGRATIONS.len() as i32);
    }
    #[test]
    fn consolidated_sql_is_repeatable() {
        let connection = Connection::open_in_memory().unwrap();
        apply_sql(&connection);
        apply_sql(&connection);
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
    fn historical_prices_migration_creates_table() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        assert_eq!(
            column_names(&connection, "historical_prices"),
            ["close_time", "close", "symbol"],
        );
        let index_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_historical_prices_close_time'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 1);
    }
    #[test]
    fn setting_table_migration_runs() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let setting_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM setting", [], |row| row.get(0))
            .unwrap();
        assert_eq!(setting_rows, 0);
    }
    #[test]
    fn transaction_migration_creates_tables_and_address_lookup_indexes() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        for index in [
            "idx_transaction_inputs_prevout_scriptpubkey_address",
            "idx_transaction_outputs_scriptpubkey_address",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing index {index}");
        }
    }
    #[test]
    fn flow_indexes_are_present_and_idempotent() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        for index in [
            "idx_transactions_block_time",
            "idx_exchange_transactions_timestamp",
            "idx_exchange_transactions_on_chain_txid",
        ] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing index {index}");
        }
        connection
            .execute_batch(include_str!("migrations/0021_flow_indexes.sql"))
            .unwrap();
    }
    #[test]
    fn btc_fee_migration_normalizes_legacy_fees_and_rebuilds_balances() {
        let connection = Connection::open_in_memory().unwrap();
        apply_sql_through(&connection, 21);
        connection
            .execute_batch(
                "INSERT INTO addresses (
                     address, chain_funded_txo_count, chain_funded_txo_sum,
                     chain_spent_txo_count, chain_spent_txo_sum, chain_tx_count,
                     mempool_funded_txo_count, mempool_funded_txo_sum,
                     mempool_spent_txo_count, mempool_spent_txo_sum,
                     mempool_tx_count, exchange_id, exchange_balance_sats
                 ) VALUES ('bc1qlegacy', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 42);
                 INSERT INTO exchange_accounts (
                     id, name, provider_id, created_at, last_modified_at
                 ) VALUES (1, 'Legacy', 1, 0, 0);
                 INSERT INTO exchange_transactions (
                     id, exchange_id, fingerprint, timestamp, tx_type,
                     amount_sats, currency, fee, fee_currency, raw_data,
                     match_state, created_at, updated_at
                 ) VALUES
                     (1, 1, 'legacy-btc-fee', 1, 'buy', 100000000, 'BTC',
                      0.0001, 'BTC', '{}', 'unmatched', 0, 0),
                     (2, 1, 'legacy-fiat-fee', 2, 'buy', 0, 'BTC',
                      2.5, 'EUR', '{}', 'unmatched', 0, 0);",
            )
            .unwrap();
        connection
            .execute_batch(include_str!(
                "migrations/0022_exchange_transactions_btc_fee_sats.sql"
            ))
            .unwrap();
        let fee: f64 = connection
            .query_row(
                "SELECT fee FROM exchange_transactions WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let fiat_fee: f64 = connection
            .query_row(
                "SELECT fee FROM exchange_transactions WHERE id = 2",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let balance: i64 = connection
            .query_row(
                "SELECT exchange_balance_sats FROM addresses WHERE address = 'bc1qlegacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fee, 10_000.0);
        assert_eq!(fiat_fee, 2.5);
        assert_eq!(balance, 99_990_000);
        connection
            .execute_batch(include_str!(
                "migrations/0022_exchange_transactions_btc_fee_sats.sql"
            ))
            .unwrap();
        let fee_after: f64 = connection
            .query_row(
                "SELECT fee FROM exchange_transactions WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let balance_after: i64 = connection
            .query_row(
                "SELECT exchange_balance_sats FROM addresses WHERE address = 'bc1qlegacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(fee_after, 10_000.0);
        assert_eq!(balance_after, 99_990_000);
    }
    #[test]
    fn btc_fee_triggers_reject_fractional_fees() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        connection
            .execute_batch(
                "INSERT INTO exchanges_types (id, name, slug) VALUES (900, 'Test', 'test');
                 INSERT INTO exchange_accounts (
                     id, name, provider_id, created_at, last_modified_at
                 ) VALUES (1, 'Test', 900, 0, 0);",
            )
            .unwrap();
        let insert = |fee: f64, currency: &str| {
            connection.execute(
                "INSERT INTO exchange_transactions (
                     exchange_id, fingerprint, timestamp, tx_type, amount_sats,
                     currency, fee, fee_currency, raw_data, match_state,
                     created_at, updated_at
                 ) VALUES (1, ?1, 1, 'buy', 100000000, 'BTC', ?2, ?3, '{}',
                           'unmatched', 0, 0)",
                rusqlite::params![format!("{fee}-{currency}"), fee, currency],
            )
        };
        assert!(insert(0.0001, "BTC").is_err());
        assert!(insert(10_000.0, "BTC").is_ok());
        assert!(insert(2.5, "EUR").is_ok());
    }
    #[test]
    fn addresses_schema_keeps_the_existing_column_order() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        assert_eq!(
            column_names(&connection, "addresses"),
            [
                "address",
                "chain_funded_txo_count",
                "chain_funded_txo_sum",
                "chain_spent_txo_count",
                "chain_spent_txo_sum",
                "chain_tx_count",
                "mempool_funded_txo_count",
                "mempool_funded_txo_sum",
                "mempool_spent_txo_count",
                "mempool_spent_txo_sum",
                "mempool_tx_count",
                "description",
                "exchange_id",
                "wallet_id",
                "tx_index_rate_limited",
                "exchange_balance_sats",
            ],
        );
        assert_eq!(
            column_names(&connection, "exchanges_types"),
            [
                "id",
                "name",
                "slug",
                "category_id",
                "custody_model",
                "brand_color"
            ],
        );
        assert_eq!(
            column_names(&connection, "wallets_types"),
            [
                "id",
                "name",
                "slug",
                "category_id",
                "custody_model",
                "platform",
                "brand_color",
            ],
        );
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
            ],
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
            ],
        );
    }
    #[test]
    fn seeded_providers_carry_a_hex_brand_color() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let colors = |sql: &str| -> Vec<String> {
            connection
                .prepare(sql)
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let exchange_colors = colors("SELECT brand_color FROM exchanges_types ORDER BY id");
        let wallet_colors = colors("SELECT brand_color FROM wallets_types ORDER BY id");
        assert_eq!(exchange_colors.len(), 16);
        assert_eq!(wallet_colors.len(), 18);
        for color in exchange_colors.iter().chain(wallet_colors.iter()) {
            assert_eq!(color.len(), 7, "unexpected brand color {color}");
            assert!(color.starts_with('#'), "unexpected brand color {color}");
            assert!(
                color[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "unexpected brand color {color}",
            );
            assert_eq!(
                color,
                &color.to_uppercase(),
                "brand color {color} is lowercase"
            );
        }
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
        assert!(insert("bc1qexchange", Some(999), None).is_err());
        assert!(insert("bc1qwallet", None, Some(999)).is_err());
        insert("bc1qexchange", Some(1), None).unwrap();
        insert("bc1qwallet", None, Some(1)).unwrap();
        insert("bc1qplain", None, None).unwrap();
        assert!(insert("bc1qboth", Some(1), Some(1)).is_err());
        let address_rows: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(address_rows, 3);
    }
    #[test]
    fn exchange_migration_creates_tables_and_address_balance_column() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
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
    fn catalog_taxonomy_data_classifies_seeded_rows() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let exchange_categories: i64 = connection
            .query_row("SELECT COUNT(*) FROM exchange_categories", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(exchange_categories, 8);
        let wallet_categories: i64 = connection
            .query_row("SELECT COUNT(*) FROM wallet_categories", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(wallet_categories, 9);
        let (slug, category, custody): (String, String, String) = connection
            .query_row(
                "SELECT t.slug, c.name, t.custody_model
                 FROM exchanges_types t
                 JOIN exchange_categories c ON c.id = t.category_id
                 WHERE t.name = 'Binance'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(slug, "binance");
        assert_eq!(category, "Centralized exchange");
        assert_eq!(custody, "custodial");
        let (slug, category, custody, platform): (String, String, String, String) = connection
            .query_row(
                "SELECT t.slug, c.name, t.custody_model, t.platform
                 FROM wallets_types t
                 JOIN wallet_categories c ON c.id = t.category_id
                 WHERE t.name = 'Ledger'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(slug, "ledger");
        assert_eq!(category, "Hardware wallet");
        assert_eq!(custody, "self_custody");
        assert_eq!(platform, "hardware");
        let duplicate_slugs: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM (
                     SELECT slug FROM exchanges_types GROUP BY slug HAVING COUNT(*) > 1
                     UNION ALL
                     SELECT slug FROM wallets_types GROUP BY slug HAVING COUNT(*) > 1
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(duplicate_slugs, 0);
    }
    #[test]
    fn complete_seed_sequence_runs_on_a_fresh_database() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        run_migrations(&mut connection, true).unwrap();
        assert_eq!(user_version(&connection), CURRENT_SCHEMA_VERSION);
        let exchange_addresses: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(exchange_addresses, 296);
        let prices: i64 = connection
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(
            prices > 290_000,
            "unexpectedly few imported prices: {prices}"
        );
    }
    #[test]
    fn partially_migrated_database_resumes_without_losing_data() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_sql(&connection);
        stamp(&connection, 15);
        connection
            .execute(
                "INSERT INTO addresses (
                    address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count, description
                 ) VALUES ('bc1qexisting', 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 'My wallet')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO setting (key, value) VALUES ('default_fiat_currency', 'EUR')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO historical_prices (close_time, close, symbol)
                 VALUES (1, 42000.0, 'USD')",
                [],
            )
            .unwrap();
        run_migrations(&mut connection, true).unwrap();
        assert_eq!(user_version(&connection), CURRENT_SCHEMA_VERSION);
        let (description, exchange_balance): (String, i64) = connection
            .query_row(
                "SELECT description, exchange_balance_sats FROM addresses
                 WHERE address = 'bc1qexisting'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(description, "My wallet");
        assert_eq!(exchange_balance, 0);
        let setting: String = connection
            .query_row(
                "SELECT value FROM setting WHERE key = 'default_fiat_currency'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(setting, "EUR");
        let prices: i64 = connection
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(prices, 1);
        let seeded_addresses: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            seeded_addresses > 0,
            "the address seed should run above the stored version",
        );
    }
    #[test]
    fn provider_brand_colors_are_backfilled_for_existing_rows() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_sql(&connection);
        stamp(&connection, 19);
        connection
            .execute("UPDATE exchanges_types SET brand_color = NULL", [])
            .unwrap();
        connection
            .execute("UPDATE wallets_types SET brand_color = NULL", [])
            .unwrap();
        connection
            .execute(
                "UPDATE wallets_types SET brand_color = '#123456' WHERE slug = 'ledger'",
                [],
            )
            .unwrap();
        run_migrations(&mut connection, false).unwrap();
        assert_eq!(user_version(&connection), CURRENT_SCHEMA_VERSION);
        let (binance, kraken): (String, String) = connection
            .query_row(
                "SELECT
                     (SELECT brand_color FROM exchanges_types WHERE slug = 'binance'),
                     (SELECT brand_color FROM exchanges_types WHERE slug = 'kraken')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(binance, "#F0B90B");
        assert_eq!(kraken, "#5741D9");
        let (ledger, metamask): (String, String) = connection
            .query_row(
                "SELECT
                     (SELECT brand_color FROM wallets_types WHERE slug = 'ledger'),
                     (SELECT brand_color FROM wallets_types WHERE slug = 'metamask')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(ledger, "#123456", "a user color must be preserved");
        assert_eq!(metamask, "#F6851B");
    }
    #[test]
    fn newer_database_fails_loudly() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_sql(&connection);
        stamp(&connection, CURRENT_SCHEMA_VERSION + 1);
        let error = run_migrations(&mut connection, false).unwrap_err();
        assert!(
            error.to_string().contains("newer than this build"),
            "unexpected error: {error}",
        );
    }
    #[test]
    fn in_memory_databases_advance_past_the_seed_versions() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        assert_eq!(user_version(&connection), CURRENT_SCHEMA_VERSION);
        assert!(column_names(&connection, "addresses")
            .iter()
            .any(|column| column == "tx_index_rate_limited"),);
        let prices: i64 = connection
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(prices, 0);
        let seeded_addresses: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE exchange_id IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seeded_addresses, 0);
    }
    #[test]
    fn sql_migration_lookup_returns_the_table_file() {
        assert!(sql_of("addresses").contains("CREATE TABLE IF NOT EXISTS addresses"));
        assert!(
            sql_of("historical_prices").contains("CREATE TABLE IF NOT EXISTS historical_prices"),
        );
    }
}
