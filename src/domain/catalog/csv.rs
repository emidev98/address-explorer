#![cfg_attr(not(feature = "server"), allow(dead_code))]
//! Create-only CSV import planning for the provider catalogs.
//!
//! The planner is pure: the service supplies the existing identities and
//! categories, and the planner returns both the preview shown to the user and
//! the normalized drafts that would be inserted. The apply path re-runs the
//! same planner against fresh database state, so the preview and the import
//! cannot disagree about names, slugs, or categories.
use crate::domain::catalog::{
    normalize_brand_color, normalize_name, normalize_slug, slugify, CatalogCategory,
    CatalogImportPreview, CatalogImportRowPreview, CatalogImportRowStatus, CatalogKind,
    CatalogTypeDraft, CustodyModel, NormalizedCatalogTypeDraft, WalletPlatform,
};
use crate::domain::csv::parse_csv_text;
use std::collections::{BTreeSet, HashSet};
/// Largest catalog CSV accepted by the importer.
pub const MAX_IMPORT_BYTES: usize = 1024 * 1024;
/// Largest number of data rows accepted by the importer.
pub const MAX_IMPORT_ROWS: usize = 5_000;
/// Rows included in the preview table. The import still processes every
/// valid row.
pub const MAX_PREVIEW_ROWS: usize = 10;
/// Preview plus the normalized rows that a successful import would insert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogImportPlan {
    pub preview: CatalogImportPreview,
    pub drafts: Vec<NormalizedCatalogTypeDraft>,
}
impl CatalogImportPlan {
    fn fatal(message: impl Into<String>) -> Self {
        Self {
            preview: CatalogImportPreview {
                rows: Vec::new(),
                total_rows: 0,
                valid_rows: 0,
                duplicate_rows: 0,
                invalid_rows: 0,
                uncategorized_rows: 0,
                unknown_categories: Vec::new(),
                fatal_error: Some(message.into()),
                rows_truncated: false,
            },
            drafts: Vec::new(),
        }
    }
}
/// Build the import preview and the drafts to insert.
///
/// `existing_names` and `existing_slugs` must be normalized (lowercase names,
/// lowercase slugs). Unknown categories do not fail a row: the row imports as
/// Uncategorized and the category name is reported in `unknown_categories`.
pub fn plan_import(
    kind: CatalogKind,
    text: &str,
    existing_names: &HashSet<String>,
    existing_slugs: &HashSet<String>,
    categories: &[CatalogCategory],
) -> CatalogImportPlan {
    if text.len() > MAX_IMPORT_BYTES {
        return CatalogImportPlan::fatal("The CSV file is too large.");
    }
    let document = match parse_csv_text(text) {
        Ok(document) => document,
        Err(error) => return CatalogImportPlan::fatal(error),
    };
    if document.rows.len() > MAX_IMPORT_ROWS {
        return CatalogImportPlan::fatal(format!(
            "The CSV file contains more than {MAX_IMPORT_ROWS} rows."
        ));
    }
    let mut fields = Columns::default();
    for header in &document.headers {
        let Some(field) = canonical_header(header) else {
            continue;
        };
        if !fields.insert(field, header) {
            return CatalogImportPlan::fatal(format!(
                "The CSV file contains more than one `{field}` column."
            ));
        }
    }
    let Some(name_header) = fields.name.clone() else {
        return CatalogImportPlan::fatal("The CSV file is missing the required `name` column.");
    };
    let mut preview = CatalogImportPreview {
        rows: Vec::new(),
        total_rows: document.rows.len(),
        valid_rows: 0,
        duplicate_rows: 0,
        invalid_rows: 0,
        uncategorized_rows: 0,
        unknown_categories: Vec::new(),
        fatal_error: None,
        rows_truncated: false,
    };
    let mut drafts = Vec::new();
    let mut seen_names = HashSet::new();
    let mut seen_slugs = HashSet::new();
    let mut unknown_categories = BTreeSet::new();
    for (index, row) in document.rows.iter().enumerate() {
        let row_number = index + 1;
        let raw_name = value(row, &Some(name_header.clone()));
        let raw_slug = value(row, &fields.slug);
        let raw_category = value(row, &fields.category);
        let raw_custody = value(row, &fields.custody);
        let raw_platform = value(row, &fields.platform);
        let raw_color = value(row, &fields.brand_color);
        let name = match normalize_name(raw_name) {
            Ok(name) => name,
            Err(error) => {
                preview.invalid_rows += 1;
                preview.rows.push(CatalogImportRowPreview {
                    row: row_number,
                    name: non_empty(raw_name),
                    slug: non_empty(raw_slug),
                    category: non_empty(raw_category),
                    status: CatalogImportRowStatus::Invalid(error.message),
                });
                continue;
            }
        };
        let slug = match non_empty(raw_slug) {
            Some(raw) => match normalize_slug(&raw) {
                Ok(slug) => slug,
                Err(error) => {
                    preview.invalid_rows += 1;
                    preview.rows.push(CatalogImportRowPreview {
                        row: row_number,
                        name: Some(name),
                        slug: Some(raw),
                        category: non_empty(raw_category),
                        status: CatalogImportRowStatus::Invalid(error.message),
                    });
                    continue;
                }
            },
            None => {
                let derived = slugify(&name);
                if derived.is_empty() {
                    preview.invalid_rows += 1;
                    preview.rows.push(CatalogImportRowPreview {
                        row: row_number,
                        name: Some(name),
                        slug: None,
                        category: non_empty(raw_category),
                        status: CatalogImportRowStatus::Invalid(
                            "Enter a slug using letters, numbers, and hyphens.".to_string(),
                        ),
                    });
                    continue;
                }
                derived
            }
        };
        let custody_model = match non_empty(raw_custody) {
            Some(raw) => match CustodyModel::parse(&raw) {
                Some(model) => Some(model),
                None => {
                    preview.invalid_rows += 1;
                    preview.rows.push(CatalogImportRowPreview {
                        row: row_number,
                        name: Some(name),
                        slug: Some(slug),
                        category: non_empty(raw_category),
                        status: CatalogImportRowStatus::Invalid(format!(
                            "Unknown custody model `{raw}`."
                        )),
                    });
                    continue;
                }
            },
            None => None,
        };
        let platform = match non_empty(raw_platform) {
            Some(raw) => match WalletPlatform::parse(&raw) {
                Some(platform) if kind == CatalogKind::Wallet => Some(platform),
                Some(_) => {
                    preview.invalid_rows += 1;
                    preview.rows.push(CatalogImportRowPreview {
                        row: row_number,
                        name: Some(name),
                        slug: Some(slug),
                        category: non_empty(raw_category),
                        status: CatalogImportRowStatus::Invalid(
                            "Platform applies only to wallet types.".to_string(),
                        ),
                    });
                    continue;
                }
                None => {
                    preview.invalid_rows += 1;
                    preview.rows.push(CatalogImportRowPreview {
                        row: row_number,
                        name: Some(name),
                        slug: Some(slug),
                        category: non_empty(raw_category),
                        status: CatalogImportRowStatus::Invalid(format!(
                            "Unknown platform `{raw}`."
                        )),
                    });
                    continue;
                }
            },
            None => None,
        };
        let brand_color = match normalize_brand_color(non_empty(raw_color).as_deref()) {
            Ok(color) => color,
            Err(error) => {
                preview.invalid_rows += 1;
                preview.rows.push(CatalogImportRowPreview {
                    row: row_number,
                    name: Some(name),
                    slug: Some(slug),
                    category: non_empty(raw_category),
                    status: CatalogImportRowStatus::Invalid(error.message),
                });
                continue;
            }
        };
        let category_id = non_empty(raw_category).and_then(|raw| {
            categories
                .iter()
                .find(|category| category.name.eq_ignore_ascii_case(&raw))
                .map(|category| category.id)
        });
        if non_empty(raw_category).is_some() && category_id.is_none() {
            if let Some(raw) = non_empty(raw_category) {
                unknown_categories.insert(raw);
            }
        }
        let draft = CatalogTypeDraft {
            name: name.clone(),
            slug: Some(slug.clone()),
            category_id,
            custody_model,
            platform,
            brand_color,
        };
        let normalized = match draft.normalize(kind) {
            Ok(normalized) => normalized,
            Err(error) => {
                preview.invalid_rows += 1;
                preview.rows.push(CatalogImportRowPreview {
                    row: row_number,
                    name: Some(name),
                    slug: Some(slug),
                    category: non_empty(raw_category),
                    status: CatalogImportRowStatus::Invalid(error.message),
                });
                continue;
            }
        };
        let name_key = normalized.name.to_lowercase();
        if existing_names.contains(&name_key) || !seen_names.insert(name_key) {
            preview.duplicate_rows += 1;
            preview.rows.push(CatalogImportRowPreview {
                row: row_number,
                name: Some(name),
                slug: Some(slug),
                category: non_empty(raw_category),
                status: CatalogImportRowStatus::Duplicate(
                    "A type with this name already exists.".to_string(),
                ),
            });
            continue;
        }
        let slug_key = normalized.slug.clone();
        if existing_slugs.contains(&slug_key) || !seen_slugs.insert(slug_key) {
            preview.duplicate_rows += 1;
            preview.rows.push(CatalogImportRowPreview {
                row: row_number,
                name: Some(name),
                slug: Some(slug),
                category: non_empty(raw_category),
                status: CatalogImportRowStatus::Duplicate(
                    "A type with this slug already exists.".to_string(),
                ),
            });
            continue;
        }
        preview.valid_rows += 1;
        if category_id.is_none() {
            preview.uncategorized_rows += 1;
        }
        preview.rows.push(CatalogImportRowPreview {
            row: row_number,
            name: Some(name),
            slug: Some(slug),
            category: non_empty(raw_category),
            status: CatalogImportRowStatus::Create,
        });
        drafts.push(normalized);
    }
    preview.unknown_categories = unknown_categories.into_iter().collect();
    preview.rows_truncated = preview.rows.len() > MAX_PREVIEW_ROWS;
    preview.rows.truncate(MAX_PREVIEW_ROWS);
    CatalogImportPlan { preview, drafts }
}
/// Header columns of the catalog import file, in their original spelling.
#[derive(Debug, Default)]
struct Columns {
    name: Option<String>,
    slug: Option<String>,
    category: Option<String>,
    custody: Option<String>,
    platform: Option<String>,
    brand_color: Option<String>,
}
impl Columns {
    /// Record the header of one canonical field; returns `false` when the
    /// field was already provided by another column.
    fn insert(&mut self, field: &'static str, header: &str) -> bool {
        let slot = match field {
            "name" => &mut self.name,
            "slug" => &mut self.slug,
            "category" => &mut self.category,
            "custody" => &mut self.custody,
            "platform" => &mut self.platform,
            "brand_color" => &mut self.brand_color,
            _ => return true,
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(header.to_string());
        true
    }
}
fn canonical_header(header: &str) -> Option<&'static str> {
    match header
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "name" | "provider" | "provider_name" => Some("name"),
        "slug" => Some("slug"),
        "category" | "category_name" => Some("category"),
        "custody" | "custody_model" => Some("custody"),
        "platform" | "form_factor" => Some("platform"),
        "brand_color" | "color" => Some("brand_color"),
        _ => None,
    }
}
fn value<'a>(
    row: &'a std::collections::BTreeMap<String, String>,
    header: &Option<String>,
) -> &'a str {
    header
        .as_ref()
        .and_then(|header| row.get(header))
        .map(String::as_str)
        .unwrap_or("")
}
fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn categories() -> Vec<CatalogCategory> {
        vec![
            CatalogCategory {
                id: 1,
                name: "Centralized exchange".to_string(),
                provider_count: 0,
            },
            CatalogCategory {
                id: 2,
                name: "Hardware wallet".to_string(),
                provider_count: 0,
            },
        ]
    }
    fn plan(kind: CatalogKind, text: &str) -> CatalogImportPlan {
        plan_import(kind, text, &HashSet::new(), &HashSet::new(), &categories())
    }
    #[test]
    fn valid_rows_become_drafts_with_resolved_categories() {
        let text = "name,slug,category,custody_model,platform,brand_color\n\
                    \"Binance\",binance,Centralized exchange,custodial,,\n\
                    Ledger,,Hardware wallet,self-custody,hardware,#abc\n";
        let plan = plan(CatalogKind::Wallet, text);
        assert_eq!(plan.preview.total_rows, 2);
        assert_eq!(plan.preview.valid_rows, 2);
        assert_eq!(plan.preview.invalid_rows, 0);
        assert_eq!(plan.preview.uncategorized_rows, 0);
        assert_eq!(plan.drafts.len(), 2);
        assert_eq!(plan.drafts[0].category_id, Some(1));
        assert_eq!(plan.drafts[1].slug, "ledger");
        assert_eq!(plan.drafts[1].category_id, Some(2));
        assert_eq!(
            plan.drafts[1].custody_model,
            Some(CustodyModel::SelfCustody)
        );
        assert_eq!(plan.drafts[1].platform, Some(WalletPlatform::Hardware));
        assert_eq!(plan.drafts[1].brand_color, Some("#AABBCC".to_string()));
    }
    #[test]
    fn unknown_categories_fall_back_to_uncategorized() {
        let text = "name,category\nAcme,Unknown category\n";
        let plan = plan(CatalogKind::Exchange, text);
        assert_eq!(plan.preview.valid_rows, 1);
        assert_eq!(plan.preview.uncategorized_rows, 1);
        assert_eq!(plan.drafts[0].category_id, None);
        assert_eq!(plan.preview.unknown_categories, ["Unknown category"]);
    }
    #[test]
    fn duplicates_against_the_database_and_the_file_are_skipped() {
        let mut names = HashSet::new();
        names.insert("binance".to_string());
        let text = "name,slug\nBinance,binance\nAcme,acme\nAcme,acme-2\n";
        let plan = plan_import(
            CatalogKind::Exchange,
            text,
            &names,
            &HashSet::new(),
            &categories(),
        );
        assert_eq!(plan.preview.valid_rows, 1);
        assert_eq!(plan.preview.duplicate_rows, 2);
        assert_eq!(plan.drafts[0].slug, "acme");
    }
    #[test]
    fn invalid_values_are_reported_per_row() {
        let text = "name,platform,brand_color\n\
                    Ledger,phone,#xyz\n\
                    ,hardware,\n\
                    Trezor,,\n";
        let plan = plan(CatalogKind::Wallet, text);
        assert_eq!(plan.preview.total_rows, 3);
        assert_eq!(plan.preview.valid_rows, 1);
        assert_eq!(plan.preview.invalid_rows, 2);
        assert!(matches!(
            &plan.preview.rows[0].status,
            CatalogImportRowStatus::Invalid(message)
            if message.contains("platform")
        ),);
        assert!(matches!(
            &plan.preview.rows[1].status,
            CatalogImportRowStatus::Invalid(message)
            if message.contains("name")
        ),);
    }
    #[test]
    fn platform_is_rejected_for_exchange_rows() {
        let text = "name,platform\nBinance,software\n";
        let plan = plan(CatalogKind::Exchange, text);
        assert_eq!(plan.preview.invalid_rows, 1);
        assert!(matches!(
            &plan.preview.rows[0].status,
            CatalogImportRowStatus::Invalid(message)
            if message.contains("wallet")
        ),);
    }
    #[test]
    fn missing_name_column_is_fatal() {
        let plan = plan(CatalogKind::Exchange, "slug,category\nbinance,Broker\n");
        assert!(plan.preview.fatal_error.is_some());
        assert!(plan.preview.rows.is_empty());
        assert!(plan.drafts.is_empty());
    }
    #[test]
    fn duplicate_columns_are_fatal() {
        let plan = plan(CatalogKind::Exchange, "name,provider\nBinance,Binance\n");
        assert!(plan.preview.fatal_error.is_some());
    }
    #[test]
    fn the_preview_is_truncated_but_every_valid_row_is_planned() {
        let mut text = String::from("name\n");
        for index in 0..(MAX_PREVIEW_ROWS + 5) {
            text.push_str(&format!("Provider {index}\n"));
        }
        let plan = plan(CatalogKind::Wallet, &text);
        assert_eq!(plan.preview.valid_rows, MAX_PREVIEW_ROWS + 5);
        assert_eq!(plan.preview.rows.len(), MAX_PREVIEW_ROWS);
        assert!(plan.preview.rows_truncated);
        assert_eq!(plan.drafts.len(), MAX_PREVIEW_ROWS + 5);
    }
}
