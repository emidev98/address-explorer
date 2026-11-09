//! Exchange ledger endpoints.
#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::exchanges::ExchangeService;
use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountDetail, ExchangeAccountList, ExchangeAccountTableQuery,
    ExchangeLinkOutcome, ExchangeReconcileBatchSummary, ExchangeTransaction,
    ExchangeTransactionSaveOutcome, ExchangeTransactionSaveRequest, ExchangeTransactionTableQuery,
    ImportDataMapping, ImportDataPreview, ImportDataSummary, OnChainLinkCandidateView,
};
use crate::domain::table::TablePage;
use dioxus::prelude::*;
#[server]
#[post("/api/exchanges/list")]
pub async fn list_exchange_accounts_api(
    query: ExchangeAccountTableQuery,
) -> Result<ExchangeAccountList, ServerFnError> {
    remote_post!("/api/exchanges/list", [], [], [query]);
    ExchangeService::connect(open_database()?)
        .list_accounts(&query)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[get("/api/exchanges/:id")]
pub async fn get_exchange_account_api(id: i64) -> Result<ExchangeAccountDetail, ServerFnError> {
    remote_get!("/api/exchanges/{}", [id], []);
    ExchangeService::connect(open_database()?)
        .detail(id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Create an account without importing rows. `provider_id = None` is the
/// explicit Manual Import mode.
#[server]
#[post("/api/exchanges")]
pub async fn create_exchange_account_api(
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    remote_post!("/api/exchanges", [], [], [name, provider_id]);
    ExchangeService::connect(open_database()?)
        .create_account(&name, provider_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[put("/api/exchanges/:id")]
pub async fn update_exchange_account_api(
    id: i64,
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    remote_put!("/api/exchanges/{}", [id], [], [name, provider_id]);
    ExchangeService::connect(open_database()?)
        .update_account(id, &name, provider_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Delete several exchange accounts and their cascaded ledger rows in one
/// transaction. Returns the number of deleted accounts.
#[server]
#[post("/api/exchanges/delete")]
pub async fn delete_exchange_accounts_api(ids: Vec<i64>) -> Result<usize, ServerFnError> {
    remote_post!("/api/exchanges/delete", [], [], [ids]);
    ExchangeService::connect(open_database()?)
        .delete_accounts(&ids)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/exchanges/csv/preview")]
pub async fn preview_import_data_api(
    csv_text: String,
    mapping: Option<ImportDataMapping>,
) -> Result<ImportDataPreview, ServerFnError> {
    remote_post!("/api/exchanges/csv/preview", [], [], [csv_text, mapping]);
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
    remote_post!(
        "/api/exchanges/csv/import",
        [],
        [],
        [exchange_id, name, provider_id, csv_text, mapping]
    );
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
    remote_post!("/api/exchanges/transactions/list", [], [], [query]);
    ExchangeService::connect(open_database()?)
        .list_transactions(&query)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[get("/api/exchanges/:account_id/transactions/:transaction_id")]
pub async fn get_exchange_transaction_api(
    account_id: i64,
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    remote_get!(
        "/api/exchanges/{}/transactions/{}",
        [account_id, transaction_id],
        []
    );
    ExchangeService::connect(open_database()?)
        .get_transaction(account_id, transaction_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/exchanges/transactions/save")]
pub async fn save_exchange_transaction_api(
    request: ExchangeTransactionSaveRequest,
) -> Result<ExchangeTransactionSaveOutcome, ServerFnError> {
    remote_post!("/api/exchanges/transactions/save", [], [], [request]);
    ExchangeService::connect(open_database()?)
        .save_transaction(&request)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Delete several ledger transactions of one account in one transaction.
/// Returns the number of deleted rows.
#[server]
#[post("/api/exchanges/:account_id/transactions/delete")]
pub async fn delete_exchange_transactions_api(
    account_id: i64,
    ids: Vec<i64>,
) -> Result<usize, ServerFnError> {
    remote_post!(
        "/api/exchanges/{}/transactions/delete",
        [account_id],
        [],
        [ids]
    );
    ExchangeService::connect(open_database()?)
        .delete_transactions(account_id, &ids)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[post("/api/exchanges/:account_id/reconcile")]
pub async fn reconcile_exchange_account_api(account_id: i64) -> Result<(), ServerFnError> {
    remote_post!("/api/exchanges/{}/reconcile", [account_id], [], []);
    ExchangeService::connect(open_database()?)
        .reconcile(account_id)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Reconcile several exchange accounts in one request. Each account is
/// reconciled independently; the summary reports partial failures.
#[server]
#[post("/api/exchanges/reconcile")]
pub async fn reconcile_exchange_accounts_api(
    ids: Vec<i64>,
) -> Result<ExchangeReconcileBatchSummary, ServerFnError> {
    remote_post!("/api/exchanges/reconcile", [], [], [ids]);
    ExchangeService::connect(open_database()?)
        .reconcile_many(&ids)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[get("/api/exchanges/transactions/:transaction_id/candidates?search")]
pub async fn exchange_link_candidates_api(
    transaction_id: i64,
    search: String,
) -> Result<Vec<OnChainLinkCandidateView>, ServerFnError> {
    remote_get!(
        "/api/exchanges/transactions/{}/candidates",
        [transaction_id],
        [search]
    );
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
    remote_post!(
        "/api/exchanges/transactions/{}/link",
        [transaction_id],
        [],
        [txid, allow_duplicate]
    );
    ExchangeService::connect(open_database()?)
        .link(transaction_id, &txid, allow_duplicate)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
#[server]
#[delete("/api/exchanges/transactions/:transaction_id/link")]
pub async fn clear_exchange_transaction_link_api(
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    remote_delete!("/api/exchanges/transactions/{}/link", [transaction_id], []);
    ExchangeService::connect(open_database()?)
        .clear_link(transaction_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
