//! Browser wrappers for the exchange ledger endpoints.

use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountDetail, ExchangeAccountList, ExchangeAccountTableQuery,
    ExchangeLinkOutcome, ExchangeReconcileBatchSummary, ExchangeTransaction,
    ExchangeTransactionSaveOutcome, ExchangeTransactionSaveRequest, ExchangeTransactionTableQuery,
    ImportDataMapping, ImportDataPreview, ImportDataSummary, OnChainLinkCandidateView,
};
use crate::domain::table::TablePage;
use dioxus_fullstack_core::ServerFnError;

pub async fn list_exchange_accounts_api(
    query: ExchangeAccountTableQuery,
) -> Result<ExchangeAccountList, ServerFnError> {
    crate::web_post_body!("exchanges.list", "/api/exchanges/list", { "query": query })
}

pub async fn get_exchange_account_api(id: i64) -> Result<ExchangeAccountDetail, ServerFnError> {
    let path = format!("/api/exchanges/{id}");
    crate::web_get_query!("exchanges.get", &path, { "id": id })
}

pub async fn create_exchange_account_api(
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    crate::web_post_body!(
        "exchanges.create",
        "/api/exchanges",
        { "name": name, "provider_id": provider_id }
    )
}

pub async fn update_exchange_account_api(
    id: i64,
    name: String,
    provider_id: Option<i64>,
) -> Result<ExchangeAccount, ServerFnError> {
    let path = format!("/api/exchanges/{id}");
    crate::web_put_body!(
        "exchanges.update",
        &path,
        { "id": id, "name": name, "provider_id": provider_id }
    )
}

pub async fn delete_exchange_accounts_api(ids: Vec<i64>) -> Result<usize, ServerFnError> {
    crate::web_post_body!("exchanges.delete", "/api/exchanges/delete", { "ids": ids })
}

pub async fn preview_import_data_api(
    csv_text: String,
    mapping: Option<ImportDataMapping>,
) -> Result<ImportDataPreview, ServerFnError> {
    crate::web_post_body!(
        "exchanges.csv_preview",
        "/api/exchanges/csv/preview",
        { "csv_text": csv_text, "mapping": mapping }
    )
}

pub async fn import_data_api(
    exchange_id: Option<i64>,
    name: Option<String>,
    provider_id: Option<i64>,
    csv_text: String,
    mapping: Option<ImportDataMapping>,
) -> Result<ImportDataSummary, ServerFnError> {
    crate::web_post_body!(
        "exchanges.csv_import",
        "/api/exchanges/csv/import",
        {
            "exchange_id": exchange_id,
            "name": name,
            "provider_id": provider_id,
            "csv_text": csv_text,
            "mapping": mapping
        }
    )
}

pub async fn list_exchange_transactions_api(
    query: ExchangeTransactionTableQuery,
) -> Result<TablePage<ExchangeTransaction>, ServerFnError> {
    crate::web_post_body!(
        "exchanges.transactions_list",
        "/api/exchanges/transactions/list",
        { "query": query }
    )
}

pub async fn get_exchange_transaction_api(
    account_id: i64,
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    let path = format!("/api/exchanges/{account_id}/transactions/{transaction_id}");
    crate::web_get_query!(
        "exchanges.transaction_get",
        &path,
        { "account_id": account_id, "transaction_id": transaction_id }
    )
}

pub async fn save_exchange_transaction_api(
    request: ExchangeTransactionSaveRequest,
) -> Result<ExchangeTransactionSaveOutcome, ServerFnError> {
    crate::web_post_body!(
        "exchanges.transaction_save",
        "/api/exchanges/transactions/save",
        { "request": request }
    )
}

pub async fn delete_exchange_transactions_api(
    account_id: i64,
    ids: Vec<i64>,
) -> Result<usize, ServerFnError> {
    let path = format!("/api/exchanges/{account_id}/transactions/delete");
    crate::web_post_body!(
        "exchanges.transactions_delete",
        &path,
        { "account_id": account_id, "ids": ids }
    )
}

pub async fn reconcile_exchange_account_api(account_id: i64) -> Result<(), ServerFnError> {
    let path = format!("/api/exchanges/{account_id}/reconcile");
    crate::web_post_body!("exchanges.reconcile", &path, { "account_id": account_id })
}

pub async fn reconcile_exchange_accounts_api(
    ids: Vec<i64>,
) -> Result<ExchangeReconcileBatchSummary, ServerFnError> {
    crate::web_post_body!("exchanges.reconcile_many", "/api/exchanges/reconcile", { "ids": ids })
}

pub async fn exchange_link_candidates_api(
    transaction_id: i64,
    search: String,
) -> Result<Vec<OnChainLinkCandidateView>, ServerFnError> {
    let path = format!("/api/exchanges/transactions/{transaction_id}/candidates");
    crate::web_get_query!(
        "exchanges.link_candidates",
        &path,
        { "transaction_id": transaction_id, "search": search }
    )
}

pub async fn link_exchange_transaction_api(
    transaction_id: i64,
    txid: String,
    allow_duplicate: bool,
) -> Result<ExchangeLinkOutcome, ServerFnError> {
    let path = format!("/api/exchanges/transactions/{transaction_id}/link");
    crate::web_post_body!(
        "exchanges.link",
        &path,
        { "transaction_id": transaction_id, "txid": txid, "allow_duplicate": allow_duplicate }
    )
}

pub async fn clear_exchange_transaction_link_api(
    transaction_id: i64,
) -> Result<ExchangeTransaction, ServerFnError> {
    let path = format!("/api/exchanges/transactions/{transaction_id}/link");
    crate::web_delete_query!(
        "exchanges.clear_link",
        &path,
        { "transaction_id": transaction_id }
    )
}
