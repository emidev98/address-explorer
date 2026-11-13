//! Browser wrappers for the encryption endpoints.

use crate::domain::encryption::{EncryptionStatus, IdleTimeout, UnlockResult};
use crate::domain::secret::SecretString;
use dioxus_fullstack_core::ServerFnError;

fn password_transport_error() -> Option<String> {
    crate::backend::remote_password_transport_error()
}

fn store_session(result: UnlockResult) -> UnlockResult {
    if let Some(token) = result.session_token.as_deref() {
        crate::backend::set_session_token(token);
    }
    result
}

pub async fn get_encryption_status_api() -> Result<EncryptionStatus, ServerFnError> {
    crate::web_get_query!("encryption.status", "/api/encryption/status", {})
}

pub async fn setup_encryption_api(password: SecretString) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    let result = crate::web_post_body!(
        "encryption.setup",
        "/api/encryption/setup",
        { "password": password.as_str() }
    )?;
    Ok(store_session(result))
}

pub async fn unlock_encryption_api(password: SecretString) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    let result = crate::web_post_body!(
        "encryption.unlock",
        "/api/encryption/unlock",
        { "password": password.as_str() }
    )?;
    Ok(store_session(result))
}

pub async fn lock_encryption_api() -> Result<EncryptionStatus, ServerFnError> {
    let result = crate::web_post_body!("encryption.lock", "/api/encryption/lock", {});
    if result.is_ok() {
        crate::backend::clear_session_token();
    }
    result
}

pub async fn touch_encryption_api() -> Result<(), ServerFnError> {
    crate::web_post_body!("encryption.touch", "/api/encryption/touch", {})
}

pub async fn change_encryption_password_api(
    current_password: SecretString,
    new_password: SecretString,
) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    let result = crate::web_post_body!(
        "encryption.change_password",
        "/api/encryption/change-password",
        { "current_password": current_password.as_str(), "new_password": new_password.as_str() }
    )?;
    Ok(store_session(result))
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

pub async fn reset_encrypted_database_api(
    confirmation: String,
) -> Result<UnlockResult, ServerFnError> {
    let result = crate::web_post_body!(
        "encryption.reset",
        "/api/encryption/reset",
        { "confirmation": confirmation }
    )?;
    Ok(store_session(result))
}
