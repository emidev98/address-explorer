//! Browser wrappers for the address endpoints.

use crate::domain::address::{
    AddressOverview, AddressOverviewQuery, AddressSaveOutcome, StoredAddress,
};
use dioxus_fullstack_core::ServerFnError;

pub async fn addresses_overview_api(
    query: AddressOverviewQuery,
) -> Result<AddressOverview, ServerFnError> {
    crate::web_post_body!("addresses.overview", "/api/addresses/overview", { "query": query })
}

pub async fn validate_address_api(address: String) -> Result<String, ServerFnError> {
    crate::web_post_body!("addresses.validate", "/api/addresses/validate", { "address": address })
}

pub async fn save_address_api(
    address: String,
    description: Option<String>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<AddressSaveOutcome, ServerFnError> {
    crate::web_post_body!(
        "addresses.save",
        "/api/addresses",
        {
            "address": address,
            "description": description,
            "exchange_id": exchange_id,
            "wallet_id": wallet_id
        }
    )
}

pub async fn update_address_api(
    address: String,
    description: Option<String>,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<StoredAddress, ServerFnError> {
    let path = format!("/api/addresses/{address}");
    crate::web_put_body!(
        "addresses.update",
        &path,
        {
            "address": address,
            "description": description,
            "exchange_id": exchange_id,
            "wallet_id": wallet_id
        }
    )
}

pub async fn delete_addresses_api(addresses: Vec<String>) -> Result<usize, ServerFnError> {
    crate::web_post_body!("addresses.delete", "/api/addresses/delete", { "addresses": addresses })
}
