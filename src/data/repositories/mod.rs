//! SQL access grouped by domain entity.
//!
//! These modules are the only place SQL appears in the application
//!. Functions return domain types and take
//! the shared [`Database`](crate::data::Database) handle; callers run them
//! away from the UI path.
pub mod addresses;
pub mod catalogs;
pub mod exchanges;
pub mod fiat_rates;
pub mod flow;
pub mod prices;
pub mod settings;
pub mod transactions;
/// SQLite placeholders for one `IN (...)` list of bound values. The count is
/// derived from an internal length, never from client input reaching SQL as
/// an identifier.
pub(crate) fn placeholders(count: usize) -> String {
    std::iter::repeat_n("?", count)
        .collect::<Vec<_>>()
        .join(",")
}
