//! Encryption endpoints for the active local or remote database.

#[cfg(feature = "server")]
use crate::application::encryption;
use crate::domain::encryption::{EncryptionStatus, IdleTimeout, UnlockResult};
use crate::domain::secret::SecretString;
use dioxus::prelude::*;

fn password_transport_error() -> Option<String> {
    crate::backend::remote_password_transport_error()
}

#[cfg(feature = "server")]
fn finish(status: EncryptionStatus) -> Result<UnlockResult, ServerFnError> {
    if crate::backend::pure_server() {
        let token = crate::security::session::issue()
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        Ok(UnlockResult::with_token(status, token))
    } else {
        Ok(UnlockResult::local(status))
    }
}

#[cfg(feature = "server")]
async fn run_crypto<F>(operation: F) -> Result<EncryptionStatus, ServerFnError>
where
    F: FnOnce() -> Result<EncryptionStatus, crate::data::encryption::EncryptionError>
        + Send
        + 'static,
{
    let gate = crate::security::policy::argon2_gate();
    let permit = gate
        .acquire_owned()
        .await
        .map_err(|_| ServerFnError::new("the encryption work queue is unavailable"))?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        operation()
    })
    .await
    .map_err(|error| ServerFnError::new(format!("the encryption worker failed: {error}")))?
    .map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(feature = "server")]
fn check_unlock_backoff() -> Result<(), ServerFnError> {
    if !crate::backend::pure_server() {
        return Ok(());
    }
    crate::security::rate_limit::unlock_allowed().map_err(|seconds| {
        ServerFnError::new(format!(
            "too many failed unlock attempts; retry in {seconds} seconds"
        ))
    })
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
pub async fn setup_encryption_api(password: SecretString) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "server")]
    check_unlock_backoff()?;
    #[cfg(feature = "remote-backend")]
    let password_value = password.as_str();
    remote_post_session!("/api/encryption/setup", { "password": password_value });
    let result = {
        #[cfg(feature = "server")]
        {
            if crate::backend::pure_server() {
                run_crypto(move || encryption::setup(password.as_str())).await
            } else {
                encryption::setup(password.as_str())
                    .map_err(|error| ServerFnError::new(error.to_string()))
            }
        }
        #[cfg(not(feature = "server"))]
        {
            encryption::setup(password.as_str())
                .map_err(|error| ServerFnError::new(error.to_string()))
        }
    };
    let status = match result {
        Ok(status) => status,
        Err(error) => {
            #[cfg(feature = "server")]
            if crate::backend::pure_server() {
                crate::security::rate_limit::record_unlock_failure();
            }
            return Err(error);
        }
    };
    #[cfg(feature = "server")]
    if crate::backend::pure_server() {
        crate::security::rate_limit::record_unlock_success();
    }
    #[cfg(feature = "server")]
    return finish(status);
    #[cfg(not(feature = "server"))]
    Ok(UnlockResult::local(status))
}

/// Unlock the active encrypted database with the user's password.
#[server]
#[post("/api/encryption/unlock")]
pub async fn unlock_encryption_api(password: SecretString) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "server")]
    check_unlock_backoff()?;
    #[cfg(feature = "remote-backend")]
    let password_value = password.as_str();
    remote_post_session!("/api/encryption/unlock", { "password": password_value });
    let result = {
        #[cfg(feature = "server")]
        {
            if crate::backend::pure_server() {
                run_crypto(move || encryption::unlock(password.as_str())).await
            } else {
                encryption::unlock(password.as_str())
                    .map_err(|error| ServerFnError::new(error.to_string()))
            }
        }
        #[cfg(not(feature = "server"))]
        {
            encryption::unlock(password.as_str())
                .map_err(|error| ServerFnError::new(error.to_string()))
        }
    };
    let status = match result {
        Ok(status) => status,
        Err(error) => {
            #[cfg(feature = "server")]
            if crate::backend::pure_server() {
                crate::security::rate_limit::record_unlock_failure();
            }
            return Err(error);
        }
    };
    #[cfg(feature = "server")]
    if crate::backend::pure_server() {
        crate::security::rate_limit::record_unlock_success();
    }
    #[cfg(feature = "server")]
    return finish(status);
    #[cfg(not(feature = "server"))]
    Ok(UnlockResult::local(status))
}

/// Discard the in-memory key and lock the active database.
#[server]
#[post("/api/encryption/lock")]
pub async fn lock_encryption_api() -> Result<EncryptionStatus, ServerFnError> {
    remote_post!("/api/encryption/lock", [], [], []);
    encryption::lock().map_err(|error| ServerFnError::new(error.to_string()))
}

/// Extend the vault inactivity timer after user activity.
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
    current_password: SecretString,
    new_password: SecretString,
) -> Result<UnlockResult, ServerFnError> {
    if let Some(error) = password_transport_error() {
        return Err(ServerFnError::new(error));
    }
    #[cfg(feature = "remote-backend")]
    let current_value = current_password.as_str();
    #[cfg(feature = "remote-backend")]
    let new_value = new_password.as_str();
    remote_post_session!(
        "/api/encryption/change-password",
        { "current_password": current_value, "new_password": new_value }
    );
    let result = {
        #[cfg(feature = "server")]
        {
            if crate::backend::pure_server() {
                run_crypto(move || {
                    encryption::change_password(current_password.as_str(), new_password.as_str())
                })
                .await
            } else {
                encryption::change_password(current_password.as_str(), new_password.as_str())
                    .map_err(|error| ServerFnError::new(error.to_string()))
            }
        }
        #[cfg(not(feature = "server"))]
        {
            encryption::change_password(current_password.as_str(), new_password.as_str())
                .map_err(|error| ServerFnError::new(error.to_string()))
        }
    };
    let status = result?;
    #[cfg(feature = "server")]
    return finish(status);
    #[cfg(not(feature = "server"))]
    Ok(UnlockResult::local(status))
}

/// Persist the inactivity timeout for the vault key and the server session.
#[server]
#[post("/api/encryption/idle-timeout")]
pub async fn save_encryption_idle_timeout_api(
    timeout: IdleTimeout,
) -> Result<EncryptionStatus, ServerFnError> {
    remote_post!("/api/encryption/idle-timeout", [], [], [timeout]);
    encryption::save_idle_timeout(timeout).map_err(|error| ServerFnError::new(error.to_string()))
}

/// Erase user data while keeping encryption when the vault is unlocked.
///
/// A locked vault cannot be reset remotely; the server operator must run the
/// host CLI command. This removes the lost-password ransom window while still
/// honoring the typed ERASE confirmation.
#[server]
#[post("/api/encryption/reset")]
pub async fn reset_encrypted_database_api(
    confirmation: String,
) -> Result<UnlockResult, ServerFnError> {
    if confirmation.trim() != "ERASE" {
        return Err(ServerFnError::new(
            "type ERASE to confirm erasing the encrypted database",
        ));
    }
    #[cfg(feature = "remote-backend")]
    let confirmation_value = confirmation.as_str();
    remote_post_session!(
        "/api/encryption/reset",
        { "confirmation": confirmation_value }
    );
    #[cfg(feature = "server")]
    {
        let status = encryption::status().map_err(|error| ServerFnError::new(error.to_string()))?;
        if status.locked || !status.configured {
            return Err(ServerFnError::new(
                "reset_locked: run `address-explorer reset-vault` on the server host",
            ));
        }
        let status = encryption::reset_to_defaults()
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        finish(status)
    }
    #[cfg(not(feature = "server"))]
    {
        let status = encryption::reset().map_err(|error| ServerFnError::new(error.to_string()))?;
        Ok(UnlockResult::local(status))
    }
}
