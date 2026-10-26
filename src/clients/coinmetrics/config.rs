use reqwest::header::HeaderMap;
use std::time::Duration;

/// Connection settings for the Coin Metrics API v4 client.
///
/// The default configuration targets the free Community API, which needs no API
/// key. Use [`CoinMetricsConfig::pro`] for the paid API with an API key.
#[derive(Clone, Debug)]
pub struct CoinMetricsConfig {
    /// Hostname (and optional port, e.g. "localhost:8000") of the API server.
    pub hostname: String,
    /// API key sent as the `api_key` query parameter. Not needed for the
    /// Community API.
    pub api_key: Option<String>,
    pub protocol: Option<String>,
    pub headers: Option<HeaderMap>,
    pub timeout: Option<Duration>,
}

impl CoinMetricsConfig {
    /// Configuration for the free Community API (`community-api.coinmetrics.io`).
    pub fn community() -> Self {
        Self::default()
    }

    /// Configuration for the paid API (`api.coinmetrics.io`) using `api_key`.
    pub fn pro(api_key: impl Into<String>) -> Self {
        Self {
            hostname: "api.coinmetrics.io".to_string(),
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }
}

impl Default for CoinMetricsConfig {
    fn default() -> Self {
        Self {
            hostname: "community-api.coinmetrics.io".to_string(),
            api_key: None,
            protocol: None,
            headers: None,
            timeout: None,
        }
    }
}

impl CoinMetricsConfig {
    /// Builds the HTTP base URL of the v4 API, for example
    /// `https://community-api.coinmetrics.io/v4/`.
    pub fn base_url(&self) -> String {
        format!("{}://{}/v4/", self.protocol(), self.hostname)
    }

    /// Builds the WebSocket base URL of the v4 API, for example
    /// `wss://community-api.coinmetrics.io/v4/`.
    pub fn ws_base_url(&self) -> String {
        let protocol = match self.protocol().as_str() {
            "http" | "ws" => "ws".to_string(),
            _ => "wss".to_string(),
        };
        format!("{}://{}/v4/", protocol, self.hostname)
    }

    fn protocol(&self) -> String {
        self.protocol.clone().unwrap_or_else(|| {
            if self.hostname.contains("localhost") {
                "http".to_string()
            } else {
                "https".to_string()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_targets_the_community_api() {
        let cfg = CoinMetricsConfig::default();
        assert_eq!(cfg.base_url(), "https://community-api.coinmetrics.io/v4/");
        assert_eq!(cfg.ws_base_url(), "wss://community-api.coinmetrics.io/v4/");
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn pro_uses_the_paid_api() {
        let cfg = CoinMetricsConfig::pro("secret");
        assert_eq!(cfg.hostname, "api.coinmetrics.io");
        assert_eq!(cfg.api_key.as_deref(), Some("secret"));
        assert_eq!(cfg.base_url(), "https://api.coinmetrics.io/v4/");
    }

    #[test]
    fn localhost_falls_back_to_http_and_ws() {
        let cfg = CoinMetricsConfig {
            hostname: "localhost:8000".into(),
            ..Default::default()
        };
        assert_eq!(cfg.base_url(), "http://localhost:8000/v4/");
        assert_eq!(cfg.ws_base_url(), "ws://localhost:8000/v4/");
    }

    #[test]
    fn explicit_protocol_overrides_the_default() {
        let cfg = CoinMetricsConfig {
            protocol: Some("http".into()),
            ..Default::default()
        };
        assert_eq!(cfg.base_url(), "http://community-api.coinmetrics.io/v4/");
        assert_eq!(cfg.ws_base_url(), "ws://community-api.coinmetrics.io/v4/");
    }
}
