#![cfg_attr(not(feature = "server"), allow(dead_code))]
//! Exchange and wallet provider catalogs and their two-level taxonomy.
//!
//! Each kind has its own provider table (`exchanges_types` /
//! `wallets_types`) and its own category table (`exchange_categories` /
//! `wallet_categories`). [`CatalogKind`] selects both; the repository maps it
//! to a fixed table allowlist, never to client-provided identifiers.
//!
//! The provider catalogs are seeded with stable explicit ids
//! (`exchanges_types_data.sql` / `wallets_types_data.sql`), so stored
//! addresses can reference well-known platforms by id instead of storing
//! free-text names. The provider tables already carry the category, custody,
//! platform, slug, and brand-color columns, and the category tables hold the
//! initial category vocabulary. The seeded provider rows are fully user-owned
//! once created: they can be renamed, recategorized, and deleted like
//! user-created rows.
pub mod csv;
use crate::domain::table::TablePage;
use serde::{Deserialize, Serialize};
/// Which provider catalog a request addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogKind {
    Exchange,
    Wallet,
}
impl CatalogKind {
    /// API/route segment of the kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Exchange => "exchange",
            Self::Wallet => "wallet",
        }
    }
    /// Parse the API/route segment. Unknown values return `None` so the
    /// controller can reject them before any service call.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "exchange" | "exchanges" => Some(Self::Exchange),
            "wallet" | "wallets" => Some(Self::Wallet),
            _ => None,
        }
    }
    /// Title of the sub-settings entry.
    pub fn label(self) -> &'static str {
        match self {
            Self::Exchange => "Exchange Types",
            Self::Wallet => "Wallet Types",
        }
    }
    pub fn singular_label(self) -> &'static str {
        match self {
            Self::Exchange => "exchange type",
            Self::Wallet => "wallet type",
        }
    }
    pub fn plural_label(self) -> &'static str {
        match self {
            Self::Exchange => "exchange types",
            Self::Wallet => "wallet types",
        }
    }
}
/// Who controls the private keys of a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustodyModel {
    SelfCustody,
    Custodial,
}
impl CustodyModel {
    pub const ALL: [Self; 2] = [Self::SelfCustody, Self::Custodial];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SelfCustody => "self_custody",
            Self::Custodial => "custodial",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::SelfCustody => "Self-custody",
            Self::Custodial => "Custodial",
        }
    }
    /// Accept the spellings used by the CSV importer and the stored values.
    pub fn parse(value: &str) -> Option<Self> {
        match value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_")
            .as_str()
        {
            "self_custody" | "selfcustody" | "non_custodial" => Some(Self::SelfCustody),
            "custodial" => Some(Self::Custodial),
            _ => None,
        }
    }
}
/// Wallet form factor. Exchanges have no platform column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletPlatform {
    Hardware,
    Software,
}
impl WalletPlatform {
    pub const ALL: [Self; 2] = [Self::Hardware, Self::Software];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hardware => "hardware",
            Self::Software => "software",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Hardware => "Hardware",
            Self::Software => "Software",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hardware" => Some(Self::Hardware),
            "software" => Some(Self::Software),
            _ => None,
        }
    }
}
/// One provider row with its category resolved and its address usage counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogType {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub custody_model: Option<CustodyModel>,
    /// Always `None` for exchange rows.
    pub platform: Option<WalletPlatform>,
    pub brand_color: Option<String>,
    /// Number of addresses classified under this provider.
    pub address_count: i64,
}
/// Editable fields of a provider. `slug = None` derives it from the name.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogTypeDraft {
    pub name: String,
    pub slug: Option<String>,
    pub category_id: Option<i64>,
    pub custody_model: Option<CustodyModel>,
    pub platform: Option<WalletPlatform>,
    pub brand_color: Option<String>,
}
/// A draft whose textual fields have been normalized and whose kind-specific
/// rules have been checked. Repositories only accept this type for writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCatalogTypeDraft {
    pub name: String,
    pub slug: String,
    pub category_id: Option<i64>,
    pub custody_model: Option<CustodyModel>,
    pub platform: Option<WalletPlatform>,
    pub brand_color: Option<String>,
}
/// One page of providers plus the global KPI totals of the collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CatalogTypePage {
    pub page: TablePage<CatalogType>,
    pub totals: CatalogTotals,
}
/// Global totals shown by the KPI cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CatalogTotals {
    pub providers: i64,
    pub categories: i64,
    /// Addresses classified under any provider of this kind.
    pub classified_addresses: i64,
    pub uncategorized: i64,
}
/// One category with the number of providers using it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogCategory {
    pub id: i64,
    pub name: String,
    pub provider_count: i64,
}
/// Provider option used by the pickers across the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub category_name: Option<String>,
    pub brand_color: Option<String>,
}
/// Impact of deleting one or more providers, or a category and its providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CatalogDeletePreview {
    pub provider_count: u64,
    pub category_count: u64,
    pub address_count: u64,
    pub transaction_count: u64,
    pub exchange_account_count: u64,
    pub ledger_transaction_count: u64,
}
impl CatalogDeletePreview {
    /// Related rows that would be removed together with the selected rows.
    /// The provider/category rows themselves are not "related" data.
    pub fn related_rows(self) -> u64 {
        self.address_count
            + self.transaction_count
            + self.exchange_account_count
            + self.ledger_transaction_count
    }
    pub fn has_related_rows(self) -> bool {
        self.related_rows() > 0
    }
}
/// Counts actually removed by a confirmed deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CatalogDeleteSummary {
    pub provider_count: u64,
    pub category_count: u64,
    pub address_count: u64,
    pub transaction_count: u64,
    pub exchange_account_count: u64,
    pub ledger_transaction_count: u64,
}
/// Deletion result: either the rows were removed, or the caller must confirm
/// the cascade after seeing the impact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatalogDeleteOutcome {
    Deleted(CatalogDeleteSummary),
    ConfirmationRequired(Box<CatalogDeletePreview>),
}
/// One row of an import preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogImportRowPreview {
    /// 1-based data row (the header is not counted).
    pub row: usize,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub category: Option<String>,
    pub status: CatalogImportRowStatus,
}
/// Outcome of one CSV row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatalogImportRowStatus {
    Create,
    Duplicate(String),
    Invalid(String),
}
/// Validation preview of a create-only CSV import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogImportPreview {
    pub rows: Vec<CatalogImportRowPreview>,
    pub total_rows: usize,
    pub valid_rows: usize,
    pub duplicate_rows: usize,
    pub invalid_rows: usize,
    /// Valid rows that will be stored without a category.
    pub uncategorized_rows: usize,
    /// Category names that do not exist yet; those rows fall back to
    /// Uncategorized instead of failing.
    pub unknown_categories: Vec<String>,
    /// Set when the file cannot be processed at all (missing header, too
    /// large, malformed CSV). When set, `rows` is empty.
    pub fatal_error: Option<String>,
    /// The preview table is truncated while the import still processes every
    /// valid row.
    pub rows_truncated: bool,
}
/// Result of applying a create-only CSV import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CatalogImportSummary {
    pub created: usize,
    pub skipped_duplicates: usize,
    pub invalid_rows: usize,
    pub uncategorized_rows: usize,
}
/// Field of a provider form that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogField {
    Name,
    Slug,
    Category,
    Custody,
    Platform,
    BrandColor,
}
impl CatalogField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Slug => "Slug",
            Self::Category => "Category",
            Self::Custody => "Custody",
            Self::Platform => "Platform",
            Self::BrandColor => "Brand color",
        }
    }
}
/// Validation failure that can be shown next to its form field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogValidationError {
    pub field: CatalogField,
    pub message: String,
}
impl CatalogValidationError {
    fn new(field: CatalogField, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for CatalogValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
impl std::error::Error for CatalogValidationError {}
/// Longest accepted provider name.
pub const MAX_NAME_LENGTH: usize = 100;
/// Longest accepted slug.
pub const MAX_SLUG_LENGTH: usize = 64;
impl CatalogTypeDraft {
    /// Trim and validate the textual fields, derive a missing slug, and
    /// enforce the kind-specific rules. Category existence is checked by the
    /// service because it needs database access.
    pub fn normalize(
        self,
        kind: CatalogKind,
    ) -> Result<NormalizedCatalogTypeDraft, CatalogValidationError> {
        let name = normalize_name(&self.name)?;
        let slug = match self
            .slug
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(raw) => normalize_slug(raw)?,
            None => {
                let derived = slugify(&name);
                if derived.is_empty() {
                    return Err(CatalogValidationError::new(
                        CatalogField::Slug,
                        "Enter a slug using letters, numbers, and hyphens.",
                    ));
                }
                derived
            }
        };
        let brand_color = normalize_brand_color(self.brand_color.as_deref())?;
        if kind == CatalogKind::Exchange && self.platform.is_some() {
            return Err(CatalogValidationError::new(
                CatalogField::Platform,
                "Platform applies only to wallet types.",
            ));
        }
        Ok(NormalizedCatalogTypeDraft {
            name,
            slug,
            category_id: self.category_id,
            custody_model: self.custody_model,
            platform: self.platform,
            brand_color,
        })
    }
}
/// Trim a provider name and check its length.
pub fn normalize_name(name: &str) -> Result<String, CatalogValidationError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CatalogValidationError::new(
            CatalogField::Name,
            "Enter a name.",
        ));
    }
    if name.chars().count() > MAX_NAME_LENGTH {
        return Err(CatalogValidationError::new(
            CatalogField::Name,
            format!("The name is limited to {MAX_NAME_LENGTH} characters."),
        ));
    }
    Ok(name.to_string())
}
/// Build a slug from a provider name. Non-ASCII characters act as separators
/// because slugs are restricted to ASCII letters, numbers, and hyphens.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !slug.is_empty() && !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug.truncate(MAX_SLUG_LENGTH);
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}
/// Normalize and validate a user-provided slug.
pub fn normalize_slug(raw: &str) -> Result<String, CatalogValidationError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(CatalogValidationError::new(
            CatalogField::Slug,
            "Enter a slug using letters, numbers, and hyphens.",
        ));
    }
    let mut slug = String::new();
    let mut last_was_dash = false;
    for character in trimmed.chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if character == '-' || character == '_' || character.is_whitespace() {
            if !slug.is_empty() && !last_was_dash {
                slug.push('-');
                last_was_dash = true;
            }
        } else {
            return Err(CatalogValidationError::new(
                CatalogField::Slug,
                "Slugs may only contain letters, numbers, and hyphens.",
            ));
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        return Err(CatalogValidationError::new(
            CatalogField::Slug,
            "Enter a slug using letters, numbers, and hyphens.",
        ));
    }
    if slug.chars().count() > MAX_SLUG_LENGTH {
        return Err(CatalogValidationError::new(
            CatalogField::Slug,
            format!("The slug is limited to {MAX_SLUG_LENGTH} characters."),
        ));
    }
    Ok(slug)
}
/// Normalize an optional `#RGB` / `#RRGGBB` color to uppercase `#RRGGBB`.
/// Blank input means "no stored color"; the UI derives a fallback color.
pub fn normalize_brand_color(raw: Option<&str>) -> Result<Option<String>, CatalogValidationError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    let expanded = match hex.len() {
        3 => hex
            .chars()
            .flat_map(|character| [character, character])
            .collect(),
        6 => hex.to_string(),
        _ => {
            return Err(CatalogValidationError::new(
                CatalogField::BrandColor,
                "Enter a color like #6750A4.",
            ));
        }
    };
    if !expanded
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return Err(CatalogValidationError::new(
            CatalogField::BrandColor,
            "Enter a color like #6750A4.",
        ));
    }
    Ok(Some(format!("#{}", expanded.to_ascii_uppercase())))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kinds_parse_their_api_segments_case_insensitively() {
        assert_eq!(CatalogKind::parse("exchange"), Some(CatalogKind::Exchange));
        assert_eq!(CatalogKind::parse(" Wallets "), Some(CatalogKind::Wallet));
        assert_eq!(CatalogKind::parse("provider"), None);
        assert_eq!(CatalogKind::Exchange.as_str(), "exchange");
        assert_eq!(CatalogKind::Wallet.label(), "Wallet Types");
    }
    #[test]
    fn slugs_are_derived_from_brand_names() {
        assert_eq!(slugify("Crypto.com"), "crypto-com");
        assert_eq!(slugify("OKX Wallet"), "okx-wallet");
        assert_eq!(slugify("Blockchain.com Wallet"), "blockchain-com-wallet");
        assert_eq!(slugify("  Trust   Wallet  "), "trust-wallet");
        assert_eq!(slugify("Café"), "caf");
        assert_eq!(slugify("!!!"), "");
    }
    #[test]
    fn slug_normalization_collapses_separators_and_rejects_symbols() {
        assert_eq!(normalize_slug("  My_Wallet  ").unwrap(), "my-wallet");
        assert_eq!(normalize_slug("okx--wallet").unwrap(), "okx-wallet");
        assert!(normalize_slug("wallet$").is_err());
        assert!(normalize_slug("   ").is_err());
    }
    #[test]
    fn names_are_trimmed_and_capped() {
        assert_eq!(normalize_name("  Ledger ").unwrap(), "Ledger");
        assert!(normalize_name(" ").is_err());
        assert!(normalize_name(&"x".repeat(MAX_NAME_LENGTH + 1)).is_err());
    }
    #[test]
    fn colors_normalize_to_uppercase_six_digit_hex() {
        assert_eq!(
            normalize_brand_color(Some("#abc")).unwrap(),
            Some("#AABBCC".to_string()),
        );
        assert_eq!(
            normalize_brand_color(Some(" 6750a4 ")).unwrap(),
            Some("#6750A4".to_string()),
        );
        assert_eq!(normalize_brand_color(Some("  ")).unwrap(), None);
        assert!(normalize_brand_color(Some("#12")).is_err());
        assert!(normalize_brand_color(Some("#GGGGGG")).is_err());
    }
    #[test]
    fn drafts_derive_missing_slugs_and_reject_platform_on_exchanges() {
        let draft = CatalogTypeDraft {
            name: "  OKX Wallet ".to_string(),
            ..CatalogTypeDraft::default()
        };
        let normalized = draft.normalize(CatalogKind::Wallet).unwrap();
        assert_eq!(normalized.name, "OKX Wallet");
        assert_eq!(normalized.slug, "okx-wallet");
        let draft = CatalogTypeDraft {
            name: "Binance".to_string(),
            platform: Some(WalletPlatform::Software),
            ..CatalogTypeDraft::default()
        };
        let error = draft.normalize(CatalogKind::Exchange).unwrap_err();
        assert_eq!(error.field, CatalogField::Platform);
    }
    #[test]
    fn delete_preview_distinguishes_related_rows() {
        let empty = CatalogDeletePreview {
            provider_count: 1,
            ..CatalogDeletePreview::default()
        };
        assert!(!empty.has_related_rows());
        let with_addresses = CatalogDeletePreview {
            address_count: 3,
            transaction_count: 10,
            ..CatalogDeletePreview::default()
        };
        assert_eq!(with_addresses.related_rows(), 13);
        assert!(with_addresses.has_related_rows());
    }
}
