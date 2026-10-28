//! System clipboard access through the platform webview.
//!
//! Clipboard permissions are controlled by the browser or webview. Failures
//! surface as empty results or errors instead of blocking the UI, so manual
//! entry remains possible when clipboard access is denied.

use dioxus::prelude::*;

/// Read text from the system clipboard.
///
/// Returns an empty string when access is denied, letting callers remain
/// usable for manual input instead of failing the whole flow.
pub async fn read_clipboard_text() -> Result<String, String> {
    let mut eval = document::eval(
        r#"
        (async () => {
            try {
                const text = await navigator.clipboard.readText();
                dioxus.send(typeof text === "string" ? text : "");
            } catch (error) {
                dioxus.send("");
            }
        })();
        "#,
    );

    eval.recv::<String>()
        .await
        .map_err(|error| error.to_string())
}

/// Copy text to the system clipboard.
///
/// Fails when the webview denies clipboard-write access; the caller reports
/// the failure to the user.
pub async fn write_clipboard_text(text: &str) -> Result<(), String> {
    let text = serde_json::to_string(text).map_err(|error| error.to_string())?;
    let mut eval = document::eval(&format!(
        r#"
        (async () => {{
            try {{
                await navigator.clipboard.writeText({text});
                dioxus.send(true);
            }} catch (error) {{
                dioxus.send(false);
            }}
        }})();
        "#,
    ));

    let copied = eval
        .recv::<bool>()
        .await
        .map_err(|error| error.to_string())?;
    if copied {
        Ok(())
    } else {
        Err("Clipboard access was denied.".to_string())
    }
}
