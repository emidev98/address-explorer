//! Transaction endpoints.

use dioxus::prelude::*;

use crate::domain::transaction::TransactionSyncSummary;

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
