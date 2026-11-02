//! SQL access to the `exchanges_types` / `wallets_types` provider catalogs,
//! their category tables, and the cascade used when a provider or category is
//! deleted.
//!
//! Table and column names come from [`CatalogKind`] through a fixed match;
//! client input never reaches an SQL identifier.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, TransactionBehavior};

use crate::data::{Database, DatabaseError};
use crate::domain::catalog::{
    CatalogCategory, CatalogDeletePreview, CatalogDeleteSummary, CatalogEntry, CatalogKind,
    CatalogTotals, CatalogType, CustodyModel, NormalizedCatalogTypeDraft, WalletPlatform,
};
use crate::domain::table::{SortDirection, TablePage};

/// Sortable columns of a provider listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogTypeSortColumn {
    Name,
    Category,
    Custody,
    Platform,
    AddressCount,
}

impl CatalogTypeSortColumn {
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "name" => Some(Self::Name),
            "category" => Some(Self::Category),
            "custody" => Some(Self::Custody),
            "platform" => Some(Self::Platform),
            "addresses" | "address_count" => Some(Self::AddressCount),
            _ => None,
        }
    }

    fn order_expression(self, kind: CatalogKind) -> &'static str {
        match self {
            Self::Name => "t.name COLLATE NOCASE",
            Self::Category => "COALESCE(c.name, '') COLLATE NOCASE",
            Self::Custody => "COALESCE(t.custody_model, '')",
            Self::Platform => {
                if kind == CatalogKind::Wallet {
                    "COALESCE(t.platform, '')"
                } else {
                    "NULL"
                }
            }
            Self::AddressCount => "address_count",
        }
    }
}

/// Paging and sorting of one provider listing.
#[derive(Debug, Clone)]
pub struct CatalogListParams {
    pub sort: Option<(CatalogTypeSortColumn, SortDirection)>,
    pub limit: u32,
    pub offset: u64,
}

#[derive(Debug, Clone, Copy)]
struct CatalogTables {
    types: &'static str,
    categories: &'static str,
    owner_column: &'static str,
}

fn tables(kind: CatalogKind) -> CatalogTables {
    match kind {
        CatalogKind::Exchange => CatalogTables {
            types: "exchanges_types",
            categories: "exchange_categories",
            owner_column: "exchange_id",
        },
        CatalogKind::Wallet => CatalogTables {
            types: "wallets_types",
            categories: "wallet_categories",
            owner_column: "wallet_id",
        },
    }
}

/// Explicit columns of a provider row. Exchanges have no platform column, so
/// they select a typed NULL to keep one row shape for both kinds.
fn select_columns(kind: CatalogKind) -> &'static str {
    match kind {
        CatalogKind::Exchange => {
            "t.id, t.name, t.slug, t.category_id, c.name, t.custody_model,
             NULL AS platform, t.brand_color,
             (SELECT COUNT(*) FROM addresses a WHERE a.exchange_id = t.id) AS address_count"
        }
        CatalogKind::Wallet => {
            "t.id, t.name, t.slug, t.category_id, c.name, t.custody_model,
             t.platform, t.brand_color,
             (SELECT COUNT(*) FROM addresses a WHERE a.wallet_id = t.id) AS address_count"
        }
    }
}

fn type_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogType> {
    let custody: Option<String> = row.get(5)?;
    let platform: Option<String> = row.get(6)?;
    Ok(CatalogType {
        id: row.get(0)?,
        name: row.get(1)?,
        slug: row.get(2)?,
        category_id: row.get(3)?,
        category_name: row.get(4)?,
        custody_model: custody.and_then(|value| CustodyModel::parse(&value)),
        platform: platform.and_then(|value| WalletPlatform::parse(&value)),
        brand_color: row.get(7)?,
        address_count: row.get(8)?,
    })
}

/// Read one page of providers.
pub fn list_types(
    database: &Database,
    kind: CatalogKind,
    params: &CatalogListParams,
) -> Result<TablePage<CatalogType>, DatabaseError> {
    let tables = tables(kind);
    let (order, direction) = params
        .sort
        .map(|(column, direction)| (column.order_expression(kind), direction.as_sql()))
        .unwrap_or(("t.name COLLATE NOCASE", "ASC"));

    let sql = format!(
        "SELECT {columns}
         FROM {types} t
         LEFT JOIN {categories} c ON c.id = t.category_id
         ORDER BY {order} {direction}, t.id {direction}
         LIMIT ?1 OFFSET ?2",
        columns = select_columns(kind),
        types = tables.types,
        categories = tables.categories,
    );

    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![
            i64::from(params.limit),
            i64::try_from(params.offset).unwrap_or(i64::MAX)
        ],
        type_from_row,
    )?;
    let mut items = Vec::new();
    for row in rows {
        items.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    drop(statement);

    let total: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM {types}", types = tables.types),
        [],
        |row| row.get(0),
    )?;

    Ok(TablePage {
        items,
        total: total as u64,
    })
}

/// Global KPI totals of one provider collection.
pub fn totals(database: &Database, kind: CatalogKind) -> Result<CatalogTotals, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT (SELECT COUNT(*) FROM {types}),
                (SELECT COUNT(*) FROM {categories}),
                (SELECT COUNT(*) FROM addresses WHERE {owner} IS NOT NULL),
                (SELECT COUNT(*) FROM {types} WHERE category_id IS NULL)",
        types = tables.types,
        categories = tables.categories,
        owner = tables.owner_column,
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, [], |row| {
            Ok(CatalogTotals {
                providers: row.get(0)?,
                categories: row.get(1)?,
                classified_addresses: row.get(2)?,
                uncategorized: row.get(3)?,
            })
        })
        .map_err(Into::into)
}

/// All providers of a kind ordered for the pickers.
pub fn list_options(
    database: &Database,
    kind: CatalogKind,
) -> Result<Vec<CatalogEntry>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT t.id, t.name, t.slug, c.name, t.brand_color
         FROM {types} t
         LEFT JOIN {categories} c ON c.id = t.category_id
         ORDER BY t.name COLLATE NOCASE, t.id",
        types = tables.types,
        categories = tables.categories,
    );
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok(CatalogEntry {
            id: row.get(0)?,
            name: row.get(1)?,
            slug: row.get(2)?,
            category_name: row.get(3)?,
            brand_color: row.get(4)?,
        })
    })?;
    let mut entries = Vec::new();
    for entry in rows {
        entries.push(entry.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(entries)
}

/// Read one provider with its category and address count.
pub fn get_type(
    database: &Database,
    kind: CatalogKind,
    id: i64,
) -> Result<Option<CatalogType>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT {columns}
         FROM {types} t
         LEFT JOIN {categories} c ON c.id = t.category_id
         WHERE t.id = ?1",
        columns = select_columns(kind),
        types = tables.types,
        categories = tables.categories,
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, [id], type_from_row)
        .optional()
        .map_err(Into::into)
}

/// Find a provider id by name, case-insensitively. `exclude_id` lets an
/// update ignore the row being edited.
pub fn find_by_name(
    database: &Database,
    kind: CatalogKind,
    name: &str,
    exclude_id: Option<i64>,
) -> Result<Option<i64>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT t.id FROM {types} t
         WHERE t.name = ?1 COLLATE NOCASE AND (?2 IS NULL OR t.id <> ?2)
         LIMIT 1",
        types = tables.types
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, params![name, exclude_id], |row| row.get(0))
        .optional()
        .map_err(Into::into)
}

/// Whether a provider id exists in the kind's table.
pub fn type_exists(database: &Database, kind: CatalogKind, id: i64) -> Result<bool, DatabaseError> {
    let tables = tables(kind);
    let connection = database.connection()?;
    connection
        .query_row(
            &format!("SELECT 1 FROM {types} WHERE id = ?1", types = tables.types),
            [id],
            |_| Ok(()),
        )
        .optional()
        .map(|found| found.is_some())
        .map_err(Into::into)
}

/// Compatibility helper used by the address service.
pub fn exchange_type_exists(database: &Database, id: i64) -> Result<bool, DatabaseError> {
    type_exists(database, CatalogKind::Exchange, id)
}

/// Compatibility helper used by the address service.
pub fn wallet_type_exists(database: &Database, id: i64) -> Result<bool, DatabaseError> {
    type_exists(database, CatalogKind::Wallet, id)
}

/// Find a provider id by slug.
pub fn find_by_slug(
    database: &Database,
    kind: CatalogKind,
    slug: &str,
    exclude_id: Option<i64>,
) -> Result<Option<i64>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT t.id FROM {types} t
         WHERE t.slug = ?1 AND (?2 IS NULL OR t.id <> ?2)
         LIMIT 1",
        types = tables.types
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, params![slug, exclude_id], |row| row.get(0))
        .optional()
        .map_err(Into::into)
}

fn insert_type_row(
    connection: &Connection,
    kind: CatalogKind,
    draft: &NormalizedCatalogTypeDraft,
) -> Result<i64, DatabaseError> {
    match kind {
        CatalogKind::Exchange => {
            connection.execute(
                "INSERT INTO exchanges_types
                    (name, slug, category_id, custody_model, brand_color)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    draft.name.as_str(),
                    draft.slug.as_str(),
                    draft.category_id,
                    draft.custody_model.map(CustodyModel::as_str),
                    draft.brand_color.as_deref(),
                ],
            )?;
        }
        CatalogKind::Wallet => {
            connection.execute(
                "INSERT INTO wallets_types
                    (name, slug, category_id, custody_model, platform, brand_color)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    draft.name.as_str(),
                    draft.slug.as_str(),
                    draft.category_id,
                    draft.custody_model.map(CustodyModel::as_str),
                    draft.platform.map(WalletPlatform::as_str),
                    draft.brand_color.as_deref(),
                ],
            )?;
        }
    }
    Ok(connection.last_insert_rowid())
}

/// Insert one provider and return its id.
pub fn insert_type(
    database: &Database,
    kind: CatalogKind,
    draft: &NormalizedCatalogTypeDraft,
) -> Result<i64, DatabaseError> {
    let connection = database.connection()?;
    insert_type_row(&connection, kind, draft)
}

/// Update one provider. Returns whether the row existed.
pub fn update_type(
    database: &Database,
    kind: CatalogKind,
    id: i64,
    draft: &NormalizedCatalogTypeDraft,
) -> Result<bool, DatabaseError> {
    let connection = database.connection()?;
    let updated = match kind {
        CatalogKind::Exchange => connection.execute(
            "UPDATE exchanges_types SET
                name = ?2, slug = ?3, category_id = ?4, custody_model = ?5,
                brand_color = ?6
             WHERE id = ?1",
            params![
                id,
                draft.name.as_str(),
                draft.slug.as_str(),
                draft.category_id,
                draft.custody_model.map(CustodyModel::as_str),
                draft.brand_color.as_deref(),
            ],
        )?,
        CatalogKind::Wallet => connection.execute(
            "UPDATE wallets_types SET
                name = ?2, slug = ?3, category_id = ?4, custody_model = ?5,
                platform = ?6, brand_color = ?7
             WHERE id = ?1",
            params![
                id,
                draft.name.as_str(),
                draft.slug.as_str(),
                draft.category_id,
                draft.custody_model.map(CustodyModel::as_str),
                draft.platform.map(WalletPlatform::as_str),
                draft.brand_color.as_deref(),
            ],
        )?,
    };
    Ok(updated == 1)
}

/// Existing name/slug identities of one kind, used by the import planner.
pub fn list_identities(
    database: &Database,
    kind: CatalogKind,
) -> Result<Vec<(String, String)>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!("SELECT name, slug FROM {types}", types = tables.types);
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let mut identities = Vec::new();
    for row in rows {
        identities.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(identities)
}

/// Categories of one kind ordered by name, with their provider counts.
pub fn list_categories(
    database: &Database,
    kind: CatalogKind,
) -> Result<Vec<CatalogCategory>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT c.id, c.name,
                (SELECT COUNT(*) FROM {types} t WHERE t.category_id = c.id)
         FROM {categories} c
         ORDER BY c.name COLLATE NOCASE, c.id",
        types = tables.types,
        categories = tables.categories,
    );
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], category_from_row)?;
    let mut categories = Vec::new();
    for category in rows {
        categories.push(category.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(categories)
}

fn category_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CatalogCategory> {
    Ok(CatalogCategory {
        id: row.get(0)?,
        name: row.get(1)?,
        provider_count: row.get(2)?,
    })
}

/// Read one category with its provider count.
pub fn get_category(
    database: &Database,
    kind: CatalogKind,
    id: i64,
) -> Result<Option<CatalogCategory>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT c.id, c.name,
                (SELECT COUNT(*) FROM {types} t WHERE t.category_id = c.id)
         FROM {categories} c
         WHERE c.id = ?1",
        types = tables.types,
        categories = tables.categories,
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, [id], category_from_row)
        .optional()
        .map_err(Into::into)
}

/// Find a category id by name, case-insensitively.
pub fn find_category_by_name(
    database: &Database,
    kind: CatalogKind,
    name: &str,
    exclude_id: Option<i64>,
) -> Result<Option<i64>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT c.id FROM {categories} c
         WHERE c.name = ?1 COLLATE NOCASE AND (?2 IS NULL OR c.id <> ?2)
         LIMIT 1",
        categories = tables.categories
    );
    let connection = database.connection()?;
    connection
        .query_row(&sql, params![name, exclude_id], |row| row.get(0))
        .optional()
        .map_err(Into::into)
}

/// Insert a category and return its id.
pub fn insert_category(
    database: &Database,
    kind: CatalogKind,
    name: &str,
) -> Result<i64, DatabaseError> {
    let tables = tables(kind);
    let connection = database.connection()?;
    connection.execute(
        &format!(
            "INSERT INTO {categories} (name) VALUES (?1)",
            categories = tables.categories
        ),
        [name],
    )?;
    Ok(connection.last_insert_rowid())
}

/// Rename a category. Returns whether the row existed.
pub fn rename_category(
    database: &Database,
    kind: CatalogKind,
    id: i64,
    name: &str,
) -> Result<bool, DatabaseError> {
    let tables = tables(kind);
    let connection = database.connection()?;
    let updated = connection.execute(
        &format!(
            "UPDATE {categories} SET name = ?2 WHERE id = ?1",
            categories = tables.categories
        ),
        params![id, name],
    )?;
    Ok(updated == 1)
}

fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}

fn provider_ids_for_category(
    connection: &Connection,
    kind: CatalogKind,
    category_id: i64,
) -> Result<Vec<i64>, DatabaseError> {
    let tables = tables(kind);
    let sql = format!(
        "SELECT id FROM {types} WHERE category_id = ?1",
        types = tables.types
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([category_id], |row| row.get(0))?;
    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|error| DatabaseError::new(error.to_string()))?);
    }
    Ok(ids)
}

fn delete_preview_with_connection(
    connection: &Connection,
    kind: CatalogKind,
    ids: &[i64],
) -> Result<CatalogDeletePreview, DatabaseError> {
    if ids.is_empty() {
        return Ok(CatalogDeletePreview::default());
    }
    let tables = tables(kind);
    let placeholders = placeholders(ids.len());

    let provider_count: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM {types} WHERE id IN ({placeholders})",
            types = tables.types
        ),
        params_from_iter(ids.iter()),
        |row| row.get(0),
    )?;
    let address_count: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM addresses WHERE {owner} IN ({placeholders})",
            owner = tables.owner_column
        ),
        params_from_iter(ids.iter()),
        |row| row.get(0),
    )?;
    let transaction_count: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM transactions
             WHERE address IN (
                 SELECT address FROM addresses WHERE {owner} IN ({placeholders})
             )",
            owner = tables.owner_column
        ),
        params_from_iter(ids.iter()),
        |row| row.get(0),
    )?;

    let (exchange_account_count, ledger_transaction_count) = match kind {
        CatalogKind::Wallet => (0, 0),
        CatalogKind::Exchange => {
            let accounts: i64 = connection.query_row(
                &format!(
                    "SELECT COUNT(*) FROM exchange_accounts
                     WHERE provider_id IN ({placeholders})"
                ),
                params_from_iter(ids.iter()),
                |row| row.get(0),
            )?;
            let ledger: i64 = connection.query_row(
                &format!(
                    "SELECT COUNT(*) FROM exchange_transactions
                     WHERE exchange_id IN (
                         SELECT id FROM exchange_accounts
                         WHERE provider_id IN ({placeholders})
                     )"
                ),
                params_from_iter(ids.iter()),
                |row| row.get(0),
            )?;
            (accounts, ledger)
        }
    };

    Ok(CatalogDeletePreview {
        provider_count: provider_count as u64,
        category_count: 0,
        address_count: address_count as u64,
        transaction_count: transaction_count as u64,
        exchange_account_count: exchange_account_count as u64,
        ledger_transaction_count: ledger_transaction_count as u64,
    })
}

/// Impact of deleting the given providers.
pub fn delete_preview(
    database: &Database,
    kind: CatalogKind,
    ids: &[i64],
) -> Result<CatalogDeletePreview, DatabaseError> {
    let connection = database.connection()?;
    delete_preview_with_connection(&connection, kind, ids)
}

fn delete_types_in_transaction(
    transaction: &rusqlite::Transaction<'_>,
    kind: CatalogKind,
    ids: &[i64],
) -> Result<CatalogDeleteSummary, DatabaseError> {
    if ids.is_empty() {
        return Ok(CatalogDeleteSummary::default());
    }
    let tables = tables(kind);
    let placeholders = placeholders(ids.len());
    let preview = delete_preview_with_connection(transaction, kind, ids)?;

    transaction.execute(
        &format!(
            "DELETE FROM addresses WHERE {owner} IN ({placeholders})",
            owner = tables.owner_column
        ),
        params_from_iter(ids.iter()),
    )?;
    if kind == CatalogKind::Exchange {
        transaction.execute(
            &format!("DELETE FROM exchange_accounts WHERE provider_id IN ({placeholders})"),
            params_from_iter(ids.iter()),
        )?;
    }
    let providers = transaction.execute(
        &format!(
            "DELETE FROM {types} WHERE id IN ({placeholders})",
            types = tables.types
        ),
        params_from_iter(ids.iter()),
    )?;

    Ok(CatalogDeleteSummary {
        provider_count: providers as u64,
        category_count: 0,
        address_count: preview.address_count,
        transaction_count: preview.transaction_count,
        exchange_account_count: preview.exchange_account_count,
        ledger_transaction_count: preview.ledger_transaction_count,
    })
}

/// Delete providers and every row that depends on them in one transaction.
/// Addresses cascade to their stored transactions; exchange accounts cascade
/// to their ledger rows.
pub fn delete_types(
    database: &Database,
    kind: CatalogKind,
    ids: &[i64],
) -> Result<CatalogDeleteSummary, DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let summary = delete_types_in_transaction(&transaction, kind, ids)?;
    transaction.commit()?;
    Ok(summary)
}

/// Impact of deleting one category: its providers and their related data.
pub fn delete_category_preview(
    database: &Database,
    kind: CatalogKind,
    category_id: i64,
) -> Result<CatalogDeletePreview, DatabaseError> {
    let connection = database.connection()?;
    let tables = tables(kind);
    let exists: Option<i64> = connection
        .query_row(
            &format!(
                "SELECT id FROM {categories} WHERE id = ?1",
                categories = tables.categories
            ),
            [category_id],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(CatalogDeletePreview::default());
    }

    let provider_ids = provider_ids_for_category(&connection, kind, category_id)?;
    let mut preview = delete_preview_with_connection(&connection, kind, &provider_ids)?;
    preview.category_count = 1;
    Ok(preview)
}

/// Delete one category, cascading through its providers and their data.
pub fn delete_category(
    database: &Database,
    kind: CatalogKind,
    category_id: i64,
) -> Result<CatalogDeleteSummary, DatabaseError> {
    let mut connection = database.connection()?;
    let tables = tables(kind);
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let provider_ids = provider_ids_for_category(&transaction, kind, category_id)?;
    let mut summary = if provider_ids.is_empty() {
        CatalogDeleteSummary::default()
    } else {
        delete_types_in_transaction(&transaction, kind, &provider_ids)?
    };
    let categories = transaction.execute(
        &format!(
            "DELETE FROM {categories} WHERE id = ?1",
            categories = tables.categories
        ),
        [category_id],
    )?;
    transaction.commit()?;

    summary.category_count = categories as u64;
    Ok(summary)
}

/// Insert already validated import rows in one transaction.
pub fn insert_imported_types(
    database: &Database,
    kind: CatalogKind,
    drafts: &[NormalizedCatalogTypeDraft],
) -> Result<usize, DatabaseError> {
    if drafts.is_empty() {
        return Ok(0);
    }
    let mut connection = database.connection()?;
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for draft in drafts {
        insert_type_row(&transaction, kind, draft)?;
    }
    transaction.commit()?;
    Ok(drafts.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::repositories::{
        addresses as address_repository, exchanges as exchange_repository,
    };
    use crate::domain::address::AddressEntity;
    use crate::domain::catalog::CatalogTypeDraft;

    fn draft(name: &str, category_id: Option<i64>) -> NormalizedCatalogTypeDraft {
        CatalogTypeDraft {
            name: name.to_string(),
            category_id,
            ..CatalogTypeDraft::default()
        }
        .normalize(CatalogKind::Wallet)
        .unwrap()
    }

    fn exchange_draft(name: &str) -> NormalizedCatalogTypeDraft {
        CatalogTypeDraft {
            name: name.to_string(),
            category_id: Some(1),
            custody_model: Some(CustodyModel::Custodial),
            ..CatalogTypeDraft::default()
        }
        .normalize(CatalogKind::Exchange)
        .unwrap()
    }

    fn address(value: &str, exchange_id: Option<i64>, wallet_id: Option<i64>) -> AddressEntity {
        AddressEntity {
            address: value.to_string(),
            exchange_id,
            wallet_id,
            ..AddressEntity::default()
        }
    }

    fn list_params() -> CatalogListParams {
        CatalogListParams {
            sort: Some((CatalogTypeSortColumn::Name, SortDirection::Asc)),
            limit: 25,
            offset: 0,
        }
    }

    #[test]
    fn seeded_catalogs_keep_their_names_and_gain_categories() {
        let database = Database::open_in_memory().unwrap();
        let exchanges = list_categories(&database, CatalogKind::Exchange).unwrap();
        assert_eq!(exchanges.len(), 8);
        let wallets = list_categories(&database, CatalogKind::Wallet).unwrap();
        assert_eq!(wallets.len(), 9);

        let binance_id = find_by_name(&database, CatalogKind::Exchange, "Binance", None)
            .unwrap()
            .unwrap();
        let binance = get_type(&database, CatalogKind::Exchange, binance_id)
            .unwrap()
            .unwrap();
        assert_eq!(binance.name, "Binance");
        assert_eq!(binance.slug, "binance");
        assert_eq!(
            binance.category_name.as_deref(),
            Some("Centralized exchange")
        );
        assert_eq!(binance.custody_model, Some(CustodyModel::Custodial));

        let ledger_id = find_by_name(&database, CatalogKind::Wallet, "Ledger", None)
            .unwrap()
            .unwrap();
        let ledger = get_type(&database, CatalogKind::Wallet, ledger_id)
            .unwrap()
            .unwrap();
        assert_eq!(ledger.category_name.as_deref(), Some("Hardware wallet"));
        assert_eq!(ledger.platform, Some(WalletPlatform::Hardware));
        assert_eq!(ledger.custody_model, Some(CustodyModel::SelfCustody));
    }

    #[test]
    fn options_are_sorted_by_name_and_carry_their_category() {
        let database = Database::open_in_memory().unwrap();
        let options = list_options(&database, CatalogKind::Wallet).unwrap();
        assert_eq!(options.len(), 18);
        assert_eq!(options[0].name, "Base Wallet");
        assert_eq!(options[0].category_name.as_deref(), Some("Mobile wallet"));
    }

    #[test]
    fn totals_count_providers_categories_and_addresses() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qwallet", None, Some(1)))
            .unwrap();
        let totals = totals(&database, CatalogKind::Wallet).unwrap();
        assert_eq!(totals.providers, 18);
        assert_eq!(totals.categories, 9);
        assert_eq!(totals.classified_addresses, 1);
        assert_eq!(totals.uncategorized, 0);
    }

    #[test]
    fn sorting_and_pagination_work() {
        let database = Database::open_in_memory().unwrap();
        let params = CatalogListParams {
            sort: Some((CatalogTypeSortColumn::Name, SortDirection::Desc)),
            limit: 2,
            ..list_params()
        };
        let page = list_types(&database, CatalogKind::Wallet, &params).unwrap();
        assert_eq!(page.total, 18);
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].name, "Wasabi Wallet");
        assert_eq!(page.items[1].name, "Trust Wallet");

        let second = list_types(
            &database,
            CatalogKind::Wallet,
            &CatalogListParams {
                offset: 2,
                ..params
            },
        )
        .unwrap();
        assert_eq!(second.items.len(), 2);
        assert_eq!(second.items[0].name, "Trezor");
    }

    #[test]
    fn provider_crud_checks_duplicates_and_locks_nothing_at_this_layer() {
        let database = Database::open_in_memory().unwrap();
        let created = insert_type(&database, CatalogKind::Wallet, &draft("Acme", Some(2))).unwrap();
        let stored = get_type(&database, CatalogKind::Wallet, created)
            .unwrap()
            .unwrap();
        assert_eq!(stored.slug, "acme");
        assert_eq!(stored.category_id, Some(2));

        assert!(find_by_name(&database, CatalogKind::Wallet, "acme", None)
            .unwrap()
            .is_some());
        assert!(
            find_by_name(&database, CatalogKind::Wallet, "ACME", Some(created))
                .unwrap()
                .is_none()
        );
        assert!(find_by_slug(&database, CatalogKind::Wallet, "acme", None)
            .unwrap()
            .is_some());

        let mut updated = draft("Acme Two", Some(3));
        updated.custody_model = Some(CustodyModel::SelfCustody);
        assert!(update_type(&database, CatalogKind::Wallet, created, &updated).unwrap());
        let stored = get_type(&database, CatalogKind::Wallet, created)
            .unwrap()
            .unwrap();
        assert_eq!(stored.name, "Acme Two");
        assert_eq!(stored.category_name.as_deref(), Some("Mobile wallet"));

        assert!(!update_type(&database, CatalogKind::Wallet, 9999, &updated).unwrap());
        assert!(get_type(&database, CatalogKind::Wallet, 9999)
            .unwrap()
            .is_none());
    }

    #[test]
    fn category_crud_uses_case_insensitive_names() {
        let database = Database::open_in_memory().unwrap();
        let id = insert_category(&database, CatalogKind::Wallet, "Test category").unwrap();
        // The unique index rejects a differently-cased duplicate.
        assert!(insert_category(&database, CatalogKind::Wallet, "TEST CATEGORY").is_err());

        assert!(rename_category(&database, CatalogKind::Wallet, id, "Renamed category").unwrap());
        let category = get_category(&database, CatalogKind::Wallet, id)
            .unwrap()
            .unwrap();
        assert_eq!(category.name, "Renamed category");
        assert_eq!(category.provider_count, 0);
        assert!(
            find_category_by_name(&database, CatalogKind::Wallet, "renamed CATEGORY", None)
                .unwrap()
                .is_some()
        );
    }

    fn insert_transaction(connection: &Connection, address: &str, txid: &str) {
        connection
            .execute(
                "INSERT INTO transactions
                    (address, txid, version, locktime, size, weight, fee, confirmed)
                 VALUES (?1, ?2, 1, 0, 100, 400, 0, 1)",
                params![address, txid],
            )
            .unwrap();
    }

    fn insert_ledger_row(connection: &Connection, exchange_id: i64) {
        connection
            .execute(
                "INSERT INTO exchange_transactions
                    (exchange_id, fingerprint, timestamp, tx_type, amount_sats, currency,
                     raw_data, match_state, created_at, updated_at)
                 VALUES (?1, 'fp', 1, 'buy', 1, 'BTC', '{}', 'unmatched', 1, 1)",
                [exchange_id],
            )
            .unwrap();
    }

    #[test]
    fn delete_preview_counts_addresses_transactions_accounts_and_ledger_rows() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qbinance", Some(1), None))
            .unwrap();
        let account = exchange_repository::create_account(&database, "Binance", Some(1)).unwrap();
        let connection = database.connection().unwrap();
        insert_transaction(&connection, "bc1qbinance", "tx1");
        insert_ledger_row(&connection, account.id);
        drop(connection);

        let preview = delete_preview(&database, CatalogKind::Exchange, &[1]).unwrap();
        assert_eq!(preview.provider_count, 1);
        assert_eq!(preview.address_count, 1);
        assert_eq!(preview.transaction_count, 1);
        assert_eq!(preview.exchange_account_count, 1);
        assert_eq!(preview.ledger_transaction_count, 1);
        assert!(preview.has_related_rows());
    }

    #[test]
    fn deleting_an_exchange_cascades_its_addresses_accounts_and_ledger() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qbinance", Some(1), None))
            .unwrap();
        address_repository::insert_address(&database, &address("bc1qcoinbase", Some(2), None))
            .unwrap();
        let account = exchange_repository::create_account(&database, "Binance", Some(1)).unwrap();
        let connection = database.connection().unwrap();
        insert_transaction(&connection, "bc1qbinance", "tx1");
        insert_transaction(&connection, "bc1qcoinbase", "tx2");
        insert_ledger_row(&connection, account.id);
        drop(connection);

        let summary = delete_types(&database, CatalogKind::Exchange, &[1]).unwrap();
        assert_eq!(summary.provider_count, 1);
        assert_eq!(summary.address_count, 1);
        assert_eq!(summary.transaction_count, 1);
        assert_eq!(summary.exchange_account_count, 1);
        assert_eq!(summary.ledger_transaction_count, 1);

        assert!(get_type(&database, CatalogKind::Exchange, 1)
            .unwrap()
            .is_none());
        assert!(get_type(&database, CatalogKind::Exchange, 2)
            .unwrap()
            .is_some());
        let connection = database.connection().unwrap();
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM addresses WHERE address = 'bc1qcoinbase'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        let remaining_tx: i64 = connection
            .query_row("SELECT COUNT(*) FROM transactions", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining_tx, 1);
        let remaining_accounts: i64 = connection
            .query_row("SELECT COUNT(*) FROM exchange_accounts", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(remaining_accounts, 0);
    }

    #[test]
    fn deleting_a_wallet_cascades_addresses_and_transactions() {
        let database = Database::open_in_memory().unwrap();
        address_repository::insert_address(&database, &address("bc1qledger", None, Some(1)))
            .unwrap();
        let connection = database.connection().unwrap();
        insert_transaction(&connection, "bc1qledger", "tx1");
        drop(connection);

        let summary = delete_types(&database, CatalogKind::Wallet, &[1]).unwrap();
        assert_eq!(summary.provider_count, 1);
        assert_eq!(summary.address_count, 1);
        assert_eq!(summary.transaction_count, 1);
        assert_eq!(summary.exchange_account_count, 0);

        assert!(get_type(&database, CatalogKind::Wallet, 1)
            .unwrap()
            .is_none());
        let connection = database.connection().unwrap();
        let addresses: i64 = connection
            .query_row("SELECT COUNT(*) FROM addresses", [], |row| row.get(0))
            .unwrap();
        assert_eq!(addresses, 0);
    }

    #[test]
    fn deleting_a_category_cascades_through_its_providers() {
        let database = Database::open_in_memory().unwrap();
        let category = insert_category(&database, CatalogKind::Wallet, "Test category").unwrap();
        let provider = insert_type(
            &database,
            CatalogKind::Wallet,
            &draft("Test wallet", Some(category)),
        )
        .unwrap();
        address_repository::insert_address(&database, &address("bc1qtest", None, Some(provider)))
            .unwrap();

        let preview = delete_category_preview(&database, CatalogKind::Wallet, category).unwrap();
        assert_eq!(preview.category_count, 1);
        assert_eq!(preview.provider_count, 1);
        assert_eq!(preview.address_count, 1);

        let summary = delete_category(&database, CatalogKind::Wallet, category).unwrap();
        assert_eq!(summary.category_count, 1);
        assert_eq!(summary.provider_count, 1);
        assert_eq!(summary.address_count, 1);
        assert!(get_type(&database, CatalogKind::Wallet, provider)
            .unwrap()
            .is_none());
        assert!(get_category(&database, CatalogKind::Wallet, category)
            .unwrap()
            .is_none());
    }

    #[test]
    fn import_inserts_valid_rows_in_one_transaction() {
        let database = Database::open_in_memory().unwrap();
        let drafts = [draft("Acme", Some(2)), draft("Beta", None)];
        assert_eq!(
            insert_imported_types(&database, CatalogKind::Wallet, &drafts).unwrap(),
            2
        );
        let acme_id = find_by_name(&database, CatalogKind::Wallet, "Acme", None)
            .unwrap()
            .unwrap();
        let acme = get_type(&database, CatalogKind::Wallet, acme_id)
            .unwrap()
            .unwrap();
        assert_eq!(acme.category_id, Some(2));
        assert_eq!(
            list_identities(&database, CatalogKind::Wallet)
                .unwrap()
                .len(),
            20
        );
    }

    #[test]
    fn exchange_rows_have_no_platform_column() {
        let database = Database::open_in_memory().unwrap();
        let created = insert_type(
            &database,
            CatalogKind::Exchange,
            &exchange_draft("Acme exchange"),
        )
        .unwrap();
        let stored = get_type(&database, CatalogKind::Exchange, created)
            .unwrap()
            .unwrap();
        assert_eq!(stored.platform, None);
        assert_eq!(
            stored.category_name.as_deref(),
            Some("Centralized exchange")
        );
    }
}
