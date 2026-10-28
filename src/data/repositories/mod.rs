//! SQL access grouped by domain entity.
//!
//! These modules are the only place SQL appears in the application
//! (Section 6.3 of CONTRIBUTING.md). Functions return domain types and take
//! the shared [`Database`](crate::data::Database) handle; callers run them
//! away from the UI path.

pub mod addresses;
pub mod catalogs;
pub mod exchanges;
pub mod fiat_rates;
pub mod prices;
pub mod settings;
pub mod transactions;
