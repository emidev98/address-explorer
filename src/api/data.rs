//! Data management endpoints for the active local or remote database.
#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::database_backup;
use base64::Engine;
use dioxus::prelude::*;

/// Export the active database as a base64-encoded SQLite file.
#[server]
#[post("/api/data/export")]
pub async fn export_data_api() -> Result<String, ServerFnError> {
    remote_post_with_timeout!(
        "/api/data/export",
        [],
        [],
        [],
        crate::api::remote::DATA_TRANSFER_TIMEOUT
    );
    let database = open_database()?;
    let bytes = database_backup::export_database(&database)
        .map_err(|error| ServerFnError::new(error.to_string()))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Validate and import a base64-encoded SQLite backup into the active database.
#[server]
#[post("/api/data/import")]
pub async fn import_data_api(file: String) -> Result<(), ServerFnError> {
    remote_post_with_timeout!(
        "/api/data/import",
        [],
        [],
        [file],
        crate::api::remote::DATA_TRANSFER_TIMEOUT
    );
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(file.trim())
        .map_err(|_| ServerFnError::new("the backup upload is not valid base64 data"))?;
    let database = open_database()?;
    database_backup::import_database(&database, &bytes)
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
    let database = open_database()?;
    database_backup::reset_database(&database)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
