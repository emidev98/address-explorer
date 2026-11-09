//! Browser wrappers for the transaction endpoints.

use crate::domain::transaction::{TransactionSyncBatchSummary, TransactionSyncSummary};
use dioxus_fullstack_core::ServerFnError;

pub async fn sync_address_transactions_api(
    address: String,
) -> Result<TransactionSyncSummary, ServerFnError> {
    let path = format!("/api/addresses/{address}/transactions/sync");
    crate::web_post_body!(
        "transactions.sync_address",
        &path,
        { "address": address }
    )
}

pub async fn sync_addresses_transactions_api(
    addresses: Vec<String>,
) -> Result<TransactionSyncBatchSummary, ServerFnError> {
    crate::web_post_body!(
        "transactions.sync_addresses",
        "/api/addresses/transactions/sync",
        { "addresses": addresses }
    )
}
