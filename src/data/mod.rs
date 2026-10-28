//! SQLite persistence: connection management, migrations, and repositories.
//!
//! This module is the only part of the application that owns a
//! `rusqlite::Connection`. SQL appears only in [`repositories`] (Section 6.3
//! of CONTRIBUTING.md); every other layer reaches the database through the
//! application services.

mod database;
pub(crate) mod migrations;
pub(crate) mod pricing_format;
pub mod repositories;
pub(crate) mod seeds;

pub use database::{Database, DatabaseError};
