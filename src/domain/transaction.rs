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
