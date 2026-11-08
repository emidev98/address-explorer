//! SQL access for the funds-flow screen.
//!
//! The repository applies the requested population filters before loading any
//! transaction: only transactions touching a selected address or exchange
//! provider are read, and the complete inputs/outputs of those transactions
//! are then loaded as context. Ownership metadata is resolved in batches, so
//! building the graph never makes one query per address.
use crate::data::{Database, DatabaseError};
use crate::domain::exchange::{ExchangeTxType, MatchState};
use crate::domain::flow::{
    FlowAddressOption, FlowAddressOwner, FlowDataset, FlowFilterOption, FlowFilterOptions,
    FlowInput, FlowLedgerRow, FlowOutput, FlowQuery, FlowTimeBounds, FlowTransaction,
    MAX_FILTER_VALUES,
};
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
/// Sortable columns of the aggregated flows table. The key allowlist is owned
/// here even though the rows are derived from the graph rather than a SQL
/// table; the service applies the comparator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowLinkSortColumn {
    Value,
    Source,
    Target,
    TxCount,
    FirstTime,
    LastTime,
}
impl FlowLinkSortColumn {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "value" => Some(Self::Value),
            "source" => Some(Self::Source),
            "target" => Some(Self::Target),
            "txs" => Some(Self::TxCount),
            "first_tx" => Some(Self::FirstTime),
            "last_tx" => Some(Self::LastTime),
            _ => None,
        }
    }
}
/// Sortable columns of the evidence drill-down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowEvidenceSortColumn {
    Date,
    Value,
}
impl FlowEvidenceSortColumn {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "date" => Some(Self::Date),
            "value" => Some(Self::Value),
            _ => None,
        }
    }
}
/// Read every option the filter bar needs.
pub fn filter_options(database: &Database) -> Result<FlowFilterOptions, DatabaseError> {
    let connection = database.connection()?;
    let exchanges = exchange_options(&connection)?;
    let addresses = address_options(&connection)?;
    let time_bounds = time_bounds(&connection)?;
    Ok(FlowFilterOptions {
        exchanges,
        addresses,
        time_bounds,
    })
}
/// Load the filtered dataset used by both the transaction and summary builders.
pub fn load_dataset(database: &Database, query: &FlowQuery) -> Result<FlowDataset, DatabaseError> {
    let connection = database.connection()?;
    let focus_owners = load_focus_owners(&connection, query)?;
    let focus_addresses: Vec<String> = focus_owners.keys().cloned().collect();
    let mut txids = focus_txids(&connection, &focus_addresses, query)?;
    let ledger = if query.has_exchange_filter() {
        let ledger = load_ledger_rows(&connection, query)?;
        for row in &ledger {
            if let Some(txid) = row.on_chain_txid.as_deref() {
                txids.insert(txid.to_string());
            }
        }
        ledger
    } else if query.has_population_filter() {
        load_linked_ledger_rows(&connection, query, &txids)?
    } else {
        load_ledger_rows(&connection, query)?
    };
    let txids = txids.into_iter().collect::<Vec<_>>();
    let transactions = load_transactions(&connection, &txids)?
        .into_iter()
        .filter(|transaction| {
            query
                .from_time
                .is_none_or(|from_time| transaction.block_time >= from_time)
                && query
                    .to_time
                    .is_none_or(|to_time| transaction.block_time <= to_time)
        })
        .collect::<Vec<_>>();
    let mut address_set: HashSet<String> = focus_addresses.iter().cloned().collect();
    for transaction in &transactions {
        for input in &transaction.inputs {
            if let Some(address) = input.address.as_deref() {
                address_set.insert(address.to_string());
            }
        }
        for output in &transaction.outputs {
            if let Some(address) = output.address.as_deref() {
                address_set.insert(address.to_string());
            }
        }
    }
    let mut owners = focus_owners;
    for (address, owner) in load_owners(&connection, &address_set.into_iter().collect::<Vec<_>>())?
    {
        owners.insert(address, owner);
    }
    Ok(FlowDataset {
        transactions,
        ledger,
        owners,
        focus_addresses,
    })
}
fn exchange_options(connection: &Connection) -> Result<Vec<FlowFilterOption>, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT p.id, p.name, p.brand_color,
                (SELECT COUNT(*) FROM addresses a WHERE a.exchange_id = p.id),
                (SELECT COUNT(*) FROM exchange_accounts ea WHERE ea.provider_id = p.id)
         FROM exchanges_types p
         ORDER BY p.name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(FlowFilterOption {
            id: row.get(0)?,
            name: row.get(1)?,
            brand_color: row.get(2)?,
            address_count: row.get(3)?,
            account_count: row.get(4)?,
        })
    })?;
    let mut values = collect_rows(rows)?;
    values.retain(|value| value.address_count > 0 || value.account_count > 0);
    values.sort_by(|left, right| {
        right
            .account_count
            .cmp(&left.account_count)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(values)
}
fn address_options(connection: &Connection) -> Result<Vec<FlowAddressOption>, DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT a.address, a.description, a.exchange_id, e.name, e.brand_color,
                a.wallet_id, w.name, w.brand_color,
                CASE WHEN a.exchange_id IS NOT NULL
                    THEN a.exchange_balance_sats
                    ELSE a.chain_funded_txo_sum - a.chain_spent_txo_sum END,
                (SELECT COUNT(*) FROM transactions t
                 WHERE t.address = a.address AND t.confirmed = 1),
                a.tx_index_rate_limited
         FROM addresses a
         LEFT JOIN exchanges_types e ON e.id = a.exchange_id
         LEFT JOIN wallets_types w ON w.id = a.wallet_id
         ORDER BY a.address",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(FlowAddressOption {
            address: row.get(0)?,
            description: row.get(1)?,
            exchange_id: row.get(2)?,
            exchange_name: row.get(3)?,
            exchange_color: row.get(4)?,
            wallet_id: row.get(5)?,
            wallet_name: row.get(6)?,
            wallet_color: row.get(7)?,
            balance_sats: row.get(8)?,
            confirmed_tx_count: row.get(9)?,
            tx_index_rate_limited: row.get::<_, i64>(10)? == 1,
        })
    })?;
    let mut values = collect_rows(rows)?;
    values.sort_by(|left, right| {
        right
            .confirmed_tx_count
            .cmp(&left.confirmed_tx_count)
            .then_with(|| left.address.cmp(&right.address))
    });
    Ok(values)
}
fn time_bounds(connection: &Connection) -> Result<FlowTimeBounds, DatabaseError> {
    let (first_tx_time, last_tx_time): (Option<i64>, Option<i64>) = connection.query_row(
        "SELECT MIN(block_time), MAX(block_time) FROM transactions
         WHERE confirmed = 1 AND block_time IS NOT NULL",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (first_ledger_time, last_ledger_time): (Option<i64>, Option<i64>) = connection.query_row(
        "SELECT MIN(timestamp), MAX(timestamp) FROM exchange_transactions",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(FlowTimeBounds {
        first_tx_time,
        last_tx_time,
        first_ledger_time,
        last_ledger_time,
    })
}
fn load_focus_owners(
    connection: &Connection,
    query: &FlowQuery,
) -> Result<BTreeMap<String, FlowAddressOwner>, DatabaseError> {
    let mut sql = String::from(
        "SELECT a.address, a.description, a.exchange_id, e.name, e.brand_color,
                a.wallet_id, w.name, w.brand_color,
                CASE WHEN a.exchange_id IS NOT NULL
                    THEN a.exchange_balance_sats
                    ELSE a.chain_funded_txo_sum - a.chain_spent_txo_sum END,
                (SELECT COUNT(*) FROM transactions t
                 WHERE t.address = a.address AND t.confirmed = 1),
                a.tx_index_rate_limited
         FROM addresses a
         LEFT JOIN exchanges_types e ON e.id = a.exchange_id
         LEFT JOIN wallets_types w ON w.id = a.wallet_id
         WHERE 1 = 1",
    );
    let mut params = Vec::<Value>::new();
    if !query.addresses.is_empty() {
        push_in(
            &mut sql,
            &mut params,
            "a.address",
            query
                .addresses
                .iter()
                .map(|address| Value::Text(address.clone()))
                .collect(),
        );
    } else if !query.exchange_ids.is_empty() {
        push_in(
            &mut sql,
            &mut params,
            "a.exchange_id",
            query
                .exchange_ids
                .iter()
                .map(|id| Value::Integer(*id))
                .collect(),
        );
    }
    sql.push_str(" ORDER BY a.address");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params), owner_from_row)?;
    let mut values = BTreeMap::new();
    for row in rows {
        let owner = row.map_err(|error| DatabaseError::new(error.to_string()))?;
        values.insert(owner.address.clone(), owner);
    }
    Ok(values)
}
fn load_owners(
    connection: &Connection,
    addresses: &[String],
) -> Result<BTreeMap<String, FlowAddressOwner>, DatabaseError> {
    let mut values = BTreeMap::new();
    for chunk in addresses.chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let placeholders = placeholders(chunk.len());
        let sql = format!(
            "SELECT a.address, a.description, a.exchange_id, e.name, e.brand_color,
                    a.wallet_id, w.name, w.brand_color,
                    CASE WHEN a.exchange_id IS NOT NULL
                        THEN a.exchange_balance_sats
                        ELSE a.chain_funded_txo_sum - a.chain_spent_txo_sum END,
                    (SELECT COUNT(*) FROM transactions t
                     WHERE t.address = a.address AND t.confirmed = 1),
                    a.tx_index_rate_limited
             FROM addresses a
             LEFT JOIN exchanges_types e ON e.id = a.exchange_id
             LEFT JOIN wallets_types w ON w.id = a.wallet_id
             WHERE a.address IN ({placeholders})",
        );
        let mut statement = connection.prepare(&sql)?;
        let params = chunk.iter().map(|address| Value::Text(address.clone()));
        let rows = statement.query_map(params_from_iter(params), owner_from_row)?;
        for row in rows {
            let owner = row.map_err(|error| DatabaseError::new(error.to_string()))?;
            values.insert(owner.address.clone(), owner);
        }
    }
    Ok(values)
}
fn owner_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FlowAddressOwner> {
    Ok(FlowAddressOwner {
        address: row.get(0)?,
        description: row.get(1)?,
        exchange_id: row.get(2)?,
        exchange_name: row.get(3)?,
        exchange_color: row.get(4)?,
        wallet_id: row.get(5)?,
        wallet_name: row.get(6)?,
        wallet_color: row.get(7)?,
        balance_sats: row.get(8)?,
        confirmed_tx_count: row.get(9)?,
        tx_index_rate_limited: row.get::<_, i64>(10)? == 1,
    })
}
const LEDGER_SELECT: &str =
    "SELECT t.id, t.exchange_id, a.name, a.provider_id, p.name, p.brand_color,
            t.timestamp, t.tx_type, t.amount_sats, t.match_state, t.on_chain_txid,
            t.matched_tx_id, t.on_chain_local_addresses
     FROM exchange_transactions t
     JOIN exchange_accounts a ON a.id = t.exchange_id
     LEFT JOIN exchanges_types p ON p.id = a.provider_id";
fn ledger_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FlowLedgerRow> {
    let tx_type: String = row.get(7)?;
    let match_state: String = row.get(9)?;
    let local_addresses: Option<String> = row.get(12)?;
    Ok(FlowLedgerRow {
        id: row.get(0)?,
        account_id: row.get(1)?,
        account_name: row.get(2)?,
        provider_id: row.get(3)?,
        provider_name: row.get(4)?,
        provider_color: row.get(5)?,
        timestamp: row.get(6)?,
        tx_type: ExchangeTxType::from_str(&tx_type).unwrap_or(ExchangeTxType::Unknown),
        amount_sats: row.get(8)?,
        match_state: MatchState::from_str(&match_state).unwrap_or(MatchState::Unmatched),
        on_chain_txid: row
            .get::<_, Option<String>>(10)?
            .or_else(|| row.get::<_, Option<String>>(11).ok().flatten()),
        local_addresses: local_addresses
            .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
            .unwrap_or_default(),
    })
}
fn load_ledger_rows(
    connection: &Connection,
    query: &FlowQuery,
) -> Result<Vec<FlowLedgerRow>, DatabaseError> {
    let mut sql = format!("{LEDGER_SELECT} WHERE 1 = 1");
    let mut params = Vec::<Value>::new();
    push_ledger_date_filters(&mut sql, &mut params, query);
    if !query.exchange_ids.is_empty() {
        push_in(
            &mut sql,
            &mut params,
            "p.id",
            query
                .exchange_ids
                .iter()
                .map(|id| Value::Integer(*id))
                .collect(),
        );
    }
    sql.push_str(" ORDER BY t.timestamp, t.id");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(params), ledger_from_row)?;
    collect_rows(rows)
}
fn load_linked_ledger_rows(
    connection: &Connection,
    query: &FlowQuery,
    txids: &BTreeSet<String>,
) -> Result<Vec<FlowLedgerRow>, DatabaseError> {
    let mut values = Vec::new();
    for chunk in txids.iter().collect::<Vec<_>>().chunks(400) {
        if chunk.is_empty() {
            continue;
        }
        let placeholders = placeholders(chunk.len());
        let mut sql = format!(
            "{LEDGER_SELECT} WHERE (t.on_chain_txid IN ({placeholders})
                OR t.matched_tx_id IN ({placeholders}))",
        );
        let mut params = Vec::<Value>::new();
        for txid in chunk {
            params.push(Value::Text((*txid).clone()));
        }
        for txid in chunk {
            params.push(Value::Text((*txid).clone()));
        }
        push_ledger_date_filters(&mut sql, &mut params, query);
        sql.push_str(" ORDER BY t.timestamp, t.id");
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params), ledger_from_row)?;
        values.extend(collect_rows(rows)?);
    }
    values.sort_by(|left, right| {
        left.timestamp
            .cmp(&right.timestamp)
            .then(left.id.cmp(&right.id))
    });
    Ok(values)
}
fn push_ledger_date_filters(sql: &mut String, params: &mut Vec<Value>, query: &FlowQuery) {
    if let Some(from_time) = query.from_time {
        sql.push_str(" AND t.timestamp >= ?");
        params.push(Value::Integer(from_time));
    }
    if let Some(to_time) = query.to_time {
        sql.push_str(" AND t.timestamp <= ?");
        params.push(Value::Integer(to_time));
    }
}
fn focus_txids(
    connection: &Connection,
    focus_addresses: &[String],
    query: &FlowQuery,
) -> Result<BTreeSet<String>, DatabaseError> {
    let mut txids = BTreeSet::new();
    for chunk in focus_addresses.chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let values = vec!["(?)"; chunk.len()].join(",");
        let mut sql = format!(
            "WITH focus(address) AS (VALUES {values})
             SELECT DISTINCT t.txid
             FROM transactions t
             WHERE t.confirmed = 1",
        );
        let mut params: Vec<Value> = chunk
            .iter()
            .map(|address| Value::Text(address.clone()))
            .collect();
        if let Some(from_time) = query.from_time {
            sql.push_str(" AND t.block_time >= ?");
            params.push(Value::Integer(from_time));
        }
        if let Some(to_time) = query.to_time {
            sql.push_str(" AND t.block_time <= ?");
            params.push(Value::Integer(to_time));
        }
        sql.push_str(
            " AND (
                t.address IN (SELECT address FROM focus)
                OR EXISTS (
                    SELECT 1 FROM transaction_inputs i
                    WHERE i.transaction_id = t.id
                      AND i.prevout_scriptpubkey_address IN (SELECT address FROM focus)
                )
                OR EXISTS (
                    SELECT 1 FROM transaction_outputs o
                    WHERE o.transaction_id = t.id
                      AND o.scriptpubkey_address IN (SELECT address FROM focus)
                )
              )",
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(params), |row| row.get::<_, String>(0))?;
        for row in rows {
            txids.insert(row.map_err(|error| DatabaseError::new(error.to_string()))?);
        }
    }
    Ok(txids)
}
#[derive(Debug, Clone)]
struct TransactionRow {
    representative_id: i64,
    txid: String,
    block_time: i64,
    fee_sats: i64,
}
fn load_transactions(
    connection: &Connection,
    txids: &[String],
) -> Result<Vec<FlowTransaction>, DatabaseError> {
    let mut rows = Vec::new();
    for chunk in txids.chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let values = placeholders(chunk.len());
        let sql = format!(
            "SELECT MIN(id), txid, COALESCE(MAX(block_time), 0), COALESCE(MAX(fee), 0)
             FROM transactions
             WHERE confirmed = 1 AND txid IN ({values})
             GROUP BY txid
             ORDER BY COALESCE(MAX(block_time), 0), txid",
        );
        let mut statement = connection.prepare(&sql)?;
        let params = chunk.iter().map(|txid| Value::Text(txid.clone()));
        let mapped = statement.query_map(params_from_iter(params), |row| {
            Ok(TransactionRow {
                representative_id: row.get(0)?,
                txid: row.get(1)?,
                block_time: row.get(2)?,
                fee_sats: row.get(3)?,
            })
        })?;
        for row in mapped {
            rows.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
        }
    }
    let mut inputs_by_transaction: HashMap<i64, Vec<FlowInput>> = HashMap::new();
    let mut outputs_by_transaction: HashMap<i64, Vec<FlowOutput>> = HashMap::new();
    let ids = rows
        .iter()
        .map(|row| row.representative_id)
        .collect::<Vec<_>>();
    for chunk in ids.chunks(500) {
        if chunk.is_empty() {
            continue;
        }
        let values = placeholders(chunk.len());
        let input_sql = format!(
            "SELECT transaction_id, prev_txid, prev_vout, prevout_scriptpubkey_address,
                    COALESCE(prevout_value, 0), is_coinbase
             FROM transaction_inputs
             WHERE transaction_id IN ({values})
             ORDER BY transaction_id, input_index",
        );
        let mut statement = connection.prepare(&input_sql)?;
        let params = chunk.iter().map(|id| Value::Integer(*id));
        let mapped = statement.query_map(params_from_iter(params), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                FlowInput {
                    prev_txid: row.get(1)?,
                    prev_vout: row.get(2)?,
                    address: row.get(3)?,
                    value_sats: row.get(4)?,
                    is_coinbase: row.get::<_, i64>(5)? == 1,
                },
            ))
        })?;
        for row in mapped {
            let (transaction_id, input) =
                row.map_err(|error| DatabaseError::new(error.to_string()))?;
            inputs_by_transaction
                .entry(transaction_id)
                .or_default()
                .push(input);
        }
        let output_sql = format!(
            "SELECT transaction_id, output_index, scriptpubkey_address, value, scriptpubkey_type
             FROM transaction_outputs
             WHERE transaction_id IN ({values})
             ORDER BY transaction_id, output_index",
        );
        let mut statement = connection.prepare(&output_sql)?;
        let params = chunk.iter().map(|id| Value::Integer(*id));
        let mapped = statement.query_map(params_from_iter(params), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                FlowOutput {
                    output_index: row.get(1)?,
                    address: row.get(2)?,
                    value_sats: row.get(3)?,
                    scriptpubkey_type: row.get(4)?,
                },
            ))
        })?;
        for row in mapped {
            let (transaction_id, output) =
                row.map_err(|error| DatabaseError::new(error.to_string()))?;
            outputs_by_transaction
                .entry(transaction_id)
                .or_default()
                .push(output);
        }
    }
    let transactions = rows
        .into_iter()
        .map(|row| FlowTransaction {
            txid: row.txid,
            block_time: row.block_time,
            fee_sats: row.fee_sats,
            inputs: inputs_by_transaction
                .remove(&row.representative_id)
                .unwrap_or_default(),
            outputs: outputs_by_transaction
                .remove(&row.representative_id)
                .unwrap_or_default(),
        })
        .collect();
    Ok(transactions)
}
fn push_in(sql: &mut String, params: &mut Vec<Value>, column: &str, values: Vec<Value>) {
    if values.is_empty() {
        return;
    }
    let placeholders = placeholders(values.len());
    sql.push_str(&format!(" AND {column} IN ({placeholders})"));
    params.extend(values);
}
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(",")
}
fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, DatabaseError> {
    let mut values = Vec::new();
    for row in rows {
        values.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(values)
}
/// Validate a query before it reaches SQL or graph construction.
pub fn validate_query(query: &FlowQuery) -> Result<(), String> {
    if query.exchange_ids.len() > MAX_FILTER_VALUES || query.addresses.len() > MAX_FILTER_VALUES {
        return Err(format!(
            "Select at most {MAX_FILTER_VALUES} values per filter."
        ));
    }
    if let (Some(from_time), Some(to_time)) = (query.from_time, query.to_time) {
        if from_time > to_time {
            return Err("The start date must be before the end date.".to_string());
        }
    }
    Ok(())
}
/// Parse the table sort key, returning the repository-owned allowlist value.
pub fn parse_link_sort(key: Option<&str>) -> Result<Option<FlowLinkSortColumn>, String> {
    match key {
        None => Ok(None),
        Some(key) => FlowLinkSortColumn::from_key(key)
            .map(Some)
            .ok_or_else(|| format!("unknown sort column: {key}")),
    }
}
/// Parse the evidence sort key.
pub fn parse_evidence_sort(key: Option<&str>) -> Result<Option<FlowEvidenceSortColumn>, String> {
    match key {
        None => Ok(None),
        Some(key) => FlowEvidenceSortColumn::from_key(key)
            .map(Some)
            .ok_or_else(|| format!("unknown sort column: {key}")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::repositories::addresses as address_repository;
    use crate::domain::address::StoredAddress;
    fn insert_address(
        database: &Database,
        address: &str,
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
        description: Option<&str>,
        balance_sats: i64,
    ) {
        address_repository::insert_address(
            database,
            &StoredAddress {
                address: address.to_string(),
                description: description.map(str::to_string),
                exchange_id,
                wallet_id,
                chain_funded_txo_sum: balance_sats,
                ..StoredAddress::default()
            },
        )
        .unwrap();
    }
    fn insert_transaction(
        database: &Database,
        address: &str,
        txid: &str,
        block_time: i64,
        inputs: &[(&str, i64, &str, i64, bool)],
        outputs: &[(&str, i64)],
    ) {
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO transactions (
                    address, txid, version, locktime, size, weight, fee,
                    confirmed, block_time
                 ) VALUES (?1, ?2, 2, 0, 200, 500, 10, 1, ?3)",
                rusqlite::params![address, txid, block_time],
            )
            .unwrap();
        let transaction_id = connection.last_insert_rowid();
        for (index, (prev_txid, prev_vout, prev_address, value, coinbase)) in
            inputs.iter().enumerate()
        {
            connection
                .execute(
                    "INSERT INTO transaction_inputs (
                        transaction_id, input_index, prev_txid, prev_vout,
                        prevout_scriptpubkey_address, prevout_value, scriptsig,
                        scriptsig_asm, is_coinbase, sequence
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', '', ?7, 0)",
                    rusqlite::params![
                        transaction_id,
                        index as i64,
                        prev_txid,
                        prev_vout,
                        prev_address,
                        value,
                        i64::from(*coinbase)
                    ],
                )
                .unwrap();
        }
        for (index, (output_address, value)) in outputs.iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO transaction_outputs (
                        transaction_id, output_index, scriptpubkey, scriptpubkey_asm,
                        scriptpubkey_type, scriptpubkey_address, value
                     ) VALUES (?1, ?2, '00', 'OP_0', 'v0_p2wpkh', ?3, ?4)",
                    rusqlite::params![transaction_id, index as i64, output_address, value],
                )
                .unwrap();
        }
    }
    fn fixture() -> Database {
        let database = Database::open_in_memory().unwrap();
        insert_address(&database, "wallet-a", None, Some(1), Some("Wallet A"), 700);
        insert_address(&database, "plain-b", None, None, Some("Plain B"), 0);
        insert_address(&database, "binance-c", Some(1), None, None, 0);
        insert_address(&database, "plain-d", None, None, None, 0);
        insert_transaction(
            &database,
            "wallet-a",
            "tx-one",
            100,
            &[("prev-one", 0, "external-src", 1_000, false)],
            &[("plain-b", 700), ("external-dst", 200)],
        );
        insert_transaction(
            &database,
            "binance-c",
            "tx-one",
            100,
            &[("prev-one", 0, "external-src", 1_000, false)],
            &[("plain-b", 700), ("external-dst", 200)],
        );
        insert_transaction(
            &database,
            "plain-b",
            "tx-two",
            200,
            &[("tx-one", 0, "plain-b", 700, false)],
            &[("external-final", 690)],
        );
        insert_transaction(
            &database,
            "plain-d",
            "tx-unrelated",
            300,
            &[("prev-unrelated", 0, "external-unrelated", 50, false)],
            &[("external-unrelated-output", 40)],
        );
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO exchange_accounts (
                    id, name, provider_id, created_at, last_modified_at
                 ) VALUES (1, 'Main', 1, 0, 0)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO exchange_transactions (
                    exchange_id, fingerprint, timestamp, tx_type, amount_sats, currency,
                    raw_data, match_state, matched_tx_id, on_chain_txid, created_at, updated_at
                 ) VALUES (1, 'linked', 210, 'withdrawal', 690, 'BTC', '{}',
                    'hash_match', 'tx-two', 'tx-two', 0, 0)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO exchange_transactions (
                    exchange_id, fingerprint, timestamp, tx_type, amount_sats, currency,
                    raw_data, match_state, created_at, updated_at
                 ) VALUES (1, 'unmatched', 220, 'deposit', 500, 'BTC', '{}',
                    'unmatched', 0, 0)",
                [],
            )
            .unwrap();
        drop(connection);
        database
    }
    fn query() -> FlowQuery {
        FlowQuery::default()
    }
    #[test]
    fn query_validation_rejects_inverted_ranges_and_oversized_filters() {
        let mut filter = query();
        filter.from_time = Some(200);
        filter.to_time = Some(100);
        assert!(validate_query(&filter).is_err());
        let mut filter = query();
        filter.addresses = vec!["a".to_string(); MAX_FILTER_VALUES + 1];
        assert!(validate_query(&filter).is_err());
        assert!(validate_query(&query()).is_ok());
    }
    #[test]
    fn options_count_data_bearing_catalogs_and_addresses() {
        let options = filter_options(&fixture()).unwrap();
        assert_eq!(options.addresses.len(), 4);
        assert!(options
            .exchanges
            .iter()
            .find(|entry| entry.id == 1)
            .is_some_and(|entry| { entry.address_count == 1 && entry.account_count == 1 }),);
    }
    #[test]
    fn options_sort_exchanges_by_ledger_accounts_and_addresses_by_transactions() {
        let database = fixture();
        {
            let connection = database.connection().unwrap();
            for (id, name) in [(2, "Coinbase A"), (3, "Coinbase B")] {
                connection
                    .execute(
                        "INSERT INTO exchange_accounts (
                            id, name, provider_id, created_at, last_modified_at
                         ) VALUES (?1, ?2, 2, 0, 0)",
                        rusqlite::params![id, name],
                    )
                    .unwrap();
            }
        }
        insert_transaction(
            &database,
            "wallet-a",
            "tx-extra",
            400,
            &[("prev-extra", 0, "external-extra", 100, false)],
            &[("external-extra-out", 90)],
        );
        let options = filter_options(&database).unwrap();
        assert_eq!(options.exchanges[0].id, 2);
        assert_eq!(options.exchanges[0].account_count, 2);
        assert_eq!(options.addresses[0].address, "wallet-a");
        assert_eq!(options.addresses[0].confirmed_tx_count, 2);
    }
    #[test]
    fn address_filter_loads_only_touching_transactions_and_their_context() {
        let database = fixture();
        let mut filter = query();
        filter.addresses = vec!["wallet-a".to_string()];
        let dataset = load_dataset(&database, &filter).unwrap();
        assert_eq!(dataset.transactions.len(), 1);
        assert_eq!(dataset.transactions[0].txid, "tx-one");
        assert_eq!(dataset.transactions[0].inputs.len(), 1);
        assert_eq!(dataset.transactions[0].outputs.len(), 2);
        assert!(dataset.owners.contains_key("plain-b"));
        assert!(!dataset.owners.contains_key("external-src"));
        assert!(dataset.ledger.is_empty());
    }
    #[test]
    fn exchange_filter_loads_linked_on_chain_transactions() {
        let database = fixture();
        let mut filter = query();
        filter.exchange_ids = vec![1];
        let dataset = load_dataset(&database, &filter).unwrap();
        let txids: BTreeSet<&str> = dataset
            .transactions
            .iter()
            .map(|transaction| transaction.txid.as_str())
            .collect();
        assert!(txids.contains("tx-one"));
        assert!(txids.contains("tx-two"));
        assert!(!txids.contains("tx-unrelated"));
        assert_eq!(dataset.ledger.len(), 2);
    }
    #[test]
    fn explicit_addresses_take_precedence_over_owner_facets() {
        let database = fixture();
        let mut filter = query();
        filter.addresses = vec!["wallet-a".to_string()];
        filter.exchange_ids = vec![1];
        let dataset = load_dataset(&database, &filter).unwrap();
        assert_eq!(dataset.focus_addresses, vec!["wallet-a".to_string()]);
        assert_eq!(dataset.ledger.len(), 2);
        assert!(dataset
            .transactions
            .iter()
            .any(|transaction| transaction.txid == "tx-two"),);
    }
    #[test]
    fn date_range_filters_both_address_and_linked_ledger_transactions() {
        let database = fixture();
        let mut filter = query();
        filter.from_time = Some(150);
        filter.to_time = Some(250);
        let dataset = load_dataset(&database, &filter).unwrap();
        let txids: BTreeSet<&str> = dataset
            .transactions
            .iter()
            .map(|transaction| transaction.txid.as_str())
            .collect();
        assert_eq!(txids, ["tx-two"].into_iter().collect());
        assert!(dataset.ledger.iter().all(|row| row.timestamp <= 250));
    }
    #[test]
    fn mirrored_transaction_rows_are_deduplicated_in_the_dataset() {
        let database = fixture();
        let dataset = load_dataset(&database, &query()).unwrap();
        let tx_one = dataset
            .transactions
            .iter()
            .filter(|transaction| transaction.txid == "tx-one")
            .count();
        assert_eq!(tx_one, 1);
        let stored = dataset
            .transactions
            .iter()
            .find(|transaction| transaction.txid == "tx-one")
            .unwrap();
        assert_eq!(stored.inputs.len(), 1);
        assert_eq!(stored.outputs.len(), 2);
    }
    #[test]
    fn transaction_view_keeps_the_focus_address_and_external_context() {
        let database = fixture();
        let dataset = load_dataset(&database, &query()).unwrap();
        let graph = crate::domain::flow::build_transaction_graph(&dataset);
        assert!(graph
            .nodes
            .iter()
            .any(|node| node.kind == crate::domain::flow::FlowNodeKind::Transaction),);
        assert!(graph
            .links
            .iter()
            .any(|link| link.source.starts_with("src:")));
        assert!(graph
            .links
            .iter()
            .any(|link| link.target.starts_with("dst:")));
    }
    #[test]
    fn summary_views_are_bounded_and_preserve_the_ledger_unlinked_edge() {
        let database = fixture();
        let dataset = load_dataset(&database, &query()).unwrap();
        let addresses = crate::domain::flow::build_address_summary(&dataset);
        assert!(addresses.nodes.len() < 20);
        assert!(addresses
            .links
            .iter()
            .any(|link| { link.kind == crate::domain::flow::FlowLinkKind::LedgerUnlinked }),);
    }
}
