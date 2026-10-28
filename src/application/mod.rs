//! Application services: the business logic of the application, one module
//! per domain.
//!
//! Services own the rules and workflows of their domain and orchestrate the
//! repositories (SQLite) and the external API clients to fulfill one use case
//! per public function. They MUST NOT contain SQL text, HTTP details, RSX, or
//! Dioxus signal code (Section 7.2 of CONTRIBUTING.md).

pub mod addresses;
pub mod catalogs;
pub mod exchange_fiat;
pub mod exchanges;
pub mod prices;
pub mod settings;
pub mod transactions;
