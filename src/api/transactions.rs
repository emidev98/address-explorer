//! Transaction endpoints.

use dioxus::prelude::*;

use crate::domain::transaction::{TransactionSyncBatchSummary, TransactionSyncSummary};

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::transactions::TransactionService;

/// Fetch and persist the transaction history for one stored address.
#[server]
#[post("/api/addresses/:address/transactions/sync")]
pub async fn sync_address_transactions_api(
    address: String,
) -> Result<TransactionSyncSummary, ServerFnError> {
    TransactionService::connect(open_database()?)
        .sync_address_transactions(&address)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Fetch and persist the transaction history of several selected addresses.
///
/// The selection is deduplicated and an empty one is rejected by the
/// service. Per-address failures do not fail the whole request: they are
/// reported in the batch summary so the UI can show the addresses that
/// completed alongside the ones that failed.
#[server]
#[post("/api/addresses/transactions/sync")]
pub async fn sync_addresses_transactions_api(
    addresses: Vec<String>,
) -> Result<TransactionSyncBatchSummary, ServerFnError> {
    TransactionService::connect(open_database()?)
        .sync_addresses_transactions(&addresses)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
