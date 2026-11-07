//! Domain types for the persisted Bitcoin transaction history.
//!
//! Transactions are indexed per address: `transactions.address` references
//! `addresses.address`, and the `(address, txid)` uniqueness constraint makes
//! repeated syncs idempotent. Inputs and outputs reference the stored
//! transaction row and are replaced as part of the same database transaction.
//!
//! Two sync modes exist. Ordinary addresses are indexed by walking the
//! mempool address history page by page. Addresses claimed by an exchange
//! (`addresses.exchange_id`) are far too large to index completely, so they
//! run a reverse search instead: transactions already stored for the known
//! addresses in the database are scanned for inputs and outputs that involve
//! the exchange address, and only those matching transactions are mirrored
//! under its row.

use serde::{Deserialize, Serialize};

/// Result of syncing the transaction history of one address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSyncSummary {
    pub address: String,
    /// Unique transactions returned by the mempool API.
    pub fetched: usize,
    /// Transactions that did not already exist for this address.
    pub inserted: usize,
    /// Total transactions stored for this address after the sync.
    pub stored: usize,
    /// Whether the API page limit was reached before the full history was read.
    pub truncated: bool,
    /// Whether the walk stopped on a mempool rate limit. The pages fetched
    /// before the limit are stored, and the next sync resumes from the
    /// oldest indexed transaction.
    pub rate_limited: bool,
}

/// Result of syncing the transaction history of several addresses at once.
///
/// A bulk sync is sequential and continues past a failing address, so one
/// unreachable or unknown address never discards the progress made for the
/// others. The `synced` field holds the summaries of the addresses that
/// completed, `failed` the per-address failures.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSyncBatchSummary {
    /// Summaries of the addresses whose sync completed.
    pub synced: Vec<TransactionSyncSummary>,
    /// Failures of the addresses whose sync did not complete.
    pub failed: Vec<TransactionSyncFailure>,
}

impl TransactionSyncBatchSummary {
    /// Total transactions stored across the successfully synced addresses.
    pub fn stored(&self) -> usize {
        self.synced.iter().map(|summary| summary.stored).sum()
    }

    /// Transactions that were new across the successfully synced addresses.
    pub fn inserted(&self) -> usize {
        self.synced.iter().map(|summary| summary.inserted).sum()
    }

    /// Number of successfully synced addresses that paused on the mempool
    /// rate limit and can resume on a later sync.
    pub fn rate_limited_count(&self) -> usize {
        self.synced
            .iter()
            .filter(|summary| summary.rate_limited)
            .count()
    }

    /// Number of successfully synced addresses that reached the page limit.
    pub fn truncated_count(&self) -> usize {
        self.synced
            .iter()
            .filter(|summary| summary.truncated)
            .count()
    }
}

/// One address that failed inside a bulk transaction sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionSyncFailure {
    pub address: String,
    /// User-facing description of the failure.
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(
        address: &str,
        stored: usize,
        inserted: usize,
        truncated: bool,
        rate_limited: bool,
    ) -> TransactionSyncSummary {
        TransactionSyncSummary {
            address: address.to_string(),
            fetched: stored,
            inserted,
            stored,
            truncated,
            rate_limited,
        }
    }

    #[test]
    fn batch_summary_aggregates_only_the_successful_addresses() {
        let batch = TransactionSyncBatchSummary {
            synced: vec![
                summary("bc1qone", 10, 2, false, false),
                summary("bc1qtwo", 5, 0, true, false),
                summary("bc1qthree", 7, 1, false, true),
            ],
            failed: vec![TransactionSyncFailure {
                address: "bc1qfour".to_string(),
                error: "address not found".to_string(),
            }],
        };

        assert_eq!(batch.stored(), 22);
        assert_eq!(batch.inserted(), 3);
        assert_eq!(batch.truncated_count(), 1);
        assert_eq!(batch.rate_limited_count(), 1);
        assert_eq!(batch.synced.len(), 3);
        assert_eq!(batch.failed.len(), 1);
    }

    #[test]
    fn empty_batch_has_zero_totals() {
        let batch = TransactionSyncBatchSummary::default();

        assert_eq!(batch.stored(), 0);
        assert_eq!(batch.inserted(), 0);
        assert_eq!(batch.truncated_count(), 0);
        assert_eq!(batch.rate_limited_count(), 0);
    }
}
