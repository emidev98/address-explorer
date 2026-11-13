//! Configurable security policy shared by every backend.
//!
//! The compiled defaults are deliberately the recommended settings. A database
//! may store stricter or slightly more permissive values inside the safe
//! ranges, but it can never turn off the protections required by the security
//! contract.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Permission mode for exported files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExportFileMode {
    /// Owner read/write (0600).
    #[default]
    OwnerReadWrite,
    /// Owner read-only after creation (0400).
    OwnerReadOnly,
}

impl ExportFileMode {
    pub const ALL: [Self; 2] = [Self::OwnerReadWrite, Self::OwnerReadOnly];

    pub const fn label(self) -> &'static str {
        match self {
            Self::OwnerReadWrite => "Owner read/write (0600)",
            Self::OwnerReadOnly => "Owner read-only after export (0400)",
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OwnerReadWrite => "owner_read_write",
            Self::OwnerReadOnly => "owner_read_only",
        }
    }

    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }
}

/// One validated security policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecuritySettings {
    pub password_min_length: u8,
    pub max_json_body_bytes: u64,
    pub max_backup_bytes: u64,
    pub session_token_bytes: u8,
    pub max_concurrent_argon2: u8,
    pub unlock_failure_threshold: u32,
    pub unlock_backoff_secs: u64,
    pub api_requests_per_minute: u32,
    pub api_rate_burst: u32,
    pub external_timeout_secs: u64,
    pub external_response_max_bytes: u64,
    pub export_file_mode: ExportFileMode,
    pub stale_temp_cleanup_hours: u64,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            password_min_length: 12,
            max_json_body_bytes: 1024 * 1024,
            max_backup_bytes: 128 * 1024 * 1024,
            session_token_bytes: 32,
            max_concurrent_argon2: 1,
            unlock_failure_threshold: 5,
            unlock_backoff_secs: 30,
            api_requests_per_minute: 60,
            api_rate_burst: 20,
            external_timeout_secs: 30,
            external_response_max_bytes: 8 * 1024 * 1024,
            export_file_mode: ExportFileMode::OwnerReadWrite,
            stale_temp_cleanup_hours: 1,
        }
    }
}

impl SecuritySettings {
    /// Fill missing or unknown fields with defaults. Used when reading older
    /// rows and when the stored JSON is incomplete.
    pub fn with_defaults(mut self) -> Self {
        let defaults = Self::default();
        if !(MIN_PASSWORD_MIN_LENGTH..=MAX_PASSWORD_MIN_LENGTH)
            .contains(&(self.password_min_length as u64))
        {
            self.password_min_length = defaults.password_min_length;
        }
        self.max_json_body_bytes = clamp(
            self.max_json_body_bytes,
            MIN_JSON_BODY_BYTES,
            MAX_JSON_BODY_BYTES,
        );
        self.max_backup_bytes = clamp(self.max_backup_bytes, MIN_BACKUP_BYTES, MAX_BACKUP_BYTES);
        self.session_token_bytes =
            clamp(self.session_token_bytes, MIN_TOKEN_BYTES, MAX_TOKEN_BYTES);
        self.max_concurrent_argon2 = clamp(
            self.max_concurrent_argon2,
            MIN_ARGON2_CONCURRENCY,
            MAX_ARGON2_CONCURRENCY,
        );
        self.unlock_failure_threshold = clamp(
            self.unlock_failure_threshold,
            MIN_UNLOCK_FAILURES,
            MAX_UNLOCK_FAILURES,
        );
        self.unlock_backoff_secs = clamp(
            self.unlock_backoff_secs,
            MIN_UNLOCK_BACKOFF_SECS,
            MAX_UNLOCK_BACKOFF_SECS,
        );
        self.api_requests_per_minute = clamp(
            self.api_requests_per_minute,
            MIN_API_REQUESTS_PER_MINUTE,
            MAX_API_REQUESTS_PER_MINUTE,
        );
        self.api_rate_burst = clamp(self.api_rate_burst, MIN_API_BURST, MAX_API_BURST);
        self.external_timeout_secs = clamp(
            self.external_timeout_secs,
            MIN_EXTERNAL_TIMEOUT_SECS,
            MAX_EXTERNAL_TIMEOUT_SECS,
        );
        self.external_response_max_bytes = clamp(
            self.external_response_max_bytes,
            MIN_RESPONSE_BYTES,
            MAX_RESPONSE_BYTES,
        );
        self.stale_temp_cleanup_hours = clamp(
            self.stale_temp_cleanup_hours,
            MIN_STALE_TEMP_HOURS,
            MAX_STALE_TEMP_HOURS,
        );
        self
    }

    /// Reject values outside the safe ranges. Called by every save path so a
    /// malformed client cannot persist an unsafe policy.
    pub fn validate(&self) -> Result<(), SecuritySettingsError> {
        check_range(
            "password minimum length",
            self.password_min_length as u64,
            MIN_PASSWORD_MIN_LENGTH,
            MAX_PASSWORD_MIN_LENGTH,
        )?;
        check_range(
            "normal API body limit",
            self.max_json_body_bytes,
            MIN_JSON_BODY_BYTES,
            MAX_JSON_BODY_BYTES,
        )?;
        check_range(
            "maximum imported backup",
            self.max_backup_bytes,
            MIN_BACKUP_BYTES,
            MAX_BACKUP_BYTES,
        )?;
        check_range(
            "session token length",
            self.session_token_bytes as u64,
            MIN_TOKEN_BYTES as u64,
            MAX_TOKEN_BYTES as u64,
        )?;
        check_range(
            "concurrent Argon2 operations",
            self.max_concurrent_argon2 as u64,
            MIN_ARGON2_CONCURRENCY as u64,
            MAX_ARGON2_CONCURRENCY as u64,
        )?;
        check_range(
            "failed-unlock threshold",
            self.unlock_failure_threshold as u64,
            MIN_UNLOCK_FAILURES as u64,
            MAX_UNLOCK_FAILURES as u64,
        )?;
        check_range(
            "unlock backoff",
            self.unlock_backoff_secs,
            MIN_UNLOCK_BACKOFF_SECS,
            MAX_UNLOCK_BACKOFF_SECS,
        )?;
        check_range(
            "API requests per minute",
            self.api_requests_per_minute as u64,
            MIN_API_REQUESTS_PER_MINUTE as u64,
            MAX_API_REQUESTS_PER_MINUTE as u64,
        )?;
        check_range(
            "API rate burst",
            self.api_rate_burst as u64,
            MIN_API_BURST as u64,
            MAX_API_BURST as u64,
        )?;
        check_range(
            "external request timeout",
            self.external_timeout_secs,
            MIN_EXTERNAL_TIMEOUT_SECS,
            MAX_EXTERNAL_TIMEOUT_SECS,
        )?;
        check_range(
            "external response cap",
            self.external_response_max_bytes,
            MIN_RESPONSE_BYTES,
            MAX_RESPONSE_BYTES,
        )?;
        check_range(
            "stale temporary-file age",
            self.stale_temp_cleanup_hours,
            MIN_STALE_TEMP_HOURS,
            MAX_STALE_TEMP_HOURS,
        )?;
        Ok(())
    }

    /// Largest JSON request body that can carry the configured backup.
    pub fn max_backup_request_bytes(&self) -> u64 {
        let base64 = self.max_backup_bytes.saturating_add(2) / 3 * 4;
        base64.saturating_add(64 * 1024)
    }
}

/// Validation failure of one or more security policy fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecuritySettingsError {
    Message(String),
}

impl std::fmt::Display for SecuritySettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Message(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for SecuritySettingsError {}

fn clamp<T: Ord>(value: T, min: T, max: T) -> T {
    value.max(min).min(max)
}

fn check_range(field: &str, value: u64, min: u64, max: u64) -> Result<(), SecuritySettingsError> {
    if (min..=max).contains(&value) {
        return Ok(());
    }
    Err(SecuritySettingsError::Message(format!(
        "the {field} must be between {min} and {max}"
    )))
}

/// Enforce the configured password length without imposing composition rules.
pub fn validate_password(password: &str, min_length: u8) -> Result<(), String> {
    if password.trim().is_empty() {
        return Err("the password must not be empty".to_string());
    }
    if password.chars().count() < min_length as usize {
        return Err(format!(
            "the password must contain at least {min_length} characters"
        ));
    }
    if password.len() > 1024 {
        return Err("the password must not exceed 1024 bytes".to_string());
    }
    Ok(())
}

pub const MIN_PASSWORD_MIN_LENGTH: u64 = 12;
pub const MAX_PASSWORD_MIN_LENGTH: u64 = 64;
pub const MIN_JSON_BODY_BYTES: u64 = 64 * 1024;
pub const MAX_JSON_BODY_BYTES: u64 = 16 * 1024 * 1024;
pub const MIN_BACKUP_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_BACKUP_BYTES: u64 = 512 * 1024 * 1024;
pub const MIN_TOKEN_BYTES: u8 = 32;
pub const MAX_TOKEN_BYTES: u8 = 64;
pub const MIN_ARGON2_CONCURRENCY: u8 = 1;
pub const MAX_ARGON2_CONCURRENCY: u8 = 4;
pub const MIN_UNLOCK_FAILURES: u32 = 3;
pub const MAX_UNLOCK_FAILURES: u32 = 10;
pub const MIN_UNLOCK_BACKOFF_SECS: u64 = 5;
pub const MAX_UNLOCK_BACKOFF_SECS: u64 = 300;
pub const MIN_API_REQUESTS_PER_MINUTE: u32 = 10;
pub const MAX_API_REQUESTS_PER_MINUTE: u32 = 600;
pub const MIN_API_BURST: u32 = 5;
pub const MAX_API_BURST: u32 = 100;
pub const MIN_EXTERNAL_TIMEOUT_SECS: u64 = 5;
pub const MAX_EXTERNAL_TIMEOUT_SECS: u64 = 120;
pub const MIN_RESPONSE_BYTES: u64 = 1024 * 1024;
pub const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
pub const MIN_STALE_TEMP_HOURS: u64 = 1;
pub const MAX_STALE_TEMP_HOURS: u64 = 168;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_security_contract() {
        let settings = SecuritySettings::default();
        assert_eq!(settings.password_min_length, 12);
        assert_eq!(settings.max_json_body_bytes, 1024 * 1024);
        assert_eq!(settings.max_backup_bytes, 128 * 1024 * 1024);
        assert_eq!(settings.session_token_bytes, 32);
        assert_eq!(settings.max_concurrent_argon2, 1);
        assert_eq!(settings.unlock_failure_threshold, 5);
        assert_eq!(settings.unlock_backoff_secs, 30);
        assert_eq!(settings.external_timeout_secs, 30);
        assert_eq!(settings.external_response_max_bytes, 8 * 1024 * 1024);
        assert_eq!(settings.export_file_mode, ExportFileMode::OwnerReadWrite);
        assert_eq!(settings.stale_temp_cleanup_hours, 1);
    }

    #[test]
    fn validation_rejects_unsafe_values() {
        assert!(SecuritySettings {
            password_min_length: 8,
            ..SecuritySettings::default()
        }
        .validate()
        .is_err());
        assert!(SecuritySettings {
            session_token_bytes: 16,
            ..SecuritySettings::default()
        }
        .validate()
        .is_err());
        assert!(SecuritySettings {
            max_backup_bytes: 2 * 1024 * 1024 * 1024,
            ..SecuritySettings::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn loading_clamps_corrupt_values() {
        let settings = SecuritySettings {
            password_min_length: 1,
            max_json_body_bytes: u64::MAX,
            max_backup_bytes: 1,
            session_token_bytes: 1,
            max_concurrent_argon2: 99,
            unlock_failure_threshold: 0,
            unlock_backoff_secs: 1,
            api_requests_per_minute: 1,
            api_rate_burst: 1,
            external_timeout_secs: 1,
            external_response_max_bytes: 1,
            export_file_mode: ExportFileMode::OwnerReadOnly,
            stale_temp_cleanup_hours: 0,
        }
        .with_defaults();
        assert_eq!(settings.password_min_length, 12);
        assert_eq!(settings.max_json_body_bytes, MAX_JSON_BODY_BYTES);
        assert_eq!(settings.max_backup_bytes, MIN_BACKUP_BYTES);
        assert_eq!(settings.session_token_bytes, MIN_TOKEN_BYTES);
        assert_eq!(settings.max_concurrent_argon2, MAX_ARGON2_CONCURRENCY);
        assert_eq!(settings.unlock_failure_threshold, MIN_UNLOCK_FAILURES);
        assert_eq!(settings.stale_temp_cleanup_hours, MIN_STALE_TEMP_HOURS);
    }

    #[test]
    fn password_policy_enforces_length_without_composition_rules() {
        assert!(validate_password("short", 12).is_err());
        assert!(validate_password("twelve chars", 12).is_ok());
        assert!(validate_password("            ", 12).is_err());
        assert!(validate_password(&"x".repeat(1025), 12).is_err());
    }

    #[test]
    fn backup_request_cap_covers_base64_overhead() {
        let settings = SecuritySettings::default();
        assert!(settings.max_backup_request_bytes() > settings.max_backup_bytes);
    }

    #[test]
    fn older_policy_json_with_the_removed_server_timeout_still_loads() {
        let mut value = serde_json::to_value(SecuritySettings::default()).unwrap(); // allow-unwrap: test setup.
        value["server_idle_timeout_max"] = serde_json::json!("never");
        let parsed: SecuritySettings = serde_json::from_value(value).unwrap(); // allow-unwrap: test setup.
        assert_eq!(parsed, SecuritySettings::default());
    }
}
