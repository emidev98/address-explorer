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
use crate::domain::transaction::TransactionSyncSummary;

/// Failures of the transaction sync service.
#[derive(Debug, thiserror::Error)]
pub enum TransactionSyncError {
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
