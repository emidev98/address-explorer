//! Runtime backend selection: bundled local SQLite or a remote Address
//! Explorer REST server.
//!
//! Native builds are always all-in-one. The saved mode only decides which
//! backend the API controllers use; the remote URL is a local preference and
//! never lives in the backend database.
//!
//! Several helpers are only used by native all-in-one builds or by the
//! Settings UI, so the module allows dead code in web/server-only builds.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex, RwLock};
#[cfg(not(target_arch = "wasm32"))]
use zeroize::Zeroizing;

#[cfg(target_arch = "wasm32")]
use crate::domain::health::Health;
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::health::{Health, API_VERSION};

/// Which backend the API controllers should use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendMode {
    Local,
    Remote,
}

/// Persisted backend preference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Config schema version.
    pub version: u32,
    /// Active mode.
    pub mode: BackendMode,
    /// Remote URL. Required in Remote mode; retained as the last-used URL in
    /// Local mode so switching back is convenient.
    #[serde(alias = "remote_url")]
    pub base_url: Option<String>,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            version: 1,
            mode: BackendMode::Local,
            base_url: None,
        }
    }
}

impl BackendConfig {
    fn with_mode(mode: BackendMode, base_url: Option<String>) -> Self {
        Self {
            version: 1,
            mode,
            base_url,
        }
    }
}

/// A backend-level alert that should be shown to the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendAlert {
    VersionMismatch {
        client: u32,
        server: u32,
        url: String,
    },
    Unreachable {
        url: String,
        message: String,
    },
    Blocked {
        message: String,
    },
}

impl BackendAlert {
    /// User-facing message.
    pub fn message(&self) -> String {
        match self {
            Self::VersionMismatch { client, server, url } => format!(
                "Remote backend API version {server} at {url} does not match this app (API {client}). Using the local database."
            ),
            Self::Unreachable { url, message } => format!(
                "Remote backend {url} is unreachable: {message}. Using the local database."
            ),
            Self::Blocked { message } => message.clone(),
        }
    }
}

static CONFIG: LazyLock<RwLock<BackendConfig>> =
    LazyLock::new(|| RwLock::new(BackendConfig::default()));
static STARTUP_ALERT: LazyLock<Mutex<Option<BackendAlert>>> = LazyLock::new(|| Mutex::new(None));
static REMOTE_BLOCK: LazyLock<Mutex<Option<BackendAlert>>> = LazyLock::new(|| Mutex::new(None));
#[cfg(not(target_arch = "wasm32"))]
static SESSION_TOKEN: LazyLock<Mutex<Option<Zeroizing<String>>>> =
    LazyLock::new(|| Mutex::new(None));

#[cfg(target_arch = "wasm32")]
const WEB_SESSION_KEY: &str = "address-explorer.session";

/// Read the active unlock-issued session token.
pub fn session_token() -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        SESSION_TOKEN
            .lock()
            .ok()
            .and_then(|token| token.as_ref().map(|token| token.as_str().to_string()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.session_storage().ok().flatten())
            .and_then(|storage| storage.get_item(WEB_SESSION_KEY).ok().flatten())
            .filter(|token| !token.is_empty())
    }
}

/// Store a new session token, replacing any previous token.
pub fn set_session_token(token: &str) {
    if token.is_empty() {
        clear_session_token();
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(mut slot) = SESSION_TOKEN.lock() {
            *slot = Some(Zeroizing::new(token.to_string()));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) =
            web_sys::window().and_then(|window| window.session_storage().ok().flatten())
        {
            let _ = storage.set_item(WEB_SESSION_KEY, token);
        }
    }
}

/// Drop the session token after a lock, reset, or 401 response.
pub fn clear_session_token() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(mut slot) = SESSION_TOKEN.lock() {
            *slot = None;
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(storage) =
            web_sys::window().and_then(|window| window.session_storage().ok().flatten())
        {
            let _ = storage.remove_item(WEB_SESSION_KEY);
        }
    }
}

/// Whether the binary contains the bundled local backend. Native builds are
/// all-in-one, so this is true there; web and pure-server builds are not
/// interactive local applications.
pub const fn local_available() -> bool {
    (cfg!(feature = "server") && cfg!(any(feature = "desktop", feature = "mobile")))
        || (cfg!(target_arch = "wasm32") && cfg!(feature = "web-client"))
}

/// Whether the native Settings UI should offer the remote backend option.
pub const fn remote_available() -> bool {
    (cfg!(feature = "remote-backend") && cfg!(any(feature = "desktop", feature = "mobile")))
        || (cfg!(target_arch = "wasm32") && cfg!(feature = "web-client"))
}

/// Whether the Backend section should be rendered.
pub const fn settings_section_available() -> bool {
    local_available() && remote_available()
}

/// A server build has no interactive local backend and must never forward to
/// another server.
pub const fn pure_server() -> bool {
    cfg!(all(
        feature = "server",
        not(any(feature = "desktop", feature = "mobile"))
    ))
}

/// Load the saved preference and resolve the active mode for this process.
///
/// Native startup with a saved Remote mode performs a short health check
/// before accepting it. A mismatched or unreachable server falls back to the
/// local backend and leaves an alert for the app root to toast.
pub fn init() {
    #[cfg(target_arch = "wasm32")]
    {
        *CONFIG.write().expect("backend config lock poisoned") = load_config();
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let resolved = if pure_server() {
            BackendConfig::with_mode(BackendMode::Local, None)
        } else {
            resolve_native_config(load_config())
        };
        *CONFIG.write().expect("backend config lock poisoned") = resolved;
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_native_config(mut config: BackendConfig) -> BackendConfig {
    if config.mode != BackendMode::Remote {
        return config;
    }

    let Some(url) = config
        .base_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    else {
        set_startup_alert(BackendAlert::Unreachable {
            url: "(none)".to_string(),
            message: "no remote backend URL is configured".to_string(),
        });
        config.mode = BackendMode::Local;
        return config;
    };

    match verify_remote(url) {
        Ok(health) if health.api_version == API_VERSION => config,
        Ok(health) => {
            set_startup_alert(BackendAlert::VersionMismatch {
                client: API_VERSION,
                server: health.api_version,
                url: url.to_string(),
            });
            config.mode = BackendMode::Local;
            config
        }
        Err(alert) => {
            set_startup_alert(alert);
            config.mode = BackendMode::Local;
            config
        }
    }
}

#[cfg(all(feature = "remote-backend", not(target_arch = "wasm32")))]
fn verify_remote(url: &str) -> Result<Health, BackendAlert> {
    use std::time::Duration;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| BackendAlert::Unreachable {
            url: url.to_string(),
            message: format!("could not start the health-check runtime: {error}"),
        })?;

    match runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), crate::api::remote::health_at(url)).await
    }) {
        Ok(Ok(health)) => Ok(health),
        Ok(Err(error)) => Err(BackendAlert::Unreachable {
            url: url.to_string(),
            message: error.to_string(),
        }),
        Err(_) => Err(BackendAlert::Unreachable {
            url: url.to_string(),
            message: "health check timed out".to_string(),
        }),
    }
}

#[cfg(not(all(feature = "remote-backend", not(target_arch = "wasm32"))))]
fn verify_remote(url: &str) -> Result<Health, BackendAlert> {
    Err(BackendAlert::Unreachable {
        url: url.to_string(),
        message: "remote backend support is not compiled into this build".to_string(),
    })
}

/// Current active configuration.
pub fn config() -> BackendConfig {
    CONFIG.read().expect("backend config lock poisoned").clone()
}

/// Replace the active configuration. A backend change always triggers a hard
/// reload, so this only needs to update the process-local value.
pub fn set_config(config: BackendConfig) {
    clear_remote_block();
    *CONFIG.write().expect("backend config lock poisoned") = config;
}

/// Current active mode.
pub fn mode() -> BackendMode {
    CONFIG.read().expect("backend config lock poisoned").mode
}

/// Whether the local backend is active.
pub fn is_local() -> bool {
    mode() == BackendMode::Local
}

/// Whether the remote backend is active.
pub fn is_remote() -> bool {
    mode() == BackendMode::Remote
}

/// Configured remote URL, if the active mode is Remote.
pub fn remote_base_url() -> Option<String> {
    let config = CONFIG.read().expect("backend config lock poisoned");
    if config.mode == BackendMode::Remote {
        config.base_url.clone()
    } else {
        None
    }
}

/// Take the one-time startup alert left by [`init`], if any.
pub fn take_startup_alert() -> Option<BackendAlert> {
    STARTUP_ALERT
        .lock()
        .expect("backend alert lock poisoned")
        .take()
}

fn set_startup_alert(alert: BackendAlert) {
    *STARTUP_ALERT.lock().expect("backend alert lock poisoned") = Some(alert);
}

/// Record a runtime block. The remote helper refuses further requests and the
/// app root shows the alert once.
pub fn block_remote(alert: BackendAlert) {
    *REMOTE_BLOCK.lock().expect("backend block lock poisoned") = Some(alert);
}

/// Current runtime block, if any.
pub fn remote_blocked() -> Option<BackendAlert> {
    REMOTE_BLOCK
        .lock()
        .expect("backend block lock poisoned")
        .clone()
}

/// Clear a runtime block after the user chooses a new backend.
pub fn clear_remote_block() {
    *REMOTE_BLOCK.lock().expect("backend block lock poisoned") = None;
}

/// Validate and normalize a backend base URL.
pub fn validate_base_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Enter a backend URL.".to_string());
    }

    let mut url = reqwest::Url::parse(trimmed).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("The backend URL must use https, or http on loopback.".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("The backend URL must not contain credentials.".to_string());
    }
    if url.host_str().is_none() {
        return Err("The backend URL must include a host.".to_string());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("The backend URL must not contain a query or fragment.".to_string());
    }
    if url.scheme() == "http" && !is_loopback_host(url.host_str().unwrap_or_default()) {
        return Err("Plain HTTP is only allowed on loopback. Use HTTPS.".to_string());
    }

    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(if path.is_empty() { "/" } else { &path });
    url.set_query(None);
    url.set_fragment(None);

    let mut normalized = url.to_string();
    while normalized.ends_with('/') && !normalized.ends_with("://") {
        normalized.pop();
    }
    Ok(normalized)
}

/// Reject a remote backend that is not protected by TLS.
///
/// Loopback HTTP is allowed for local development; every other remote origin
/// must use HTTPS so session tokens, passwords, and database contents cannot
/// be observed on the network.
pub fn remote_transport_error(base_url: &str) -> Option<String> {
    let Ok(parsed) = reqwest::Url::parse(base_url) else {
        return Some("The remote backend URL is invalid.".to_string());
    };
    if parsed.scheme() == "https" {
        return None;
    }
    let host = parsed.host_str().unwrap_or_default();
    if parsed.scheme() == "http" && is_loopback_host(host) {
        return None;
    }
    Some(
        "Refusing to send data over plain HTTP. Use an HTTPS remote server, or loopback HTTP for local development."
            .to_string(),
    )
}

/// Reject sending a password to a remote backend over plain HTTP.
pub fn remote_password_transport_error() -> Option<String> {
    if !is_remote() {
        return None;
    }
    let url = remote_base_url()?;
    remote_transport_error(&url)
}

/// True when a URL is plain HTTP to a non-loopback host. Used by Settings to
/// explain why an insecure URL is rejected.
pub fn uses_insecure_remote_http(base_url: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(base_url) else {
        return false;
    };
    if url.scheme() != "http" {
        return false;
    }
    let Some(host) = url.host_str() else {
        return true;
    };
    !is_loopback_host(host)
}

fn is_loopback_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Web builds persist the backend preference in this origin's localStorage.
#[cfg(target_arch = "wasm32")]
const WEB_CONFIG_KEY: &str = "address-explorer.backend";

/// Read the persisted backend preference.
#[cfg(not(target_arch = "wasm32"))]
pub fn load_config() -> BackendConfig {
    let Some(path) = config_path() else {
        return BackendConfig::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BackendConfig::default();
    };
    let Ok(mut config) = serde_json::from_str::<BackendConfig>(&text) else {
        eprintln!("ignoring invalid backend config at {}", path.display());
        return BackendConfig::default();
    };
    if config.version != 1 {
        eprintln!(
            "ignoring unsupported backend config version {} at {}",
            config.version,
            path.display()
        );
        return BackendConfig::default();
    }

    config.base_url = config.base_url.and_then(|url| validate_base_url(&url).ok());
    if config.mode == BackendMode::Remote && config.base_url.is_none() {
        eprintln!("remote backend config has no valid URL; using the local backend");
        config.mode = BackendMode::Local;
    }
    config
}

/// Web builds read the versioned backend preference from localStorage.
#[cfg(target_arch = "wasm32")]
pub fn load_config() -> BackendConfig {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return BackendConfig::default();
    };
    let Ok(Some(text)) = storage.get_item(WEB_CONFIG_KEY) else {
        return BackendConfig::default();
    };
    let Ok(mut config) = serde_json::from_str::<BackendConfig>(&text) else {
        return BackendConfig::default();
    };
    if config.version != 1 {
        return BackendConfig::default();
    }
    config.base_url = config.base_url.and_then(|url| validate_base_url(&url).ok());
    if config.mode == BackendMode::Remote && config.base_url.is_none() {
        config.mode = BackendMode::Local;
    }
    config
}

/// Persist the backend preference.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_config(config: &BackendConfig) -> Result<(), String> {
    let Some(path) = config_path() else {
        return Err("Could not determine the application-data directory.".to_string());
    };
    if let Some(parent) = path.parent() {
        if let Err(error) = crate::security::files::ensure_private_dir(parent) {
            eprintln!(
                "could not create backend config directory {}: {error}",
                parent.display()
            );
            return Err("Could not save the backend preference.".to_string());
        }
    }

    let mut config = config.clone();
    config.version = 1;
    config.base_url = config.base_url.and_then(|url| validate_base_url(&url).ok());
    if config.mode == BackendMode::Remote && config.base_url.is_none() {
        return Err("A valid remote backend URL is required.".to_string());
    }
    let bytes = serde_json::to_vec_pretty(&config).map_err(|error| {
        eprintln!("could not serialize backend config: {error}");
        "Could not save the backend preference.".to_string()
    })?;
    crate::security::files::write_private(&path, &bytes).map_err(|error| {
        eprintln!("could not write {}: {error}", path.display());
        "Could not save the backend preference.".to_string()
    })
}

/// Web builds persist the backend preference in localStorage.
#[cfg(target_arch = "wasm32")]
pub fn save_config(config: &BackendConfig) -> Result<(), String> {
    let Some(storage) = web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    else {
        return Err(
            "Browser storage is unavailable; the backend preference cannot be saved.".to_string(),
        );
    };
    let mut config = config.clone();
    config.version = 1;
    config.base_url = config.base_url.and_then(|url| validate_base_url(&url).ok());
    if config.mode == BackendMode::Remote && config.base_url.is_none() {
        return Err("A valid remote backend URL is required.".to_string());
    }
    let text = serde_json::to_string(&config)
        .map_err(|error| format!("Could not serialize the backend preference: {error}"))?;
    storage
        .set_item(WEB_CONFIG_KEY, &text)
        .map_err(|_| "Could not save the backend preference in browser storage.".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn config_path() -> Option<std::path::PathBuf> {
    if let Ok(path) = std::env::var("BACKEND_CONFIG_PATH") {
        let path = path.trim();
        if !path.is_empty() {
            return Some(std::path::PathBuf::from(path));
        }
    }
    dirs::data_dir().map(|directory| directory.join("address-explorer").join("backend.json"))
}

/// Apply a backend change. Desktop restarts the process; web reloads the page.
/// Mobile builds use a full-root remount in the Settings component because
/// process restart is unavailable.
pub fn hard_reload() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = dioxus::document::eval("window.location.reload()");
        Ok(())
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        any(target_os = "android", target_os = "ios")
    ))]
    {
        Err("Close and reopen the application to finish switching backends.".to_string())
    }

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(any(target_os = "android", target_os = "ios"))
    ))]
    {
        let executable = std::env::current_exe().map_err(|error| {
            eprintln!("could not locate the application executable: {error}");
            "Could not restart the application.".to_string()
        })?;
        std::process::Command::new(executable)
            .spawn()
            .map_err(|error| {
                eprintln!("could not restart the application: {error}");
                "Could not restart the application.".to_string()
            })?;
        std::process::exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_base_url_normalizes_trailing_slashes() {
        assert_eq!(
            validate_base_url("https://example.com/").unwrap(),
            "https://example.com"
        );
        assert_eq!(
            validate_base_url("http://127.0.0.1:8080/app/").unwrap(),
            "http://127.0.0.1:8080/app"
        );
    }

    #[test]
    fn validate_base_url_rejects_bad_inputs() {
        assert!(validate_base_url("").is_err());
        assert!(validate_base_url("ftp://example.com").is_err());
        assert!(validate_base_url("https://user:pass@example.com").is_err());
        assert!(validate_base_url("https://example.com?x=1").is_err());
        assert!(validate_base_url("https://example.com/#fragment").is_err());
        assert!(validate_base_url("http://example.com").is_err());
        assert!(validate_base_url("http://10.evil.com").is_err());
        assert!(validate_base_url("http://10.0.0.5").is_err());
    }

    #[test]
    fn insecure_http_is_flagged_for_every_non_loopback_host() {
        assert!(!uses_insecure_remote_http("http://127.0.0.1:8080"));
        assert!(!uses_insecure_remote_http("http://localhost:8080"));
        assert!(uses_insecure_remote_http("http://10.0.0.5:8080"));
        assert!(uses_insecure_remote_http("http://10.evil.com"));
        assert!(!uses_insecure_remote_http("https://example.com"));
        assert!(uses_insecure_remote_http("http://example.com"));
    }

    #[test]
    fn config_round_trips_through_json() {
        let config =
            BackendConfig::with_mode(BackendMode::Remote, Some("https://example.com".to_string()));
        let encoded = serde_json::to_string(&config).unwrap();
        let decoded: BackendConfig = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, config);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn config_round_trips_through_the_local_file() {
        let path = std::env::temp_dir().join(format!(
            "address-explorer-backend-{}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        std::env::set_var("BACKEND_CONFIG_PATH", &path);

        let remote = BackendConfig::with_mode(
            BackendMode::Remote,
            Some("http://127.0.0.1:8080".to_string()),
        );
        save_config(&remote).expect("save remote config");
        let loaded = load_config();
        assert_eq!(loaded.mode, BackendMode::Remote);
        assert_eq!(loaded.base_url.as_deref(), Some("http://127.0.0.1:8080"));

        let local =
            BackendConfig::with_mode(BackendMode::Local, Some("https://example.com".to_string()));
        save_config(&local).expect("save local config");
        let loaded = load_config();
        assert_eq!(loaded.mode, BackendMode::Local);
        assert_eq!(loaded.base_url.as_deref(), Some("https://example.com"));

        let _ = std::fs::remove_file(&path);
        std::env::remove_var("BACKEND_CONFIG_PATH");
    }
}
