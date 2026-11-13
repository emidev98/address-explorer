//! Zeroizing wrapper for passwords and other short secrets.
//!
//! The wrapper guarantees that the owned copy is overwritten when it is
//! dropped. Framework and HTTP-client copies that exist before deserialization
//! or inside a request body cannot be controlled from here; those are
//! documented as best-effort limitations.
#![allow(dead_code)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

/// An owned secret string that zeroizes its buffer on drop.
pub struct SecretString(Zeroizing<String>);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_inner(self) -> Zeroizing<String> {
        self.0
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([redacted])")
    }
}

impl Serialize for SecretString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::new(String::deserialize(deserializer)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_redacted() {
        let secret = SecretString::new("correct horse battery staple");
        assert!(!format!("{secret:?}").contains("horse"));
    }

    #[test]
    fn serializes_as_a_plain_json_string() {
        let secret = SecretString::new("secret");
        let encoded = serde_json::to_string(&secret).unwrap(); // allow-unwrap: test setup.
        assert_eq!(encoded, "\"secret\"");
        let decoded: SecretString = serde_json::from_str("\"secret\"").unwrap(); // allow-unwrap: test setup.
        assert_eq!(decoded.as_str(), "secret");
    }
}
