//! Browser wrappers for the data management endpoints.
use dioxus_fullstack_core::ServerFnError;

pub async fn export_data_api() -> Result<String, ServerFnError> {
    crate::web_post_body!("data.export", "/api/data/export", {})
}

pub async fn import_data_api(file: String) -> Result<(), ServerFnError> {
    crate::web_post_body!("data.import", "/api/data/import", { "file": file })
}

pub async fn clear_data_api() -> Result<(), ServerFnError> {
    crate::web_post_body!("data.clear", "/api/data/clear", {})
}
