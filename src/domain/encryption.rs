//! Database encryption preferences and status shared by every backend.
//!
//! The password itself never appears in these types. The vault holds the
//! Argon2id-derived key in zeroizing memory only while the database is
//! unlocked.

use serde::{Deserialize, Serialize};

/// How long an unlocked database may stay idle before the key is discarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IdleTimeout {
    OneMinute,
    FiveMinutes,
    TenMinutes,
    #[default]
    OneHour,
    SixHours,
    TwentyFourHours,
    Never,
}

impl IdleTimeout {
    pub const ALL: [Self; 7] = [
        Self::OneMinute,
        Self::FiveMinutes,
        Self::TenMinutes,
        Self::OneHour,
        Self::SixHours,
        Self::TwentyFourHours,
        Self::Never,
    ];

    /// Stable value stored in the `setting` table and sent over the API.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneMinute => "1m",
            Self::FiveMinutes => "5m",
            Self::TenMinutes => "10m",
            Self::OneHour => "1h",
            Self::SixHours => "6h",
            Self::TwentyFourHours => "24h",
            Self::Never => "never",
        }
    }

    /// User-facing option label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::OneMinute => "1 minute",
            Self::FiveMinutes => "5 minutes",
            Self::TenMinutes => "10 minutes",
            Self::OneHour => "1 hour",
            Self::SixHours => "6 hours",
            Self::TwentyFourHours => "24 hours",
            Self::Never => "Never",
        }
    }

    /// Idle duration in seconds, or `None` when the vault never auto-locks.
    #[cfg_attr(not(any(test, feature = "server")), allow(dead_code))]
    pub const fn as_secs(self) -> Option<u64> {
        match self {
            Self::OneMinute => Some(60),
            Self::FiveMinutes => Some(5 * 60),
            Self::TenMinutes => Some(10 * 60),
            Self::OneHour => Some(60 * 60),
            Self::SixHours => Some(6 * 60 * 60),
            Self::TwentyFourHours => Some(24 * 60 * 60),
            Self::Never => None,
        }
    }

    /// Parse a value previously written by [`Self::as_str`].
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|timeout| timeout.as_str() == value)
    }
}

/// Encryption state of the active backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncryptionStatus {
    /// Whether the active database is encrypted at rest.
    pub configured: bool,
    /// Whether the encrypted database is waiting for its password.
    pub locked: bool,
    /// Current idle-lock preference.
    pub idle_timeout: IdleTimeout,
    /// Whether the first-run encryption prompt was already answered.
    pub prompt_seen: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_values_round_trip() {
        for timeout in IdleTimeout::ALL {
            assert_eq!(IdleTimeout::from_value(timeout.as_str()), Some(timeout));
        }
        assert_eq!(IdleTimeout::from_value("2h"), None);
    }

    #[test]
    fn default_timeout_is_one_hour() {
        assert_eq!(IdleTimeout::default(), IdleTimeout::OneHour);
        assert_eq!(IdleTimeout::default().as_secs(), Some(3600));
        assert_eq!(IdleTimeout::Never.as_secs(), None);
    }
}
