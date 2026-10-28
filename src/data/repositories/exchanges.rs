//! SQL access to exchange accounts, their ledger transactions, and the
//! aggregate provider balance stored on addresses.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::data::{Database, DatabaseError};
use crate::domain::exchange::matching::{OnChainInput, OnChainOutput, OnChainTransaction};
use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountRow, ExchangeOverviewTotals, ExchangeTransaction,
    ExchangeTxType, MatchState, OnChainLink, OnChainLinkSource,
};
use crate::domain::table::{SortDirection, TablePage};

/// Sortable columns of the exchanges list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeAccountSortColumn {
    Name,
    TxCount,
    OnChainOperations,
    FirstTxTime,
    LastTxTime,
    LastModifiedAt,
}

impl ExchangeAccountSortColumn {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "name" => Some(Self::Name),
            "txs" => Some(Self::TxCount),
            "onchain" | "matched" => Some(Self::OnChainOperations),
            "first_tx" => Some(Self::FirstTxTime),
            "last_tx" => Some(Self::LastTxTime),
            "last_modified" => Some(Self::LastModifiedAt),
            _ => None,
        }
    }

    fn order_expression(self) -> &'static str {
        match self {
            Self::Name => "a.name COLLATE NOCASE",
            Self::TxCount => "tx_count",
            Self::OnChainOperations => "onchain_matched_count",
            Self::FirstTxTime => "first_tx_time",
            Self::LastTxTime => "last_tx_time",
            Self::LastModifiedAt => "a.last_modified_at",
        }
    }
}

/// Sortable columns of one exchange's transaction table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeTransactionSortColumn {
    Timestamp,
    TxType,
    Amount,
    Fee,
    OnChain,
    TxHash,
    Description,
}

impl ExchangeTransactionSortColumn {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "date" => Some(Self::Timestamp),
            "type" => Some(Self::TxType),
            "amount" => Some(Self::Amount),
            "fee" => Some(Self::Fee),
            "on_chain" => Some(Self::OnChain),
            "tx_hash" => Some(Self::TxHash),
            "description" => Some(Self::Description),
            _ => None,
        }
    }

    fn order_expression(self) -> &'static str {
        match self {
            Self::Timestamp => "timestamp",
            Self::TxType => "tx_type",
            Self::Amount => "amount_sats",
            Self::Fee => "fee",
            Self::OnChain => "COALESCE(on_chain_txid, matched_tx_id)",
            Self::TxHash => "tx_hash",
            Self::Description => "description",
        }
    }
}

/// Paging/sorting of one exchange transaction listing.
#[derive(Debug, Clone, Copy)]
pub struct TransactionPageQuery {
    pub exchange_id: i64,
    pub limit: u32,
    pub offset: u64,
    pub sort: Option<(ExchangeTransactionSortColumn, SortDirection)>,
}

/// One row written by the inspector or the CSV importer.
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeTransactionWrite {
    pub fingerprint: String,
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    pub amount_sats: i64,
    pub currency: String,
    pub fiat_amount: Option<f64>,
    pub fiat_currency: Option<String>,
    pub fee: Option<f64>,
    pub fee_currency: Option<String>,
    pub tx_hash: Option<String>,
    pub description: Option<String>,
    pub raw_data: BTreeMap<String, String>,
    pub match_state: MatchState,
    pub matched_tx_id: Option<String>,
    pub on_chain_link: Option<OnChainLink>,
    pub is_manually_edited: bool,
    pub is_intentional_duplicate: bool,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

/// Read one page of exchange accounts and the global KPI totals.
pub fn list_accounts(
    database: &Database,
    limit: u32,
    offset: u64,
    sort: Option<(ExchangeAccountSortColumn, SortDirection)>,
) -> Result<(TablePage<ExchangeAccountRow>, ExchangeOverviewTotals), DatabaseError> {
    let order = match sort {
        Some((ExchangeAccountSortColumn::OnChainOperations, direction)) => format!(
            "onchain_matched_count {} NULLS LAST, onchain_total_count {} NULLS LAST, a.id {}",
            direction.as_sql(),
            direction.as_sql(),
            direction.as_sql()
        ),
        Some((column, direction)) => format!(
            "{} {} NULLS LAST, a.id {}",
            column.order_expression(),
            direction.as_sql(),
            direction.as_sql()
        ),
        None => "tx_count DESC NULLS LAST, a.id DESC".to_string(),
    };
    let sql = format!(
        "SELECT a.id, a.name, a.provider_id, p.name, a.last_modified_at,
                COUNT(t.id) AS tx_count,
                COALESCE(SUM(CASE WHEN t.tx_type IN ('deposit', 'withdrawal')
                    AND t.match_state <> 'unmatched' THEN 1 ELSE 0 END), 0) AS onchain_matched_count,
                COALESCE(SUM(CASE WHEN t.tx_type IN ('deposit', 'withdrawal')
                    THEN 1 ELSE 0 END), 0) AS onchain_total_count,
                MIN(t.timestamp) AS first_tx_time,
                MAX(t.timestamp) AS last_tx_time
         FROM exchange_accounts a
         LEFT JOIN exchanges_types p ON p.id = a.provider_id
         LEFT JOIN exchange_transactions t ON t.exchange_id = a.id
         GROUP BY a.id
         ORDER BY {order}
         LIMIT ?1 OFFSET ?2"
    );
    let connection = database.connection()?;
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![limit, offset], |row| {
        Ok(ExchangeAccountRow {
            id: row.get(0)?,
            name: row.get(1)?,
            provider_id: row.get(2)?,
            provider_name: row.get(3)?,
            last_modified_at: row.get(4)?,
            tx_count: row.get(5)?,
            onchain_matched_count: row.get(6)?,
            onchain_total_count: row.get(7)?,
            first_tx_time: row.get(8)?,
            last_tx_time: row.get(9)?,
        })
    })?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    let total = connection.query_row("SELECT COUNT(*) FROM exchange_accounts", [], |row| {
        row.get::<_, i64>(0)
    })? as u64;
    let totals = connection.query_row(
        "SELECT (SELECT COUNT(*) FROM exchange_accounts),
                (SELECT COUNT(*) FROM exchange_transactions),
                (SELECT COUNT(*) FROM exchange_transactions
                 WHERE tx_type IN ('deposit', 'withdrawal') AND match_state <> 'unmatched'),
                (SELECT COUNT(*) FROM exchange_transactions
                 WHERE tx_type IN ('deposit', 'withdrawal'))",
        [],
        |row| {
            Ok(ExchangeOverviewTotals {
                exchange_count: row.get(0)?,
                total_transactions: row.get(1)?,
                onchain_matched_count: row.get(2)?,
                onchain_total_count: row.get(3)?,
            })
        },
    )?;
    Ok((TablePage { items, total }, totals))
}

pub fn get_account(database: &Database, id: i64) -> Result<Option<ExchangeAccount>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT a.id, a.name, a.provider_id, p.name, a.created_at,
                    a.last_imported_at, a.last_modified_at
             FROM exchange_accounts a
             LEFT JOIN exchanges_types p ON p.id = a.provider_id
             WHERE a.id = ?1",
            [id],
            account_from_row,
        )
        .optional()
        .map_err(Into::into)
}

fn account_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExchangeAccount> {
    Ok(ExchangeAccount {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_id: row.get(2)?,
        provider_name: row.get(3)?,
        created_at: row.get(4)?,
        last_imported_at: row.get(5)?,
        last_modified_at: row.get(6)?,
    })
}

pub fn create_account(
    database: &Database,
    name: &str,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "INSERT INTO exchange_accounts (name, provider_id, created_at, last_modified_at)
         VALUES (?1, ?2, ?3, ?3)",
        params![name, provider_id, now],
    )?;
    let id = transaction.last_insert_rowid();
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    drop(connection);
    get_account(database, id)?.ok_or_else(|| DatabaseError::new("exchange account was not stored"))
}

pub fn update_account(
    database: &Database,
    id: i64,
    name: &str,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let old_provider: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let updated = transaction.execute(
        "UPDATE exchange_accounts SET name = ?2, provider_id = ?3, last_modified_at = ?4
         WHERE id = ?1",
        params![id, name, provider_id, now],
    )?;
    if updated == 0 {
        return Err(DatabaseError::new("exchange account not found"));
    }
    if let Some(old) = old_provider {
        refresh_provider_balance_tx(&transaction, old)?;
    }
    if let Some(new) = provider_id {
        refresh_provider_balance_tx(&transaction, new)?;
    }
    transaction.commit()?;
    drop(connection);
    get_account(database, id)?.ok_or_else(|| DatabaseError::new("exchange account was not stored"))
}

pub fn delete_account(database: &Database, id: i64) -> Result<bool, DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let provider_id: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let deleted = transaction.execute("DELETE FROM exchange_accounts WHERE id = ?1", [id])?;
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    Ok(deleted == 1)
}

/// Refresh the stored ledger balance of one provider.
///
/// The alphabetically first address of the provider receives the aggregate
/// balance of every account linked to it; all other addresses become zero.
/// The real on-chain statistics are never touched.
pub fn refresh_provider_balance(
    connection: &Connection,
    provider_id: i64,
) -> Result<(), DatabaseError> {
    let total: f64 = connection.query_row(
        "SELECT COALESCE(SUM(
             CASE t.tx_type
                 WHEN 'buy' THEN ABS(t.amount_sats)
                 WHEN 'deposit' THEN ABS(t.amount_sats)
                 WHEN 'reward' THEN ABS(t.amount_sats)
                 WHEN 'sell' THEN -ABS(t.amount_sats)
                 WHEN 'withdrawal' THEN -ABS(t.amount_sats)
                 ELSE t.amount_sats
             END
             - CASE WHEN UPPER(COALESCE(t.fee_currency, '')) IN ('BTC', 'XBT')
                    THEN ABS(COALESCE(t.fee, 0)) ELSE 0 END
         ), 0)
         FROM exchange_transactions t
         JOIN exchange_accounts a ON a.id = t.exchange_id
         WHERE a.provider_id = ?1",
        [provider_id],
        |row| row.get(0),
    )?;
    let total = total.round() as i64;
    let anchor: Option<String> = connection.query_row(
        "SELECT MIN(address) FROM addresses WHERE exchange_id = ?1",
        [provider_id],
        |row| row.get(0),
    )?;
    if let Some(anchor) = anchor {
        connection.execute(
            "UPDATE addresses
             SET exchange_balance_sats = CASE WHEN address = ?2 THEN ?3 ELSE 0 END
             WHERE exchange_id = ?1",
            params![provider_id, anchor, total],
        )?;
    }
    Ok(())
}

/// Transaction-scoped wrapper for [`refresh_provider_balance`].
pub fn refresh_provider_balance_tx(
    transaction: &Transaction<'_>,
    provider_id: i64,
) -> Result<(), DatabaseError> {
    refresh_provider_balance(transaction, provider_id)
}

/// Account transactions sorted and paged in the backend.
pub fn list_transactions(
    database: &Database,
    query: &TransactionPageQuery,
) -> Result<TablePage<ExchangeTransaction>, DatabaseError> {
    let (column, direction) = query
        .sort
        .map(|(column, direction)| (column.order_expression(), direction.as_sql()))
        .unwrap_or(("timestamp", "DESC"));
    let sql = format!(
        "SELECT {TRANSACTION_COLUMNS}
         FROM exchange_transactions
         WHERE exchange_id = ?1
         ORDER BY {column} {direction} NULLS LAST, id {direction}
         LIMIT ?2 OFFSET ?3"
    );
    let connection = database.connection()?;
    let offset = i64::try_from(query.offset).unwrap_or(i64::MAX);
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![query.exchange_id, query.limit, offset],
        transaction_from_row,
    )?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    let total = connection.query_row(
        "SELECT COUNT(*) FROM exchange_transactions WHERE exchange_id = ?1",
        [query.exchange_id],
        |row| row.get::<_, i64>(0),
    )? as u64;
    Ok(TablePage { items, total })
}

/// All transactions of an account, used for fiat KPI conversion.
pub fn list_account_transactions(
    database: &Database,
    exchange_id: i64,
) -> Result<Vec<ExchangeTransaction>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(&format!(
        "SELECT {TRANSACTION_COLUMNS} FROM exchange_transactions
         WHERE exchange_id = ?1 ORDER BY timestamp DESC, id DESC"
    ))?;
    let rows = statement.query_map([exchange_id], transaction_from_row)?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(items)
}

pub fn get_transaction(
    database: &Database,
    id: i64,
) -> Result<Option<ExchangeTransaction>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            &format!("SELECT {TRANSACTION_COLUMNS} FROM exchange_transactions WHERE id = ?1"),
            [id],
            transaction_from_row,
        )
        .optional()
        .map_err(Into::into)
}

const TRANSACTION_COLUMNS: &str = "id, exchange_id, fingerprint, timestamp, tx_type,
    amount_sats, currency, fiat_amount, fiat_currency, fee, fee_currency, tx_hash,
    description, raw_data, match_state, matched_tx_id, on_chain_txid,
    on_chain_source, on_chain_linked_at, on_chain_local_addresses,
    is_manually_edited, is_intentional_duplicate";

fn transaction_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExchangeTransaction> {
    let raw_data: String = row.get(13)?;
    let raw_data = serde_json::from_str(&raw_data).unwrap_or_default();
    let match_state: String = row.get(14)?;
    let match_state = MatchState::from_str(&match_state).unwrap_or(MatchState::Unmatched);
    let tx_type: String = row.get(4)?;
    let tx_type = ExchangeTxType::from_str(&tx_type).unwrap_or(ExchangeTxType::Unknown);
    let on_chain_txid: Option<String> = row.get(16)?;
    let on_chain_source: Option<String> = row.get(17)?;
    let on_chain_linked_at: Option<i64> = row.get(18)?;
    let local_addresses: Option<String> = row.get(19)?;
    let on_chain_link = match (on_chain_txid, on_chain_source, on_chain_linked_at) {
        (Some(txid), Some(source), Some(linked_at)) => Some(OnChainLink {
            txid,
            local_addresses: local_addresses
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default(),
            source: OnChainLinkSource::from_str(&source).unwrap_or(OnChainLinkSource::Manual),
            linked_at,
        }),
        _ => None,
    };
    Ok(ExchangeTransaction {
        id: row.get(0)?,
        exchange_id: row.get(1)?,
        fingerprint: row.get(2)?,
        timestamp: row.get(3)?,
        tx_type,
        amount_sats: row.get(5)?,
        currency: row.get(6)?,
        fiat_amount: row.get(7)?,
        fiat_currency: row.get(8)?,
        converted_fiat_amount: None,
        converted_fiat_currency: None,
        fee: row.get(9)?,
        fee_currency: row.get(10)?,
        tx_hash: row.get(11)?,
        description: row.get(12)?,
        raw_data,
        match_state,
        matched_tx_id: row.get(15)?,
        on_chain_link,
        is_manually_edited: row.get::<_, i64>(20)? == 1,
        is_intentional_duplicate: row.get::<_, i64>(21)? == 1,
    })
}

pub fn find_by_fingerprint(
    database: &Database,
    exchange_id: i64,
    fingerprint: &str,
    exclude_id: Option<i64>,
) -> Result<Option<ExchangeTransaction>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            &format!(
                "SELECT {TRANSACTION_COLUMNS} FROM exchange_transactions
                 WHERE exchange_id = ?1 AND fingerprint = ?2
                   AND (?3 IS NULL OR id <> ?3)
                 ORDER BY id LIMIT 1"
            ),
            params![exchange_id, fingerprint, exclude_id],
            transaction_from_row,
        )
        .optional()
        .map_err(Into::into)
}

/// Generate a fingerprint string for a draft. Kept here so every repository
/// write uses the same identity rules as the CSV parser.
pub fn fingerprint_for(
    timestamp: i64,
    amount_sats: i64,
    currency: &str,
    tx_type: ExchangeTxType,
    tx_hash: Option<&str>,
) -> String {
    crate::domain::exchange::csv::build_fingerprint(
        timestamp,
        amount_sats as f64,
        currency,
        tx_type,
        tx_hash,
    )
}

/// Insert or update one manually edited transaction and refresh its provider.
pub fn save_transaction(
    database: &Database,
    exchange_id: i64,
    transaction_id: Option<i64>,
    replace_id: Option<i64>,
    write: &ExchangeTransactionWrite,
) -> Result<ExchangeTransaction, DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let provider_id: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [exchange_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if let Some(replace_id) = replace_id {
        transaction.execute(
            "DELETE FROM exchange_transactions WHERE id = ?1 AND exchange_id = ?2",
            params![replace_id, exchange_id],
        )?;
    }
    let id = if let Some(id) = transaction_id {
        let updated = transaction.execute(
            "UPDATE exchange_transactions SET
                fingerprint = ?3, timestamp = ?4, tx_type = ?5, amount_sats = ?6,
                currency = ?7, fiat_amount = ?8, fiat_currency = ?9, fee = ?10,
                fee_currency = ?11, tx_hash = ?12, description = ?13, raw_data = ?14,
                match_state = ?15, matched_tx_id = ?16, on_chain_txid = ?17,
                on_chain_source = ?18, on_chain_linked_at = ?19,
                on_chain_local_addresses = ?20, is_manually_edited = ?21,
                is_intentional_duplicate = ?22, updated_at = ?23
             WHERE id = ?1 AND exchange_id = ?2",
            params![
                id,
                exchange_id,
                write.fingerprint,
                write.timestamp,
                write.tx_type.as_str(),
                write.amount_sats,
                write.currency,
                write.fiat_amount,
                write.fiat_currency,
                write.fee,
                write.fee_currency,
                write.tx_hash,
                write.description,
                serde_json::to_string(&write.raw_data).unwrap_or_else(|_| "{}".to_string()),
                write.match_state.as_str(),
                write.matched_tx_id,
                write.on_chain_link.as_ref().map(|link| link.txid.as_str()),
                write
                    .on_chain_link
                    .as_ref()
                    .map(|link| link.source.as_str()),
                write.on_chain_link.as_ref().map(|link| link.linked_at),
                write.on_chain_link.as_ref().map(|link| {
                    serde_json::to_string(&link.local_addresses)
                        .unwrap_or_else(|_| "[]".to_string())
                }),
                i64::from(write.is_manually_edited),
                i64::from(write.is_intentional_duplicate),
                now,
            ],
        )?;
        if updated == 0 {
            return Err(DatabaseError::new("exchange transaction not found"));
        }
        id
    } else {
        transaction.execute(
            "INSERT INTO exchange_transactions (
                exchange_id, fingerprint, timestamp, tx_type, amount_sats, currency,
                fiat_amount, fiat_currency, fee, fee_currency, tx_hash, description,
                raw_data, match_state, matched_tx_id, on_chain_txid, on_chain_source,
                on_chain_linked_at, on_chain_local_addresses, is_manually_edited,
                is_intentional_duplicate, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?22)",
            params![
                exchange_id,
                write.fingerprint,
                write.timestamp,
                write.tx_type.as_str(),
                write.amount_sats,
                write.currency,
                write.fiat_amount,
                write.fiat_currency,
                write.fee,
                write.fee_currency,
                write.tx_hash,
                write.description,
                serde_json::to_string(&write.raw_data).unwrap_or_else(|_| "{}".to_string()),
                write.match_state.as_str(),
                write.matched_tx_id,
                write.on_chain_link.as_ref().map(|link| link.txid.as_str()),
                write
                    .on_chain_link
                    .as_ref()
                    .map(|link| link.source.as_str()),
                write.on_chain_link.as_ref().map(|link| link.linked_at),
                write.on_chain_link.as_ref().map(|link| {
                    serde_json::to_string(&link.local_addresses)
                        .unwrap_or_else(|_| "[]".to_string())
                }),
                i64::from(write.is_manually_edited),
                i64::from(write.is_intentional_duplicate),
                now,
            ],
        )?;
        transaction.last_insert_rowid()
    };
    transaction.execute(
        "UPDATE exchange_accounts SET last_modified_at = ?2 WHERE id = ?1",
        params![exchange_id, now],
    )?;
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    drop(connection);
    get_transaction(database, id)?
        .ok_or_else(|| DatabaseError::new("exchange transaction was not stored"))
}

pub fn delete_transaction(
    database: &Database,
    exchange_id: i64,
    id: i64,
) -> Result<bool, DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let provider_id: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [exchange_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let deleted = transaction.execute(
        "DELETE FROM exchange_transactions WHERE id = ?1 AND exchange_id = ?2",
        params![id, exchange_id],
    )?;
    if deleted == 1 {
        transaction.execute(
            "UPDATE exchange_accounts SET last_modified_at = ?2 WHERE id = ?1",
            params![exchange_id, now],
        )?;
    }
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    Ok(deleted == 1)
}

/// Insert already deduplicated imported rows and refresh the provider.
pub fn insert_imported_transactions(
    database: &Database,
    exchange_id: i64,
    rows: &[ExchangeTransactionWrite],
    last_imported_at: i64,
) -> Result<(), DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let provider_id: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [exchange_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    for write in rows {
        transaction.execute(
            "INSERT INTO exchange_transactions (
                exchange_id, fingerprint, timestamp, tx_type, amount_sats, currency,
                fiat_amount, fiat_currency, fee, fee_currency, tx_hash, description,
                raw_data, match_state, matched_tx_id, on_chain_txid, on_chain_source,
                on_chain_linked_at, on_chain_local_addresses, is_manually_edited,
                is_intentional_duplicate, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?22)",
            params![
                exchange_id,
                write.fingerprint,
                write.timestamp,
                write.tx_type.as_str(),
                write.amount_sats,
                write.currency,
                write.fiat_amount,
                write.fiat_currency,
                write.fee,
                write.fee_currency,
                write.tx_hash,
                write.description,
                serde_json::to_string(&write.raw_data).unwrap_or_else(|_| "{}".to_string()),
                write.match_state.as_str(),
                write.matched_tx_id,
                write.on_chain_link.as_ref().map(|link| link.txid.as_str()),
                write
                    .on_chain_link
                    .as_ref()
                    .map(|link| link.source.as_str()),
                write.on_chain_link.as_ref().map(|link| link.linked_at),
                write.on_chain_link.as_ref().map(|link| {
                    serde_json::to_string(&link.local_addresses)
                        .unwrap_or_else(|_| "[]".to_string())
                }),
                i64::from(write.is_manually_edited),
                i64::from(write.is_intentional_duplicate),
                last_imported_at,
            ],
        )?;
    }
    transaction.execute(
        "UPDATE exchange_accounts SET last_imported_at = ?2, last_modified_at = ?2 WHERE id = ?1",
        params![exchange_id, last_imported_at],
    )?;
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    Ok(())
}

/// Existing fingerprints and linked on-chain txids of one account.
pub fn existing_identities(
    database: &Database,
    exchange_id: i64,
) -> Result<(HashSet<String>, HashSet<String>), DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT fingerprint, COALESCE(on_chain_txid, matched_tx_id)
         FROM exchange_transactions WHERE exchange_id = ?1",
    )?;
    let rows = statement.query_map([exchange_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;
    let mut fingerprints = HashSet::new();
    let mut linked = HashSet::new();
    for row in rows {
        let (fingerprint, txid) = row.map_err(|error| DatabaseError::new(error.to_string()))?;
        fingerprints.insert(fingerprint);
        if let Some(txid) = txid {
            linked.insert(txid);
        }
    }
    Ok((fingerprints, linked))
}

/// Load every cached confirmed on-chain transaction for reconciliation.
pub fn load_on_chain_transactions(
    database: &Database,
) -> Result<Vec<OnChainTransaction>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT MIN(t.id), t.txid, MIN(t.fee), COALESCE(MAX(t.block_time), 0),
                GROUP_CONCAT(DISTINCT t.address)
         FROM transactions t
         WHERE t.confirmed = 1
         GROUP BY t.txid
         ORDER BY t.txid",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut values = Vec::new();
    let mut id_to_index = HashMap::new();
    for row in rows {
        let (id, txid, fee_sats, block_time, addresses) =
            row.map_err(|error| DatabaseError::new(error.to_string()))?;
        id_to_index.insert(id, values.len());
        values.push(OnChainTransaction {
            txid,
            fee_sats,
            block_time,
            inputs: Vec::new(),
            outputs: Vec::new(),
            local_addresses: addresses
                .unwrap_or_default()
                .split(',')
                .filter(|address| !address.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    if values.is_empty() {
        return Ok(values);
    }

    let representative_ids = id_to_index.keys().copied().collect::<Vec<_>>();
    let mut placeholders = Vec::new();
    let mut params = Vec::new();
    for id in &representative_ids {
        placeholders.push("?");
        params.push(*id);
    }
    let placeholders = placeholders.join(",");
    let input_sql = format!(
        "SELECT transaction_id, COALESCE(prevout_value, 0),
                prevout_scriptpubkey_address
         FROM transaction_inputs WHERE transaction_id IN ({placeholders})"
    );
    let mut input_statement = connection.prepare(&input_sql)?;
    let input_rows =
        input_statement.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                OnChainInput {
                    value: row.get(1)?,
                    address: row.get(2)?,
                },
            ))
        })?;
    for row in input_rows {
        let (id, input) = row.map_err(|error| DatabaseError::new(error.to_string()))?;
        if let Some(index) = id_to_index.get(&id) {
            values[*index].inputs.push(input);
        }
    }

    let output_sql = format!(
        "SELECT transaction_id, value, scriptpubkey_address, scriptpubkey_type
         FROM transaction_outputs WHERE transaction_id IN ({placeholders})"
    );
    let mut output_statement = connection.prepare(&output_sql)?;
    let output_rows =
        output_statement.query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                OnChainOutput {
                    value: row.get(1)?,
                    address: row.get(2)?,
                    scriptpubkey_type: row.get(3)?,
                },
            ))
        })?;
    for row in output_rows {
        let (id, output) = row.map_err(|error| DatabaseError::new(error.to_string()))?;
        if let Some(index) = id_to_index.get(&id) {
            values[*index].outputs.push(output);
        }
    }
    Ok(values)
}

/// Update only the reconciliation state/link of one ledger row. Ledger
/// amounts are untouched, so the provider balance does not change.
pub fn set_link(
    database: &Database,
    exchange_id: i64,
    id: i64,
    match_state: MatchState,
    matched_tx_id: Option<&str>,
    link: Option<&OnChainLink>,
) -> Result<ExchangeTransaction, DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let updated = transaction.execute(
        "UPDATE exchange_transactions SET
            match_state = ?3, matched_tx_id = ?4, on_chain_txid = ?5,
            on_chain_source = ?6, on_chain_linked_at = ?7,
            on_chain_local_addresses = ?8, updated_at = ?9
         WHERE id = ?1 AND exchange_id = ?2",
        params![
            id,
            exchange_id,
            match_state.as_str(),
            matched_tx_id,
            link.map(|link| link.txid.as_str()),
            link.map(|link| link.source.as_str()),
            link.map(|link| link.linked_at),
            link.map(|link| {
                serde_json::to_string(&link.local_addresses).unwrap_or_else(|_| "[]".to_string())
            }),
            now,
        ],
    )?;
    if updated == 0 {
        return Err(DatabaseError::new("exchange transaction not found"));
    }
    transaction.execute(
        "UPDATE exchange_accounts SET last_modified_at = ?2 WHERE id = ?1",
        params![exchange_id, now],
    )?;
    transaction.commit()?;
    drop(connection);
    get_transaction(database, id)?
        .ok_or_else(|| DatabaseError::new("exchange transaction was not stored"))
}

/// Persist reconciliation results for all rows of one account atomically.
pub fn update_match_states(
    database: &Database,
    exchange_id: i64,
    updates: &[(i64, MatchState, Option<String>, Option<OnChainLink>)],
) -> Result<(), DatabaseError> {
    let now = unix_now();
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let provider_id: Option<i64> = transaction
        .query_row(
            "SELECT provider_id FROM exchange_accounts WHERE id = ?1",
            [exchange_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    for (id, state, matched_txid, link) in updates {
        transaction.execute(
            "UPDATE exchange_transactions SET
                match_state = ?3, matched_tx_id = ?4, on_chain_txid = ?5,
                on_chain_source = ?6, on_chain_linked_at = ?7,
                on_chain_local_addresses = ?8, updated_at = ?9
             WHERE id = ?1 AND exchange_id = ?2",
            params![
                id,
                exchange_id,
                state.as_str(),
                matched_txid,
                link.as_ref().map(|link| link.txid.as_str()),
                link.as_ref().map(|link| link.source.as_str()),
                link.as_ref().map(|link| link.linked_at),
                link.as_ref().map(|link| {
                    serde_json::to_string(&link.local_addresses)
                        .unwrap_or_else(|_| "[]".to_string())
                }),
                now,
            ],
        )?;
    }
    transaction.execute(
        "UPDATE exchange_accounts SET last_modified_at = ?2 WHERE id = ?1",
        params![exchange_id, now],
    )?;
    if let Some(provider_id) = provider_id {
        refresh_provider_balance_tx(&transaction, provider_id)?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::repositories::addresses as address_repository;
    use crate::domain::address::AddressEntity;
    use crate::domain::exchange::ExchangeTxType;

    fn address(value: &str, exchange_id: Option<i64>) -> AddressEntity {
        AddressEntity {
            address: value.to_string(),
            exchange_id,
            ..AddressEntity::default()
        }
    }

    fn ledger_write(
        amount_sats: i64,
        tx_type: ExchangeTxType,
        fee: Option<f64>,
    ) -> ExchangeTransactionWrite {
        ExchangeTransactionWrite {
            fingerprint: format!("{amount_sats}-{}-{fee:?}", tx_type.as_str()),
            timestamp: 1_700_000_000,
            tx_type,
            amount_sats,
            currency: "BTC".into(),
            fiat_amount: None,
            fiat_currency: None,
            fee,
            fee_currency: fee.map(|_| "BTC".to_string()),
            tx_hash: None,
            description: None,
            raw_data: BTreeMap::new(),
            match_state: MatchState::Unmatched,
            matched_tx_id: None,
            on_chain_link: None,
            is_manually_edited: false,
            is_intentional_duplicate: false,
        }
    }

    fn stored_balance(database: &Database, address: &str) -> i64 {
        database
            .connection()
            .unwrap()
            .query_row(
                "SELECT exchange_balance_sats FROM addresses WHERE address = ?1",
                [address],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn ledger_balance_uses_type_direction_and_subtracts_btc_fees() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qa", Some(1))).unwrap();
        address_repository::insert_address(&database, &address("bc1qz", Some(1))).unwrap();
        let account = create_account(&database, "Binance", Some(1)).unwrap();
        let writes = vec![
            ledger_write(100_000_000, ExchangeTxType::Buy, None),
            ledger_write(-30_000_000, ExchangeTxType::Sell, None),
            ledger_write(-10_000_000, ExchangeTxType::Withdrawal, None),
            ledger_write(20_000_000, ExchangeTxType::Deposit, None),
            ledger_write(5_000_000, ExchangeTxType::Reward, None),
            ledger_write(1_000_000, ExchangeTxType::Buy, Some(25_000.0)),
        ];
        insert_imported_transactions(&database, account.id, &writes, 1_700_000_000).unwrap();

        // 100 - 30 - 10 + 20 + 5 + 1 - 0.00025 = 85.975 BTC.
        assert_eq!(stored_balance(&database, "bc1qa"), 85_975_000);
        assert_eq!(stored_balance(&database, "bc1qz"), 0);
    }

    #[test]
    fn multiple_accounts_sum_and_providerless_accounts_are_ignored() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qa", Some(1))).unwrap();
        let first = create_account(&database, "Binance", Some(1)).unwrap();
        let second = create_account(&database, "Binance savings", Some(1)).unwrap();
        let ignored = create_account(&database, "Unlinked", None).unwrap();
        insert_imported_transactions(
            &database,
            first.id,
            &[ledger_write(10_000_000, ExchangeTxType::Buy, None)],
            1,
        )
        .unwrap();
        insert_imported_transactions(
            &database,
            second.id,
            &[ledger_write(5_000_000, ExchangeTxType::Deposit, None)],
            1,
        )
        .unwrap();
        insert_imported_transactions(
            &database,
            ignored.id,
            &[ledger_write(99_000_000, ExchangeTxType::Buy, None)],
            1,
        )
        .unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 15_000_000);
    }

    #[test]
    fn anchor_moves_with_alphabetical_order_and_deletion() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qm", Some(1))).unwrap();
        address_repository::insert_address(&database, &address("bc1qz", Some(1))).unwrap();
        let account = create_account(&database, "Kraken", Some(1)).unwrap();
        insert_imported_transactions(
            &database,
            account.id,
            &[ledger_write(7_000_000, ExchangeTxType::Buy, None)],
            1,
        )
        .unwrap();
        assert_eq!(stored_balance(&database, "bc1qm"), 7_000_000);

        // A new earlier address becomes the anchor.
        address_repository::insert_address(&database, &address("bc1qa", Some(1))).unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 7_000_000);
        assert_eq!(stored_balance(&database, "bc1qm"), 0);

        // Deleting the anchor promotes the next alphabetical address.
        address_repository::delete_address(&database, "bc1qa").unwrap();
        assert_eq!(stored_balance(&database, "bc1qm"), 7_000_000);
        assert_eq!(stored_balance(&database, "bc1qz"), 0);
    }

    #[test]
    fn stats_refresh_preserves_the_stored_ledger_balance() {
        let database = Database::open_in_memory().unwrap();
        let mut anchor = address("bc1qa", Some(1));
        anchor.chain_funded_txo_sum = 100;
        address_repository::insert_address(&database, &anchor).unwrap();
        let account = create_account(&database, "OKX", Some(1)).unwrap();
        insert_imported_transactions(
            &database,
            account.id,
            &[ledger_write(4_000_000, ExchangeTxType::Deposit, None)],
            1,
        )
        .unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 4_000_000);

        let mut refreshed = address("bc1qa", Some(1));
        refreshed.chain_funded_txo_sum = 900;
        address_repository::insert_address(&database, &refreshed).unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 4_000_000);
    }

    #[test]
    fn deleting_a_transaction_and_moving_account_provider_refresh_balances() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qa", Some(1))).unwrap();
        address_repository::insert_address(&database, &address("bc1qb", Some(2))).unwrap();
        let account = create_account(&database, "Exchange", Some(1)).unwrap();
        let write = ledger_write(3_000_000, ExchangeTxType::Buy, None);
        insert_imported_transactions(&database, account.id, std::slice::from_ref(&write), 1)
            .unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 3_000_000);

        update_account(&database, account.id, "Exchange", Some(2)).unwrap();
        assert_eq!(stored_balance(&database, "bc1qa"), 0);
        assert_eq!(stored_balance(&database, "bc1qb"), 3_000_000);

        let transactions = list_account_transactions(&database, account.id).unwrap();
        delete_transaction(&database, account.id, transactions[0].id).unwrap();
        assert_eq!(stored_balance(&database, "bc1qb"), 0);
    }

    #[test]
    fn account_sort_keys_include_onchain_and_the_legacy_matched_alias() {
        assert_eq!(
            ExchangeAccountSortColumn::from_key("onchain"),
            Some(ExchangeAccountSortColumn::OnChainOperations)
        );
        assert_eq!(
            ExchangeAccountSortColumn::from_key("matched"),
            Some(ExchangeAccountSortColumn::OnChainOperations)
        );
        assert_eq!(ExchangeAccountSortColumn::from_key("unmatched"), None);
    }
    #[test]
    fn account_list_and_totals_include_transaction_and_onchain_stats() {
        let database = Database::open_in_memory().unwrap();
        let account = create_account(&database, "Coinbase", Some(1)).unwrap();
        let mut buy = ledger_write(10_000_000, ExchangeTxType::Buy, None);
        buy.match_state = MatchState::HashMatch;
        let mut sell = ledger_write(5_000_000, ExchangeTxType::Sell, None);
        sell.match_state = MatchState::AmountDateMatch;
        let deposit = ledger_write(3_000_000, ExchangeTxType::Deposit, None);
        let mut withdrawal = ledger_write(-2_000_000, ExchangeTxType::Withdrawal, None);
        withdrawal.match_state = MatchState::ManualMatch;
        insert_imported_transactions(&database, account.id, &[buy, sell, deposit, withdrawal], 1)
            .unwrap();
        let (page, totals) = list_accounts(
            &database,
            25,
            0,
            Some((ExchangeAccountSortColumn::TxCount, SortDirection::Desc)),
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].tx_count, 4);
        // Matched buys/sells never count as on-chain operations.
        assert_eq!(page.items[0].onchain_matched_count, 1);
        assert_eq!(page.items[0].onchain_total_count, 2);
        assert_eq!(totals.exchange_count, 1);
        assert_eq!(totals.total_transactions, 4);
        assert_eq!(totals.onchain_matched_count, 1);
        assert_eq!(totals.onchain_total_count, 2);
    }

    #[test]
    fn onchain_operations_sort_uses_matched_then_total_counts() {
        let database = Database::open_in_memory().unwrap();
        let account_a = create_account(&database, "A", Some(1)).unwrap();
        insert_imported_transactions(
            &database,
            account_a.id,
            &[ledger_write(1_000_000, ExchangeTxType::Deposit, None), {
                let mut row = ledger_write(-1_000_000, ExchangeTxType::Withdrawal, None);
                row.match_state = MatchState::HashMatch;
                row
            }],
            1,
        )
        .unwrap();
        let account_b = create_account(&database, "B", Some(1)).unwrap();
        insert_imported_transactions(
            &database,
            account_b.id,
            &[
                {
                    let mut row = ledger_write(2_000_000, ExchangeTxType::Deposit, None);
                    row.match_state = MatchState::HashMatch;
                    row
                },
                {
                    let mut row = ledger_write(-2_000_000, ExchangeTxType::Withdrawal, None);
                    row.match_state = MatchState::HashMatch;
                    row
                },
            ],
            1,
        )
        .unwrap();
        let account_c = create_account(&database, "C", Some(1)).unwrap();
        insert_imported_transactions(
            &database,
            account_c.id,
            &[ledger_write(3_000_000, ExchangeTxType::Buy, None)],
            1,
        )
        .unwrap();

        let (page, _) = list_accounts(
            &database,
            25,
            0,
            Some((
                ExchangeAccountSortColumn::OnChainOperations,
                SortDirection::Desc,
            )),
        )
        .unwrap();
        let names: Vec<&str> = page.items.iter().map(|row| row.name.as_str()).collect();
        assert_eq!(names, ["B", "A", "C"]);
        assert_eq!(page.items[2].onchain_matched_count, 0);
        assert_eq!(page.items[2].onchain_total_count, 0);
    }
}
