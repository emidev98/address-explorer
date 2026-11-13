//! Single active server session.
//!
//! A successful vault unlock issues one random bearer token. A later unlock
//! replaces it, so only one client can use the process-global vault at a time.
//! The token is held only in memory and is cleared whenever the vault locks.
//! Session lifetime is the vault inactivity timer; there is no second clock.
#![allow(dead_code, unused_imports)]

use crate::security::policy;
use base64::Engine;
use dioxus::server::axum::{
    extract::Request,
    http::{header, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use std::sync::{LazyLock, RwLock};
use zeroize::Zeroizing;

struct ActiveSession {
    token: Zeroizing<Vec<u8>>,
}

static SESSION: LazyLock<RwLock<Option<ActiveSession>>> = LazyLock::new(|| RwLock::new(None));

/// Issue a new session, invalidating the previous one.
pub fn issue() -> Result<String, String> {
    let length = policy::current().session_token_bytes as usize;
    let mut token = Zeroizing::new(vec![0u8; length]);
    getrandom::getrandom(token.as_mut_slice())
        .map_err(|_| "the operating system random generator failed".to_string())?;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token.as_slice());
    let session = ActiveSession { token };
    let mut guard = write()?;
    *guard = Some(session);
    Ok(encoded)
}

/// Validate a bearer token against the active session.
pub fn validate(token: &str) -> bool {
    let Ok(decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(token) else {
        return false;
    };
    let Ok(guard) = SESSION.read() else {
        return false;
    };
    let Some(session) = guard.as_ref() else {
        return false;
    };
    constant_time_eq(session.token.as_slice(), decoded.as_slice())
}

/// Drop the active session.
pub fn clear() {
    if let Ok(mut guard) = SESSION.write() {
        *guard = None;
    }
}

/// Authorize every non-public API request.
///
/// Health, encryption status, setup, unlock, the CA download, and CORS
/// preflight are public. Reset while locked is refused with the host CLI
/// recovery instructions. The vault inactivity timer is the only session
/// lifetime; a lock clears the token.
pub async fn guard(request: Request, next: Next) -> Response {
    let path = request.uri().path();
    if !path.contains("/api/") || request.method() == Method::OPTIONS || is_public_path(path) {
        return next.run(request).await;
    }

    if path.ends_with("/api/encryption/reset") && vault_is_locked() {
        let body = Json(serde_json::json!({
            "message": "reset_locked: run `address-explorer reset-vault` on the server host",
            "code": StatusCode::FORBIDDEN.as_u16(),
            "data": null,
        }));
        return (StatusCode::FORBIDDEN, body).into_response();
    }

    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(token) = token {
        if validate(token) {
            return next.run(request).await;
        }
    }

    let body = Json(serde_json::json!({
        "message": "authentication required; unlock the database to start a session",
        "code": StatusCode::UNAUTHORIZED.as_u16(),
        "data": null,
    }));
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
        body,
    )
        .into_response()
}

fn is_public_path(path: &str) -> bool {
    path.ends_with("/api/health")
        || path.ends_with("/api/encryption/status")
        || path.ends_with("/api/encryption/setup")
        || path.ends_with("/api/encryption/unlock")
        || path.ends_with("/api/tls/ca")
}

fn vault_is_locked() -> bool {
    crate::data::vault::status()
        .map(|status| status.locked)
        .unwrap_or(false)
}

fn write() -> Result<std::sync::RwLockWriteGuard<'static, Option<ActiveSession>>, String> {
    SESSION
        .write()
        .map_err(|_| "the session lock was poisoned".to_string())
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

    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn a_new_session_replaces_the_previous_one() {
        let _guard = TEST_LOCK.lock().expect("session test lock"); // allow-unwrap: test setup.
        clear();
        let first = issue().expect("session"); // allow-unwrap: test setup.
        assert!(validate(&first));
        let second = issue().expect("session"); // allow-unwrap: test setup.
        assert!(!validate(&first));
        assert!(validate(&second));
        clear();
    }

    #[test]
    fn malformed_tokens_are_rejected() {
        let _guard = TEST_LOCK.lock().expect("session test lock"); // allow-unwrap: test setup.
        clear();
        let _token = issue().expect("session"); // allow-unwrap: test setup.
        assert!(!validate("not-base64"));
        clear();
    }
}
