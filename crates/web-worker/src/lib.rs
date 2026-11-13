//! SharedWorker WASM entry point for the browser-local backend.
//!
//! The worker compiles the repository's domain, data, application, and client
//! modules directly. This keeps one implementation of the business rules for
//! native and web while ensuring the main PWA WASM never links SQLite.
#![cfg(target_arch = "wasm32")]

#[path = "../../../src/application/mod.rs"]
pub mod application;
#[path = "../../../src/clients/mod.rs"]
pub mod clients;
#[path = "../../../src/data/mod.rs"]
pub mod data;
#[path = "../../../src/domain/mod.rs"]
pub mod domain;
mod security;

mod db;
mod rpc;
mod vault;
mod worker;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    worker::start()
}
