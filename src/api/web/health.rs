//! Browser health endpoint: the local database reports the same API version,
//! and Remote mode asks the configured server.

use crate::domain::health::{Health, API_VERSION};
use dioxus_fullstack_core::ServerFnError;

pub async fn health_api() -> Result<Health, ServerFnError> {
    if crate::backend::is_remote() {
        crate::api::remote::health_at_current().await
    } else {
        Ok(Health {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            api_version: API_VERSION,
            tls_fingerprint: None,
        })
    }
}
