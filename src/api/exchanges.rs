//! Exchange ledger endpoints.

use dioxus::prelude::*;

use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountList, ExchangeAccountTableQuery, ExchangeDetail,
    ExchangeLinkOutcome, ExchangeTransaction, ExchangeTransactionSaveOutcome,
    ExchangeTransactionSaveRequest, ExchangeTransactionTableQuery, ImportDataMapping,
    ImportDataPreview, ImportDataSummary, OnChainLinkCandidateView,
};
use crate::domain::table::TablePage;

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::exchanges::ExchangeService;

#[server]
#[post("/api/exchanges/list")]
pub async fn list_exchanges_api(
    query: ExchangeAccountTableQuery,
) -> Result<ExchangeAccountList, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .list_accounts(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/exchanges/:id")]
pub async fn get_exchange_api(id: i64) -> Result<ExchangeDetail, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .detail(id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Create an account without importing rows. `provider_id = None` is the
/// explicit Manual Import mode.
#[server]
#[post("/api/exchanges")]
pub async fn create_exchange_api(
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .create_account(&name, provider_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[put("/api/exchanges/:id")]
pub async fn update_exchange_api(
    id: i64,
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .update_account(id, &name, provider_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[delete("/api/exchanges/:id")]
pub async fn delete_exchange_api(id: i64) -> Result<bool, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .delete_account(id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/exchanges/csv/preview")]
pub async fn preview_import_data_api(
    csv_text: String,
    mapping: Option<ImportDataMapping>,
) -> Result<ImportDataPreview, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .preview_import_data(&csv_text, mapping.as_ref())
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Import a CSV into a new account (`exchange_id = None`) or append to an
/// existing one. `provider_id = None` is the explicit Manual Import mode.
#[server]
#[post("/api/exchanges/csv/import")]
pub async fn import_data_api(
    exchange_id: Option<i64>,
    name: Option<String>,
    provider_id: Option<i64>,
    csv_text: String,
    mapping: Option<ImportDataMapping>,
) -> Result<ImportDataSummary, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .import_data(
            exchange_id,
            name.as_deref(),
            provider_id,
            &csv_text,
            mapping.as_ref(),
        )
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/exchanges/transactions/list")]
pub async fn list_exchange_transactions_api(
    query: ExchangeTransactionTableQuery,
) -> Result<TablePage<ExchangeTransaction>, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .list_transactions(&query)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/exchanges/:exchange_id/transactions/:transaction_id")]
pub async fn get_exchange_transaction_api(
    exchange_id: i64,
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .get_transaction(exchange_id, transaction_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/exchanges/transactions/save")]
pub async fn save_exchange_transaction_api(
    request: ExchangeTransactionSaveRequest,
) -> Result<ExchangeTransactionSaveOutcome, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .save_transaction(&request)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[delete("/api/exchanges/:exchange_id/transactions/:transaction_id")]
pub async fn delete_exchange_transaction_api(
    exchange_id: i64,
    transaction_id: i64,
) -> Result<bool, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .delete_transaction(exchange_id, transaction_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/exchanges/:exchange_id/reconcile")]
pub async fn reconcile_exchange_api(exchange_id: i64) -> Result<(), ServerFnError> {
    ExchangeService::connect(open_database()?)
        .reconcile(exchange_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/exchanges/transactions/:transaction_id/candidates?search")]
pub async fn exchange_link_candidates_api(
    transaction_id: i64,
    search: String,
) -> Result<Vec<OnChainLinkCandidateView>, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .candidates(transaction_id, &search)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/exchanges/transactions/:transaction_id/link")]
pub async fn link_exchange_transaction_api(
    transaction_id: i64,
    txid: String,
    allow_duplicate: bool,
) -> Result<ExchangeLinkOutcome, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .link(transaction_id, &txid, allow_duplicate)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[delete("/api/exchanges/transactions/:transaction_id/link")]
pub async fn clear_exchange_transaction_link_api(
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    ExchangeService::connect(open_database()?)
        .clear_link(transaction_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
