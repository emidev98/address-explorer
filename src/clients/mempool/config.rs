use reqwest::header::HeaderMap;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct MempoolConfig {
    pub hostname: String,
    pub network: Option<String>,
    pub protocol: Option<String>,
    pub headers: Option<HeaderMap>,
    pub timeout: Option<Duration>,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            hostname: "mempool.space".to_string(),
            network: Some("main".to_string()),
            protocol: None,
            headers: None,
            timeout: None,
        }
    }
}

impl MempoolConfig {
    pub fn bitcoin_base_url(&self) -> String {
        let protocol = self.protocol.clone().unwrap_or_else(|| {
            if self.hostname.contains("localhost") {
                "http".to_string()
            } else {
                "https".to_string()
            }
        });
        let network = self.network.clone().unwrap_or_else(|| "main".to_string());
        let network_path = match network.as_str() {
            "testnet" | "testnet4" | "signet" => format!("/{}", network),
            _ => String::new(),
        };
        format!("{}://{}{}/api/", protocol, self.hostname, network_path)
    }

    pub fn liquid_base_url(&self) -> String {
        let protocol = self.protocol.clone().unwrap_or_else(|| {
            if self.hostname.contains("localhost") {
                "http".to_string()
            } else {
                "https".to_string()
            }
        });
        let network = self.network.clone().unwrap_or_else(|| "main".to_string());
        let network_path = match network.as_str() {
            "testnet" | "liquidtestnet" => "/liquidtestnet".to_string(),
            _ => "/liquid".to_string(),
        };
        format!("{}://{}{}/api/", protocol, self.hostname, network_path)
    }

    pub fn bitcoin_ws_endpoint(&self) -> String {
        ws_endpoint(
            &self.hostname,
            &self.network.clone().unwrap_or_default(),
            self.protocol.as_deref(),
            true,
        )
    }

    pub fn liquid_ws_endpoint(&self) -> String {
        ws_endpoint(
            &self.hostname,
            &self.network.clone().unwrap_or_default(),
            self.protocol.as_deref(),
            false,
        )
    }
}

fn ws_endpoint(hostname: &str, network: &str, protocol: Option<&str>, bitcoin: bool) -> String {
    let protocol = match protocol {
        None => {
            if hostname.contains("localhost") {
                "ws"
            } else {
                "wss"
            }
        }
        Some("http") | Some("ws") => "ws",
        _ => "wss",
    };

    let network_path = if bitcoin {
        match network {
            "testnet" | "testnet4" | "signet" => format!("/{}", network),
            _ => String::new(),
        }
    } else {
        match network {
            "testnet" | "liquidtestnet" => "/liquidtestnet".to_string(),
            _ => "/liquid".to_string(),
        }
    };

    format!("{}://{}{}/api/v1/ws", protocol, hostname, network_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitcoin_default_url() {
        let cfg = MempoolConfig::default();
        assert_eq!(cfg.bitcoin_base_url(), "https://mempool.space/api/");
    }

    #[test]
    fn bitcoin_testnet_url() {
        let cfg = MempoolConfig {
            network: Some("testnet".into()),
            ..Default::default()
        };
        assert_eq!(cfg.bitcoin_base_url(), "https://mempool.space/testnet/api/");
    }

    #[test]
    fn liquid_default_url() {
        let cfg = MempoolConfig::default();
        assert_eq!(cfg.liquid_base_url(), "https://mempool.space/liquid/api/");
    }
}
