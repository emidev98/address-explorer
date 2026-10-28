//! Catalog service: the exchange and wallet type catalogs referenced by
//! stored addresses.

use crate::data::repositories::catalogs as catalog_repository;
use crate::data::{Database, DatabaseError};
use crate::domain::catalog::CatalogEntry;

/// Every exchange type ordered by stable id.
pub fn list_exchange_types(database: &Database) -> Result<Vec<CatalogEntry>, DatabaseError> {
    catalog_repository::list_exchange_types(database)
}

/// Every wallet type ordered by stable id.
pub fn list_wallet_types(database: &Database) -> Result<Vec<CatalogEntry>, DatabaseError> {
    catalog_repository::list_wallet_types(database)
}
