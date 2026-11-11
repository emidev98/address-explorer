//! Encryption endpoints for the active local or remote database.

#[cfg(feature = "server")]
use crate::application::encryption;
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};
use dioxus::prelude::*;
use zeroize::Zeroizing;

fn password_transport_error() -> Option<String> {
    crate::backend::remote_password_transport_error()
}

/// Read the active database encryption status.
#[server]
#[get("/api/encryption/status")]
pub async fn get_encryption_status_api() -> Result<EncryptionStatus, ServerFnError> {
    remote_get!("/api/encryption/status", [], []);
    encryption::status().map_err(|error| ServerFnError::new(error.to_string()))
}

/// Encrypt the active plaintext database with a new password.
#[server]
#[post("/api/encryption/setup")]
pub async fn setup_encryption_api(password: String) -> Result<EncryptionStatus, ServerFnError> {
    let password = Zeroizing::new(password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "remote-backend")]
    let password_value = password.as_str();
    remote_post!("/api/encryption/setup", [], [], [password_value]);
    encryption::setup(&password).map_err(|error| ServerFnError::new(error.to_string()))
}

/// Unlock the active encrypted database with the user's password.
#[server]
#[post("/api/encryption/unlock")]
pub async fn unlock_encryption_api(password: String) -> Result<EncryptionStatus, ServerFnError> {
    let password = Zeroizing::new(password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "remote-backend")]
    let password_value = password.as_str();
    remote_post!("/api/encryption/unlock", [], [], [password_value]);
    encryption::unlock(&password).map_err(|error| ServerFnError::new(error.to_string()))
}

/// Discard the in-memory key and lock the active database.
#[server]
#[post("/api/encryption/lock")]
pub async fn lock_encryption_api() -> Result<EncryptionStatus, ServerFnError> {
    remote_post!("/api/encryption/lock", [], [], []);
    encryption::lock().map_err(|error| ServerFnError::new(error.to_string()))
}

/// Extend the idle timer after user activity.
#[server]
#[post("/api/encryption/touch")]
pub async fn touch_encryption_api() -> Result<(), ServerFnError> {
    remote_post!("/api/encryption/touch", [], [], []);
    encryption::touch().map_err(|error| ServerFnError::new(error.to_string()))
}

/// Re-encrypt the database under a new password.
#[server]
#[post("/api/encryption/change-password")]
pub async fn change_encryption_password_api(
    current_password: String,
    new_password: String,
) -> Result<EncryptionStatus, ServerFnError> {
    let current_password = Zeroizing::new(current_password);
    let new_password = Zeroizing::new(new_password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "remote-backend")]
    let current_value = current_password.as_str();
    #[cfg(feature = "remote-backend")]
    let new_value = new_password.as_str();
    remote_post!(
        "/api/encryption/change-password",
        [],
        [],
        [current_value, new_value]
    );
    encryption::change_password(&current_password, &new_password)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist the idle-lock preference.
#[server]
#[post("/api/encryption/idle-timeout")]
pub async fn save_encryption_idle_timeout_api(
    timeout: IdleTimeout,
) -> Result<EncryptionStatus, ServerFnError> {
    remote_post!("/api/encryption/idle-timeout", [], [], [timeout]);
    encryption::save_idle_timeout(timeout).map_err(|error| ServerFnError::new(error.to_string()))
}

/// Record that the user declined encryption on first run.
#[server]
#[post("/api/encryption/dismiss-prompt")]
pub async fn dismiss_encryption_prompt_api() -> Result<EncryptionStatus, ServerFnError> {
    remote_post!("/api/encryption/dismiss-prompt", [], [], []);
    encryption::dismiss_prompt().map_err(|error| ServerFnError::new(error.to_string()))
}

/// Erase the active database and remove encryption. The typed confirmation
/// prevents accidental invocation; there is no password recovery path.
#[server]
#[post("/api/encryption/reset")]
pub async fn reset_encrypted_database_api(
    confirmation: String,
) -> Result<EncryptionStatus, ServerFnError> {
    if confirmation.trim() != "ERASE" {
        return Err(ServerFnError::new(
            "type ERASE to confirm erasing the encrypted database",
        ));
    }
    remote_post!("/api/encryption/reset", [], [], [confirmation]);
    encryption::reset().map_err(|error| ServerFnError::new(error.to_string()))
}
