//! General Settings → Security policy.
//!
//! Every value is stored in the active database. The server validates the safe
//! ranges again, so a modified client cannot weaken the policy below the
//! documented floor.

use crate::api::settings::{get_security_settings_api, save_security_settings_api};
use crate::components::use_toast;
use crate::domain::security_settings::{
    ExportFileMode, SecuritySettings, MAX_API_BURST, MAX_API_REQUESTS_PER_MINUTE,
    MAX_ARGON2_CONCURRENCY, MAX_BACKUP_BYTES, MAX_EXTERNAL_TIMEOUT_SECS, MAX_JSON_BODY_BYTES,
    MAX_PASSWORD_MIN_LENGTH, MAX_RESPONSE_BYTES, MAX_STALE_TEMP_HOURS, MAX_TOKEN_BYTES,
    MAX_UNLOCK_BACKOFF_SECS, MAX_UNLOCK_FAILURES, MIN_API_BURST, MIN_API_REQUESTS_PER_MINUTE,
    MIN_ARGON2_CONCURRENCY, MIN_BACKUP_BYTES, MIN_EXTERNAL_TIMEOUT_SECS, MIN_JSON_BODY_BYTES,
    MIN_PASSWORD_MIN_LENGTH, MIN_RESPONSE_BYTES, MIN_STALE_TEMP_HOURS, MIN_TOKEN_BYTES,
    MIN_UNLOCK_BACKOFF_SECS, MIN_UNLOCK_FAILURES,
};
use dioxus::prelude::*;

#[component]
pub fn SecuritySettingsSection() -> Element {
    let mut toast = use_toast();
    let mut draft = use_signal(SecuritySettings::default);
    let mut loaded = use_signal(|| false);
    let mut saving = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    use_future(move || async move {
        match get_security_settings_api().await {
            Ok(settings) => {
                draft.set(settings);
                loaded.set(true);
                error.set(None);
            }
            Err(failure) => error.set(Some(failure.to_string())),
        }
    });

    let save = move |_| {
        if *saving.read() {
            return;
        }
        saving.set(true);
        error.set(None);
        let candidate = draft();
        let mut draft = draft;
        let mut toast = toast;
        spawn(async move {
            match save_security_settings_api(candidate).await {
                Ok(saved) => {
                    draft.set(saved);
                    toast.success("Security policy saved.");
                }
                Err(failure) => {
                    let message = format!("Could not save the security policy: {failure}");
                    error.set(Some(message.clone()));
                    toast.error(message);
                }
            }
            saving.set(false);
        });
    };

    let reset = EventHandler::new(move |_: ()| {
        draft.set(SecuritySettings::default());
        error.set(None);
        toast.success("Recommended security defaults restored. Save to apply them.");
    });

    let current = draft();
    rsx! {
        section { class: "md3-settings-section md3-settings-span",
            div {
                h3 { class: "text-title-medium leading-title-medium text-on-surface",
                    "Security policy"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "These limits are stored in the active database. Unlocking the database replaces the compiled defaults with these values. Values outside the safe ranges are rejected by the server."
                }
                p { class: "text-body-small text-on-surface-variant",
                    "Live database files, directories, and TLS keys remain owner-only (0600/0700) and are not configurable."
                }
            }
            if !loaded() {
                if let Some(message) = error() {
                    p {
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "{message}"
                    }
                } else {
                    p { class: "text-body-small text-on-surface-variant",
                        "Loading security policy…"
                    }
                }
            } else {
                div { class: "md3-settings-grid",
                    SecurityField {
                        id: "security-password-min".to_string(),
                        label: "Minimum password length".to_string(),
                        hint: format!(
                            "Characters. Allowed: {MIN_PASSWORD_MIN_LENGTH}–{MAX_PASSWORD_MIN_LENGTH}.",
                        ),
                        value: current.password_min_length.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u8>() {
                                let mut next = draft();
                                next.password_min_length = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-json-body".to_string(),
                        label: "Normal API body limit (MiB)".to_string(),
                        hint: format!(
                            "Allowed: {} KiB–{} MiB.",
                            MIN_JSON_BODY_BYTES / 1024,
                            MAX_JSON_BODY_BYTES / (1024 * 1024),
                        ),
                        value: (current.max_json_body_bytes / (1024 * 1024)).to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                if let Some(bytes) = parsed.checked_mul(1024 * 1024) {
                                    let mut next = draft();
                                    next.max_json_body_bytes = bytes;
                                    draft.set(next);
                                }
                            }
                        },
                    }
                    SecurityField {
                        id: "security-backup-max".to_string(),
                        label: "Maximum imported backup (MiB)".to_string(),
                        hint: format!(
                            "Allowed: {}–{} MiB. The transport body limit is derived from this value.",
                            MIN_BACKUP_BYTES / (1024 * 1024),
                            MAX_BACKUP_BYTES / (1024 * 1024),
                        ),
                        value: (current.max_backup_bytes / (1024 * 1024)).to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                if let Some(bytes) = parsed.checked_mul(1024 * 1024) {
                                    let mut next = draft();
                                    next.max_backup_bytes = bytes;
                                    draft.set(next);
                                }
                            }
                        },
                    }
                    SecurityField {
                        id: "security-token-bytes".to_string(),
                        label: "Session token length (bytes)".to_string(),
                        hint: format!("Allowed: {MIN_TOKEN_BYTES}–{MAX_TOKEN_BYTES}. Applies to new sessions."),
                        value: current.session_token_bytes.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u8>() {
                                let mut next = draft();
                                next.session_token_bytes = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-argon2".to_string(),
                        label: "Concurrent Argon2 operations".to_string(),
                        hint: format!(
                            "Allowed: {MIN_ARGON2_CONCURRENCY}–{MAX_ARGON2_CONCURRENCY}. Each operation uses about 64 MiB.",
                        ),
                        value: current.max_concurrent_argon2.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u8>() {
                                let mut next = draft();
                                next.max_concurrent_argon2 = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-unlock-failures".to_string(),
                        label: "Failed unlocks before backoff".to_string(),
                        hint: format!("Allowed: {MIN_UNLOCK_FAILURES}–{MAX_UNLOCK_FAILURES}."),
                        value: current.unlock_failure_threshold.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u32>() {
                                let mut next = draft();
                                next.unlock_failure_threshold = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-unlock-backoff".to_string(),
                        label: "Unlock backoff (seconds)".to_string(),
                        hint: format!(
                            "Allowed: {MIN_UNLOCK_BACKOFF_SECS}–{MAX_UNLOCK_BACKOFF_SECS}. Grows exponentially after the threshold.",
                        ),
                        value: current.unlock_backoff_secs.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                let mut next = draft();
                                next.unlock_backoff_secs = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-api-rate".to_string(),
                        label: "API requests per minute".to_string(),
                        hint: format!(
                            "Allowed: {MIN_API_REQUESTS_PER_MINUTE}–{MAX_API_REQUESTS_PER_MINUTE} per client.",
                        ),
                        value: current.api_requests_per_minute.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u32>() {
                                let mut next = draft();
                                next.api_requests_per_minute = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-api-burst".to_string(),
                        label: "API rate burst".to_string(),
                        hint: format!("Allowed: {MIN_API_BURST}–{MAX_API_BURST}."),
                        value: current.api_rate_burst.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u32>() {
                                let mut next = draft();
                                next.api_rate_burst = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-external-timeout".to_string(),
                        label: "External request timeout (seconds)".to_string(),
                        hint: format!(
                            "Allowed: {MIN_EXTERNAL_TIMEOUT_SECS}–{MAX_EXTERNAL_TIMEOUT_SECS}. Applies to mempool, Coin Metrics, and Frankfurter.",
                        ),
                        value: current.external_timeout_secs.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                let mut next = draft();
                                next.external_timeout_secs = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    SecurityField {
                        id: "security-response-cap".to_string(),
                        label: "External response cap (MiB)".to_string(),
                        hint: format!(
                            "Allowed: {}–{} MiB.",
                            MIN_RESPONSE_BYTES / (1024 * 1024),
                            MAX_RESPONSE_BYTES / (1024 * 1024),
                        ),
                        value: (current.external_response_max_bytes / (1024 * 1024)).to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                if let Some(bytes) = parsed.checked_mul(1024 * 1024) {
                                    let mut next = draft();
                                    next.external_response_max_bytes = bytes;
                                    draft.set(next);
                                }
                            }
                        },
                    }
                    SecurityField {
                        id: "security-stale-temp".to_string(),
                        label: "Stale temporary-file age (hours)".to_string(),
                        hint: format!(
                            "Allowed: {MIN_STALE_TEMP_HOURS}–{MAX_STALE_TEMP_HOURS}. Older Address Explorer temp files are removed at startup.",
                        ),
                        value: current.stale_temp_cleanup_hours.to_string(),
                        disabled: *saving.read(),
                        on_input: move |value: String| {
                            if let Ok(parsed) = value.parse::<u64>() {
                                let mut next = draft();
                                next.stale_temp_cleanup_hours = parsed;
                                draft.set(next);
                            }
                        },
                    }
                    div { class: "md3-settings-field",
                        label {
                            class: "text-body-medium text-on-surface-variant",
                            r#for: "security-export-mode",
                            "Exported file permissions"
                        }
                        div { class: "md3-text-field",
                            select {
                                id: "security-export-mode",
                                class: "md3-input",
                                value: current.export_file_mode.as_str(),
                                disabled: *saving.read(),
                                onchange: move |event| {
                                    if let Some(mode) = ExportFileMode::from_value(&event.value()) {
                                        let mut next = draft();
                                        next.export_file_mode = mode;
                                        draft.set(next);
                                    }
                                },
                                for mode in ExportFileMode::ALL {
                                    option {
                                        value: "{mode.as_str()}",
                                        selected: current.export_file_mode == mode,
                                        "{mode.label()}"
                                    }
                                }
                            }
                        }
                        p { class: "text-body-small text-on-surface-variant",
                            "Only stricter export modes are available. Live database and key files stay 0600."
                        }
                    }
                }
                if let Some(message) = error() {
                    p {
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "{message}"
                    }
                }
                div { class: "md3-chip-group",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *saving.read(),
                        onclick: move |_| reset.call(()),
                        "Reset to recommended defaults"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *saving.read(),
                        onclick: save,
                        if *saving.read() {
                            "Saving…"
                        } else {
                            "Save security policy"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SecurityField(
    id: String,
    label: String,
    hint: String,
    value: String,
    #[props(default)] disabled: bool,
    on_input: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "md3-settings-field",
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "{id}",
                "{label}"
            }
            div { class: "md3-text-field",
                input {
                    id: "{id}",
                    class: "md3-input",
                    r#type: "number",
                    inputmode: "numeric",
                    value: "{value}",
                    disabled,
                    oninput: move |event| on_input.call(event.value()),
                }
            }
            p { class: "text-body-small text-on-surface-variant", "{hint}" }
        }
    }
}
