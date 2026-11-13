//! Database encryption preferences and status shared by every backend.
//!
//! The password itself never appears in these types. The vault holds the
//! Argon2id-derived key in zeroizing memory only while the database is
//! unlocked.

use serde::{Deserialize, Serialize};

/// How long an unlocked database may stay idle before the key is discarded.
///
/// The same value also bounds the remote server session, so there is exactly
/// one inactivity timer for the application. `Never` is intentionally not an
/// option: every build must lock eventually.
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
}

#[allow(dead_code)]
impl IdleTimeout {
    pub const ALL: [Self; 6] = [
        Self::OneMinute,
        Self::FiveMinutes,
        Self::TenMinutes,
        Self::OneHour,
        Self::SixHours,
        Self::TwentyFourHours,
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
        }
    }

    /// Whether a network server may serve this timeout. A server must never
    /// stay unlocked for longer than one hour.
    pub const fn is_server_allowed(self) -> bool {
        matches!(
            self,
            Self::OneMinute | Self::FiveMinutes | Self::TenMinutes | Self::OneHour
        )
    }

    /// The value a network server actually applies. A database carried over
    /// from a local build keeps its stored preference but is never served
    /// above the server ceiling.
    pub const fn for_network_server(self) -> Self {
        if self.is_server_allowed() {
            self
        } else {
            Self::OneHour
        }
    }

    /// Idle duration in seconds.
    #[cfg_attr(not(any(test, feature = "server")), allow(dead_code))]
    pub const fn as_secs(self) -> u64 {
        match self {
            Self::OneMinute => 60,
            Self::FiveMinutes => 5 * 60,
            Self::TenMinutes => 10 * 60,
            Self::OneHour => 60 * 60,
            Self::SixHours => 6 * 60 * 60,
            Self::TwentyFourHours => 24 * 60 * 60,
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
    /// Whether the active database has a password and is encrypted at rest.
    pub configured: bool,
    /// Whether the encrypted database is waiting for its password.
    pub locked: bool,
    /// Current inactivity timeout for the database key and the server session.
    pub idle_timeout: IdleTimeout,
    /// Whether this backend is a network server, which caps the timeout at
    /// one hour and refuses larger values.
    pub server_capped: bool,
}

/// Result of setup, unlock, change-password, and reset.
///
/// The token is present only for a pure server. Local desktop, mobile, and the
/// browser worker return `None` because their server functions execute
/// in-process and do not have a network session to authorize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlockResult {
    pub status: EncryptionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
}

#[allow(dead_code)]
impl UnlockResult {
    pub fn local(status: EncryptionStatus) -> Self {
        Self {
            status,
            session_token: None,
        }
    }

    pub fn with_token(status: EncryptionStatus, token: String) -> Self {
        Self {
            status,
            session_token: Some(token),
        }
    }
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
        assert_eq!(IdleTimeout::from_value("never"), None);
    }

    #[test]
    fn default_timeout_is_one_hour() {
        assert_eq!(IdleTimeout::default(), IdleTimeout::OneHour);
        assert_eq!(IdleTimeout::default().as_secs(), 3600);
    }

    #[test]
    fn network_servers_are_capped_at_one_hour() {
        for timeout in IdleTimeout::ALL {
            let applied = timeout.for_network_server();
            assert!(applied.is_server_allowed());
            assert!(applied.as_secs() <= IdleTimeout::OneHour.as_secs());
        }
        assert_eq!(
            IdleTimeout::TwentyFourHours.for_network_server(),
            IdleTimeout::OneHour
        );
        assert_eq!(
            IdleTimeout::FiveMinutes.for_network_server(),
            IdleTimeout::FiveMinutes
        );
    }
}
