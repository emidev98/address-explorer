//! Data management endpoints for the active local or remote database.
use crate::domain::secret::SecretString;
use base64::Engine;
use dioxus::prelude::*;

/// Export an encrypted SQLCipher backup with a separate passphrase.
#[server]
#[post("/api/data/export-encrypted")]
pub async fn export_encrypted_data_api(passphrase: SecretString) -> Result<String, ServerFnError> {
    #[cfg(feature = "remote-backend")]
    if crate::backend::is_remote() {
        return crate::api::remote::call_with_timeout(
            dioxus::fullstack::Method::POST,
            "/api/data/export-encrypted",
            Some(serde_json::json!({})),
            Some(serde_json::json!({ "passphrase": passphrase.as_str() })),
            crate::api::remote::DATA_TRANSFER_TIMEOUT,
        )
        .await;
    }
    let bytes = crate::data::vault::export_encrypted(passphrase.as_str())
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Import an encrypted SQLCipher backup with its backup passphrase.
#[server]
#[post("/api/data/import-encrypted")]
pub async fn import_encrypted_data_api(
    file: String,
    passphrase: SecretString,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "remote-backend")]
    if crate::backend::is_remote() {
        return crate::api::remote::call_with_timeout(
            dioxus::fullstack::Method::POST,
            "/api/data/import-encrypted",
            Some(serde_json::json!({})),
            Some(serde_json::json!({ "file": &file, "passphrase": passphrase.as_str() })),
            crate::api::remote::DATA_TRANSFER_TIMEOUT,
        )
        .await;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(file.trim())
        .map_err(|_| ServerFnError::new("the backup upload is not valid base64 data"))?;
    crate::data::vault::import_encrypted(&bytes, passphrase.as_str())
        .map(|_| ())
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Reset the active database to its bundled defaults.
#[server]
#[post("/api/data/clear")]
pub async fn clear_data_api() -> Result<(), ServerFnError> {
    remote_post_with_timeout!(
        "/api/data/clear",
        [],
        [],
        [],
        crate::api::remote::DATA_TRANSFER_TIMEOUT
    );
    let _operation = crate::data::vault::begin_operation()
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    crate::data::vault::reset_to_defaults()
        .map(|_| ())
        .map_err(|error| ServerFnError::new(error.to_string()))
}
