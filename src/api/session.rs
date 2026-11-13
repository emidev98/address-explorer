//! Session status probe that does not count as user activity.

use dioxus::prelude::*;

/// Return successfully only when the caller has a valid session. The UI uses
/// this to choose between the unlock screen and the app without extending the
/// idle timer in the background.
#[server]
#[get("/api/session/status")]
pub async fn session_status_api() -> Result<(), ServerFnError> {
    remote_get!("/api/session/status", [], []);
    Ok(())
}
