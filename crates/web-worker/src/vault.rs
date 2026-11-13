//! Browser worker vault state. Only the Argon2id-derived key is retained while
//! unlocked; locking drops it and closes the SQLite connection.
//!
//! A browser database always requires a password, so the worker starts in
//! `NeedsSetup` and refuses data requests until setup completes.

use crate::data::encryption::{VaultKey, SALT_LEN};
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};

pub enum VaultMode {
    NeedsSetup,
    Locked { salt: [u8; SALT_LEN] },
    Unlocked { key: VaultKey, salt: [u8; SALT_LEN] },
}

pub struct WorkerVault {
    mode: VaultMode,
    idle_timeout: IdleTimeout,
    last_activity_ms: f64,
}

impl Default for WorkerVault {
    fn default() -> Self {
        Self {
            mode: VaultMode::NeedsSetup,
            idle_timeout: IdleTimeout::default(),
            last_activity_ms: now_ms(),
        }
    }
}

impl WorkerVault {
    pub fn status(&self) -> EncryptionStatus {
        EncryptionStatus {
            configured: !matches!(self.mode, VaultMode::NeedsSetup),
            locked: matches!(self.mode, VaultMode::Locked { .. }),
            idle_timeout: self.idle_timeout,
            server_capped: false,
        }
    }

    pub fn locked(&self) -> bool {
        matches!(self.mode, VaultMode::Locked { .. })
    }

    pub fn unlocked(&self) -> bool {
        matches!(self.mode, VaultMode::Unlocked { .. })
    }

    pub fn configured(&self) -> bool {
        !matches!(self.mode, VaultMode::NeedsSetup)
    }

    pub fn salt(&self) -> Option<[u8; SALT_LEN]> {
        match &self.mode {
            VaultMode::Locked { salt } => Some(*salt),
            VaultMode::Unlocked { salt, .. } => Some(*salt),
            VaultMode::NeedsSetup => None,
        }
    }

    pub fn key(&self) -> Option<VaultKey> {
        match &self.mode {
            VaultMode::Unlocked { key, .. } => Some(key.clone()),
            _ => None,
        }
    }

    pub fn idle_timeout(&self) -> IdleTimeout {
        self.idle_timeout
    }

    pub fn set_needs_setup(&mut self) {
        self.mode = VaultMode::NeedsSetup;
        self.idle_timeout = IdleTimeout::default();
        self.last_activity_ms = now_ms();
    }

    pub fn set_locked(&mut self, salt: [u8; SALT_LEN]) {
        self.mode = VaultMode::Locked { salt };
    }

    pub fn set_unlocked(&mut self, key: VaultKey, salt: [u8; SALT_LEN], idle_timeout: IdleTimeout) {
        self.mode = VaultMode::Unlocked { key, salt };
        self.idle_timeout = idle_timeout;
        self.last_activity_ms = now_ms();
    }

    pub fn touch(&mut self) {
        if self.unlocked() {
            self.last_activity_ms = now_ms();
        }
    }

    pub fn set_idle_timeout(&mut self, timeout: IdleTimeout) {
        self.idle_timeout = timeout;
    }

    /// Lock an expired vault. Returns true when the caller must close its
    /// database connection.
    pub fn lock_if_idle(&mut self) -> bool {
        if !self.unlocked() {
            return false;
        }
        if now_ms() - self.last_activity_ms < self.idle_timeout.as_secs() as f64 * 1000.0 {
            return false;
        }
        let salt = self.salt().unwrap_or([0u8; SALT_LEN]);
        self.mode = VaultMode::Locked { salt };
        true
    }
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}
