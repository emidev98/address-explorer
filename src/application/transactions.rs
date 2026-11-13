//! Transaction sync service: fetch and persist the transaction history of
//! stored addresses.
//!
//! Ordinary addresses are indexed by walking the mempool address history page
//! by page. Exchange addresses (`addresses.exchange_id`) skip the full history
//! walk: their histories are effectively unbounded, so only the transactions
//! that relate to the other known addresses in the database are mirrored. The
//! reverse search covers the whole exchange provider, not only the clicked
//! address, so relations stored under sibling provider addresses are found
//! too — see
//! `data::repositories::transactions::mirror_related_transactions_for_provider`.
use crate::clients::mempool::{
    bitcoin::types::Tx, config::MempoolConfig, error::MempoolError, Mempool,
};
use crate::data::repositories::{
    addresses as address_repository, transactions as transaction_repository,
};
use crate::data::{Database, DatabaseError};
use crate::domain::transaction::{
    TransactionSyncBatchSummary, TransactionSyncFailure, TransactionSyncSummary,
};
use std::collections::HashSet;
/// Failures of the transaction sync service.
#[derive(Debug, thiserror::Error)]
pub enum TransactionSyncError {
    #[error("Select at least one address.")]
    EmptySelection,
    #[error("address not found")]
    AddressNotFound,
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("mempool request failed: {0}")]
    Mempool(#[from] MempoolError),
}
const MAX_ADDRESS_SYNC_PAGES: usize = 100;
/// Facade over the transactions repository and the mempool client.
pub struct TransactionService {
    database: Database,
    mempool: Mempool,
}
impl TransactionService {
    /// Connect the mempool client with the timeout used for sync walks.
    pub fn connect(database: Database) -> Self {
        Self {
            database,
            mempool: Mempool::new(MempoolConfig {
                timeout: Some(std::time::Duration::from_secs(
                    crate::security::policy::current().external_timeout_secs,
                )),
                max_response_bytes: crate::security::policy::current().external_response_max_bytes
                    as usize,
                ..MempoolConfig::default()
            }),
        }
    }
    /// Fetch and persist the transaction history for one stored address.
    ///
    /// Network I/O completes before the database transaction starts, so the
    /// connection lock is never held while waiting on the mempool API.
    ///
    /// Exchange-owned addresses do not walk the mempool history. Their sync
    /// is a reverse search over the transactions already stored for the
    /// other declared addresses and covers every address of the exchange
    /// provider, because only the provider's balance anchor is shown in the
    /// addresses table. The returned summary therefore carries the row that
    /// started the sync as `address` and provider-wide counts.
    ///
    /// Rate limiting is a pause, not a failure: the pages fetched before the
    /// limit are stored, the address is marked `tx_index_rate_limited`, and
    /// the next sync of such an address resumes the backward walk from its
    /// oldest indexed transaction instead of re-walking known history. A
    /// completed walk clears the mark.
    pub async fn sync_address_transactions(
        &self,
        address: &str,
    ) -> Result<TransactionSyncSummary, TransactionSyncError> {
        let stored_address = address_repository::get_address(&self.database, address)?
            .ok_or(TransactionSyncError::AddressNotFound)?;
        if let Some(provider_id) = stored_address.exchange_id {
            let mut summary = transaction_repository::mirror_related_transactions_for_provider(
                &self.database,
                provider_id,
            )?;
            summary.address = address.to_string();
            return Ok(summary);
        }
        let after_txid = if stored_address.tx_index_rate_limited {
            transaction_repository::oldest_confirmed_txid(&self.database, address)?
        } else {
            None
        };
        let (transactions, stop) = self.fetch_address_transactions(address, after_txid).await?;
        let mut summary = transaction_repository::sync_transactions(
            &self.database,
            address,
            &transactions,
            matches!(stop, FetchStop::PageLimit),
        )?;
        summary.rate_limited = matches!(stop, FetchStop::RateLimited);
        address_repository::set_tx_index_rate_limited(
            &self.database,
            address,
            summary.rate_limited,
        )?;
        Ok(summary)
    }
    /// Sync the transaction histories of several stored addresses.
    ///
    /// This is the use case behind the addresses table's bulk sync action.
    /// The selection is sorted, deduplicated, and split into ordinary and
    /// exchange-owned addresses before any work starts; an empty selection
    /// is rejected. Exchange providers collapse to one sync because the
    /// reverse search covers all of their addresses. Ordinary addresses sync
    /// first because an exchange runs a reverse search over the transactions
    /// already stored for them, so the batch must never depend on the lexical
    /// order of the selection. The addresses sync sequentially so a batch
    /// never fans out concurrent mempool requests that would exhaust the
    /// provider's rate limit.
    ///
    /// Each address keeps its own database transaction: network I/O happens
    /// before the matching write, so the connection is never locked while
    /// waiting on the mempool API. A failing address is recorded in the
    /// summary's `failed` list and the batch continues with the remaining
    /// addresses, so one failure never discards their results.
    pub async fn sync_addresses_transactions(
        &self,
        addresses: &[String],
    ) -> Result<TransactionSyncBatchSummary, TransactionSyncError> {
        let addresses = normalize_selection(addresses);
        if addresses.is_empty() {
            return Err(TransactionSyncError::EmptySelection);
        }
        let (ordinary, exchanges) = partition_by_exchange_ownership(&self.database, &addresses)?;
        let mut summary = TransactionSyncBatchSummary::default();
        for address in ordinary.iter().chain(exchanges.iter()) {
            match self.sync_address_transactions(address).await {
                Ok(result) => summary.synced.push(result),
                Err(error) => summary.failed.push(TransactionSyncFailure {
                    address: address.clone(),
                    error: error.to_string(),
                }),
            }
        }
        Ok(summary)
    }
    /// Read every transaction page exposed by the mempool address endpoint,
    /// optionally resuming from `after_txid`.
    ///
    /// The endpoint returns a bounded first page and accepts the last seen
    /// txid as `after_txid`. A seen-txid set protects against an API that
    /// repeats mempool transactions on later pages or ignores the cursor.
    /// A rate-limited response ends the walk without discarding the pages
    /// already fetched, so the caller can persist the progress.
    async fn fetch_address_transactions(
        &self,
        address: &str,
        after_txid: Option<String>,
    ) -> Result<(Vec<Tx>, FetchStop), MempoolError> {
        let mut transactions = Vec::new();
        let mut seen_txids = HashSet::new();
        let mut after_txid = after_txid;
        for _ in 0..MAX_ADDRESS_SYNC_PAGES {
            let page = match self
                .mempool
                .bitcoin
                .addresses
                .get_address_txs(address, after_txid.as_deref())
                .await
            {
                Ok(page) => page,
                Err(MempoolError::RateLimited) => {
                    return Ok((transactions, FetchStop::RateLimited));
                }
                Err(error) => return Err(error),
            };
            if page.is_empty() {
                return Ok((transactions, FetchStop::Complete));
            }
            let next_after_txid = page.last().map(|transaction| transaction.txid.clone());
            let mut found_new_transaction = false;
            for transaction in page {
                if seen_txids.insert(transaction.txid.clone()) {
                    transactions.push(transaction);
                    found_new_transaction = true;
                }
            }
            if !found_new_transaction {
                return Ok((transactions, FetchStop::Complete));
            }
            if next_after_txid == after_txid {
                return Ok((transactions, FetchStop::Complete));
            }
            after_txid = next_after_txid;
        }
        Ok((transactions, FetchStop::PageLimit))
    }
}
/// Normalize a bulk selection: duplicate addresses collapse to one sync and
/// sorting makes the order deterministic even though the UI selection set
/// has no order.
fn normalize_selection(addresses: &[String]) -> Vec<String> {
    let mut addresses = addresses.to_vec();
    addresses.sort();
    addresses.dedup();
    addresses
}
/// Split a bulk selection into ordinary addresses and exchange-owned
/// addresses.
///
/// Ordinary addresses are indexed by walking their mempool history, while an
/// exchange address is mirrored from the transactions already stored for the
/// other addresses. Running the ordinary addresses first lets the exchange
/// reverse search see their freshly indexed transactions in the same batch.
/// Only the first selected address of each exchange provider is kept because
/// the provider-wide reverse search covers the rest. Addresses missing from
/// the store count as ordinary so their sync reports the not-found failure.
fn partition_by_exchange_ownership(
    database: &Database,
    addresses: &[String],
) -> Result<(Vec<String>, Vec<String>), TransactionSyncError> {
    let mut ordinary = Vec::new();
    let mut exchanges = Vec::new();
    let mut seen_providers = HashSet::new();
    for address in addresses {
        let exchange_id = address_repository::get_address(database, address)?
            .and_then(|stored| stored.exchange_id);
        match exchange_id {
            Some(provider_id) => {
                if seen_providers.insert(provider_id) {
                    exchanges.push(address.clone());
                }
            }
            None => ordinary.push(address.clone()),
        }
    }
    Ok((ordinary, exchanges))
}
/// Why a transaction history walk stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchStop {
    /// The API had no more pages (or repeated itself): the stored history
    /// now covers the address completely.
    Complete,
    /// The page limit `MAX_ADDRESS_SYNC_PAGES` was reached; more history
    /// may remain.
    PageLimit,
    /// The API answered 429; the walk can resume later from the oldest
    /// indexed transaction.
    RateLimited,
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::address::StoredAddress;
    /// Drive a future to completion without an async runtime. The empty
    /// selection resolves before any network request is started.
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        use std::task::{Context, RawWaker, RawWakerVTable, Waker};
        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn noop(_: *const ()) {}
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => output,
            std::task::Poll::Pending => panic!("test future unexpectedly yielded"),
        }
    }
    #[test]
    fn bulk_selection_is_sorted_and_deduplicated() {
        let selection = vec![
            "bc1qsecond".to_string(),
            "bc1qfirst".to_string(),
            "bc1qsecond".to_string(),
        ];
        assert_eq!(
            normalize_selection(&selection),
            vec!["bc1qfirst".to_string(), "bc1qsecond".to_string()],
        );
        assert!(normalize_selection(&[]).is_empty());
    }
    #[test]
    fn bulk_sync_rejects_an_empty_selection() {
        let service = TransactionService::connect(Database::open_in_memory().unwrap());
        let result = block_on(service.sync_addresses_transactions(&[]));
        assert!(matches!(result, Err(TransactionSyncError::EmptySelection)));
    }
    #[test]
    fn bulk_selection_syncs_exchange_addresses_after_ordinary_ones() {
        let database = Database::open_in_memory().unwrap();
        for entity in [
            StoredAddress {
                address: "bc1qwallet".to_string(),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchange".to_string(),
                exchange_id: Some(1),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchange2".to_string(),
                exchange_id: Some(1),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchangethird".to_string(),
                exchange_id: Some(2),
                ..StoredAddress::default()
            },
        ] {
            address_repository::insert_address(&database, &entity).unwrap();
        }
        let selection = vec![
            "bc1qexchange".to_string(),
            "bc1qexchange2".to_string(),
            "bc1qexchangethird".to_string(),
            "bc1qwallet".to_string(),
        ];
        let (ordinary, exchanges) = partition_by_exchange_ownership(&database, &selection).unwrap();
        assert_eq!(ordinary, vec!["bc1qwallet"]);
        assert_eq!(exchanges, vec!["bc1qexchange", "bc1qexchangethird"]);
        let missing = vec!["bc1qmissing".to_string()];
        let (ordinary, exchanges) = partition_by_exchange_ownership(&database, &missing).unwrap();
        assert_eq!(ordinary, vec!["bc1qmissing"]);
        assert!(exchanges.is_empty());
    }
    fn insert_wallet_transaction(
        database: &Database,
        address: &str,
        txid: &str,
        output_address: &str,
    ) {
        let connection = database.connection().unwrap();
        connection
            .execute(
                "INSERT INTO transactions (
                    address, txid, version, locktime, size, weight, fee,
                    confirmed, block_time
                 ) VALUES (?1, ?2, 2, 0, 100, 400, 1000, 1, 1700000000)",
                rusqlite::params![address, txid],
            )
            .unwrap();
        let transaction_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO transaction_outputs (
                    transaction_id, output_index, scriptpubkey, scriptpubkey_asm,
                    scriptpubkey_type, scriptpubkey_address, value
                 ) VALUES (?1, 0, '0014abcd', 'OP_0 OP_PUSHBYTES_20 abcd',
                    'v0_p2wpkh', ?2, 1000)",
                rusqlite::params![transaction_id, output_address],
            )
            .unwrap();
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
    fn exchange_sync_covers_every_address_of_the_provider() {
        let database = Database::open_in_memory().unwrap();
        for entity in [
            StoredAddress {
                address: "bc1qwallet".to_string(),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchangeanchor".to_string(),
                exchange_id: Some(1),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchangesibling".to_string(),
                exchange_id: Some(1),
                ..StoredAddress::default()
            },
            StoredAddress {
                address: "bc1qexchangeother".to_string(),
                exchange_id: Some(2),
                ..StoredAddress::default()
            },
        ] {
            address_repository::insert_address(&database, &entity).unwrap();
        }
        address_repository::set_tx_index_rate_limited(&database, "bc1qexchangeanchor", true)
            .unwrap();
        address_repository::set_tx_index_rate_limited(&database, "bc1qexchangesibling", true)
            .unwrap();
        insert_wallet_transaction(&database, "bc1qwallet", "tx-related", "bc1qexchangesibling");
        insert_wallet_transaction(
            &database,
            "bc1qwallet",
            "tx-other-provider",
            "bc1qexchangeother",
        );
        let service = TransactionService::connect(database.clone());
        let summary = block_on(service.sync_address_transactions("bc1qexchangeanchor")).unwrap();
        assert_eq!(summary.address, "bc1qexchangeanchor");
        assert_eq!(summary.fetched, 1);
        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.stored, 1);
        assert_eq!(
            stored_txids_for(&database, "bc1qexchangesibling"),
            ["tx-related"]
        );
        assert!(stored_txids_for(&database, "bc1qexchangeanchor").is_empty());
        assert!(stored_txids_for(&database, "bc1qexchangeother").is_empty());
        for address in ["bc1qexchangeanchor", "bc1qexchangesibling"] {
            assert!(
                !address_repository::get_address(&database, address)
                    .unwrap()
                    .unwrap()
                    .tx_index_rate_limited,
            );
        }
    }
}
