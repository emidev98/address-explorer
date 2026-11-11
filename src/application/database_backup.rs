//! Database export service for the active SQLite database.
//!
//! Import and reset are handled by the encryption vault because they may need
//! to replace an encrypted file; this service owns the plaintext snapshot.

use crate::data::backup;
use crate::data::{Database, DatabaseError};

/// Export a consistent copy of the database as a SQLite file.
pub fn export_database(database: &Database) -> Result<Vec<u8>, DatabaseError> {
    backup::export_to_bytes(database)
}
