//! Protocol shared by the web app and its SharedWorker.
//!
//! The worker is a separate WASM artifact. Keeping the envelope and version in
//! one crate prevents the two sides from drifting while still allowing the
//! worker to compile the application/data layer independently.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Version of the worker message contract. Bump whenever a request/response
/// payload or an endpoint name changes incompatibly.
pub const WORKER_PROTOCOL_VERSION: u32 = 2;

/// Name of the SQLite file inside the browser VFS.
pub const WEB_DATABASE_FILE: &str = "address-explorer.db";

/// Request sent from a tab to the SharedWorker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerRequest {
    /// Monotonic per-tab request id.
    pub id: u64,
    /// Endpoint name, for example `settings.get_theme`.
    pub endpoint: String,
    /// JSON payload with the endpoint arguments.
    #[serde(default)]
    pub payload: Value,
}

/// Response sent from the SharedWorker to one tab.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub id: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WorkerResponse {
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }
}

/// One-time handshake sent before the first endpoint request.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerHello {
    pub protocol_version: u32,
    pub app_version: String,
    /// URL of the PWA-cached packed pricing asset.
    pub seed_url: String,
    /// Clear the browser database before opening it (used by import/tests).
    #[serde(default)]
    pub reset: bool,
}

/// Worker readiness message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerReady {
    pub protocol_version: u32,
    pub app_version: String,
    pub database: String,
}

/// Broadcast after a write endpoint commits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataChanged {
    pub revision: u64,
}

/// Broadcast with import/export/seed progress.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerProgress {
    pub operation: String,
    pub current: u64,
    pub total: u64,
    #[serde(default)]
    pub message: String,
}
