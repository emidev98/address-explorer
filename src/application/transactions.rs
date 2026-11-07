//! Transaction sync service: fetch and persist the transaction history of
//! stored addresses.
//!
//! Ordinary addresses are indexed by walking the mempool address history page
//! by page. Exchange addresses (`addresses.exchange_id`) skip the full history
//! walk: their histories are effectively unbounded, so only the transactions
//! that relate to the other known addresses in the database are mirrored —
//! see `data::repositories::transactions::mirror_related_transactions`.

use std::time::Duration;

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
const MEMPOOL_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
                timeout: Some(MEMPOOL_REQUEST_TIMEOUT),
                ..MempoolConfig::default()
            }),
        }
    }

    /// Fetch and persist the transaction history for one stored address.
    ///
    /// Network I/O completes before the database transaction starts, so the
    /// connection lock is never held while waiting on the mempool API.
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

        if stored_address.exchange_id.is_some() {
            return Ok(transaction_repository::mirror_related_transactions(
                &self.database,
                address,
            )?);
        }

        // An address marked rate-limited resumes where the interrupted walk
        // stopped; any other address re-walks from the newest transactions
        // so freshly confirmed activity is picked up.
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
    /// is rejected. Ordinary addresses sync first because an exchange runs a
    /// reverse search over the transactions already stored for them, so the
    /// batch must never depend on the lexical order of the selection. The
    /// addresses sync sequentially so a batch never fans out concurrent
    /// mempool requests that would exhaust the provider's rate limit.
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
        use std::collections::HashSet;

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
/// Addresses missing from the store count as ordinary so their sync reports
/// the not-found failure.
fn partition_by_exchange_ownership(
    database: &Database,
    addresses: &[String],
) -> Result<(Vec<String>, Vec<String>), TransactionSyncError> {
    let mut ordinary = Vec::new();
    let mut exchanges = Vec::new();

    for address in addresses {
        let is_exchange = address_repository::get_address(database, address)?
            .is_some_and(|stored| stored.exchange_id.is_some());
        if is_exchange {
            exchanges.push(address.clone());
        } else {
            ordinary.push(address.clone());
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
            // SAFETY: the vtable never dereferences the null data pointer:
            // cloning manufactures another null waker and the remaining
            // operations are no-ops, so no requirement of `RawWaker` is
            // violated.
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }

        // SAFETY: `raw_waker` builds a `RawWaker` whose vtable satisfies the
        // contract documented above; the data pointer is never dereferenced.
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
            vec!["bc1qfirst".to_string(), "bc1qsecond".to_string()]
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
        ] {
            address_repository::insert_address(&database, &entity).unwrap();
        }

        let selection = vec!["bc1qexchange".to_string(), "bc1qwallet".to_string()];
        let (ordinary, exchanges) = partition_by_exchange_ownership(&database, &selection).unwrap();

        assert_eq!(ordinary, vec!["bc1qwallet"]);
        assert_eq!(exchanges, vec!["bc1qexchange"]);
        // An address that vanished from the store keeps its not-found
        // failure in the ordinary pass instead of being treated as exchange.
        let missing = vec!["bc1qmissing".to_string()];
        let (ordinary, exchanges) = partition_by_exchange_ownership(&database, &missing).unwrap();
        assert_eq!(ordinary, vec!["bc1qmissing"]);
        assert!(exchanges.is_empty());
    }
}
