//! Public local-CA download.

use dioxus::prelude::*;

/// Return the PEM of the app-generated local CA, when one is active.
#[server]
#[get("/api/tls/ca")]
pub async fn tls_ca_api() -> Result<Option<String>, ServerFnError> {
    Ok(crate::security::tls::ca_pem().and_then(|pem| String::from_utf8(pem).ok()))
}
