//! Health endpoint used by clients to discover the server API version.
use crate::domain::health::Health;
use dioxus::prelude::*;

#[get("/api/health")]
pub async fn health_api() -> Result<Health, ServerFnError> {
    Ok(Health {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: crate::domain::health::API_VERSION,
    })
}
