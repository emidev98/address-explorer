//! Catalog service: CRUD, the category taxonomy, cascade deletion, and the
//! create-only CSV import for the exchange and wallet provider catalogs.

use std::collections::HashSet;

use crate::data::repositories::catalogs as catalog_repository;
use crate::data::repositories::catalogs::{CatalogListParams, CatalogTypeSortColumn};
use crate::data::{Database, DatabaseError};
use crate::domain::catalog::csv as catalog_csv;
use crate::domain::catalog::{
    CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview, CatalogEntry,
    CatalogImportPreview, CatalogImportSummary, CatalogKind, CatalogType, CatalogTypeDraft,
    CatalogTypePage, NormalizedCatalogTypeDraft, MAX_NAME_LENGTH,
};
use crate::domain::table::{SortDirection, TableQuery};

/// Largest page a listing may request.
const MAX_PAGE_SIZE: u32 = 100;
/// Largest number of providers one delete may address.
const MAX_DELETE_IDS: usize = 100;

/// Failures of the catalog service.
#[derive(Debug, thiserror::Error)]
pub enum CatalogServiceError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    NotFound(String),
    #[error("A {kind} named \"{name}\" already exists.")]
    DuplicateName { kind: &'static str, name: String },
    #[error("The slug \"{slug}\" is already used by another {kind}.")]
    DuplicateSlug { kind: &'static str, slug: String },
    #[error("catalog access failed: {0}")]
    Database(#[from] DatabaseError),
}

/// Use cases of the provider catalogs of both kinds.
pub struct CatalogService {
    database: Database,
}

impl CatalogService {
    pub fn connect(database: Database) -> Self {
        Self { database }
    }

    /// One page of providers plus the global KPI totals.
    pub fn list_types(
        &self,
        kind: CatalogKind,
        query: &TableQuery,
    ) -> Result<CatalogTypePage, CatalogServiceError> {
        let sort = parse_sort(query.sort_by.as_deref(), query.sort_dir, kind)?;
        let page_size = query.page_size.clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from(query.page) * u64::from(page_size);
        let page = catalog_repository::list_types(
            &self.database,
            kind,
            &CatalogListParams {
                sort,
                limit: page_size,
                offset,
            },
        )?;
        let totals = catalog_repository::totals(&self.database, kind)?;
        Ok(CatalogTypePage { page, totals })
    }

    pub fn get_type(&self, kind: CatalogKind, id: i64) -> Result<CatalogType, CatalogServiceError> {
        catalog_repository::get_type(&self.database, kind, id)?.ok_or_else(|| {
            CatalogServiceError::NotFound(format!("The {} was not found.", kind.singular_label()))
        })
    }

    pub fn create_type(
        &self,
        kind: CatalogKind,
        draft: CatalogTypeDraft,
    ) -> Result<CatalogType, CatalogServiceError> {
        let draft = normalize_draft(kind, draft)?;
        self.ensure_name_available(kind, &draft.name, None)?;
        self.ensure_slug_available(kind, &draft.slug, None)?;
        self.ensure_category(kind, draft.category_id)?;
        let id = catalog_repository::insert_type(&self.database, kind, &draft)?;
        self.get_type(kind, id)
    }

    pub fn update_type(
        &self,
        kind: CatalogKind,
        id: i64,
        mut draft: CatalogTypeDraft,
    ) -> Result<CatalogType, CatalogServiceError> {
        let existing = self.get_type(kind, id)?;
        if draft
            .slug
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            // Omitting the slug on update means "keep the current one".
            draft.slug = Some(existing.slug.clone());
        }
        let draft = normalize_draft(kind, draft)?;
        if draft.slug != existing.slug {
            return Err(CatalogServiceError::Validation(
                "The slug cannot change after the type is created.".to_string(),
            ));
        }
        self.ensure_name_available(kind, &draft.name, Some(id))?;
        self.ensure_category(kind, draft.category_id)?;
        if !catalog_repository::update_type(&self.database, kind, id, &draft)? {
            return Err(CatalogServiceError::NotFound(format!(
                "The {} was not found.",
                kind.singular_label()
            )));
        }
        self.get_type(kind, id)
    }

    /// Impact of deleting the given providers.
    pub fn delete_preview(
        &self,
        kind: CatalogKind,
        ids: &[i64],
    ) -> Result<CatalogDeletePreview, CatalogServiceError> {
        let ids = normalize_ids(ids)?;
        let preview = catalog_repository::delete_preview(&self.database, kind, &ids)?;
        if preview.provider_count == 0 {
            return Err(CatalogServiceError::NotFound(format!(
                "No {} was found to delete.",
                kind.plural_label()
            )));
        }
        Ok(preview)
    }

    /// Delete providers. Without `cascade`, rows with related addresses,
    /// transactions, or exchange accounts ask for confirmation first.
    pub fn delete_types(
        &self,
        kind: CatalogKind,
        ids: &[i64],
        cascade: bool,
    ) -> Result<CatalogDeleteOutcome, CatalogServiceError> {
        let ids = normalize_ids(ids)?;
        let preview = catalog_repository::delete_preview(&self.database, kind, &ids)?;
        if preview.provider_count == 0 {
            return Err(CatalogServiceError::NotFound(format!(
                "No {} was found to delete.",
                kind.plural_label()
            )));
        }
        if preview.has_related_rows() && !cascade {
            return Ok(CatalogDeleteOutcome::ConfirmationRequired(Box::new(
                preview,
            )));
        }
        let summary = catalog_repository::delete_types(&self.database, kind, &ids)?;
        Ok(CatalogDeleteOutcome::Deleted(summary))
    }

    pub fn list_categories(
        &self,
        kind: CatalogKind,
    ) -> Result<Vec<CatalogCategory>, CatalogServiceError> {
        Ok(catalog_repository::list_categories(&self.database, kind)?)
    }

    pub fn create_category(
        &self,
        kind: CatalogKind,
        name: &str,
    ) -> Result<CatalogCategory, CatalogServiceError> {
        let name = normalize_category_name(name)?;
        if catalog_repository::find_category_by_name(&self.database, kind, &name, None)?.is_some() {
            return Err(CatalogServiceError::Validation(format!(
                "A category named \"{name}\" already exists."
            )));
        }
        let id = catalog_repository::insert_category(&self.database, kind, &name)?;
        self.get_category(kind, id)
    }

    pub fn rename_category(
        &self,
        kind: CatalogKind,
        id: i64,
        name: &str,
    ) -> Result<CatalogCategory, CatalogServiceError> {
        let name = normalize_category_name(name)?;
        self.get_category(kind, id)?;
        if catalog_repository::find_category_by_name(&self.database, kind, &name, Some(id))?
            .is_some()
        {
            return Err(CatalogServiceError::Validation(format!(
                "A category named \"{name}\" already exists."
            )));
        }
        if !catalog_repository::rename_category(&self.database, kind, id, &name)? {
            return Err(CatalogServiceError::NotFound(
                "The category was not found.".to_string(),
            ));
        }
        self.get_category(kind, id)
    }

    /// Impact of deleting one category, including its providers and their
    /// related data.
    pub fn delete_category_preview(
        &self,
        kind: CatalogKind,
        id: i64,
    ) -> Result<CatalogDeletePreview, CatalogServiceError> {
        let preview = catalog_repository::delete_category_preview(&self.database, kind, id)?;
        if preview.category_count == 0 {
            return Err(CatalogServiceError::NotFound(
                "The category was not found.".to_string(),
            ));
        }
        Ok(preview)
    }

    /// Delete a category. If it still has providers, the deletion cascades
    /// through them and requires `cascade = true`.
    pub fn delete_category(
        &self,
        kind: CatalogKind,
        id: i64,
        cascade: bool,
    ) -> Result<CatalogDeleteOutcome, CatalogServiceError> {
        let preview = self.delete_category_preview(kind, id)?;
        if preview.provider_count > 0 && !cascade {
            return Ok(CatalogDeleteOutcome::ConfirmationRequired(Box::new(
                preview,
            )));
        }
        let summary = catalog_repository::delete_category(&self.database, kind, id)?;
        Ok(CatalogDeleteOutcome::Deleted(summary))
    }

    /// Every provider of one kind for the pickers across the app.
    pub fn options(&self, kind: CatalogKind) -> Result<Vec<CatalogEntry>, CatalogServiceError> {
        Ok(catalog_repository::list_options(&self.database, kind)?)
    }

    pub fn preview_import(
        &self,
        kind: CatalogKind,
        csv_text: &str,
    ) -> Result<CatalogImportPreview, CatalogServiceError> {
        Ok(self.plan_import(kind, csv_text)?.preview)
    }

    /// Create every valid row of a CSV file. Duplicate and invalid rows are
    /// skipped and reported in the summary.
    pub fn import_csv(
        &self,
        kind: CatalogKind,
        csv_text: &str,
    ) -> Result<CatalogImportSummary, CatalogServiceError> {
        let plan = self.plan_import(kind, csv_text)?;
        if let Some(error) = &plan.preview.fatal_error {
            return Err(CatalogServiceError::Validation(error.clone()));
        }
        let created =
            catalog_repository::insert_imported_types(&self.database, kind, &plan.drafts)?;
        Ok(CatalogImportSummary {
            created,
            skipped_duplicates: plan.preview.duplicate_rows,
            invalid_rows: plan.preview.invalid_rows,
            uncategorized_rows: plan.preview.uncategorized_rows,
        })
    }

    fn plan_import(
        &self,
        kind: CatalogKind,
        csv_text: &str,
    ) -> Result<catalog_csv::CatalogImportPlan, CatalogServiceError> {
        let identities = catalog_repository::list_identities(&self.database, kind)?;
        let mut existing_names = HashSet::new();
        let mut existing_slugs = HashSet::new();
        for (name, slug) in identities {
            existing_names.insert(name.to_lowercase());
            existing_slugs.insert(slug);
        }
        let categories = catalog_repository::list_categories(&self.database, kind)?;
        Ok(catalog_csv::plan_import(
            kind,
            csv_text,
            &existing_names,
            &existing_slugs,
            &categories,
        ))
    }

    fn get_category(
        &self,
        kind: CatalogKind,
        id: i64,
    ) -> Result<CatalogCategory, CatalogServiceError> {
        catalog_repository::get_category(&self.database, kind, id)?
            .ok_or_else(|| CatalogServiceError::NotFound("The category was not found.".to_string()))
    }

    fn ensure_name_available(
        &self,
        kind: CatalogKind,
        name: &str,
        exclude_id: Option<i64>,
    ) -> Result<(), CatalogServiceError> {
        if catalog_repository::find_by_name(&self.database, kind, name, exclude_id)?.is_some() {
            return Err(CatalogServiceError::DuplicateName {
                kind: kind.singular_label(),
                name: name.to_string(),
            });
        }
        Ok(())
    }

    fn ensure_slug_available(
        &self,
        kind: CatalogKind,
        slug: &str,
        exclude_id: Option<i64>,
    ) -> Result<(), CatalogServiceError> {
        if catalog_repository::find_by_slug(&self.database, kind, slug, exclude_id)?.is_some() {
            return Err(CatalogServiceError::DuplicateSlug {
                kind: kind.singular_label(),
                slug: slug.to_string(),
            });
        }
        Ok(())
    }

    fn ensure_category(
        &self,
        kind: CatalogKind,
        category_id: Option<i64>,
    ) -> Result<(), CatalogServiceError> {
        if let Some(category_id) = category_id {
            if catalog_repository::get_category(&self.database, kind, category_id)?.is_none() {
                return Err(CatalogServiceError::Validation(
                    "Choose an existing category for this kind.".to_string(),
                ));
            }
        }
        Ok(())
    }
}

fn normalize_draft(
    kind: CatalogKind,
    draft: CatalogTypeDraft,
) -> Result<NormalizedCatalogTypeDraft, CatalogServiceError> {
    draft.normalize(kind).map_err(|error| {
        CatalogServiceError::Validation(format!("{}: {}", error.field.label(), error.message))
    })
}

/// Deduplicate and bound a delete id list.
fn normalize_ids(ids: &[i64]) -> Result<Vec<i64>, CatalogServiceError> {
    let mut unique = ids.to_vec();
    unique.sort_unstable();
    unique.dedup();
    if unique.is_empty() {
        return Err(CatalogServiceError::Validation(
            "Select at least one type.".to_string(),
        ));
    }
    if unique.len() > MAX_DELETE_IDS {
        return Err(CatalogServiceError::Validation(format!(
            "Select at most {MAX_DELETE_IDS} types at once."
        )));
    }
    Ok(unique)
}

fn normalize_category_name(name: &str) -> Result<String, CatalogServiceError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CatalogServiceError::Validation(
            "Enter a category name.".to_string(),
        ));
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(CatalogServiceError::Validation(format!(
            "The category name is limited to {MAX_NAME_LENGTH} characters."
        )));
    }
    Ok(name.to_string())
}

fn parse_sort(
    key: Option<&str>,
    direction: SortDirection,
    kind: CatalogKind,
) -> Result<Option<(CatalogTypeSortColumn, SortDirection)>, CatalogServiceError> {
    let Some(key) = key else {
        return Ok(None);
    };
    let column = CatalogTypeSortColumn::from_key(key)
        .ok_or_else(|| CatalogServiceError::Validation(format!("Unknown sort column: {key}")))?;
    if kind == CatalogKind::Exchange && column == CatalogTypeSortColumn::Platform {
        return Err(CatalogServiceError::Validation(
            "Platform applies only to wallet types.".to_string(),
        ));
    }
    Ok(Some((column, direction)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::repositories::addresses as address_repository;
    use crate::domain::address::AddressEntity;
    use crate::domain::catalog::{CustodyModel, WalletPlatform};
    use crate::domain::table::TableQuery;

    fn query() -> TableQuery {
        TableQuery {
            page_size: 25,
            ..TableQuery::default()
        }
    }

    fn draft(name: &str, category_id: Option<i64>) -> CatalogTypeDraft {
        CatalogTypeDraft {
            name: name.to_string(),
            category_id,
            ..CatalogTypeDraft::default()
        }
    }

    #[test]
    fn create_and_update_validate_names_slugs_and_platforms() {
        let database = Database::open_in_memory().unwrap();
        let service = CatalogService::connect(database);

        let created = service
            .create_type(CatalogKind::Wallet, draft("Acme", Some(2)))
            .unwrap();
        assert_eq!(created.slug, "acme");
        assert_eq!(created.category_name.as_deref(), Some("Software wallet"));

        let duplicate = service
            .create_type(CatalogKind::Wallet, draft("acme", None))
            .unwrap_err();
        assert!(matches!(
            duplicate,
            CatalogServiceError::DuplicateName { .. }
        ));

        let duplicate_slug = service
            .create_type(
                CatalogKind::Wallet,
                CatalogTypeDraft {
                    name: "Another".to_string(),
                    slug: Some("acme".to_string()),
                    ..CatalogTypeDraft::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            duplicate_slug,
            CatalogServiceError::DuplicateSlug { .. }
        ));

        let updated = service
            .update_type(
                CatalogKind::Wallet,
                created.id,
                CatalogTypeDraft {
                    name: "Acme Renamed".to_string(),
                    slug: Some("acme".to_string()),
                    category_id: Some(3),
                    custody_model: Some(CustodyModel::SelfCustody),
                    platform: Some(WalletPlatform::Software),
                    brand_color: Some("#abc".to_string()),
                },
            )
            .unwrap();
        assert_eq!(updated.name, "Acme Renamed");
        assert_eq!(updated.slug, "acme");
        assert_eq!(updated.brand_color, Some("#AABBCC".to_string()));

        let locked = service
            .update_type(
                CatalogKind::Wallet,
                created.id,
                CatalogTypeDraft {
                    slug: Some("acme-nuevo".to_string()),
                    ..draft("Acme Renamed", None)
                },
            )
            .unwrap_err();
        assert!(matches!(locked, CatalogServiceError::Validation(_)));

        let platform = service
            .create_type(
                CatalogKind::Exchange,
                CatalogTypeDraft {
                    name: "Not a wallet".to_string(),
                    platform: Some(WalletPlatform::Software),
                    ..CatalogTypeDraft::default()
                },
            )
            .unwrap_err();
        assert!(matches!(platform, CatalogServiceError::Validation(_)));
    }

    #[test]
    fn listings_reject_platform_sorting_for_exchanges_and_clamp_pages() {
        let database = Database::open_in_memory().unwrap();
        let service = CatalogService::connect(database);

        let mut exchange_query = query();
        exchange_query.sort_by = Some("platform".to_string());
        assert!(matches!(
            service.list_types(CatalogKind::Exchange, &exchange_query),
            Err(CatalogServiceError::Validation(_))
        ));

        let mut wallet_query = query();
        wallet_query.page_size = 0;
        wallet_query.sort_by = Some("platform".to_string());
        let page = service
            .list_types(CatalogKind::Wallet, &wallet_query)
            .unwrap();
        assert_eq!(page.page.items.len(), 1);
        assert_eq!(page.totals.providers, 18);
    }

    #[test]
    fn deletion_requires_confirmation_only_when_related_rows_exist() {
        let database = Database::open_in_memory().unwrap();
        let service = CatalogService::connect(database.clone());
        let created = service
            .create_type(CatalogKind::Wallet, draft("Acme", Some(2)))
            .unwrap();

        let outcome = service
            .delete_types(CatalogKind::Wallet, &[created.id], false)
            .unwrap();
        assert!(matches!(outcome, CatalogDeleteOutcome::Deleted(_)));

        address_repository::insert_address(
            &database,
            &AddressEntity {
                address: "bc1qseeded".to_string(),
                exchange_id: Some(1),
                ..AddressEntity::default()
            },
        )
        .unwrap();
        let seeded = service
            .delete_types(CatalogKind::Exchange, &[1], false)
            .unwrap();
        assert!(matches!(
            seeded,
            CatalogDeleteOutcome::ConfirmationRequired(_)
        ));
    }

    #[test]
    fn category_crud_and_cascade_preview() {
        let database = Database::open_in_memory().unwrap();
        let service = CatalogService::connect(database);

        let category = service
            .create_category(CatalogKind::Wallet, "Test category")
            .unwrap();
        assert_eq!(category.provider_count, 0);
        assert!(service
            .create_category(CatalogKind::Wallet, "test CATEGORY")
            .is_err());

        let renamed = service
            .rename_category(CatalogKind::Wallet, category.id, "Renamed")
            .unwrap();
        assert_eq!(renamed.name, "Renamed");

        let preview = service
            .delete_category_preview(CatalogKind::Wallet, category.id)
            .unwrap();
        assert_eq!(preview.category_count, 1);
        assert_eq!(preview.provider_count, 0);
        let outcome = service
            .delete_category(CatalogKind::Wallet, category.id, false)
            .unwrap();
        assert!(matches!(outcome, CatalogDeleteOutcome::Deleted(_)));
    }

    #[test]
    fn csv_preview_and_import_agree() {
        let database = Database::open_in_memory().unwrap();
        let service = CatalogService::connect(database);
        let csv = "name,category,custody_model,platform,brand_color\n\
                   Acme,Unknown category,self-custody,hardware,#abc\n\
                   Beta,Hardware wallet,self-custody,hardware,\n\
                   Acme,Unknown category,,\n";

        let preview = service.preview_import(CatalogKind::Wallet, csv).unwrap();
        assert_eq!(preview.total_rows, 3);
        assert_eq!(preview.valid_rows, 2);
        assert_eq!(preview.duplicate_rows, 1);
        assert_eq!(preview.uncategorized_rows, 1);
        assert_eq!(preview.unknown_categories, ["Unknown category"]);

        let summary = service.import_csv(CatalogKind::Wallet, csv).unwrap();
        assert_eq!(summary.created, 2);
        assert_eq!(summary.skipped_duplicates, 1);
        assert_eq!(summary.invalid_rows, 0);
        assert_eq!(summary.uncategorized_rows, 1);

        let page = service
            .list_types(
                CatalogKind::Wallet,
                &TableQuery {
                    page_size: 100,
                    ..TableQuery::default()
                },
            )
            .unwrap();
        let acme = page
            .page
            .items
            .iter()
            .find(|item| item.name == "Acme")
            .unwrap();
        assert_eq!(acme.category_id, None);
    }
}
