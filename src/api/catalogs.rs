//! Catalog endpoints.

use dioxus::prelude::*;

use crate::domain::catalog::CatalogEntry;

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::catalogs;

#[server]
#[get("/api/exchanges/types")]
pub async fn list_exchange_types_api() -> Result<Vec<CatalogEntry>, ServerFnError> {
    catalogs::list_exchange_types(&open_database()?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/wallets/types")]
pub async fn list_wallet_types_api() -> Result<Vec<CatalogEntry>, ServerFnError> {
    catalogs::list_wallet_types(&open_database()?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
