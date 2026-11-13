//! Health endpoint used by clients to discover the server API version.
use crate::domain::health::Health;
use dioxus::prelude::*;

#[get("/api/health")]
pub async fn health_api() -> Result<Health, ServerFnError> {
    #[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
    let tls_fingerprint = crate::security::tls::fingerprint();
    #[cfg(not(all(feature = "server", not(any(feature = "desktop", feature = "mobile")))))]
    let tls_fingerprint = None;
    Ok(Health {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        api_version: crate::domain::health::API_VERSION,
        tls_fingerprint,
    })
}
