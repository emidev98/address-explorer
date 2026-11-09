//! Database maintenance service: export, import, and reset the active
//! SQLite database.
//!
//! The transport layer decides which database is active; this service applies
//! the same validation and rollback rules wherever the request originated.

use crate::data::backup;
use crate::data::{Database, DatabaseError};

/// Export a consistent copy of the database as a SQLite file.
pub fn export_database(database: &Database) -> Result<Vec<u8>, DatabaseError> {
    backup::export_to_bytes(database)
}

/// Validate, upgrade, and replace the database with a backup file.
pub fn import_database(database: &Database, bytes: &[u8]) -> Result<(), DatabaseError> {
    backup::import_from_bytes(database, bytes)
}

/// Replace the database with a freshly migrated and seeded database.
pub fn reset_database(database: &Database) -> Result<(), DatabaseError> {
    backup::reset_to_defaults(database)
}
