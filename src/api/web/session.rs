//! Browser session status probe.

use dioxus_fullstack_core::ServerFnError;

pub async fn session_status_api() -> Result<(), ServerFnError> {
    crate::web_get_query!("session.status", "/api/session/status", {})
}
