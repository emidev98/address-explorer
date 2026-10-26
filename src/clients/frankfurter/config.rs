use reqwest::header::HeaderMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct FrankfurterConfig {
    /// Hostname (and optional port, e.g. "localhost:4000") of the API server.
    pub hostname: String,
    pub protocol: Option<String>,
    pub headers: Option<HeaderMap>,
    pub timeout: Option<Duration>,
}

impl Default for FrankfurterConfig {
    fn default() -> Self {
        Self {
            hostname: "api.frankfurter.dev".to_string(),
            protocol: None,
            headers: None,
            timeout: None,
        }
    }
}

impl FrankfurterConfig {
    /// Builds the base URL of the v2 API, for example `https://api.frankfurter.dev/v2/`.
    pub fn base_url(&self) -> String {
        let protocol = self.protocol.clone().unwrap_or_else(|| {
            if self.hostname.contains("localhost") {
                "http".to_string()
            } else {
                "https".to_string()
            }
        });
        format!("{}://{}/v2/", protocol, self.hostname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_base_url_is_public_v2_api() {
        let cfg = FrankfurterConfig::default();
        assert_eq!(cfg.base_url(), "https://api.frankfurter.dev/v2/");
    }

    #[test]
    fn localhost_falls_back_to_http() {
        let cfg = FrankfurterConfig {
            hostname: "localhost:4000".into(),
            ..Default::default()
        };
        assert_eq!(cfg.base_url(), "http://localhost:4000/v2/");
    }

    #[test]
    fn explicit_protocol_overrides_the_default() {
        let cfg = FrankfurterConfig {
            protocol: Some("http".into()),
            ..Default::default()
        };
        assert_eq!(cfg.base_url(), "http://api.frankfurter.dev/v2/");
    }
}
