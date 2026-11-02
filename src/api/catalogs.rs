//! Catalog endpoints: the exchange and wallet provider catalogs, their
//! categories, cascade deletion, and the create-only CSV import.
//!
//! The generic endpoints take the kind as a path segment (`exchange` or
//! `wallet`); the controller parses it before any service call. The legacy
//! `/api/exchanges/types` and `/api/wallets/types` picker endpoints remain as
//! thin wrappers over the same service.

use dioxus::prelude::*;

use crate::domain::catalog::{
    CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview, CatalogEntry,
    CatalogImportPreview, CatalogImportSummary, CatalogType, CatalogTypeDraft, CatalogTypePage,
};
use crate::domain::table::TableQuery;

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::catalogs::CatalogService;
#[cfg(feature = "server")]
use crate::domain::catalog::CatalogKind;

#[cfg(feature = "server")]
fn parse_kind(value: &str) -> Result<CatalogKind, ServerFnError> {
    CatalogKind::parse(value)
        .ok_or_else(|| ServerFnError::new(format!("Unknown catalog kind: {value}")))
}

#[server]
#[post("/api/settings/catalogs/:kind/types/list")]
pub async fn list_catalog_types_api(
    kind: String,
    query: TableQuery,
) -> Result<CatalogTypePage, ServerFnError> {
    CatalogService::connect(open_database()?)
        .list_types(parse_kind(&kind)?, &query)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/settings/catalogs/:kind/types/:id")]
pub async fn get_catalog_type_api(kind: String, id: i64) -> Result<CatalogType, ServerFnError> {
    CatalogService::connect(open_database()?)
        .get_type(parse_kind(&kind)?, id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/types")]
pub async fn create_catalog_type_api(
    kind: String,
    draft: CatalogTypeDraft,
) -> Result<CatalogType, ServerFnError> {
    CatalogService::connect(open_database()?)
        .create_type(parse_kind(&kind)?, draft)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[put("/api/settings/catalogs/:kind/types/:id")]
pub async fn update_catalog_type_api(
    kind: String,
    id: i64,
    draft: CatalogTypeDraft,
) -> Result<CatalogType, ServerFnError> {
    CatalogService::connect(open_database()?)
        .update_type(parse_kind(&kind)?, id, draft)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/types/delete-preview")]
pub async fn catalog_types_delete_preview_api(
    kind: String,
    ids: Vec<i64>,
) -> Result<CatalogDeletePreview, ServerFnError> {
    CatalogService::connect(open_database()?)
        .delete_preview(parse_kind(&kind)?, &ids)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/types/delete")]
pub async fn delete_catalog_types_api(
    kind: String,
    ids: Vec<i64>,
    cascade: bool,
) -> Result<CatalogDeleteOutcome, ServerFnError> {
    CatalogService::connect(open_database()?)
        .delete_types(parse_kind(&kind)?, &ids, cascade)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/settings/catalogs/:kind/categories")]
pub async fn list_catalog_categories_api(
    kind: String,
) -> Result<Vec<CatalogCategory>, ServerFnError> {
    CatalogService::connect(open_database()?)
        .list_categories(parse_kind(&kind)?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/categories")]
pub async fn create_catalog_category_api(
    kind: String,
    name: String,
) -> Result<CatalogCategory, ServerFnError> {
    CatalogService::connect(open_database()?)
        .create_category(parse_kind(&kind)?, &name)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[put("/api/settings/catalogs/:kind/categories/:id")]
pub async fn rename_catalog_category_api(
    kind: String,
    id: i64,
    name: String,
) -> Result<CatalogCategory, ServerFnError> {
    CatalogService::connect(open_database()?)
        .rename_category(parse_kind(&kind)?, id, &name)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/categories/delete-preview")]
pub async fn catalog_category_delete_preview_api(
    kind: String,
    id: i64,
) -> Result<CatalogDeletePreview, ServerFnError> {
    CatalogService::connect(open_database()?)
        .delete_category_preview(parse_kind(&kind)?, id)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/categories/delete")]
pub async fn delete_catalog_category_api(
    kind: String,
    id: i64,
    cascade: bool,
) -> Result<CatalogDeleteOutcome, ServerFnError> {
    CatalogService::connect(open_database()?)
        .delete_category(parse_kind(&kind)?, id, cascade)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/import/preview")]
pub async fn catalog_import_preview_api(
    kind: String,
    csv_text: String,
) -> Result<CatalogImportPreview, ServerFnError> {
    CatalogService::connect(open_database()?)
        .preview_import(parse_kind(&kind)?, &csv_text)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/catalogs/:kind/import")]
pub async fn catalog_import_api(
    kind: String,
    csv_text: String,
) -> Result<CatalogImportSummary, ServerFnError> {
    CatalogService::connect(open_database()?)
        .import_csv(parse_kind(&kind)?, &csv_text)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Every provider option of one kind for the pickers across the app.
#[server]
#[get("/api/settings/catalogs/:kind/options")]
pub async fn catalog_options_api(kind: String) -> Result<Vec<CatalogEntry>, ServerFnError> {
    CatalogService::connect(open_database()?)
        .options(parse_kind(&kind)?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/exchanges/types")]
pub async fn list_exchange_types_api() -> Result<Vec<CatalogEntry>, ServerFnError> {
    CatalogService::connect(open_database()?)
        .options(CatalogKind::Exchange)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/wallets/types")]
pub async fn list_wallet_types_api() -> Result<Vec<CatalogEntry>, ServerFnError> {
    CatalogService::connect(open_database()?)
        .options(CatalogKind::Wallet)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
