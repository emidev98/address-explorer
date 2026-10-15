use crate::clients::mempool::config::MempoolConfig;
use serde_json::json;

pub fn bitcoin_endpoint(config: &MempoolConfig) -> String {
    config.bitcoin_ws_endpoint()
}

pub fn liquid_endpoint(config: &MempoolConfig) -> String {
    config.liquid_ws_endpoint()
}

fn action(value: serde_json::Value) -> String {
    value.to_string()
}

pub fn init() -> String {
    action(json!({"action": "init"}))
}
pub fn want(options: &[String]) -> String {
    action(json!({"action": "want", "data": options}))
}
pub fn stop_data() -> String {
    action(json!({"action": "want", "data": []}))
}
pub fn track_address(address: &str) -> String {
    action(json!({"track-address": address}))
}
pub fn stop_tracking_address() -> String {
    action(json!({"track-address": "stop"}))
}
pub fn track_addresses(addresses: &[String]) -> String {
    action(json!({"track-addresses": addresses}))
}
pub fn stop_tracking_addresses() -> String {
    action(json!({"track-addresses": []}))
}
pub fn track_transaction(txid: &str) -> String {
    action(json!({"track-tx": txid}))
}
pub fn stop_tracking_transaction() -> String {
    action(json!({"track-tx": "stop"}))
}
pub fn track_rbf_summary() -> String {
    action(json!({"track-rbf-summary": true}))
}
pub fn stop_tracking_rbf_summary() -> String {
    action(json!({"track-rbf-summary": false}))
}
pub fn track_rbf(full_rbf: bool) -> String {
    let value = if full_rbf { "fullRbf" } else { "all" };
    action(json!({"track-rbf": value}))
}
pub fn stop_tracking_rbf() -> String {
    action(json!({"track-rbf": "stop"}))
}
pub fn track_mempool_block(index: i64) -> String {
    action(json!({"track-mempool-block": index}))
}
pub fn stop_tracking_mempool_block() -> String {
    action(json!({"track-mempool-block": -1}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn want_message() {
        let msg = want(&["blocks".to_string()]);
        assert!(msg.contains("\"want\""));
    }

    #[test]
    fn endpoint_wss() {
        let cfg = crate::clients::mempool::MempoolConfig::default();
        assert!(bitcoin_endpoint(&cfg).starts_with("wss://"));
    }
}
