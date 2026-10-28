//! Address endpoints.

use dioxus::prelude::*;

use crate::domain::address::{
    AddressEntity, AddressListItem, AddressOverview, AddressOverviewQuery, AddressSaveOutcome,
};

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::addresses::{self, AddressService, AddressServiceError};

#[server]
#[get("/api/addresses")]
pub async fn list_addresses_api() -> Result<Vec<AddressListItem>, ServerFnError> {
    AddressService::connect(open_database()?)
        .list_addresses()
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Addresses overview: the page rows, sorted and scope-filtered in the
/// backend, plus the global KPI totals of the addresses page.
#[server]
#[post("/api/addresses/overview")]
pub async fn addresses_overview_api(
    query: AddressOverviewQuery,
) -> Result<AddressOverview, ServerFnError> {
    AddressService::connect(open_database()?)
        .addresses_overview(&query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/addresses/:address")]
pub async fn get_address_api(address: String) -> Result<AddressEntity, ServerFnError> {
    AddressService::connect(open_database()?)
        .get_address(&address)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Validate and canonicalize an address without storing it.
///
/// The import dialog calls this before pasting a clipboard value so invalid
/// text never reaches the dialog input.
#[server]
#[post("/api/addresses/validate")]
pub async fn validate_address_api(address: String) -> Result<String, ServerFnError> {
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
/// This is the dialog's modify mode: only the editable details change; the
/// mempool statistics and the indexed transactions are untouched.
#[server]
#[put("/api/addresses/:address")]
pub async fn update_address_api(
    address: String,
    description: Option<String>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<AddressEntity, ServerFnError> {
    AddressService::connect(open_database()?)
        .update_address(&address, description.as_deref(), exchange_id, wallet_id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[delete("/api/addresses/:address")]
pub async fn delete_address_api(address: String) -> Result<(), ServerFnError> {
    AddressService::connect(open_database()?)
        .delete_address(&address)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
