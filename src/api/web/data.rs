//! Browser wrappers for the data management endpoints.
use crate::domain::secret::SecretString;
use dioxus_fullstack_core::ServerFnError;

pub async fn clear_data_api() -> Result<(), ServerFnError> {
    crate::web_post_body!("data.clear", "/api/data/clear", {})
}

pub async fn export_encrypted_data_api(passphrase: SecretString) -> Result<String, ServerFnError> {
    crate::web_post_body!(
        "data.export_encrypted",
        "/api/data/export-encrypted",
        { "passphrase": passphrase }
    )
}

pub async fn import_encrypted_data_api(
    file: String,
    passphrase: SecretString,
) -> Result<(), ServerFnError> {
    crate::web_post_body!(
        "data.import_encrypted",
        "/api/data/import-encrypted",
        { "file": file, "passphrase": passphrase }
    )
}
