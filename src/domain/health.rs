//! Health-reporting types shared by the server, the web client, and the
//! native remote backend.
//!
//! `Health` is intentionally small: it tells a client which application and
//! API version a server is running without touching the database.
use serde::{Deserialize, Serialize};

/// Current API contract version.
///
/// Bump this whenever a server-function request/response shape or endpoint
/// path changes incompatibly. The server rejects API requests from clients
/// with a different version, and clients reject remote servers whose version
/// does not match.
pub const API_VERSION: u32 = 4;

/// HTTP header carrying [`API_VERSION`] on API requests.
pub const API_VERSION_HEADER: &str = "x-address-explorer-api-version";

/// Version information returned by `GET /api/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// Human-readable application version (`CARGO_PKG_VERSION`).
    pub app_version: String,
    /// API contract version understood by the server.
    pub api_version: u32,
    /// SHA-256 fingerprint of the app-generated TLS leaf, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_fingerprint: Option<String>,
}
