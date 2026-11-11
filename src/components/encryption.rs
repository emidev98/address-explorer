//! Encryption gate and settings UI shared by every backend.

use crate::api::encryption::{
    change_encryption_password_api, dismiss_encryption_prompt_api, get_encryption_status_api,
    lock_encryption_api, reset_encrypted_database_api, save_encryption_idle_timeout_api,
    setup_encryption_api, touch_encryption_api, unlock_encryption_api,
};
use crate::app::App;
use crate::components::theme::apply as apply_theme;
use crate::components::{use_toast, Checkbox, Modal};
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};
use crate::domain::theme::ThemePreference;
use dioxus::prelude::*;

/// Status signal shared by the vault gate and the settings block.
#[derive(Clone, Copy)]
pub struct EncryptionContext {
    pub status: Signal<Option<EncryptionStatus>>,
}

pub fn use_encryption() -> EncryptionContext {
    use_context()
}

/// Root gate: nothing that reads database data mounts until the vault is
/// unlocked, and the first-run prompt is answered.
#[component]
pub fn VaultGate(#[props(default)] generation: u32) -> Element {
    let mut status = use_signal(|| Option::<EncryptionStatus>::None);
    let mut error = use_signal(|| Option::<String>::None);
    use_context_provider(|| EncryptionContext { status });

    use_future(move || async move {
        loop {
            match get_encryption_status_api().await {
                Ok(value) => {
                    error.set(None);
                    status.set(Some(value));
                }
                Err(failure) => {
                    error.set(Some(failure.to_string()));
                    eprintln!("failed to load encryption status: {failure}");
                }
            }
            sleep_ms(2_000).await;
        }
    });

    use_future(move || async move {
        let mut eval = document::eval(
            r#"
            let last = 0;
            const mark = () => {
                const now = Date.now();
                if (now - last < 5000) return;
                last = now;
                dioxus.send(now);
            };
            for (const name of ['pointerdown', 'keydown', 'wheel', 'touchstart', 'scroll']) {
                window.addEventListener(name, mark, { passive: true });
            }
            "#,
        );
        while eval.recv::<f64>().await.is_ok() {
            let _ = touch_encryption_api().await;
        }
    });

    let current = status.read().clone();
    match current {
        None => rsx! {
            div {
                class: "md3-screen",
                style: "display: flex; min-height: 100vh; align-items: center; justify-content: center;",
                div { role: "status", aria_live: "polite",
                    h1 { class: "text-headline-small", "Opening Address Explorer" }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Checking the active database…"
                    }
                    if let Some(message) = error() {
                        p { style: "color: var(--md3-color-error);", role: "alert", "{message}" }
                    }
                }
            }
        },
        Some(value) if value.configured && value.locked => rsx! {
            UnlockScreen {}
        },
        Some(value) => rsx! {
            App { key: "{generation}" }
            if !value.configured && !value.prompt_seen {
                SetupDialog {
                    on_done: move |saved| status.set(Some(saved)),
                    on_dismiss: move |saved| status.set(Some(saved)),
                }
            }
        },
    }
}

#[component]
fn UnlockScreen() -> Element {
    let context = use_encryption();
    let mut status = context.status;
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let mut reset_open = use_signal(|| false);
    use_effect(|| apply_theme(ThemePreference::default()));

    let submit = EventHandler::new(move |_: ()| {
        if *busy.read() || password().is_empty() {
            return;
        }
        let password_value = password();
        password.set(String::new());
        busy.set(true);
        spawn(async move {
            match unlock_encryption_api(password_value).await {
                Ok(saved) => status.set(Some(saved)),
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    rsx! {
        div {
            class: "md3-screen",
            style: "min-height: 100vh; align-items: center; justify-content: center; padding: 1.5rem;",
            section {
                class: "md3-settings-section",
                style: "width: 100%; max-width: 32rem;",
                div {
                    h1 { class: "text-headline-small", "Database locked" }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Enter the password to decrypt the database for this session."
                    }
                }
                div { class: "md3-settings-field",
                    label {
                        class: "text-body-medium text-on-surface-variant",
                        r#for: "vault-password",
                        "Password"
                    }
                    div { class: "md3-text-field",
                        input {
                            id: "vault-password",
                            class: "md3-input",
                            r#type: "password",
                            autocomplete: "current-password",
                            autofocus: true,
                            value: "{password}",
                            disabled: *busy.read(),
                            oninput: move |event| password.set(event.value()),
                            onkeydown: move |event| {
                                if event.key() == Key::Enter {
                                    submit.call(());
                                }
                            },
                        }
                    }
                }
                if let Some(message) = error() {
                    p {
                        class: "text-body-small",
                        style: "color: var(--md3-color-error);",
                        role: "alert",
                        "{message}"
                    }
                }
                div { class: "md3-chip-group",
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *busy.read() || password().is_empty(),
                        onclick: move |_| submit.call(()),
                        if *busy.read() { "Unlocking…" } else { "Unlock" }
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *busy.read(),
                        onclick: move |_| reset_open.set(true),
                        "Forgot password?"
                    }
                }
            }
        }
        if reset_open() {
            ResetEncryptedDialog {
                on_done: move |saved| {
                    reset_open.set(false);
                    status.set(Some(saved));
                },
                on_cancel: move |_| reset_open.set(false),
            }
        }
    }
}

#[component]
fn ResetEncryptedDialog(
    on_done: EventHandler<EncryptionStatus>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut confirmation = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let done = on_done;

    let submit = EventHandler::new(move |_: ()| {
        if *busy.read() || confirmation().trim() != "ERASE" {
            return;
        }
        busy.set(true);
        spawn(async move {
            match reset_encrypted_database_api(confirmation()).await {
                Ok(saved) => done.call(saved),
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    rsx! {
        Modal {
            title: "Erase encrypted database",
            title_id: "vault-reset-title",
            dismissible: !*busy.read(),
            on_close: move |_| on_cancel.call(()),
            p { class: "text-body-medium text-on-surface-variant",
                "The password cannot be recovered. This permanently erases the encrypted database and starts a new unencrypted database. Any previously exported plaintext backup can be imported afterwards."
            }
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "vault-reset-confirm",
                "Type ERASE to confirm"
            }
            div { class: "md3-text-field",
                input {
                    id: "vault-reset-confirm",
                    class: "md3-input",
                    r#type: "text",
                    value: "{confirmation}",
                    disabled: *busy.read(),
                    oninput: move |event| confirmation.set(event.value()),
                }
            }
            if let Some(message) = error() {
                p { style: "color: var(--md3-color-error);", role: "alert", "{message}" }
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *busy.read(),
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *busy.read() || confirmation().trim() != "ERASE",
                    onclick: move |_| submit.call(()),
                    if *busy.read() { "Erasing…" } else { "Erase database" }
                }
            }
        }
    }
}

#[component]
fn SetupDialog(
    on_done: EventHandler<EncryptionStatus>,
    #[props(default)] on_cancel: Option<EventHandler<()>>,
    #[props(default)] on_dismiss: Option<EventHandler<EncryptionStatus>>,
) -> Element {
    let mut password = use_signal(String::new);
    let mut confirmation = use_signal(String::new);
    let mut acknowledged = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let done = on_done;
    let cancel = on_cancel;
    let dismiss = on_dismiss;
    let dismissible = cancel.is_some();

    let submit = EventHandler::new(move |_: ()| {
        if *busy.read()
            || password().is_empty()
            || password() != confirmation()
            || !*acknowledged.read()
        {
            return;
        }
        let password_value = password();
        password.set(String::new());
        confirmation.set(String::new());
        busy.set(true);
        spawn(async move {
            match setup_encryption_api(password_value).await {
                Ok(saved) => done.call(saved),
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    rsx! {
        Modal {
            title: "Encrypt your database",
            title_id: "vault-setup-title",
            dismissible,
            on_close: move |_| {
                if let Some(cancel) = cancel {
                    cancel.call(());
                }
            },
            p { class: "text-body-medium text-on-surface-variant",
                "Set a password to encrypt all data in the active database. The password is never stored. If you forget it, the only way to keep using Address Explorer is to erase the encrypted data and start over."
            }
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "vault-new-password",
                "Password"
            }
            div { class: "md3-text-field",
                input {
                    id: "vault-new-password",
                    class: "md3-input",
                    r#type: "password",
                    autocomplete: "new-password",
                    autofocus: true,
                    value: "{password}",
                    disabled: *busy.read(),
                    oninput: move |event| password.set(event.value()),
                }
            }
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "vault-confirm-password",
                "Confirm password"
            }
            div { class: "md3-text-field",
                input {
                    id: "vault-confirm-password",
                    class: "md3-input",
                    r#type: "password",
                    autocomplete: "new-password",
                    value: "{confirmation}",
                    disabled: *busy.read(),
                    oninput: move |event| confirmation.set(event.value()),
                    onkeydown: move |event| {
                        if event.key() == Key::Enter {
                            submit.call(());
                        }
                    },
                }
            }
            Checkbox {
                checked: *acknowledged.read(),
                disabled: *busy.read(),
                label: "I understand that losing this password means the encrypted data cannot be recovered.".to_string(),
                on_change: move |checked| acknowledged.set(checked),
            }
            if !password().is_empty() && password() != confirmation() {
                p {
                    class: "text-body-small",
                    style: "color: var(--md3-color-error);",
                    role: "alert",
                    "The passwords do not match."
                }
            }
            if let Some(message) = error() {
                p { style: "color: var(--md3-color-error);", role: "alert", "{message}" }
            }
            div { class: "md3-dialog-actions",
                if let Some(cancel) = cancel {
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *busy.read(),
                        onclick: move |_| cancel.call(()),
                        "Cancel"
                    }
                } else if let Some(dismiss) = dismiss {
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *busy.read(),
                        onclick: move |_| {
                            spawn(async move {
                                match dismiss_encryption_prompt_api().await {
                                    Ok(saved) => dismiss.call(saved),
                                    Err(failure) => error.set(Some(failure.to_string())),
                                }
                            });
                        },
                        "Continue without encryption"
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *busy.read()
                        || password().is_empty()
                        || password() != confirmation()
                        || !*acknowledged.read(),
                    onclick: move |_| submit.call(()),
                    if *busy.read() { "Encrypting…" } else { "Encrypt database" }
                }
            }
        }
    }
}

#[component]
fn ChangePasswordDialog(
    on_done: EventHandler<EncryptionStatus>,
    on_cancel: EventHandler<()>,
) -> Element {
    let mut current = use_signal(String::new);
    let mut new_password = use_signal(String::new);
    let mut confirmation = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let done = on_done;

    let submit = EventHandler::new(move |_: ()| {
        if *busy.read()
            || current().is_empty()
            || new_password().is_empty()
            || new_password() != confirmation()
        {
            return;
        }
        let current_value = current();
        let new_value = new_password();
        current.set(String::new());
        new_password.set(String::new());
        confirmation.set(String::new());
        busy.set(true);
        spawn(async move {
            match change_encryption_password_api(current_value, new_value).await {
                Ok(saved) => done.call(saved),
                Err(failure) => error.set(Some(failure.to_string())),
            }
            busy.set(false);
        });
    });

    rsx! {
        Modal {
            title: "Change password",
            title_id: "vault-change-title",
            dismissible: !*busy.read(),
            on_close: move |_| on_cancel.call(()),
            p { class: "text-body-medium text-on-surface-variant",
                "The database will be re-encrypted with the new password. The old password stops working immediately."
            }
            label { class: "text-body-medium text-on-surface-variant", r#for: "vault-current-password", "Current password" }
            div { class: "md3-text-field",
                input {
                    id: "vault-current-password",
                    class: "md3-input",
                    r#type: "password",
                    autocomplete: "current-password",
                    autofocus: true,
                    value: "{current}",
                    disabled: *busy.read(),
                    oninput: move |event| current.set(event.value()),
                }
            }
            label { class: "text-body-medium text-on-surface-variant", r#for: "vault-change-new", "New password" }
            div { class: "md3-text-field",
                input {
                    id: "vault-change-new",
                    class: "md3-input",
                    r#type: "password",
                    autocomplete: "new-password",
                    value: "{new_password}",
                    disabled: *busy.read(),
                    oninput: move |event| new_password.set(event.value()),
                }
            }
            label { class: "text-body-medium text-on-surface-variant", r#for: "vault-change-confirm", "Confirm new password" }
            div { class: "md3-text-field",
                input {
                    id: "vault-change-confirm",
                    class: "md3-input",
                    r#type: "password",
                    autocomplete: "new-password",
                    value: "{confirmation}",
                    disabled: *busy.read(),
                    oninput: move |event| confirmation.set(event.value()),
                }
            }
            if let Some(message) = error() {
                p { style: "color: var(--md3-color-error);", role: "alert", "{message}" }
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *busy.read(),
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *busy.read()
                        || current().is_empty()
                        || new_password().is_empty()
                        || new_password() != confirmation(),
                    onclick: move |_| submit.call(()),
                    if *busy.read() { "Changing…" } else { "Change password" }
                }
            }
        }
    }
}

/// Encryption controls shown as their own section in Settings → General.
#[component]
pub fn EncryptionSettings() -> Element {
    let context = use_encryption();
    let mut status_signal = context.status;
    let status = status_signal.read().clone();
    let toast = use_toast();
    let mut setup_open = use_signal(|| false);
    let mut change_open = use_signal(|| false);
    let mut saving = use_signal(|| false);

    let Some(current) = status else {
        return rsx! {
            p { class: "text-body-small text-on-surface-variant", "Loading encryption status…" }
        };
    };

    let select_timeout = move |event: FormEvent| {
        let Some(next) = IdleTimeout::from_value(&event.value()) else {
            return;
        };
        if *saving.read() {
            return;
        }
        saving.set(true);
        let mut toast = toast;
        spawn(async move {
            match save_encryption_idle_timeout_api(next).await {
                Ok(saved) => {
                    status_signal.set(Some(saved));
                    toast.success("Idle-lock timeout saved.");
                }
                Err(failure) => {
                    toast.error(format!("Could not save the idle-lock timeout: {failure}"));
                }
            }
            saving.set(false);
        });
    };

    let lock_now = move |_| {
        if *saving.read() {
            return;
        }
        saving.set(true);
        let mut toast = toast;
        spawn(async move {
            match lock_encryption_api().await {
                Ok(saved) => {
                    status_signal.set(Some(saved));
                    toast.success("Database locked.");
                }
                Err(failure) => toast.error(format!("Could not lock the database: {failure}")),
            }
            saving.set(false);
        });
    };

    let configured = current.configured;
    rsx! {
        div {
            h3 { class: "text-title-medium leading-title-medium text-on-surface",
                "Encryption"
            }
            p { class: "text-body-medium text-on-surface-variant",
                if configured {
                    "The active database is encrypted at rest. The password is never stored; losing it requires erasing the encrypted data."
                } else {
                    "Encryption is off. Anyone with access to the active database can read it."
                }
            }
            p {
                class: "text-body-small text-on-surface-variant",
                role: "status",
                aria_live: "polite",
                if configured { "Encryption is on." } else { "Encryption is off." }
            }
        }
        div { class: "md3-settings-field",
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "encryption-idle-timeout",
                "Lock after inactivity"
            }
            div { class: "md3-text-field md3-settings-control",
                select {
                    id: "encryption-idle-timeout",
                    class: "md3-input",
                    value: "{current.idle_timeout.as_str()}",
                    disabled: *saving.read() || !configured,
                    onchange: select_timeout,
                    for timeout in IdleTimeout::ALL {
                        option { value: "{timeout.as_str()}", "{timeout.label()}" }
                    }
                }
            }
        }
        div { class: "md3-chip-group",
            if configured {
                button {
                    r#type: "button",
                    class: "md3-button-outlined",
                    disabled: *saving.read(),
                    onclick: move |_| change_open.set(true),
                    "Change password"
                }
                button {
                    r#type: "button",
                    class: "md3-button-outlined",
                    disabled: *saving.read(),
                    onclick: lock_now,
                    "Lock now"
                }
            } else {
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *saving.read(),
                    onclick: move |_| setup_open.set(true),
                    "Enable encryption"
                }
            }
        }
        if setup_open() {
            SetupDialog {
                on_done: move |saved| {
                    setup_open.set(false);
                    status_signal.set(Some(saved));
                },
                on_cancel: move |_| setup_open.set(false),
            }
        }
        if change_open() {
            ChangePasswordDialog {
                on_done: move |saved| {
                    change_open.set(false);
                    status_signal.set(Some(saved));
                },
                on_cancel: move |_| change_open.set(false),
            }
        }
    }
}

async fn sleep_ms(milliseconds: u32) {
    let script = format!("setTimeout(() => dioxus.send(true), {milliseconds})");
    let mut eval = document::eval(&script);
    let _ = eval.recv::<bool>().await;
}
