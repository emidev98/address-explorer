//! Exchange and wallet type catalogs referenced by `addresses.exchange_id` and
//! `addresses.wallet_id`.
//!
//! Migration 0005 seeds `exchanges_types` and `wallets_types` with stable,
//! explicit ids, so stored addresses can reference well-known platforms by id
//! instead of storing free-text names.

use serde::{Deserialize, Serialize};

/// One row of the `exchanges_types` or `wallets_types` lookup tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: i64,
    pub name: String,
}
