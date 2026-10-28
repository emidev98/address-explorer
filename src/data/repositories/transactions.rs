//! SQL access to `transactions`, `transaction_inputs`, and
//! `transaction_outputs`.
//!
//! The repository persists the mempool `Tx` wire model because that is the
//! shape being indexed; the service layer fetches the transactions and this
//! module stores them atomically.

use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};

use crate::clients::mempool::bitcoin::types::Tx;
use crate::data::{Database, DatabaseError};
use crate::domain::transaction::TransactionSyncSummary;

/// Store a batch of transactions and their inputs/outputs atomically.
///
/// Existing transactions for the same address are updated instead of
/// inserted again, and their inputs/outputs are replaced so a re-sync
/// cannot accumulate duplicate child rows.
pub fn sync_transactions(
    database: &Database,
    address: &str,
    transactions: &[Tx],
    truncated: bool,
) -> Result<TransactionSyncSummary, DatabaseError> {
    let mut connection = database.connection()?;
    let db_transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let inserted = upsert_transactions(&db_transaction, address, transactions)?;
    let stored: i64 = db_transaction.query_row(
        "SELECT COUNT(*) FROM transactions WHERE address = ?1",
        [address],
        |row| row.get(0),
    )?;

    db_transaction.commit()?;

    Ok(TransactionSyncSummary {
        address: address.to_string(),
        fetched: transactions.len(),
        inserted,
        stored: stored as usize,
        truncated,
        // The repository persists what it is given; the service layer knows
        // whether the walk stopped on a rate limit and sets the flag.
        rate_limited: false,
    })
}

/// Copy the locally stored transactions that involve an exchange address.
///
/// Full indexing of an exchange address is impractical because the mempool
/// history pages of such addresses are effectively unbounded. Instead the
/// reverse search starts from the known addresses: every transaction
/// already stored under another address is scanned for inputs and outputs
/// that reference `exchange_address`, and each matching transaction is
/// mirrored under the exchange address with all of its inputs and outputs.
///
/// The mirror is idempotent: transactions already stored for the address
/// are left untouched, so repeated syncs only report new relations.
/// Relations thus appear only once the addresses they involve have been
/// indexed themselves; syncing the exchange address first simply reports
/// that nothing is stored yet.
pub fn mirror_related_transactions(
    database: &Database,
    exchange_address: &str,
) -> Result<TransactionSyncSummary, DatabaseError> {
    let related_txids = related_txids(database, exchange_address)?;

    let mut connection = database.connection()?;
    let db_transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let mut inserted = 0;
    for txid in &related_txids {
        inserted += mirror_stored_transaction(&db_transaction, exchange_address, txid)?;
    }

    let stored: i64 = db_transaction.query_row(
        "SELECT COUNT(*) FROM transactions WHERE address = ?1",
        [exchange_address],
        |row| row.get(0),
    )?;

    db_transaction.commit()?;

    Ok(TransactionSyncSummary {
        address: exchange_address.to_string(),
        fetched: related_txids.len(),
        inserted,
        stored: stored as usize,
        // The reverse search is not paged; it either covers the local
        // store completely or fails.
        truncated: false,
        // The reverse search touches no external API, so it cannot be
        // rate-limited.
        rate_limited: false,
    })
}

/// Txid of the oldest stored confirmed transaction of the address.
///
/// This is the cursor where an interrupted backward indexing walk stopped:
/// resuming with it as `after_txid` continues into older history instead of
/// re-walking the already indexed pages. Unconfirmed rows have no block
/// time and sit at the newest end of the history, so they never qualify as
/// the resume cursor.
pub fn oldest_confirmed_txid(
    database: &Database,
    address: &str,
) -> Result<Option<String>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT txid FROM transactions
             WHERE address = ?1 AND confirmed = 1 AND block_time IS NOT NULL
             ORDER BY block_time ASC, id ASC
             LIMIT 1",
            [address],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

/// Txids of stored transactions whose inputs or outputs reference the
/// address, collected from every other address row. Ordered by txid so
/// the mirror stays deterministic across syncs.
fn related_txids(
    database: &Database,
    exchange_address: &str,
) -> Result<Vec<String>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT DISTINCT t.txid
         FROM transactions t
         JOIN transaction_outputs o ON o.transaction_id = t.id
         WHERE t.address <> ?1 AND o.scriptpubkey_address = ?1
         UNION
         SELECT DISTINCT t.txid
         FROM transactions t
         JOIN transaction_inputs i ON i.transaction_id = t.id
         WHERE t.address <> ?1 AND i.prevout_scriptpubkey_address = ?1
         ORDER BY txid",
    )?;
    let rows = statement.query_map([exchange_address], |row| row.get::<_, String>(0))?;
    let mut txids = Vec::new();
    for txid in rows {
        txids.push(txid.map_err(|err| DatabaseError::new(err.to_string()))?);
    }
    Ok(txids)
}

/// Snapshot of the originating address row for one txid.
struct StoredTransactionSource {
    id: i64,
    version: i64,
    locktime: i64,
    size: i64,
    weight: i64,
    fee: i64,
    confirmed: i64,
    block_height: Option<i64>,
    block_hash: Option<String>,
    block_time: Option<i64>,
}

/// Mirror one stored transaction under `exchange_address`.
///
/// Copying keeps the exact stored data instead of re-fetching it from the
/// mempool: the originating row was already validated and fully indexed,
/// and the mirror must not re-do that work. Returns whether a row was
/// added (already-mirrored txids are skipped entirely).
fn mirror_stored_transaction(
    db_transaction: &Transaction<'_>,
    exchange_address: &str,
    txid: &str,
) -> Result<usize, DatabaseError> {
    let source = db_transaction
        .query_row(
            "SELECT id, version, locktime, size, weight, fee, confirmed,
                block_height, block_hash, block_time
             FROM transactions
             WHERE txid = ?1 AND address <> ?2
             ORDER BY id LIMIT 1",
            params![txid, exchange_address],
            |row| {
                Ok(StoredTransactionSource {
                    id: row.get(0)?,
                    version: row.get(1)?,
                    locktime: row.get(2)?,
                    size: row.get(3)?,
                    weight: row.get(4)?,
                    fee: row.get(5)?,
                    confirmed: row.get(6)?,
                    block_height: row.get(7)?,
                    block_hash: row.get(8)?,
                    block_time: row.get(9)?,
                })
            },
        )
        .optional()?;

    let Some(source) = source else {
        return Ok(0);
    };

    let was_inserted = db_transaction.execute(
        "INSERT INTO transactions (
            address, txid, version, locktime, size, weight, fee,
            confirmed, block_height, block_hash, block_time
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(address, txid) DO NOTHING",
        params![
            exchange_address,
            txid,
            source.version,
            source.locktime,
            source.size,
            source.weight,
            source.fee,
            source.confirmed,
            source.block_height,
            source.block_hash,
            source.block_time,
        ],
    )?;

    if was_inserted == 0 {
        return Ok(0);
    }

    let transaction_id = db_transaction.last_insert_rowid();

    db_transaction.execute(
        "INSERT INTO transaction_inputs (
            transaction_id, input_index, prev_txid, prev_vout,
            prevout_scriptpubkey, prevout_scriptpubkey_asm,
            prevout_scriptpubkey_type, prevout_scriptpubkey_address,
            prevout_value, scriptsig, scriptsig_asm, is_coinbase, sequence
         )
         SELECT ?1, s.input_index, s.prev_txid, s.prev_vout,
             s.prevout_scriptpubkey, s.prevout_scriptpubkey_asm,
             s.prevout_scriptpubkey_type, s.prevout_scriptpubkey_address,
             s.prevout_value, s.scriptsig, s.scriptsig_asm, s.is_coinbase, s.sequence
         FROM transaction_inputs s WHERE s.transaction_id = ?2",
        params![transaction_id, source.id],
    )?;

    db_transaction.execute(
        "INSERT INTO transaction_outputs (
            transaction_id, output_index, scriptpubkey, scriptpubkey_asm,
            scriptpubkey_type, scriptpubkey_address, value
         )
         SELECT ?1, s.output_index, s.scriptpubkey, s.scriptpubkey_asm,
             s.scriptpubkey_type, s.scriptpubkey_address, s.value
         FROM transaction_outputs s WHERE s.transaction_id = ?2",
        params![transaction_id, source.id],
    )?;

    Ok(1)
}

fn upsert_transactions(
    db_transaction: &Transaction<'_>,
    address: &str,
    transactions: &[Tx],
) -> Result<usize, DatabaseError> {
    let mut inserted = 0;

    for transaction in transactions {
        let was_inserted = db_transaction.execute(
            "INSERT INTO transactions (
                address, txid, version, locktime, size, weight, fee,
                confirmed, block_height, block_hash, block_time
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(address, txid) DO NOTHING",
            params![
                address,
                transaction.txid,
                transaction.version,
                transaction.locktime,
                transaction.size,
                transaction.weight,
                transaction.fee,
                transaction.status.confirmed,
                transaction.status.block_height,
                transaction.status.block_hash,
                transaction.status.block_time,
            ],
        )?;

        if was_inserted == 1 {
            inserted += 1;
        } else {
            // A transaction's status changes when it moves from the
            // mempool into a block, so refresh its mutable metadata.
            db_transaction.execute(
                "UPDATE transactions SET
                    version = ?3, locktime = ?4, size = ?5, weight = ?6, fee = ?7,
                    confirmed = ?8, block_height = ?9, block_hash = ?10,
                    block_time = ?11,
                    synced_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE address = ?1 AND txid = ?2",
                params![
                    address,
                    transaction.txid,
                    transaction.version,
                    transaction.locktime,
                    transaction.size,
                    transaction.weight,
                    transaction.fee,
                    transaction.status.confirmed,
                    transaction.status.block_height,
                    transaction.status.block_hash,
                    transaction.status.block_time,
                ],
            )?;
        }

        let transaction_id: i64 = db_transaction.query_row(
            "SELECT id FROM transactions WHERE address = ?1 AND txid = ?2",
            params![address, transaction.txid],
            |row| row.get(0),
        )?;

        replace_inputs(db_transaction, transaction_id, transaction)?;
        replace_outputs(db_transaction, transaction_id, transaction)?;
    }

    Ok(inserted)
}

fn replace_inputs(
    db_transaction: &Transaction<'_>,
    transaction_id: i64,
    transaction: &Tx,
) -> Result<(), DatabaseError> {
    db_transaction.execute(
        "DELETE FROM transaction_inputs WHERE transaction_id = ?1",
        [transaction_id],
    )?;

    for (input_index, input) in transaction.vin.iter().enumerate() {
        let prevout = input.prevout.as_ref();
        db_transaction.execute(
            "INSERT INTO transaction_inputs (
                transaction_id, input_index, prev_txid, prev_vout,
                prevout_scriptpubkey, prevout_scriptpubkey_asm,
                prevout_scriptpubkey_type, prevout_scriptpubkey_address,
                prevout_value, scriptsig, scriptsig_asm, is_coinbase, sequence
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                transaction_id,
                input_index as i64,
                input.txid,
                input.vout,
                prevout.map(|value| value.scriptpubkey.as_str()),
                prevout.map(|value| value.scriptpubkey_asm.as_str()),
                prevout.map(|value| value.scriptpubkey_type.as_str()),
                prevout.and_then(|value| value.scriptpubkey_address.as_deref()),
                prevout.map(|value| value.value),
                input.scriptsig,
                input.scriptsig_asm,
                input.is_coinbase,
                input.sequence,
            ],
        )?;
    }

    Ok(())
}

fn replace_outputs(
    db_transaction: &Transaction<'_>,
    transaction_id: i64,
    transaction: &Tx,
) -> Result<(), DatabaseError> {
    db_transaction.execute(
        "DELETE FROM transaction_outputs WHERE transaction_id = ?1",
        [transaction_id],
    )?;

    for (output_index, output) in transaction.vout.iter().enumerate() {
        db_transaction.execute(
            "INSERT INTO transaction_outputs (
                transaction_id, output_index, scriptpubkey, scriptpubkey_asm,
                scriptpubkey_type, scriptpubkey_address, value
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                transaction_id,
                output_index as i64,
                output.scriptpubkey,
                output.scriptpubkey_asm,
                output.scriptpubkey_type,
                output.scriptpubkey_address.as_deref(),
                output.value,
            ],
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::mempool::bitcoin::types::{TxStatus, TxVin, Vout};
    use crate::data::repositories::addresses as address_repository;
    use crate::domain::address::AddressEntity;

    const ADDRESS: &str = "bc1qtestaddress";

    fn sample_vout(address: Option<&str>, value: i64) -> Vout {
        Vout {
            scriptpubkey: "0014abcd".to_string(),
            scriptpubkey_asm: "OP_0 OP_PUSHBYTES_20 abcd".to_string(),
            scriptpubkey_type: "v0_p2wpkh".to_string(),
            scriptpubkey_address: address.map(str::to_string),
            value,
        }
    }

    fn sample_tx(txid: &str) -> Tx {
        Tx {
            txid: txid.to_string(),
            version: 2,
            locktime: 0,
            vin: vec![TxVin {
                txid: "previous-txid".to_string(),
                vout: 1,
                prevout: Some(sample_vout(Some(ADDRESS), 1_000)),
                scriptsig: String::new(),
                scriptsig_asm: String::new(),
                is_coinbase: false,
                sequence: 0xffff_ffff,
            }],
            vout: vec![sample_vout(Some(ADDRESS), 600), sample_vout(None, 300)],
            size: 222,
            weight: 561,
            fee: 100,
            status: TxStatus {
                confirmed: true,
                block_height: Some(800_000),
                block_hash: Some("block-hash".to_string()),
                block_time: Some(1_700_000_000),
            },
        }
    }

    fn database_with_address(address: &str) -> Database {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(
            &database,
            &AddressEntity {
                address: address.to_string(),
                ..AddressEntity::default()
            },
        )
        .unwrap();
        database
    }

    fn count(database: &Database, table: &str) -> i64 {
        database
            .connection()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn syncing_the_same_transactions_twice_is_idempotent() {
        let database = database_with_address(ADDRESS);
        let transactions = vec![sample_tx("tx-one"), sample_tx("tx-two")];

        let first = sync_transactions(&database, ADDRESS, &transactions, false).unwrap();
        assert_eq!(first.fetched, 2);
        assert_eq!(first.inserted, 2);
        assert_eq!(first.stored, 2);
        assert_eq!(count(&database, "transactions"), 2);
        assert_eq!(count(&database, "transaction_inputs"), 2);
        assert_eq!(count(&database, "transaction_outputs"), 4);

        let second = sync_transactions(&database, ADDRESS, &transactions, false).unwrap();
        assert_eq!(second.inserted, 0);
        assert_eq!(second.stored, 2);
        assert_eq!(count(&database, "transactions"), 2);
        assert_eq!(count(&database, "transaction_inputs"), 2);
        assert_eq!(count(&database, "transaction_outputs"), 4);
    }

    #[test]
    fn resync_updates_confirmation_metadata() {
        let database = database_with_address(ADDRESS);

        let mut unconfirmed = sample_tx("tx-pending");
        unconfirmed.status = TxStatus {
            confirmed: false,
            block_height: None,
            block_hash: None,
            block_time: None,
        };

        sync_transactions(&database, ADDRESS, &[unconfirmed], false).unwrap();

        let mut confirmed = sample_tx("tx-pending");
        confirmed.status.block_height = Some(810_000);
        confirmed.status.block_hash = Some("new-block".to_string());
        confirmed.status.block_time = Some(1_700_000_100);

        let summary = sync_transactions(&database, ADDRESS, &[confirmed], false).unwrap();
        assert_eq!(summary.inserted, 0);

        let connection = database.connection().unwrap();
        let (confirmed, height, hash, time): (i64, i64, String, i64) = connection
            .query_row(
                "SELECT confirmed, block_height, block_hash, block_time
                 FROM transactions WHERE address = ?1 AND txid = ?2",
                (ADDRESS, "tx-pending"),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();

        assert_eq!(confirmed, 1);
        assert_eq!(height, 810_000);
        assert_eq!(hash, "new-block");
        assert_eq!(time, 1_700_000_100);
    }

    #[test]
    fn deleting_an_address_cascades_to_its_transactions() {
        let database = database_with_address(ADDRESS);
        sync_transactions(&database, ADDRESS, &[sample_tx("tx-one")], false).unwrap();

        address_repository::delete_address(&database, ADDRESS).unwrap();

        assert_eq!(count(&database, "transactions"), 0);
        assert_eq!(count(&database, "transaction_inputs"), 0);
        assert_eq!(count(&database, "transaction_outputs"), 0);
    }

    #[test]
    fn oldest_confirmed_txid_is_the_resume_cursor_of_an_interrupted_walk() {
        let database = database_with_address(ADDRESS);

        assert_eq!(oldest_confirmed_txid(&database, ADDRESS).unwrap(), None);

        let mut oldest = sample_tx("tx-old");
        oldest.status.block_time = Some(1_600_000_000);
        let mut newest = sample_tx("tx-new");
        newest.status.block_time = Some(1_700_000_000);
        let mut pending = sample_tx("tx-pending");
        pending.status = TxStatus {
            confirmed: false,
            block_height: None,
            block_hash: None,
            block_time: None,
        };
        sync_transactions(&database, ADDRESS, &[oldest, newest, pending], false).unwrap();

        // The unconfirmed row must not win even though it was stored last.
        assert_eq!(
            oldest_confirmed_txid(&database, ADDRESS).unwrap(),
            Some("tx-old".to_string())
        );
    }

    const EXCHANGE_ADDRESS: &str = "bc1qexchange";
    const UNRELATED_ADDRESS: &str = "bc1qunrelated";

    fn database_with_exchange() -> Database {
        let database = Database::open_in_memory().unwrap();
        for address in [ADDRESS, EXCHANGE_ADDRESS, UNRELATED_ADDRESS] {
            address_repository::insert_address(
                &database,
                &AddressEntity {
                    address: address.to_string(),
                    exchange_id: (address == EXCHANGE_ADDRESS).then_some(1),
                    ..AddressEntity::default()
                },
            )
            .unwrap();
        }
        database
    }

    fn stored_txids_for(database: &Database, address: &str) -> Vec<String> {
        database
            .connection()
            .unwrap()
            .prepare("SELECT txid FROM transactions WHERE address = ?1 ORDER BY txid")
            .unwrap()
            .query_map([address], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn mirroring_copies_only_transactions_related_to_the_exchange_address() {
        let database = database_with_exchange();

        // Deposit: an input spent FROM the exchange address.
        let mut deposit = sample_tx("tx-deposit");
        deposit.vin[0].prevout = Some(sample_vout(Some(EXCHANGE_ADDRESS), 5_000));
        deposit.vout = vec![sample_vout(Some(ADDRESS), 4_800), sample_vout(None, 200)];

        // Withdrawal: an output paid TO the exchange address.
        let mut withdrawal = sample_tx("tx-withdrawal");
        withdrawal.vout = vec![
            sample_vout(Some(ADDRESS), 100),
            sample_vout(Some(EXCHANGE_ADDRESS), 900),
        ];

        // Unrelated: neither inputs nor outputs involve the exchange address.
        let mut unrelated = sample_tx("tx-unrelated");
        unrelated.vout = vec![sample_vout(Some(UNRELATED_ADDRESS), 700)];

        sync_transactions(&database, ADDRESS, &[deposit, withdrawal, unrelated], false).unwrap();
        sync_transactions(&database, UNRELATED_ADDRESS, &[sample_tx("tx-two")], false).unwrap();

        let summary = mirror_related_transactions(&database, EXCHANGE_ADDRESS).unwrap();

        assert_eq!(summary.address, EXCHANGE_ADDRESS);
        assert_eq!(summary.fetched, 2);
        assert_eq!(summary.inserted, 2);
        assert_eq!(summary.stored, 2);
        assert!(!summary.truncated);
        assert_eq!(
            stored_txids_for(&database, EXCHANGE_ADDRESS),
            ["tx-deposit", "tx-withdrawal"]
        );

        // Inputs and outputs are copied, not just the transaction header.
        assert_eq!(
            count(&database, "transaction_inputs"),
            // 3 inputs under ADDRESS + 1 under UNRELATED + 2 mirrored
            3 + 1 + 2
        );
        assert_eq!(
            count(&database, "transaction_outputs"),
            // 5 outputs under ADDRESS (2 + 2 + 1) + 2 under UNRELATED + 4 mirrored (2 + 2)
            5 + 2 + 4
        );
    }

    #[test]
    fn mirroring_is_idempotent_and_reports_up_to_date() {
        let database = database_with_exchange();

        let mut deposit = sample_tx("tx-deposit");
        deposit.vin[0].prevout = Some(sample_vout(Some(EXCHANGE_ADDRESS), 5_000));

        sync_transactions(&database, ADDRESS, &[deposit], false).unwrap();

        let first = mirror_related_transactions(&database, EXCHANGE_ADDRESS).unwrap();
        assert_eq!(first.inserted, 1);

        let second = mirror_related_transactions(&database, EXCHANGE_ADDRESS).unwrap();
        assert_eq!(second.fetched, 1);
        assert_eq!(second.inserted, 0);
        assert_eq!(second.stored, 1);
        // One wallet row plus one mirrored row; re-running adds nothing.
        assert_eq!(count(&database, "transactions"), 2);
        assert_eq!(count(&database, "transaction_inputs"), 2);
        assert_eq!(count(&database, "transaction_outputs"), 4);
    }

    #[test]
    fn mirroring_an_exchange_address_without_relations_stores_nothing() {
        let database = database_with_exchange();
        sync_transactions(&database, ADDRESS, &[sample_tx("tx-only-wallet")], false).unwrap();

        let summary = mirror_related_transactions(&database, EXCHANGE_ADDRESS).unwrap();

        assert_eq!(summary.fetched, 0);
        assert_eq!(summary.inserted, 0);
        assert_eq!(summary.stored, 0);
        assert_eq!(count(&database, "transactions"), 1);
    }
}
