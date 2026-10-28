//! API controllers: the `#[server]` entry points of the application.
//!
//! Each module groups the endpoints of one domain. Controllers are thin by
//! contract (Section 7.2 of CONTRIBUTING.md): they open the database, call
//! exactly one application service operation, and translate errors into
//! `ServerFnError`. They contain no business rules and no SQL.

#[cfg(feature = "server")]
use dioxus::prelude::ServerFnError;

#[cfg(feature = "server")]
use crate::data::Database;

pub mod addresses;
pub mod catalogs;
pub mod exchanges;
pub mod prices;
pub mod settings;
pub mod transactions;

/// Open the application database for one request.
#[cfg(feature = "server")]
pub(crate) fn open_database() -> Result<Database, ServerFnError> {
    Database::open_default().map_err(|error| ServerFnError::new(error.to_string()))
}
