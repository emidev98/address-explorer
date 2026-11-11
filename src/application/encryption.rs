//! Encryption service: password setup, unlock/lock, idle timeout, and the
//! destructive lost-password reset.

use crate::data::encryption::EncryptionError;
use crate::data::vault;
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};

pub type EncryptionResult = Result<EncryptionStatus, EncryptionError>;

pub fn status() -> EncryptionResult {
    vault::status()
}

pub fn setup(password: &str) -> EncryptionResult {
    vault::setup(password)
}

pub fn unlock(password: &str) -> EncryptionResult {
    vault::unlock(password)
}

pub fn lock() -> EncryptionResult {
    vault::lock()
}

pub fn touch() -> Result<(), EncryptionError> {
    vault::touch()
}

pub fn change_password(current_password: &str, new_password: &str) -> EncryptionResult {
    vault::change_password(current_password, new_password)
}

pub fn save_idle_timeout(timeout: IdleTimeout) -> EncryptionResult {
    vault::save_idle_timeout(timeout)
}

pub fn dismiss_prompt() -> EncryptionResult {
    vault::dismiss_prompt()
}

pub fn reset() -> EncryptionResult {
    vault::reset()
}
