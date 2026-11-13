//! Backend selection used by Settings → General.
//!
//! The backend URL is a local preference, not backend data. Switching always
//! saves the preference and reloads the application so no stale resources
//! from the previous backend remain visible.
use crate::backend::BackendConfig;
#[cfg(any(
    all(feature = "remote-backend", not(target_arch = "wasm32")),
    all(feature = "web-client", target_arch = "wasm32")
))]
use crate::components::use_toast;
use dioxus::prelude::*;

/// App-wide backend preference and hard-reload state.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct BackendSettingsContext {
    pub config: Signal<BackendConfig>,
    pub reload_generation: Signal<u32>,
    pub reloading: Signal<bool>,
}

/// Reload the application after a backend or database change.
///
/// Web reloads the page, release desktop restarts the process, and mobile or a
/// debug desktop remounts the root because a managed process must not be
/// spawned again.
pub fn request_app_reload(context: BackendSettingsContext) -> Result<(), String> {
    if cfg!(all(
        not(target_arch = "wasm32"),
        any(
            any(target_os = "android", target_os = "ios"),
            debug_assertions
        ),
    )) {
        let mut reloading = context.reloading;
        reloading.set(true);
        let mut generation = context.reload_generation;
        generation.set(generation() + 1);
        Ok(())
    } else {
        crate::backend::hard_reload()
    }
}

#[cfg(any(
    all(feature = "remote-backend", not(target_arch = "wasm32")),
    all(feature = "web-client", target_arch = "wasm32")
))]
#[component]
pub fn BackendSettingsSection() -> Element {
    use crate::api::remote;
    use crate::backend::{self, BackendMode};
    use crate::domain::health::{Health, API_VERSION};

    let context = use_context::<BackendSettingsContext>();
    let mut toast = use_toast();
    let mut selected_mode = use_signal(|| context.config.read().mode);
    let mut url = use_signal(|| context.config.read().base_url.clone().unwrap_or_default());
    let mut status = use_signal(|| Option::<String>::None);
    let testing = use_signal(|| false);
    let saving = use_signal(|| false);
    let mut tested = use_signal(|| Option::<(String, Health)>::None);

    if !backend::settings_section_available() {
        return rsx! {};
    }

    let normalized = backend::validate_base_url(&url()).ok();
    let test_matches = tested().as_ref().is_some_and(|(tested_url, health)| {
        normalized.as_deref() == Some(tested_url.as_str()) && health.api_version == API_VERSION
    });
    let remote_selected = selected_mode() == BackendMode::Remote;
    let url_error = if remote_selected && !url().trim().is_empty() {
        backend::validate_base_url(&url()).err()
    } else {
        None
    };
    let url_invalid = url_error.is_some();
    let insecure_http = remote_selected && backend::uses_insecure_remote_http(url().trim());

    rsx! {
        section { class: "md3-settings-section md3-settings-span",
            div {
                h3 { class: "text-title-medium leading-title-medium text-on-surface",
                    "Backend"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Choose the bundled local database or a remote Address Explorer server. The two stores are separate; switching does not migrate data automatically. Use Data below to export or import a backup. Saving reloads the application."
                }
            }
            div {
                class: "md3-chip-group",
                role: "group",
                aria_label: "Backend mode",
                button {
                    r#type: "button",
                    class: if selected_mode() == BackendMode::Local { "md3-chip md3-chip-selected" } else { "md3-chip" },
                    aria_pressed: (selected_mode() == BackendMode::Local).to_string(),
                    disabled: *testing.read() || *saving.read(),
                    onclick: move |_| {
                        selected_mode.set(BackendMode::Local);
                        tested.set(None);
                        status.set(None);
                    },
                    "Local database"
                }
                button {
                    r#type: "button",
                    class: if selected_mode() == BackendMode::Remote { "md3-chip md3-chip-selected" } else { "md3-chip" },
                    aria_pressed: (selected_mode() == BackendMode::Remote).to_string(),
                    disabled: *testing.read() || *saving.read(),
                    onclick: move |_| {
                        selected_mode.set(BackendMode::Remote);
                        tested.set(None);
                        status.set(None);
                    },
                    "Remote server"
                }
            }
            if remote_selected {
                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "backend-url",
                    "Backend URL"
                }
                div { class: "md3-text-field",
                    input {
                        id: "backend-url",
                        class: "md3-input",
                        r#type: "url",
                        value: url(),
                        placeholder: "https://addresses.example.com",
                        aria_invalid: url_invalid.to_string(),
                        disabled: *testing.read() || *saving.read(),
                        oninput: move |event| {
                            url.set(event.value());
                            tested.set(None);
                            status.set(None);
                        },
                    }
                }
                p { class: "text-body-small text-on-surface-variant",
                    "The address of the server hosting the Address Explorer API. Do not include a trailing slash or a query. HTTPS is required; install the server's local CA on this device before testing the connection."
                }
                if let Some(error) = url_error.clone() {
                    p {
                        class: "text-body-small",
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "{error}"
                    }
                }
                if insecure_http {
                    p {
                        class: "text-body-small",
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "Plain HTTP is rejected for non-loopback servers. Use HTTPS so the session token and database traffic are encrypted."
                    }
                }
            }
            div { class: "md3-chip-group",
                if remote_selected {
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *testing.read() || *saving.read(),
                        onclick: move |_| {
                            let candidate = url();
                            let candidate = match backend::validate_base_url(&candidate) {
                                Ok(url) => url,
                                Err(error) => {
                                    status.set(Some(error.clone()));
                                    toast.error(error);
                                    return;
                                }
                            };
                            let mut testing = testing;
                            let mut status = status;
                            let mut tested = tested;
                            let mut toast = toast;
                            testing.set(true);
                            status.set(Some("Testing the remote backend…".to_string()));
                            spawn(async move {
                                match remote::health_at(&candidate).await {
                                    Ok(health) if health.api_version == API_VERSION => {
                                        status
                                            .set(
                                                Some(
                                                    format!(
                                                        "Connected: server {} · API {}.",
                                                        health.app_version,
                                                        health.api_version,
                                                    ),
                                                ),
                                            );
                                        toast.success("Remote backend connected.");
                                        tested.set(Some((candidate.clone(), health)));
                                    }
                                    Ok(health) => {
                                        let message = format!(
                                            "API version mismatch: this app uses API {API_VERSION}, the server uses API {}. The backend was not changed.",
                                            health.api_version,
                                        );
                                        status.set(Some(message.clone()));
                                        toast.error(message);
                                        tested.set(None);
                                    }
                                    Err(error) => {
                                        let message = format!(
                                            "Could not connect to the remote backend: {error}",
                                        );
                                        status.set(Some(message.clone()));
                                        toast.error(message);
                                        tested.set(None);
                                    }
                                }
                                testing.set(false);
                            });
                        },
                        "Test connection"
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *saving.read() || *testing.read() || (remote_selected && !test_matches),
                    onclick: move |_| {
                        let selected = selected_mode();
                        let valid_url = backend::validate_base_url(&url()).ok();
                        let base_url = if selected == BackendMode::Remote {
                            let Some((tested_url, health)) = tested() else {
                                let message = "Test the remote backend before saving.".to_string();
                                status.set(Some(message.clone()));
                                toast.error(message);
                                return;
                            };
                            if health.api_version != API_VERSION {
                                let message = "The remote API version does not match this app."
                                    .to_string();
                                status.set(Some(message.clone()));
                                toast.error(message);
                                return;
                            }
                            if valid_url.as_deref() != Some(tested_url.as_str()) {
                                let message = "Test the current backend URL before saving.".to_string();
                                status.set(Some(message.clone()));
                                toast.error(message);
                                return;
                            }
                            Some(tested_url)
                        } else {
                            valid_url.or_else(|| context.config.read().base_url.clone())
                        };
                        let mut saving = saving;
                        saving.set(true);
                        let config = BackendConfig {
                            version: 1,
                            mode: selected,
                            base_url,
                        };
                        match backend::save_config(&config) {
                            Ok(()) => {
                                backend::set_config(config.clone());
                                let mut config_signal = context.config;
                                config_signal.set(config);
                                status.set(Some("Backend saved. Reloading…".to_string()));
                                if let Err(error) = request_app_reload(context) {
                                    let message = format!("Could not reload the application: {error}");
                                    status.set(Some(message.clone()));
                                    toast.error(message);
                                    saving.set(false);
                                }
                            }
                            Err(error) => {
                                status.set(Some(error.clone()));
                                toast.error(error);
                                saving.set(false);
                            }
                        }
                    },
                    "Save and reload"
                }
            }
            if let Some(status) = status() {
                p {
                    class: "text-body-small text-on-surface-variant",
                    role: "status",
                    aria_live: "polite",
                    "{status}"
                }
            }
            if let Some((_, health)) = tested() {
                if let Some(fingerprint) = health.tls_fingerprint {
                    p { class: "text-body-small text-on-surface-variant",
                        "TLS certificate SHA-256: {fingerprint}"
                    }
                }
            }
        }
    }
}

#[cfg(not(any(
    all(feature = "remote-backend", not(target_arch = "wasm32")),
    all(feature = "web-client", target_arch = "wasm32")
)))]
#[component]
pub fn BackendSettingsSection() -> Element {
    rsx! {}
}
