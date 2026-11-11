//! Password-based key derivation and the SQLCipher key format.
//!
//! The password is never persisted. It is expanded with Argon2id into a
//! 256-bit key that SQLCipher uses in raw-key mode. The 16-byte salt is the
//! public SQLCipher header salt, not a secret.

use argon2::{Algorithm, Argon2, Params, Version};
use std::sync::Arc;
use zeroize::Zeroizing;

/// Length of the public SQLCipher header salt.
pub const SALT_LEN: usize = 16;
/// Length of the derived SQLCipher key.
pub const KEY_LEN: usize = 32;
/// Argon2id memory cost in KiB (64 MiB).
const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
/// Argon2id pass count.
const ARGON2_PASSES: u32 = 3;
/// Argon2id parallelism. One lane keeps browser and native derivation equal.
const ARGON2_LANES: u32 = 1;
/// Prefix of the stable error that tells clients the vault is locked.
pub const VAULT_LOCKED_MESSAGE: &str = "vault_locked: the database is locked";
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\0";

/// Failures of key derivation and encrypted database header parsing.
#[derive(Debug, thiserror::Error)]
pub enum EncryptionError {
    #[error("the operating system random generator failed")]
    Random,
    #[error("could not derive the encryption key: {0}")]
    KeyDerivation(String),
    #[error("the encrypted database header is missing or invalid")]
    InvalidHeader,
    #[error("incorrect password")]
    WrongPassword,
    #[error("encryption is already configured")]
    AlreadyConfigured,
    #[error("encryption is not configured")]
    NotConfigured,
    #[error("the database is locked; enter the password to continue")]
    Locked,
    #[error("{0}")]
    Message(String),
}

/// A 256-bit SQLCipher raw key. The buffer is zeroized when the last clone
/// is dropped.
#[derive(Clone)]
pub struct VaultKey(Arc<Zeroizing<[u8; KEY_LEN]>>);

impl VaultKey {
    pub fn new(bytes: [u8; KEY_LEN]) -> Self {
        Self(Arc::new(Zeroizing::new(bytes)))
    }

    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }

    /// SQLCipher raw-key specification containing the key and public salt.
    ///
    /// SQLCipher and SQLite3MC both accept the `x'<key><salt>'` form. The
    /// value contains only hex digits generated here, so it is safe to pass
    /// through the pragma helper.
    pub fn key_spec(&self, salt: &[u8; SALT_LEN]) -> String {
        format!("x'{}{}'", hex_encode(self.as_bytes()), hex_encode(salt))
    }

    /// Constant-time equality for the active in-memory key.
    pub fn matches(&self, other: &Self) -> bool {
        constant_time_eq(self.as_bytes(), other.as_bytes())
    }
}

impl From<crate::data::DatabaseError> for EncryptionError {
    fn from(value: crate::data::DatabaseError) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<std::io::Error> for EncryptionError {
    fn from(value: std::io::Error) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<rusqlite::Error> for EncryptionError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Message(value.to_string())
    }
}

impl From<EncryptionError> for String {
    fn from(value: EncryptionError) -> Self {
        value.to_string()
    }
}

impl std::fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VaultKey([redacted])")
    }
}

/// Generate a fresh public salt for a new encrypted database.
pub fn generate_salt() -> Result<[u8; SALT_LEN], EncryptionError> {
    let mut salt = [0u8; SALT_LEN];
    getrandom::getrandom(&mut salt).map_err(|_| EncryptionError::Random)?;
    Ok(salt)
}

/// Derive the SQLCipher raw key from a password and salt with Argon2id.
pub fn derive_key(password: &str, salt: &[u8; SALT_LEN]) -> Result<VaultKey, EncryptionError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_PASSES,
        ARGON2_LANES,
        Some(KEY_LEN),
    )
    .map_err(|error| EncryptionError::KeyDerivation(error.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; KEY_LEN];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|error| EncryptionError::KeyDerivation(error.to_string()))?;
    Ok(VaultKey::new(key))
}

/// True when the bytes are an unencrypted SQLite database.
pub fn is_plaintext_sqlite(bytes: &[u8]) -> bool {
    bytes.starts_with(SQLITE_HEADER)
}

/// Read the public salt from the first 16 bytes of an encrypted database.
pub fn salt_from_header(bytes: &[u8]) -> Result<[u8; SALT_LEN], EncryptionError> {
    if bytes.len() < SALT_LEN || is_plaintext_sqlite(bytes) {
        return Err(EncryptionError::InvalidHeader);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&bytes[..SALT_LEN]);
    Ok(salt)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_spec_contains_key_and_salt_hex() {
        let key = VaultKey::new([0xab; KEY_LEN]);
        let salt = [0xcd; SALT_LEN];
        let spec = key.key_spec(&salt);
        assert!(spec.starts_with("x'"));
        assert!(spec.ends_with('\''));
        assert_eq!(spec.len(), 2 + (KEY_LEN + SALT_LEN) * 2 + 1);
        assert!(spec[2..].chars().all(|character| character == 'a'
            || character == 'b'
            || character == 'c'
            || character == 'd'
            || character == '\''));
    }

    #[test]
    fn derived_keys_are_stable_and_salt_dependent() {
        let first = derive_key("correct horse battery staple", &[1; SALT_LEN]).unwrap(); // allow-unwrap: test setup.
        let second = derive_key("correct horse battery staple", &[1; SALT_LEN]).unwrap(); // allow-unwrap: test setup.
        let third = derive_key("correct horse battery staple", &[2; SALT_LEN]).unwrap(); // allow-unwrap: test setup.
        assert!(first.matches(&second));
        assert!(!first.matches(&third));
    }

    #[test]
    fn header_salt_distinguishes_plaintext() {
        let salt = salt_from_header(&[7; SALT_LEN]).unwrap(); // allow-unwrap: test setup.
        assert_eq!(salt, [7; SALT_LEN]);
        assert!(salt_from_header(b"SQLite format 3\0").is_err());
        assert!(salt_from_header(&[1; 4]).is_err());
    }
}
