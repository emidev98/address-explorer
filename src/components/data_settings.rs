//! Data settings: export, import, and reset the active database.
//!
//! The same component runs on web, desktop, and mobile. The API dispatch selects
//! the browser worker, the in-process local database, or the remote server; only
//! the final file destination is platform-specific.

use crate::api::data::{clear_data_api, export_data_api, import_data_api};
use crate::api::settings::save_confirm_destructive_actions_api;
use crate::backend;
use crate::components::{
    request_app_reload, use_interface_settings, use_toast, BackendSettingsContext, Checkbox,
    ConfirmDialog, Modal, Toast,
};
use crate::domain::backup::{normalize_backup_file_name, DEFAULT_BACKUP_FILE_NAME};
use base64::Engine;
use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, JsValue};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::JsFuture;

const IMPORT_INPUT_ID: &str = "data-import-file";

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingImport {
    file_name: String,
    encoded: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DataOperation {
    Idle,
    Reading,
    Exporting,
    Importing,
    Clearing,
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[derive(serde::Deserialize)]
struct MobileFile {
    name: String,
    data: String,
}

/// Data management and destructive-action safety of Settings → General.
#[component]
pub fn DataSettingsSection() -> Element {
    let toast = use_toast();
    let context = use_context::<BackendSettingsContext>();
    let mut operation = use_signal(|| DataOperation::Idle);
    let mut export_open = use_signal(|| false);
    let mut file_name = use_signal(|| DEFAULT_BACKUP_FILE_NAME.to_string());
    let mut export_error = use_signal(|| Option::<String>::None);
    let mut pending_import = use_signal(|| Option::<PendingImport>::None);
    let mut import_open = use_signal(|| false);
    let mut clear_open = use_signal(|| false);
    let mut status = use_signal(|| Option::<String>::None);
    let mut error_message = use_signal(|| Option::<String>::None);
    let storage = use_signal(|| Option::<(u64, u64)>::None);
    let import_key = use_signal(|| 0u32);
    let interface_settings = use_interface_settings();
    let confirm_saving = use_signal(|| false);

    let confirm_destructive = interface_settings.get().confirm_destructive_actions;
    let remote = backend::is_remote();
    let remote_url = backend::remote_base_url().unwrap_or_else(|| "configured server".to_string());
    let (source_label, target_label) = if remote {
        (
            format!("Remote server — {remote_url}"),
            format!("the database on the remote server ({remote_url})"),
        )
    } else {
        (
            "Local database".to_string(),
            "the local database".to_string(),
        )
    };
    let description = if remote {
        "The active backend is the remote server shown below. Export downloads a copy of its SQLite database, import replaces it, and clear resets it to bundled defaults. These actions affect every user of that server. The exported file is not encrypted."
    } else if cfg!(target_arch = "wasm32") {
        "The active backend is the local database stored in this browser profile with IndexedDB. Persistent storage is requested automatically when Local mode starts; if the browser declines, normal storage limits apply. Export a backup before clearing site data or moving browsers. The exported file is not encrypted."
    } else {
        "The active backend is the local SQLite database stored in the application data directory. Export a backup before clearing the data or moving to another device. The exported file is not encrypted."
    };
    let export_description = format!(
        "Export from {source_label}. The file is a standard SQLite database and is not encrypted."
    );
    let pending_name = {
        let pending = pending_import.read();
        pending.as_ref().map(|pending| pending.file_name.clone())
    };
    let import_message = pending_name.as_ref().map(|name| {
        format!(
            "Import \"{name}\" into {target_label}? This replaces the current database. Export a backup first."
        )
    });
    let clear_message = format!(
        "Clear {target_label}? All user data is removed and the bundled defaults are restored. This cannot be undone. Export a backup first."
    );

    use_future(move || async move {
        let mut storage = storage;
        if !remote && cfg!(target_arch = "wasm32") {
            if let Some(estimate) = storage_estimate().await {
                storage.set(Some(estimate));
            }
        }
    });

    if !backend::local_available() {
        return rsx! {};
    }

    let submit_export = move |_| {
        if operation() != DataOperation::Idle {
            return;
        }
        let normalized = match normalize_backup_file_name(&file_name()) {
            Ok(name) => name,
            Err(message) => {
                export_error.set(Some(message));
                return;
            }
        };
        file_name.set(normalized.clone());
        export_error.set(None);
        error_message.set(None);
        operation.set(DataOperation::Exporting);
        status.set(Some("Preparing the database export…".to_string()));
        let mut operation = operation;
        let mut export_open = export_open;
        let mut export_error = export_error;
        let mut status = status;
        let mut toast = toast;
        spawn(async move {
            let exported = export_data_api().await;
            let result = match exported {
                Ok(encoded) => match base64::engine::general_purpose::STANDARD.decode(encoded) {
                    Ok(bytes) => save_export(&normalized, &bytes),
                    Err(_) => Err("Could not decode the exported database.".to_string()),
                },
                Err(error) => Err(format!("Could not export the database: {error}")),
            };
            operation.set(DataOperation::Idle);
            match result {
                Ok(message) => {
                    export_open.set(false);
                    status.set(None);
                    toast.success(message);
                }
                Err(message) => {
                    status.set(None);
                    export_error.set(Some(message.clone()));
                    toast.error(message);
                }
            }
        });
    };

    let select_import = move |event: FormEvent| {
        if operation() != DataOperation::Idle {
            return;
        }
        let files = event.files();
        let Some(file) = files.first().cloned() else {
            return;
        };
        error_message.set(None);
        operation.set(DataOperation::Reading);
        status.set(Some("Reading the selected backup…".to_string()));
        let key = import_key();
        let mut operation = operation;
        let mut pending_import = pending_import;
        let mut import_open = import_open;
        let mut import_key = import_key;
        let mut status = status;
        let mut error_message = error_message;
        let mut toast = toast;
        spawn(async move {
            let result = read_import_file(file).await;
            operation.set(DataOperation::Idle);
            import_key.set(key.wrapping_add(1));
            match result {
                Ok((file_name, encoded)) => {
                    pending_import.set(Some(PendingImport { file_name, encoded }));
                    import_open.set(true);
                    status.set(None);
                }
                Err(message) => {
                    status.set(None);
                    error_message.set(Some(message.clone()));
                    toast.error(message);
                }
            }
        });
    };

    let confirm_import = move |_| {
        if operation() != DataOperation::Idle {
            return;
        }
        let Some(pending) = pending_import() else {
            return;
        };
        import_open.set(false);
        pending_import.set(None);
        error_message.set(None);
        operation.set(DataOperation::Importing);
        status.set(Some("Importing the database…".to_string()));
        let encoded = pending.encoded;
        let mut operation = operation;
        let mut status = status;
        let mut error_message = error_message;
        let mut toast = toast;
        spawn(async move {
            match import_data_api(encoded).await {
                Ok(()) => reload_after_data_change(
                    context,
                    operation,
                    status,
                    error_message,
                    toast,
                    "Database imported.",
                ),
                Err(failure) => {
                    operation.set(DataOperation::Idle);
                    status.set(None);
                    let message = format!("Could not import the database: {failure}");
                    error_message.set(Some(message.clone()));
                    toast.error(message);
                }
            }
        });
    };

    let confirm_clear = move |_| {
        if operation() != DataOperation::Idle {
            return;
        }
        clear_open.set(false);
        error_message.set(None);
        operation.set(DataOperation::Clearing);
        status.set(Some("Resetting the database…".to_string()));
        let mut operation = operation;
        let mut status = status;
        let mut error_message = error_message;
        let mut toast = toast;
        spawn(async move {
            match clear_data_api().await {
                Ok(()) => reload_after_data_change(
                    context,
                    operation,
                    status,
                    error_message,
                    toast,
                    "Database cleared.",
                ),
                Err(failure) => {
                    operation.set(DataOperation::Idle);
                    status.set(None);
                    let message = format!("Could not clear the database: {failure}");
                    error_message.set(Some(message.clone()));
                    toast.error(message);
                }
            }
        });
    };

    let busy = operation() != DataOperation::Idle;
    let export_label = if operation() == DataOperation::Exporting {
        "Exporting…"
    } else {
        "Export"
    };
    let import_label = if operation() == DataOperation::Reading {
        "Reading backup…"
    } else {
        "Import backup"
    };
    let clear_label = if operation() == DataOperation::Clearing {
        "Clearing…"
    } else {
        "Clear database"
    };

    rsx! {
        section { class: "md3-settings-section md3-settings-span",
            div {
                h3 { class: "text-title-medium leading-title-medium text-on-surface",
                    "Data"
                }
                p { class: "text-body-medium text-on-surface-variant", "{description}" }
                p { class: "text-body-small text-on-surface-variant", "Active source: {source_label}" }
            }
            if !remote {
                if let Some((usage, quota)) = storage() {
                    p { class: "text-body-small text-on-surface-variant",
                        "Storage used: {format_bytes(usage)} of {format_bytes(quota)}"
                    }
                }
            }
            div { class: "md3-chip-group",
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: busy,
                    onclick: move |_| {
                        if operation() == DataOperation::Idle {
                            export_error.set(None);
                            file_name.set(DEFAULT_BACKUP_FILE_NAME.to_string());
                            export_open.set(true);
                        }
                    },
                    "Export database"
                }
                label { class: "md3-button-outlined", r#for: IMPORT_INPUT_ID,
                    "{import_label}"
                    input {
                        id: IMPORT_INPUT_ID,
                        key: "{import_key}",
                        r#type: "file",
                        accept: ".sqlite,.sqlite3,.db,application/vnd.sqlite3,application/x-sqlite3",
                        style: "display: none;",
                        disabled: busy,
                        onchange: select_import,
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-button-outlined",
                    disabled: busy,
                    onclick: move |_| {
                        if operation() == DataOperation::Idle {
                            clear_open.set(true);
                        }
                    },
                    "{clear_label}"
                }
            }
            if let Some(message) = status() {
                p {
                    class: "text-body-small text-on-surface-variant",
                    role: "status",
                    aria_live: "polite",
                    "{message}"
                }
            }
            if let Some(message) = error_message() {
                p {
                    class: "text-body-small",
                    style: "color: var(--md-sys-color-error);",
                    role: "alert",
                    "{message}"
                }
            }
            hr { class: "md3-divider" }
            div {
                h3 { class: "text-title-medium leading-title-medium text-on-surface",
                    "Safety"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Show a confirmation dialog before removing addresses, exchange accounts, exchange transactions, catalog entries, or stored prices. When this is off, those deletes run immediately. Importing, replacing, or clearing the database always asks for confirmation."
                }
            }
            Checkbox {
                checked: confirm_destructive,
                disabled: *confirm_saving.read(),
                label: "Confirm destructive actions".to_string(),
                on_change: move |enabled| {
                    let previous = interface_settings.get();
                    let mut next = previous;
                    next.confirm_destructive_actions = enabled;
                    interface_settings.set(next);
                    let mut saving = confirm_saving;
                    saving.set(true);
                    let mut toast = toast;
                    spawn(async move {
                        match save_confirm_destructive_actions_api(enabled).await {
                            Ok(()) => {
                                toast
                                    .success(
                                        if enabled {
                                            "Destructive action confirmations enabled."
                                        } else {
                                            "Destructive action confirmations disabled."
                                        },
                                    )
                            }
                            Err(error) => {
                                interface_settings.set(previous);
                                toast
                                    .error(
                                        format!("Could not save the confirmation setting: {error}"),
                                    );
                            }
                        }
                        saving.set(false);
                    });
                },
            }
        }
        if export_open() {
            Modal {
                title: "Export database".to_string(),
                title_id: "data-export-title".to_string(),
                dismissible: operation() == DataOperation::Idle,
                on_close: move |_| {
                    if operation() == DataOperation::Idle {
                        export_open.set(false);
                    }
                },
                p { class: "text-body-medium text-on-surface-variant", "{export_description}" }
                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "data-export-file-name",
                    "File name"
                }
                div { class: "md3-text-field",
                    input {
                        id: "data-export-file-name",
                        class: "md3-input",
                        r#type: "text",
                        value: "{file_name}",
                        autofocus: true,
                        disabled: busy,
                        oninput: move |event| {
                            file_name.set(event.value());
                            export_error.set(None);
                        },
                    }
                }
                if let Some(message) = export_error() {
                    p {
                        class: "text-body-small",
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "{message}"
                    }
                }
                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: busy,
                        onclick: move |_| export_open.set(false),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: busy,
                        onclick: submit_export,
                        "{export_label}"
                    }
                }
            }
        }
        if import_open() {
            if let Some(message) = import_message {
                ConfirmDialog {
                    title: "Import backup".to_string(),
                    message,
                    confirm_text: "Import".to_string(),
                    cancel_text: "Cancel".to_string(),
                    on_confirm: confirm_import,
                    on_cancel: move |_| {
                        import_open.set(false);
                        pending_import.set(None);
                    },
                }
            }
        }
        if clear_open() {
            ConfirmDialog {
                title: "Clear database".to_string(),
                message: clear_message,
                confirm_text: "Clear database".to_string(),
                cancel_text: "Cancel".to_string(),
                on_confirm: confirm_clear,
                on_cancel: move |_| clear_open.set(false),
            }
        }
    }
}

fn reload_after_data_change(
    context: BackendSettingsContext,
    mut operation: Signal<DataOperation>,
    mut status: Signal<Option<String>>,
    mut error_message: Signal<Option<String>>,
    mut toast: Toast,
    message: &str,
) {
    status.set(Some(format!("{message} Reloading…")));
    if let Err(reload_error) = request_app_reload(context) {
        operation.set(DataOperation::Idle);
        status.set(None);
        let text = format!(
            "{message}, but the application could not reload: {reload_error}. Restart it manually."
        );
        error_message.set(Some(text.clone()));
        toast.error(text);
    }
}

async fn read_import_file(file: dioxus::html::FileData) -> Result<(String, String), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = file;
        read_mobile_file().await
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let name = file.name();
        let bytes = file
            .read_bytes()
            .await
            .map_err(|error| format!("Could not read the selected file: {error}"))?;
        if bytes.is_empty() {
            return Err("The selected SQLite file is empty.".to_string());
        }
        Ok((
            name,
            base64::engine::general_purpose::STANDARD.encode(&bytes),
        ))
    }
}

#[cfg(any(target_os = "android", target_os = "ios"))]
async fn read_mobile_file() -> Result<(String, String), String> {
    let script = r#"
        const input = document.getElementById('data-import-file');
        const file = input && input.files && input.files[0];
        if (!file) {
            dioxus.send(null);
        } else {
            const reader = new FileReader();
            reader.onload = () => dioxus.send({ name: file.name, data: reader.result });
            reader.onerror = () => dioxus.send(null);
            reader.readAsDataURL(file);
        }
    "#;
    let mut eval = document::eval(script);
    let value: Option<MobileFile> = eval
        .recv()
        .await
        .map_err(|error| format!("Could not read the selected file: {error}"))?;
    let value = value.ok_or_else(|| "Could not read the selected file.".to_string())?;
    let encoded = value
        .data
        .split_once(',')
        .map(|(_, encoded)| encoded)
        .unwrap_or(value.data.as_str());
    if encoded.is_empty() {
        return Err("The selected SQLite file is empty.".to_string());
    }
    Ok((value.name, encoded.to_string()))
}

#[cfg(target_arch = "wasm32")]
fn save_export(file_name: &str, bytes: &[u8]) -> Result<String, String> {
    download_bytes(file_name, bytes, "application/x-sqlite3")?;
    Ok("Export started. Check your browser's Downloads folder.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn save_export(file_name: &str, bytes: &[u8]) -> Result<String, String> {
    let directory = native_export_directory()?;
    let path = unique_export_path(&directory, file_name);
    std::fs::write(&path, bytes)
        .map_err(|error| format!("Could not save the database: {error}"))?;
    Ok(format!("Database exported to {}.", path.display()))
}

#[cfg(target_arch = "wasm32")]
fn download_bytes(name: &str, bytes: &[u8], content_type: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or_else(|| "Browser window is unavailable.".to_string())?;
    let array = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(content_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|error| format!("Could not create the download file: {error:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|error| format!("Could not create the download URL: {error:?}"))?;
    let document = window
        .document()
        .ok_or_else(|| "Browser document is unavailable.".to_string())?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|error| format!("Could not create the download link: {error:?}"))?
        .unchecked_into();
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url)
        .map_err(|error| format!("Could not release the download URL: {error:?}"))?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn storage_estimate() -> Option<(u64, u64)> {
    let storage = web_sys::window()?.navigator().storage();
    let value = JsFuture::from(storage.estimate().ok()?).await.ok()?;
    let usage = js_sys::Reflect::get(&value, &JsValue::from_str("usage"))
        .ok()?
        .as_f64()? as u64;
    let quota = js_sys::Reflect::get(&value, &JsValue::from_str("quota"))
        .ok()?
        .as_f64()? as u64;
    Some((usage, quota))
}

#[cfg(not(target_arch = "wasm32"))]
async fn storage_estimate() -> Option<(u64, u64)> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn native_export_directory() -> Result<std::path::PathBuf, String> {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        if let Some(directory) = dirs::document_dir() {
            candidates.push(directory);
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Documents"));
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Some(directory) = dirs::download_dir() {
            candidates.push(directory);
        }
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Downloads"));
        }
    }
    if let Some(directory) = dirs::data_dir() {
        candidates.push(directory.join("address-explorer").join("exports"));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join("address-explorer").join("exports"));
    }

    for candidate in candidates {
        if std::fs::create_dir_all(&candidate).is_ok() {
            return Ok(candidate);
        }
    }
    Err("Could not determine a folder to save the database.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn unique_export_path(directory: &std::path::Path, file_name: &str) -> std::path::PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let path = std::path::Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("address-explorer");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("sqlite");
    for index in 1..=9999 {
        let candidate = directory.join(format!("{stem} ({index}).{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(file_name)
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
