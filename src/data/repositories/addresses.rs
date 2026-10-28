//! SQL access to the `addresses` table.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use super::exchanges as exchange_repository;
use crate::data::{Database, DatabaseError};
use crate::domain::address::{AddressEntity, AddressListItem, AddressScopeFilter};
use crate::domain::table::SortDirection;

/// Sortable columns of the addresses overview table: the fixed allowlist
/// the dynamic `ORDER BY` identifier is selected from (Section 6.3 of
/// CONTRIBUTING.md). Client sort keys never reach SQL directly. The address
/// type is not sortable: it derives from the address string at the domain
/// boundary, not from a stored column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSortColumn {
    /// Address string, API key `address`.
    Address,
    /// Description, API key `label`.
    Label,
    /// Confirmed transaction count, API key `txs`.
    ConfirmedTxCount,
    /// Spendable balance in satoshis, API key `balance`.
    SpendableBalance,
    /// Balance in the latest fiat price, API key `price`. The displayed USD
    /// value is the spendable balance times one latest price shared by all
    /// rows, so ordering by the balance orders by the price exactly (a
    /// constant positive multiplier preserves order).
    CurrentPrice,
    /// Oldest stored confirmed transaction time, API key `first_tx`.
    FirstTxTime,
    /// Newest stored confirmed transaction time, API key `last_tx`.
    LastTxTime,
    /// Owner scope (exchange, then wallet, then unclassified), API key
    /// `scope`.
    Scope,
}

impl AddressSortColumn {
    /// Parse the client-facing sort key; unknown keys are rejected by the
    /// caller at the service boundary.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "address" => Some(Self::Address),
            "label" => Some(Self::Label),
            "txs" => Some(Self::ConfirmedTxCount),
            "balance" => Some(Self::SpendableBalance),
            "price" => Some(Self::CurrentPrice),
            "first_tx" => Some(Self::FirstTxTime),
            "last_tx" => Some(Self::LastTxTime),
            "scope" => Some(Self::Scope),
            _ => None,
        }
    }

    /// `ORDER BY` expression of the column. Balances and counts are stored
    /// integers and transaction times epoch seconds, so ordering follows the
    /// typed storage class, never formatted display strings (Section 5.11 of
    /// CONTRIBUTING.md).
    fn order_expression(self) -> &'static str {
        match self {
            Self::Address => "a.address",
            Self::Label => "a.description",
            Self::ConfirmedTxCount => "confirmed_tx_count",
            Self::SpendableBalance | Self::CurrentPrice => SPENDABLE_BALANCE_SATS,
            Self::FirstTxTime => "s.first_tx_time",
            Self::LastTxTime => "s.last_tx_time",
            Self::Scope => {
                "CASE WHEN a.exchange_id IS NOT NULL THEN 0 \
                 WHEN a.wallet_id IS NOT NULL THEN 1 ELSE 2 END"
            }
        }
    }
}

/// Sorting and scope filtering of a [`list_overview_rows`] call.
#[derive(Debug, Clone, Copy, Default)]
pub struct AddressOverviewFilter {
    /// Column of the [`AddressSortColumn`] allowlist and direction; `None`
    /// is the default order, spendable balance descending.
    pub sort: Option<(AddressSortColumn, SortDirection)>,
    /// Scope filter of the rows.
    pub scope: AddressScopeFilter,
}

/// Spendable balance expression shared by the overview listing, its balance
/// sorting, and the KPI totals.
///
/// Exchange-scoped addresses display their stored ledger balance. The
/// balance of a provider is kept on its alphabetically first address; the
/// other addresses of that provider store zero. The real on-chain statistics
/// remain untouched for the exchange page and reconciliation.
const SPENDABLE_BALANCE_SATS: &str =
    "CASE WHEN a.exchange_id IS NOT NULL THEN a.exchange_balance_sats \
     ELSE a.chain_funded_txo_sum - a.chain_spent_txo_sum END";

/// One row of [`list_overview_rows`]: the stored address, the resolved
/// owner catalog names, its confirmed-transaction statistics (zero/none
/// for unsynced addresses), and its reported spendable balance (the stored
/// exchange ledger balance for exchange-scoped addresses).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressOverviewRow {
    pub entity: AddressEntity,
    pub exchange_name: Option<String>,
    pub wallet_name: Option<String>,
    pub confirmed_tx_count: i64,
    pub first_tx_time: Option<i64>,
    pub last_tx_time: Option<i64>,
    pub spendable_balance_sats: i64,
}

/// The `WHERE` fragment of the scope filter. This is a fixed allowlist
/// value, never client input, so it is safe to place in the query.
fn scope_fragment(scope: AddressScopeFilter) -> &'static str {
    match scope {
        AddressScopeFilter::All => "1 = 1",
        AddressScopeFilter::Wallets => "a.wallet_id IS NOT NULL",
        AddressScopeFilter::Exchanges => "a.exchange_id IS NOT NULL",
    }
}

/// Read the addresses overview rows matching `filter`, sorted in the
/// backend. The confirmed-transaction statistics come from a grouped
/// subquery so sorting by them never leaves SQL.
pub fn list_overview_rows(
    database: &Database,
    filter: &AddressOverviewFilter,
) -> Result<Vec<AddressOverviewRow>, DatabaseError> {
    let where_scope = scope_fragment(filter.scope);
    let (column, direction) = filter
        .sort
        .map(|(column, direction)| (column.order_expression(), direction.as_sql()))
        .unwrap_or(("spendable_balance_sats", "DESC"));
    // The secondary key keeps rows with equal sort values in a deterministic
    // order (Section 5.11 of CONTRIBUTING.md).
    let sql = format!(
        "SELECT a.address,
                a.chain_funded_txo_count, a.chain_funded_txo_sum, a.chain_spent_txo_count,
                a.chain_spent_txo_sum, a.chain_tx_count,
                a.mempool_funded_txo_count, a.mempool_funded_txo_sum,
                a.mempool_spent_txo_count, a.mempool_spent_txo_sum, a.mempool_tx_count,
                a.description, a.exchange_id, a.wallet_id, a.tx_index_rate_limited,
                e.name, w.name,
                COALESCE(s.confirmed_tx_count, 0) AS confirmed_tx_count,
                s.first_tx_time, s.last_tx_time,
                {SPENDABLE_BALANCE_SATS} AS spendable_balance_sats
         FROM addresses a
         LEFT JOIN exchanges_types e ON e.id = a.exchange_id
         LEFT JOIN wallets_types w ON w.id = a.wallet_id
         LEFT JOIN (
             SELECT address, COUNT(*) AS confirmed_tx_count,
                    MIN(block_time) AS first_tx_time, MAX(block_time) AS last_tx_time
             FROM transactions WHERE confirmed = 1 GROUP BY address
         ) s ON s.address = a.address
         WHERE {where_scope}
         ORDER BY {column} {direction}, a.address {direction}"
    );
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok(AddressOverviewRow {
            entity: entity_from_row(0, 11, row)?,
            exchange_name: row.get(15)?,
            wallet_name: row.get(16)?,
            confirmed_tx_count: row.get(17)?,
            first_tx_time: row.get(18)?,
            last_tx_time: row.get(19)?,
            spendable_balance_sats: row.get(20)?,
        })
    })?;
    let mut values = Vec::new();
    for value in rows {
        values.push(value.map_err(|err| DatabaseError::new(err.to_string()))?);
    }
    Ok(values)
}

/// Global KPI totals of the addresses page: the stored address count, the
/// confirmed transactions across all of them, and the total spendable
/// balance in satoshis. The totals are unfiltered; the address count excludes
/// exchange-scoped addresses without a stored ledger balance, matching the
/// rows the overview lists, while their stored transactions still count.
/// Exchange-scoped addresses contribute their stored ledger balance (see
/// [`SPENDABLE_BALANCE_SATS`]). The cards therefore do not change when the
/// table's scope filter changes.
pub fn overview_totals(database: &Database) -> Result<(i64, i64, i64), DatabaseError> {
    let sql = format!(
        "SELECT (SELECT COUNT(*) FROM addresses a
                 WHERE a.exchange_id IS NULL OR a.exchange_balance_sats <> 0),
                (SELECT COUNT(*) FROM transactions WHERE confirmed = 1),
                (SELECT COALESCE(SUM({SPENDABLE_BALANCE_SATS}), 0)
                 FROM addresses a)"
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(Into::into)
}

/// Update the description and owner classification of a stored address.
/// Both owner ids are written as given (either may clear the previous
/// classification); the either-or invariant stays enforced by the migration
/// 0006 triggers. Returns whether the address exists.
pub fn update_address_details(
    database: &Database,
    address: &str,
    description: Option<&str>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<bool, DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous_exchange: Option<i64> = transaction
        .query_row(
            "SELECT exchange_id FROM addresses WHERE address = ?1",
            [address],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let updated = transaction.execute(
        "UPDATE addresses
         SET description = ?2, exchange_id = ?3, wallet_id = ?4
         WHERE address = ?1",
        params![address, description, exchange_id, wallet_id],
    )?;
    if updated == 1 {
        if let Some(previous) = previous_exchange {
            exchange_repository::refresh_provider_balance_tx(&transaction, previous)?;
        }
        if let Some(current) = exchange_id {
            exchange_repository::refresh_provider_balance_tx(&transaction, current)?;
        }
    }
    transaction.commit()?;
    Ok(updated == 1)
}

pub fn delete_address(database: &Database, address: &str) -> Result<(), DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let exchange_id: Option<i64> = transaction
        .query_row(
            "SELECT exchange_id FROM addresses WHERE address = ?1",
            [address],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    transaction.execute("DELETE FROM addresses WHERE address = ?1", [address])?;
    if let Some(exchange_id) = exchange_id {
        exchange_repository::refresh_provider_balance_tx(&transaction, exchange_id)?;
    }
    transaction.commit()?;
    Ok(())
}

/// Insert or refresh an address and, when provided, its classification.
///
/// The mempool statistics are always refreshed. The description is kept
/// on conflict when the incoming row omits it. For the owner reference,
/// an incoming exchange or wallet id replaces the previous classification
/// entirely (so switching from wallet to exchange clears the wallet), and
/// an incoming row without either id leaves the stored owner untouched.
/// This also matches the either-or invariant enforced by the migration
/// 0006 triggers.
///
/// `tx_index_rate_limited` is deliberately not written here: new rows take
/// the schema default, and a stats refresh must never clear the marker of
/// an interrupted indexing walk. Only
/// [`set_tx_index_rate_limited`] changes it.
pub fn insert_address(database: &Database, value: &AddressEntity) -> Result<(), DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let previous: Option<Option<i64>> = transaction
        .query_row(
            "SELECT exchange_id FROM addresses WHERE address = ?1",
            [&value.address],
            |row| row.get(0),
        )
        .optional()?;
    let existed = previous.is_some();
    let previous_exchange = previous.flatten();
    transaction.execute(
        "INSERT INTO addresses (
            address, description, exchange_id, wallet_id,
            chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
            chain_spent_txo_sum, chain_tx_count,
            mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
            mempool_spent_txo_sum, mempool_tx_count
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(address) DO UPDATE SET
            description = COALESCE(excluded.description, addresses.description),
            exchange_id = COALESCE(
                excluded.exchange_id,
                CASE WHEN excluded.wallet_id IS NOT NULL
                    THEN NULL ELSE addresses.exchange_id END),
            wallet_id = COALESCE(
                excluded.wallet_id,
                CASE WHEN excluded.exchange_id IS NOT NULL
                    THEN NULL ELSE addresses.wallet_id END),
            chain_funded_txo_count = excluded.chain_funded_txo_count,
            chain_funded_txo_sum = excluded.chain_funded_txo_sum,
            chain_spent_txo_count = excluded.chain_spent_txo_count,
            chain_spent_txo_sum = excluded.chain_spent_txo_sum,
            chain_tx_count = excluded.chain_tx_count,
            mempool_funded_txo_count = excluded.mempool_funded_txo_count,
            mempool_funded_txo_sum = excluded.mempool_funded_txo_sum,
            mempool_spent_txo_count = excluded.mempool_spent_txo_count,
            mempool_spent_txo_sum = excluded.mempool_spent_txo_sum,
            mempool_tx_count = excluded.mempool_tx_count",
        params![
            value.address,
            value.description,
            value.exchange_id,
            value.wallet_id,
            value.chain_funded_txo_count,
            value.chain_funded_txo_sum,
            value.chain_spent_txo_count,
            value.chain_spent_txo_sum,
            value.chain_tx_count,
            value.mempool_funded_txo_count,
            value.mempool_funded_txo_sum,
            value.mempool_spent_txo_count,
            value.mempool_spent_txo_sum,
            value.mempool_tx_count,
        ],
    )?;
    // A stats refresh (same owner) must preserve the stored ledger balance.
    // Only a new address or an owner change can move the anchor.
    let owner_changed =
        !existed || value.wallet_id.is_some() || value.exchange_id != previous_exchange;
    if owner_changed {
        if let Some(previous) = previous_exchange {
            exchange_repository::refresh_provider_balance_tx(&transaction, previous)?;
        }
        if let Some(current) = value.exchange_id {
            exchange_repository::refresh_provider_balance_tx(&transaction, current)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

/// Shared row mapping for the entity list and the joined item list.
///
/// `entity_first` is the column index of `address`; `description_first`
/// the index of the first classification column, with
/// `tx_index_rate_limited` right after the owner ids. Both callers must
/// keep their column lists aligned with these offsets.
fn entity_from_row(
    entity_first: usize,
    description_first: usize,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<AddressEntity> {
    Ok(AddressEntity {
        address: row.get(entity_first)?,
        description: row.get(description_first)?,
        exchange_id: row.get(description_first + 1)?,
        wallet_id: row.get(description_first + 2)?,
        tx_index_rate_limited: row.get(description_first + 3)?,
        chain_funded_txo_count: row.get(entity_first + 1)?,
        chain_funded_txo_sum: row.get(entity_first + 2)?,
        chain_spent_txo_count: row.get(entity_first + 3)?,
        chain_spent_txo_sum: row.get(entity_first + 4)?,
        chain_tx_count: row.get(entity_first + 5)?,
        mempool_funded_txo_count: row.get(entity_first + 6)?,
        mempool_funded_txo_sum: row.get(entity_first + 7)?,
        mempool_spent_txo_count: row.get(entity_first + 8)?,
        mempool_spent_txo_sum: row.get(entity_first + 9)?,
        mempool_tx_count: row.get(entity_first + 10)?,
    })
}

pub fn get_address(
    database: &Database,
    address: &str,
) -> Result<Option<AddressEntity>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT a.address,
                a.chain_funded_txo_count, a.chain_funded_txo_sum, a.chain_spent_txo_count,
                a.chain_spent_txo_sum, a.chain_tx_count,
                a.mempool_funded_txo_count, a.mempool_funded_txo_sum,
                a.mempool_spent_txo_count, a.mempool_spent_txo_sum, a.mempool_tx_count,
                a.description, a.exchange_id, a.wallet_id, a.tx_index_rate_limited
             FROM addresses a WHERE a.address = ?1",
            [address],
            |row| entity_from_row(0, 11, row),
        )
        .optional()
        .map_err(Into::into)
}

/// Stored addresses with the owner catalog names resolved for display.
///
/// The descriptions and catalog ids come through `entity_from_row`, while
/// the joined catalog names carry the user-facing labels.
pub fn list_address_items(database: &Database) -> Result<Vec<AddressListItem>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT a.address,
            a.chain_funded_txo_count, a.chain_funded_txo_sum, a.chain_spent_txo_count,
            a.chain_spent_txo_sum, a.chain_tx_count,
            a.mempool_funded_txo_count, a.mempool_funded_txo_sum,
            a.mempool_spent_txo_count, a.mempool_spent_txo_sum, a.mempool_tx_count,
            a.description, a.exchange_id, a.wallet_id, a.tx_index_rate_limited,
            e.name, w.name
         FROM addresses a
         LEFT JOIN exchanges_types e ON e.id = a.exchange_id
         LEFT JOIN wallets_types w ON w.id = a.wallet_id
         ORDER BY a.address",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AddressListItem {
            entity: entity_from_row(0, 11, row)?,
            exchange_name: row.get(15)?,
            wallet_name: row.get(16)?,
        })
    })?;
    let mut values = Vec::new();
    for value in rows {
        values.push(value.map_err(|err| DatabaseError::new(err.to_string()))?);
    }
    Ok(values)
}

/// Mark whether transaction indexing of the address stopped on a mempool
/// rate limit. Set by the sync workflow when a walk is interrupted, cleared
/// when a walk completes.
pub fn set_tx_index_rate_limited(
    database: &Database,
    address: &str,
    rate_limited: bool,
) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    connection.execute(
        "UPDATE addresses SET tx_index_rate_limited = ?2 WHERE address = ?1",
        params![address, rate_limited],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::address::AddressEntity;

    fn address(address: &str) -> AddressEntity {
        AddressEntity {
            address: address.to_string(),
            ..AddressEntity::default()
        }
    }

    #[test]
    fn classification_round_trips_through_insert_get_and_items() {
        let database = Database::open_in_memory().unwrap();

        let mut exchange = address("bc1qexchange");
        exchange.description = Some("Binance hot wallet 3".to_string());
        exchange.exchange_id = Some(1);
        insert_address(&database, &exchange).unwrap();

        let mut wallet = address("bc1qwallet");
        wallet.description = Some("Cold storage".to_string());
        wallet.wallet_id = Some(2);
        insert_address(&database, &wallet).unwrap();

        let stored = get_address(&database, "bc1qexchange").unwrap().unwrap();
        assert_eq!(stored.description.as_deref(), Some("Binance hot wallet 3"));
        assert_eq!(stored.exchange_id, Some(1));
        assert_eq!(stored.wallet_id, None);

        let items = list_address_items(&database).unwrap();
        assert_eq!(items.len(), 2);

        let exchange_item = items
            .iter()
            .find(|item| item.entity.address == "bc1qexchange")
            .unwrap();
        assert_eq!(exchange_item.exchange_name.as_deref(), Some("Binance"));
        assert_eq!(exchange_item.wallet_name, None);

        let wallet_item = items
            .iter()
            .find(|item| item.entity.address == "bc1qwallet")
            .unwrap();
        assert_eq!(wallet_item.wallet_name.as_deref(), Some("Trezor"));
        assert_eq!(wallet_item.exchange_name, None);
    }

    #[test]
    fn rate_limited_marker_round_trips_and_survives_stats_refreshes() {
        let database = Database::open_in_memory().unwrap();

        insert_address(&database, &address("bc1qflagged")).unwrap();
        let stored = get_address(&database, "bc1qflagged").unwrap().unwrap();
        assert!(!stored.tx_index_rate_limited);

        set_tx_index_rate_limited(&database, "bc1qflagged", true).unwrap();
        let stored = get_address(&database, "bc1qflagged").unwrap().unwrap();
        assert!(stored.tx_index_rate_limited);

        let items = list_address_items(&database).unwrap();
        assert!(items[0].entity.tx_index_rate_limited);

        // A stats refresh must not clear the interrupted-walk marker.
        let mut stats_only = address("bc1qflagged");
        stats_only.chain_tx_count = 9;
        insert_address(&database, &stats_only).unwrap();
        let stored = get_address(&database, "bc1qflagged").unwrap().unwrap();
        assert!(stored.tx_index_rate_limited);

        set_tx_index_rate_limited(&database, "bc1qflagged", false).unwrap();
        let stored = get_address(&database, "bc1qflagged").unwrap().unwrap();
        assert!(!stored.tx_index_rate_limited);
    }

    #[test]
    fn refreshing_stats_preserves_description_and_owner() {
        let database = Database::open_in_memory().unwrap();

        let mut classified = address("bc1qclassified");
        classified.description = Some("Savings".to_string());
        classified.wallet_id = Some(1);
        classified.chain_tx_count = 5;
        insert_address(&database, &classified).unwrap();

        // A later stats refresh without a description or owner must not wipe
        // the stored classification (COALESCE on conflict).
        let mut stats_only = address("bc1qclassified");
        stats_only.chain_tx_count = 8;
        insert_address(&database, &stats_only).unwrap();

        let stored = get_address(&database, "bc1qclassified").unwrap().unwrap();
        assert_eq!(stored.description.as_deref(), Some("Savings"));
        assert_eq!(stored.wallet_id, Some(1));
        assert_eq!(stored.exchange_id, None);
        assert_eq!(stored.chain_tx_count, 8);
    }

    #[test]
    fn reimport_with_new_owner_overrides_the_previous_owner() {
        let database = Database::open_in_memory().unwrap();

        let mut wallet = address("bc1qrecast");
        wallet.description = Some("First tag".to_string());
        wallet.wallet_id = Some(1);
        insert_address(&database, &wallet).unwrap();

        let mut exchange = address("bc1qrecast");
        exchange.description = Some("Second tag".to_string());
        exchange.exchange_id = Some(2);
        insert_address(&database, &exchange).unwrap();

        let stored = get_address(&database, "bc1qrecast").unwrap().unwrap();
        assert_eq!(stored.description.as_deref(), Some("Second tag"));
        assert_eq!(stored.exchange_id, Some(2));
        assert_eq!(stored.wallet_id, None);
    }

    #[test]
    fn owner_ids_must_exist_and_must_not_conflict() {
        let database = Database::open_in_memory().unwrap();

        // Both owners at once violate the either-or invariant, enforced by
        // the schema triggers.
        let mut both = address("bc1qboth");
        both.exchange_id = Some(1);
        both.wallet_id = Some(2);
        assert!(insert_address(&database, &both).is_err());

        // Unknown ids fall through to the foreign keys.
        let mut unknown = address("bc1qunknown");
        unknown.exchange_id = Some(999);
        assert!(insert_address(&database, &unknown).is_err());
    }

    /// Insert one address with a balance and a classification.
    fn overview_address(
        database: &Database,
        address_str: &str,
        balance_sats: i64,
        description: Option<&str>,
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
    ) {
        let mut entity = address(address_str);
        entity.chain_funded_txo_sum = balance_sats;
        entity.description = description.map(str::to_string);
        entity.exchange_id = exchange_id;
        entity.wallet_id = wallet_id;
        insert_address(database, &entity).unwrap();
    }

    /// Store one confirmed transaction for the address at `block_time`.
    fn confirmed_tx(database: &Database, address: &str, txid: &str, block_time: i64) {
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO transactions (
                    txid, address, version, locktime, size, weight, fee,
                    confirmed, block_time
                 ) VALUES (?1, ?2, 2, 0, 100, 400, 1000, 1, ?3)",
                params![txid, address, block_time],
            )
            .unwrap();
    }

    #[test]
    fn sort_keys_parse_from_the_client_facing_allowlist() {
        assert_eq!(
            AddressSortColumn::from_key("address"),
            Some(AddressSortColumn::Address)
        );
        assert_eq!(
            AddressSortColumn::from_key("label"),
            Some(AddressSortColumn::Label)
        );
        assert_eq!(
            AddressSortColumn::from_key("txs"),
            Some(AddressSortColumn::ConfirmedTxCount)
        );
        assert_eq!(
            AddressSortColumn::from_key("balance"),
            Some(AddressSortColumn::SpendableBalance)
        );
        assert_eq!(
            AddressSortColumn::from_key("price"),
            Some(AddressSortColumn::CurrentPrice)
        );
        assert_eq!(
            AddressSortColumn::from_key("first_tx"),
            Some(AddressSortColumn::FirstTxTime)
        );
        assert_eq!(
            AddressSortColumn::from_key("last_tx"),
            Some(AddressSortColumn::LastTxTime)
        );
        assert_eq!(
            AddressSortColumn::from_key("scope"),
            Some(AddressSortColumn::Scope)
        );
        assert_eq!(AddressSortColumn::from_key("type"), None);
        assert_eq!(AddressSortColumn::from_key("address; DROP TABLE"), None);
    }

    #[test]
    fn overview_rows_sort_in_the_backend_with_stats_and_a_deterministic_key() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qbbb", 100, Some("b-label"), None, None);
        overview_address(&database, "bc1qaaa", 100, Some("a-label"), None, None);
        overview_address(&database, "bc1qccc", 300, None, None, None);
        confirmed_tx(&database, "bc1qbbb", "tx-b1", 1_600_000_000);
        confirmed_tx(&database, "bc1qbbb", "tx-b2", 1_700_000_000);
        confirmed_tx(&database, "bc1qccc", "tx-c1", 1_500_000_000);

        // The default order is spendable balance descending. Equal balances
        // fall back to the address as the deterministic key, in the sort
        // direction (descending here).
        let rows = list_overview_rows(&database, &AddressOverviewFilter::default()).unwrap();
        let order: Vec<&str> = rows.iter().map(|row| row.entity.address.as_str()).collect();
        assert_eq!(order, ["bc1qccc", "bc1qbbb", "bc1qaaa"]);
        // The stats ride along, unsynced addresses read as zero.
        assert_eq!(rows[0].confirmed_tx_count, 1);
        assert_eq!(rows[0].first_tx_time, Some(1_500_000_000));
        assert_eq!(rows[1].confirmed_tx_count, 2);
        assert_eq!(rows[1].first_tx_time, Some(1_600_000_000));
        assert_eq!(rows[1].last_tx_time, Some(1_700_000_000));
        assert_eq!(rows[2].confirmed_tx_count, 0);
        assert_eq!(rows[2].first_tx_time, None);

        // Sorting by the transaction statistics never leaves SQL: bc1qbbb
        // has two confirmed transactions, bc1qccc one, bc1qaaa none.
        let rows = list_overview_rows(
            &database,
            &AddressOverviewFilter {
                sort: Some((AddressSortColumn::ConfirmedTxCount, SortDirection::Desc)),
                ..AddressOverviewFilter::default()
            },
        )
        .unwrap();
        let order: Vec<&str> = rows.iter().map(|row| row.entity.address.as_str()).collect();
        assert_eq!(order, ["bc1qbbb", "bc1qccc", "bc1qaaa"]);

        let rows = list_overview_rows(
            &database,
            &AddressOverviewFilter {
                sort: Some((AddressSortColumn::Label, SortDirection::Asc)),
                ..AddressOverviewFilter::default()
            },
        )
        .unwrap();
        let order: Vec<&str> = rows.iter().map(|row| row.entity.address.as_str()).collect();
        // NULL descriptions sort first in ascending SQLite order.
        assert_eq!(order, ["bc1qccc", "bc1qaaa", "bc1qbbb"]);
    }

    #[test]
    fn overview_rows_filter_by_scope_and_totals_stay_global() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qwallet", 100, None, None, Some(1));
        overview_address(&database, "bc1qexchange", 200, None, Some(1), None);
        overview_address(&database, "bc1qplain", 300, None, None, None);
        confirmed_tx(&database, "bc1qwallet", "tx-w1", 1_600_000_000);

        let wallets = list_overview_rows(
            &database,
            &AddressOverviewFilter {
                scope: AddressScopeFilter::Wallets,
                ..AddressOverviewFilter::default()
            },
        )
        .unwrap();
        assert_eq!(wallets.len(), 1);
        assert_eq!(wallets[0].entity.address, "bc1qwallet");
        assert_eq!(wallets[0].wallet_name.as_deref(), Some("Ledger"));
        assert_eq!(wallets[0].spendable_balance_sats, 100);

        let exchanges = list_overview_rows(
            &database,
            &AddressOverviewFilter {
                scope: AddressScopeFilter::Exchanges,
                ..AddressOverviewFilter::default()
            },
        )
        .unwrap();
        assert_eq!(exchanges.len(), 1);
        assert_eq!(exchanges[0].entity.address, "bc1qexchange");
        assert_eq!(exchanges[0].exchange_name.as_deref(), Some("Binance"));
        // Exchange balances are zero here because no ledger account exists,
        // but the stored on-chain stats are preserved.
        assert_eq!(exchanges[0].spendable_balance_sats, 0);
        assert_eq!(exchanges[0].entity.chain_funded_txo_sum, 200);

        // The KPI totals ignore the filter. The address count excludes the
        // exchange address without a ledger balance (the overview does not
        // list it); the balance total excludes exchange-scoped on-chain stats.
        let (count, confirmed, balance) = overview_totals(&database).unwrap();
        assert_eq!(count, 2);
        assert_eq!(confirmed, 1);
        assert_eq!(balance, 400);
    }

    #[test]
    fn overview_address_count_skips_exchange_addresses_without_a_ledger_balance() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qwallet", 100, None, None, Some(1));
        overview_address(&database, "bc1qplain", 300, None, None, None);
        overview_address(&database, "bc1qexchangezero", 200, None, Some(1), None);
        overview_address(&database, "bc1qexchangebalance", 400, None, Some(1), None);
        database
            .connection()
            .unwrap()
            .execute(
                "UPDATE addresses SET exchange_balance_sats = 5000 \
                 WHERE address = 'bc1qexchangebalance'",
                [],
            )
            .unwrap();

        let (count, _, balance) = overview_totals(&database).unwrap();
        // Wallet and unclassified addresses always count; an exchange
        // address counts once its ledger balance is non-zero.
        assert_eq!(count, 3);
        assert_eq!(balance, 400 + 5_000);
    }

    #[test]
    fn exchange_scoped_addresses_sort_as_zero_balance() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qwallet", 100, None, None, Some(1));
        overview_address(&database, "bc1qexchange", 9_000, None, Some(1), None);
        overview_address(&database, "bc1qplain", 300, None, None, None);

        // The stored exchange balance is the highest, but the overview
        // reports it as zero, so it sorts after the other addresses.
        let rows = list_overview_rows(&database, &AddressOverviewFilter::default()).unwrap();
        let order: Vec<&str> = rows.iter().map(|row| row.entity.address.as_str()).collect();
        assert_eq!(order, ["bc1qplain", "bc1qwallet", "bc1qexchange"]);
        assert_eq!(rows[2].spendable_balance_sats, 0);
        assert_eq!(rows[2].entity.chain_funded_txo_sum, 9_000);
    }

    #[test]
    fn stored_exchange_ledger_balance_is_used_by_overview_sort_and_totals() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qwallet", 100, None, None, Some(1));
        overview_address(&database, "bc1qexchange", 9_000, None, Some(1), None);
        database
            .connection()
            .unwrap()
            .execute(
                "UPDATE addresses SET exchange_balance_sats = 5000 WHERE address = 'bc1qexchange'",
                [],
            )
            .unwrap();

        let rows = list_overview_rows(
            &database,
            &AddressOverviewFilter {
                scope: AddressScopeFilter::Exchanges,
                ..AddressOverviewFilter::default()
            },
        )
        .unwrap();
        assert_eq!(rows[0].spendable_balance_sats, 5_000);
        // The real on-chain statistics are preserved separately.
        assert_eq!(rows[0].entity.chain_funded_txo_sum, 9_000);

        let (_, _, balance) = overview_totals(&database).unwrap();
        assert_eq!(balance, 5_100);

        let rows = list_overview_rows(&database, &AddressOverviewFilter::default()).unwrap();
        let order: Vec<&str> = rows.iter().map(|row| row.entity.address.as_str()).collect();
        assert_eq!(order, ["bc1qexchange", "bc1qwallet"]);
    }

    #[test]
    fn stats_refresh_does_not_clear_exchange_balance_column() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qexchange", 100, None, Some(1), None);
        database
            .connection()
            .unwrap()
            .execute(
                "UPDATE addresses SET exchange_balance_sats = 7000 WHERE address = 'bc1qexchange'",
                [],
            )
            .unwrap();
        // A regular stats refresh must leave the exchange ledger column alone.
        overview_address(&database, "bc1qexchange", 900, None, Some(1), None);
        let stored = get_address(&database, "bc1qexchange").unwrap().unwrap();
        assert_eq!(stored.chain_funded_txo_sum, 900);
        assert_eq!(
            database
                .connection()
                .unwrap()
                .query_row(
                    "SELECT exchange_balance_sats FROM addresses WHERE address = 'bc1qexchange'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            7_000
        );
    }
    #[test]
    fn update_details_rewrites_description_and_owner_and_reports_missing() {
        let database = Database::open_in_memory().unwrap();
        overview_address(&database, "bc1qedit", 100, Some("old"), Some(1), None);

        // Switching to a wallet clears the exchange; the description is
        // rewritten, and the stats columns are untouched.
        assert!(
            update_address_details(&database, "bc1qedit", Some("new"), None, Some(2),).unwrap()
        );
        let stored = get_address(&database, "bc1qedit").unwrap().unwrap();
        assert_eq!(stored.description.as_deref(), Some("new"));
        assert_eq!(stored.exchange_id, None);
        assert_eq!(stored.wallet_id, Some(2));
        assert_eq!(stored.chain_funded_txo_sum, 100);

        // Both owner ids may be cleared at once.
        assert!(update_address_details(&database, "bc1qedit", None, None, None).unwrap());
        let stored = get_address(&database, "bc1qedit").unwrap().unwrap();
        assert_eq!(stored.description, None);
        assert_eq!(stored.exchange_id, None);
        assert_eq!(stored.wallet_id, None);

        assert!(!update_address_details(&database, "bc1qmissing", None, None, None).unwrap());
    }
}
