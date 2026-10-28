//! Shared backend-driven table query contract: pagination and sorting, used
//! by every data-table endpoint (Section 5.11 of CONTRIBUTING.md).
//!
//! A table endpoint takes one query struct that flattens [`TableQuery`] plus
//! its table-specific filters, and returns a [`TablePage`]. Page numbers are
//! 0-based here and 1-based in the UI.

use serde::{Deserialize, Serialize};

/// Default rows per page when a client does not say otherwise; services still
/// clamp to their own maximum.
pub const DEFAULT_PAGE_SIZE: u32 = 50;

/// Sort direction of a table query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    Asc,
    /// Default direction, matching `TableQuery::default`.
    #[default]
    Desc,
}

impl SortDirection {
    /// The other direction; clicking the active sort column toggles to this.
    pub fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// SQL keyword of the direction. This is a fixed allowlist value, never
    /// client input, so it is safe to place in an `ORDER BY` clause.
    #[cfg(feature = "server")]
    pub(crate) fn as_sql(self) -> &'static str {
        match self {
            Self::Asc => "ASC",
            Self::Desc => "DESC",
        }
    }
}

/// The part of a table request every listing shares: which page, sorted by
/// what. Table-specific filters (date ranges, owners, ...) are separate
/// typed fields of the endpoint's query struct.
///
/// `sort_by` is the client-facing key of a sortable column; each service
/// validates it against the repository's fixed allowlist before any SQL is
/// built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableQuery {
    /// 0-based page index.
    pub page: u32,
    /// Rows per page; services clamp it to their maximum.
    pub page_size: u32,
    /// Key of the sortable column, `None` for the table's default order.
    pub sort_by: Option<String>,
    /// Sort direction; ignored without `sort_by`.
    pub sort_dir: SortDirection,
}

impl Default for TableQuery {
    fn default() -> Self {
        Self {
            page: 0,
            page_size: DEFAULT_PAGE_SIZE,
            sort_by: None,
            sort_dir: SortDirection::Desc,
        }
    }
}

/// One page of a table listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TablePage<T> {
    pub items: Vec<T>,
    /// Total rows matching the query's filters, across all pages.
    pub total: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_query_defaults_to_the_first_unsorted_page() {
        let query = TableQuery::default();

        assert_eq!(query.page, 0);
        assert_eq!(query.page_size, DEFAULT_PAGE_SIZE);
        assert_eq!(query.sort_by, None);
        assert_eq!(query.sort_dir, SortDirection::Desc);
    }

    #[test]
    fn missing_fields_deserialize_to_the_defaults() {
        let query: TableQuery = serde_json::from_str("{}").unwrap();

        assert_eq!(query, TableQuery::default());
    }

    #[test]
    fn sort_direction_serializes_lowercase_and_toggles() {
        assert_eq!(
            serde_json::to_string(&SortDirection::Asc).unwrap(),
            "\"asc\""
        );
        assert_eq!(
            serde_json::to_string(&SortDirection::Desc).unwrap(),
            "\"desc\""
        );
        assert_eq!(SortDirection::Asc.toggled(), SortDirection::Desc);
        assert_eq!(SortDirection::Desc.toggled(), SortDirection::Asc);
    }
}
