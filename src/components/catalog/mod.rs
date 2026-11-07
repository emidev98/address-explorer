//! Dialogs of the provider catalog sub-settings: create/edit, category
//! management, the cascade-deletion impact, and the create-only CSV import.

mod categories;
mod delete;
mod import;
mod type_dialog;

pub use categories::CatalogCategoriesDialog;
pub use delete::{catalog_delete_summary_message, CatalogDeleteDialog, CatalogDeleteImpact};
pub use import::CatalogImportDialog;
pub use type_dialog::CatalogTypeDialog;

use super::icon::{Icon, IconName};
use super::Modal;

use crate::api::catalogs::{
    catalog_category_delete_preview_api, catalog_import_api, catalog_import_preview_api,
    create_catalog_category_api, create_catalog_type_api, delete_catalog_category_api,
    list_catalog_categories_api, rename_catalog_category_api, update_catalog_type_api,
};
use crate::components::provider_badge::derived_color;
use crate::components::{use_toast, BadgeSize, ConfirmDialog, CsvDropzone, ProviderBadge, Toast};
use crate::domain::catalog::csv::MAX_PREVIEW_ROWS;
use crate::domain::catalog::{
    normalize_brand_color, slugify, CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview,
    CatalogDeleteSummary, CatalogField, CatalogImportPreview, CatalogImportSummary, CatalogKind,
    CatalogType, CatalogTypeDraft, CatalogValidationError, CustodyModel, WalletPlatform,
};
