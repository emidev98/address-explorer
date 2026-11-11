//! Browser worker vault state. Only the Argon2id-derived key is retained while
//! unlocked; locking drops it and closes the SQLite connection.

use crate::data::encryption::{VaultKey, SALT_LEN};
use crate::domain::encryption::{EncryptionStatus, IdleTimeout};

pub enum VaultMode {
    Plaintext,
    Locked { salt: [u8; SALT_LEN] },
    Unlocked { key: VaultKey, salt: [u8; SALT_LEN] },
}

pub struct WorkerVault {
    mode: VaultMode,
    idle_timeout: IdleTimeout,
    last_activity_ms: f64,
    prompt_seen: bool,
}

impl Default for WorkerVault {
    fn default() -> Self {
        Self {
            mode: VaultMode::Plaintext,
            idle_timeout: IdleTimeout::default(),
            last_activity_ms: now_ms(),
            prompt_seen: false,
        }
    }
}

impl WorkerVault {
    pub fn status(&self) -> EncryptionStatus {
        EncryptionStatus {
            configured: !matches!(self.mode, VaultMode::Plaintext),
            locked: matches!(self.mode, VaultMode::Locked { .. }),
            idle_timeout: self.idle_timeout,
            prompt_seen: self.prompt_seen,
        }
    }

    pub fn locked(&self) -> bool {
        matches!(self.mode, VaultMode::Locked { .. })
    }

    pub fn unlocked(&self) -> bool {
        matches!(self.mode, VaultMode::Unlocked { .. })
    }

    pub fn configured(&self) -> bool {
        !matches!(self.mode, VaultMode::Plaintext)
    }

    pub fn salt(&self) -> Option<[u8; SALT_LEN]> {
        match &self.mode {
            VaultMode::Locked { salt } => Some(*salt),
            VaultMode::Unlocked { salt, .. } => Some(*salt),
            VaultMode::Plaintext => None,
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

    pub fn set_plaintext(&mut self, prompt_seen: bool, idle_timeout: IdleTimeout) {
        self.mode = VaultMode::Plaintext;
        self.prompt_seen = prompt_seen;
        self.idle_timeout = idle_timeout;
        self.last_activity_ms = now_ms();
    }

    pub fn set_locked(&mut self, salt: [u8; SALT_LEN]) {
        self.mode = VaultMode::Locked { salt };
        self.prompt_seen = true;
    }

    pub fn set_unlocked(
        &mut self,
        key: VaultKey,
        salt: [u8; SALT_LEN],
        idle_timeout: IdleTimeout,
        prompt_seen: bool,
    ) {
        self.mode = VaultMode::Unlocked { key, salt };
        self.idle_timeout = idle_timeout;
        self.prompt_seen = prompt_seen;
        self.last_activity_ms = now_ms();
    }

    pub fn touch(&mut self) {
        if self.unlocked() {
            self.last_activity_ms = now_ms();
        }
    }

    pub fn set_prompt_seen(&mut self, seen: bool) {
        self.prompt_seen = seen;
    }

    pub fn set_idle_timeout(&mut self, timeout: IdleTimeout) {
        self.idle_timeout = timeout;
    }

    /// Lock an expired vault. Returns true when the caller must close its
    /// database connection.
    pub fn lock_if_idle(&mut self) -> bool {
        let Some(seconds) = self.idle_timeout.as_secs() else {
            return false;
        };
        if !self.unlocked() {
            return false;
        }
        if now_ms() - self.last_activity_ms < seconds as f64 * 1000.0 {
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
