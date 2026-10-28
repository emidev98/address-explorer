//! SQL access to the `exchanges_types` and `wallets_types` lookup tables.

use rusqlite::OptionalExtension;

use crate::data::{Database, DatabaseError};
use crate::domain::catalog::CatalogEntry;

/// Every exchange type ordered by stable id.
pub fn list_exchange_types(database: &Database) -> Result<Vec<CatalogEntry>, DatabaseError> {
    list_types(database, "exchanges_types")
}

/// Every wallet type ordered by stable id.
pub fn list_wallet_types(database: &Database) -> Result<Vec<CatalogEntry>, DatabaseError> {
    list_types(database, "wallets_types")
}

pub fn exchange_type_exists(database: &Database, id: i64) -> Result<bool, DatabaseError> {
    type_exists(database, "exchanges_types", id)
}

pub fn wallet_type_exists(database: &Database, id: i64) -> Result<bool, DatabaseError> {
    type_exists(database, "wallets_types", id)
}

fn list_types(
    database: &Database,
    table: &'static str,
) -> Result<Vec<CatalogEntry>, DatabaseError> {
    // The table name comes from a fixed allowlist of call sites, not from
    // user input, so it is safe to interpolate here.
    let sql = format!("SELECT id, name FROM {table} ORDER BY id");
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| {
        Ok(CatalogEntry {
            id: row.get(0)?,
            name: row.get(1)?,
        })
    })?;
    let mut entries = Vec::new();
    for entry in rows {
        entries.push(entry.map_err(|err| DatabaseError::new(err.to_string()))?);
    }
    Ok(entries)
}

fn type_exists(database: &Database, table: &'static str, id: i64) -> Result<bool, DatabaseError> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    let connection = database.connection()?;
    connection
        .query_row(&sql, [id], |_| Ok(()))
        .optional()
        .map(|found| found.is_some())
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::catalog::CatalogEntry;

    #[test]
    fn exchange_types_list_the_seeded_catalog() {
        let database = Database::open_in_memory().unwrap();
        let entries = list_exchange_types(&database).unwrap();
        assert_eq!(entries.len(), 16);
        assert_eq!(
            entries[0],
            CatalogEntry {
                id: 1,
                name: "Binance".to_string()
            }
        );
    }

    #[test]
    fn wallet_types_list_the_seeded_catalog() {
        let database = Database::open_in_memory().unwrap();
        let entries = list_wallet_types(&database).unwrap();
        assert_eq!(entries.len(), 18);
        assert_eq!(
            entries[0],
            CatalogEntry {
                id: 1,
                name: "Ledger".to_string(),
            }
        );
    }

    #[test]
    fn type_existence_checks_known_and_unknown_ids() {
        let database = Database::open_in_memory().unwrap();

        assert!(exchange_type_exists(&database, 1).unwrap());
        assert!(wallet_type_exists(&database, 18).unwrap());

        assert!(!exchange_type_exists(&database, 999).unwrap());
        assert!(!wallet_type_exists(&database, 999).unwrap());
    }
}
