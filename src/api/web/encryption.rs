//! Browser wrappers for the encryption endpoints.

use crate::domain::encryption::{EncryptionStatus, IdleTimeout};
use dioxus_fullstack_core::ServerFnError;
use zeroize::Zeroizing;

fn password_transport_error() -> Option<String> {
    crate::backend::remote_password_transport_error()
}

pub async fn get_encryption_status_api() -> Result<EncryptionStatus, ServerFnError> {
    crate::web_get_query!("encryption.status", "/api/encryption/status", {})
}

pub async fn setup_encryption_api(password: String) -> Result<EncryptionStatus, ServerFnError> {
    let password = Zeroizing::new(password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    crate::web_post_body!(
        "encryption.setup",
        "/api/encryption/setup",
        { "password": password.as_str() }
    )
}

pub async fn unlock_encryption_api(password: String) -> Result<EncryptionStatus, ServerFnError> {
    let password = Zeroizing::new(password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    crate::web_post_body!(
        "encryption.unlock",
        "/api/encryption/unlock",
        { "password": password.as_str() }
    )
}

pub async fn lock_encryption_api() -> Result<EncryptionStatus, ServerFnError> {
    crate::web_post_body!("encryption.lock", "/api/encryption/lock", {})
}

pub async fn touch_encryption_api() -> Result<(), ServerFnError> {
    crate::web_post_body!("encryption.touch", "/api/encryption/touch", {})
}

pub async fn change_encryption_password_api(
    current_password: String,
    new_password: String,
) -> Result<EncryptionStatus, ServerFnError> {
    let current_password = Zeroizing::new(current_password);
    let new_password = Zeroizing::new(new_password);
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    crate::web_post_body!(
        "encryption.change_password",
        "/api/encryption/change-password",
        { "current_password": current_password.as_str(), "new_password": new_password.as_str() }
    )
}

pub async fn save_encryption_idle_timeout_api(
    timeout: IdleTimeout,
) -> Result<EncryptionStatus, ServerFnError> {
    crate::web_post_body!(
        "encryption.save_idle_timeout",
        "/api/encryption/idle-timeout",
        { "timeout": timeout }
    )
}

pub async fn dismiss_encryption_prompt_api() -> Result<EncryptionStatus, ServerFnError> {
    crate::web_post_body!(
        "encryption.dismiss_prompt",
        "/api/encryption/dismiss-prompt",
        {}
    )
}

pub async fn reset_encrypted_database_api(
    confirmation: String,
) -> Result<EncryptionStatus, ServerFnError> {
    crate::web_post_body!(
        "encryption.reset",
        "/api/encryption/reset",
        { "confirmation": confirmation }
    )
}
