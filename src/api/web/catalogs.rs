//! Browser wrappers for the catalog endpoints.

use crate::domain::catalog::{
    CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview, CatalogEntry,
    CatalogImportPreview, CatalogImportSummary, CatalogType, CatalogTypeDraft, CatalogTypePage,
};
use crate::domain::table::TableQuery;
use dioxus_fullstack_core::ServerFnError;

pub async fn list_catalog_types_api(
    kind: String,
    query: TableQuery,
) -> Result<CatalogTypePage, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/types/list");
    crate::web_post_body!(
        "catalogs.list_types",
        &path,
        { "kind": kind, "query": query }
    )
}

pub async fn create_catalog_type_api(
    kind: String,
    draft: CatalogTypeDraft,
) -> Result<CatalogType, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/types");
    crate::web_post_body!(
        "catalogs.create_type",
        &path,
        { "kind": kind, "draft": draft }
    )
}

pub async fn update_catalog_type_api(
    kind: String,
    id: i64,
    draft: CatalogTypeDraft,
) -> Result<CatalogType, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/types/{id}");
    crate::web_put_body!(
        "catalogs.update_type",
        &path,
        { "kind": kind, "id": id, "draft": draft }
    )
}

pub async fn catalog_types_delete_preview_api(
    kind: String,
    ids: Vec<i64>,
) -> Result<CatalogDeletePreview, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/types/delete-preview");
    crate::web_post_body!(
        "catalogs.types_delete_preview",
        &path,
        { "kind": kind, "ids": ids }
    )
}

pub async fn delete_catalog_types_api(
    kind: String,
    ids: Vec<i64>,
    cascade: bool,
) -> Result<CatalogDeleteOutcome, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/types/delete");
    crate::web_post_body!(
        "catalogs.delete_types",
        &path,
        { "kind": kind, "ids": ids, "cascade": cascade }
    )
}

pub async fn list_catalog_categories_api(
    kind: String,
) -> Result<Vec<CatalogCategory>, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/categories");
    crate::web_get_query!("catalogs.list_categories", &path, { "kind": kind })
}

pub async fn create_catalog_category_api(
    kind: String,
    name: String,
) -> Result<CatalogCategory, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/categories");
    crate::web_post_body!(
        "catalogs.create_category",
        &path,
        { "kind": kind, "name": name }
    )
}

pub async fn rename_catalog_category_api(
    kind: String,
    id: i64,
    name: String,
) -> Result<CatalogCategory, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/categories/{id}");
    crate::web_put_body!(
        "catalogs.rename_category",
        &path,
        { "kind": kind, "id": id, "name": name }
    )
}

pub async fn catalog_category_delete_preview_api(
    kind: String,
    id: i64,
) -> Result<CatalogDeletePreview, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/categories/delete-preview");
    crate::web_post_body!(
        "catalogs.category_delete_preview",
        &path,
        { "kind": kind, "id": id }
    )
}

pub async fn delete_catalog_category_api(
    kind: String,
    id: i64,
    cascade: bool,
) -> Result<CatalogDeleteOutcome, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/categories/delete");
    crate::web_post_body!(
        "catalogs.delete_category",
        &path,
        { "kind": kind, "id": id, "cascade": cascade }
    )
}

pub async fn catalog_import_preview_api(
    kind: String,
    csv_text: String,
) -> Result<CatalogImportPreview, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/import/preview");
    crate::web_post_body!(
        "catalogs.import_preview",
        &path,
        { "kind": kind, "csv_text": csv_text }
    )
}

pub async fn catalog_import_api(
    kind: String,
    csv_text: String,
) -> Result<CatalogImportSummary, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/import");
    crate::web_post_body!(
        "catalogs.import",
        &path,
        { "kind": kind, "csv_text": csv_text }
    )
}

pub async fn catalog_options_api(kind: String) -> Result<Vec<CatalogEntry>, ServerFnError> {
    let path = format!("/api/settings/catalogs/{kind}/options");
    crate::web_get_query!("catalogs.options", &path, { "kind": kind })
}
