//! Web API facade helpers.
//!
//! The per-domain wrapper files are selected as the `crate::api::<domain>`
//! modules on WASM by `api/mod.rs`; only the shared dispatcher is exposed here.

pub mod common;
