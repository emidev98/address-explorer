//! SQLite persistence: connection management, migrations, backup/restore, and
//! repositories.
//!
//! This module is the only part of the application that owns a
//! `rusqlite::Connection`. SQL and SQLite maintenance appear in
//! [`repositories`], migrations, seeds, and the backup module; every other
//! layer reaches the database through the application services.
pub(crate) mod backup;
mod database;
pub(crate) mod encryption;
pub(crate) mod migrations;
pub(crate) mod pricing_format;
pub mod repositories;
pub(crate) mod seeds;
pub use database::{Database, DatabaseError};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod vault;
