//! Address endpoints.
#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::addresses::{self, AddressService, AddressServiceError};
use crate::domain::address::{
    AddressOverview, AddressOverviewQuery, AddressSaveOutcome, StoredAddress,
};
use dioxus::prelude::*;
/// Addresses overview: every listed row sorted and scope-filtered in the
/// backend for the KPI cards and the holdings chart, the requested table
/// page, and the global KPI totals of the addresses page.
#[server]
#[post("/api/addresses/overview")]
pub async fn addresses_overview_api(
    query: AddressOverviewQuery,
) -> Result<AddressOverview, ServerFnError> {
    remote_post!("/api/addresses/overview", [], [], [query]);
    AddressService::connect(open_database()?)
        .addresses_overview(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Validate and canonicalize an address without storing it.
///
/// The import dialog calls this before pasting a clipboard value so invalid
/// text never reaches the dialog input.
#[server]
#[post("/api/addresses/validate")]
pub async fn validate_address_api(address: String) -> Result<String, ServerFnError> {
    remote_post!("/api/addresses/validate", [], [], [address]);
    addresses::validate_address(&address).map_err(|error| ServerFnError::new(error.to_string()))
}
/// Import an address and index its transaction history.
///
/// The import is create-only: an address that is already stored yields
/// [`AddressSaveOutcome::Duplicate`] and is left untouched. Indexing never
/// fails the import: its outcome (complete, paused on a mempool rate limit,
/// or failed) rides along in the result.
#[server]
#[post("/api/addresses")]
pub async fn save_address_api(
    address: String,
    description: Option<String>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<AddressSaveOutcome, ServerFnError> {
    remote_post!(
        "/api/addresses",
        [],
        [],
        [address, description, exchange_id, wallet_id]
    );
    let result = AddressService::connect(open_database()?)
        .import_address_and_sync(&address, description.as_deref(), exchange_id, wallet_id)
        .await;
    match result {
        Ok(imported) => Ok(AddressSaveOutcome::Imported(Box::new(imported))),
        Err(AddressServiceError::AlreadyExists) => Ok(AddressSaveOutcome::Duplicate),
        Err(error) => Err(ServerFnError::new(error.to_string())),
    }
}
/// Update the description and owner classification of a stored address.
///
/// This is the dialog's modify mode: the editable details change without
/// touching the mempool statistics. Changing the owner to an exchange
/// deletes the address's previously indexed on-chain transactions in the
/// same transaction; the provider-wide reverse search rebuilds only the
/// related ones.
#[server]
#[put("/api/addresses/:address")]
pub async fn update_address_api(
    address: String,
    description: Option<String>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<StoredAddress, ServerFnError> {
    remote_put!(
        "/api/addresses/{}",
        [address],
        [],
        [description, exchange_id, wallet_id]
    );
    AddressService::connect(open_database()?)
        .update_address(&address, description.as_deref(), exchange_id, wallet_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
/// Delete several stored addresses and their cascaded transactions in one
/// transaction. Returns the number of deleted rows.
#[server]
#[post("/api/addresses/delete")]
pub async fn delete_addresses_api(addresses: Vec<String>) -> Result<usize, ServerFnError> {
    remote_post!("/api/addresses/delete", [], [], [addresses]);
    AddressService::connect(open_database()?)
        .delete_addresses(&addresses)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
