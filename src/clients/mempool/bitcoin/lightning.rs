use crate::clients::mempool::bitcoin::types::{Channel, NetworkStats, Node, NodeStats};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Lightning {
    http: HttpClient,
}

impl Lightning {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_network_stats(&self) -> Result<NetworkStats, MempoolError> {
        self.http.get("/v1/lightning/statistics/latest").await
    }

    pub async fn get_nodes_in_country(&self, country: &str) -> Result<Vec<Node>, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/nodes/country/{}", country))
            .await
    }

    pub async fn get_nodes_stats_per_country(&self) -> Result<serde_json::Value, MempoolError> {
        self.http.get("/v1/lightning/nodes/countries").await
    }

    pub async fn get_nodes_hosted_by_isp(&self, isp: i64) -> Result<Vec<Node>, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/nodes/isp/{}", isp))
            .await
    }

    pub async fn get_isp_ranking(&self) -> Result<serde_json::Value, MempoolError> {
        self.http.get("/v1/lightning/nodes/isp-ranking").await
    }

    pub async fn get_liquidity_ranking(&self) -> Result<Vec<Node>, MempoolError> {
        self.http
            .get("/v1/lightning/nodes/rankings/liquidity")
            .await
    }

    pub async fn get_connectivity_ranking(&self) -> Result<Vec<Node>, MempoolError> {
        self.http
            .get("/v1/lightning/nodes/rankings/connectivity")
            .await
    }

    pub async fn get_oldest_nodes(&self) -> Result<Vec<Node>, MempoolError> {
        self.http.get("/v1/lightning/nodes/rankings/age").await
    }

    pub async fn get_node_stats(&self, public_key: &str) -> Result<Node, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/nodes/{}", public_key))
            .await
    }

    pub async fn get_historical_node_stats(
        &self,
        public_key: &str,
    ) -> Result<Vec<NodeStats>, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/nodes/{}/statistics", public_key))
            .await
    }

    pub async fn get_channel(&self, id: &str) -> Result<Channel, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/channels/{}", id))
            .await
    }

    pub async fn get_channels_from_txids(
        &self,
        tx_ids: &[String],
    ) -> Result<Vec<serde_json::Value>, MempoolError> {
        let query = tx_ids
            .iter()
            .map(|id| format!("txId[]={}", id))
            .collect::<Vec<_>>()
            .join("&");
        self.http
            .get(&format!("/v1/lightning/channels/txids?{}", query))
            .await
    }

    pub async fn get_channels_from_node_public_key(
        &self,
        public_key: &str,
        status: &str,
        index: Option<i64>,
    ) -> Result<Vec<Channel>, MempoolError> {
        let idx = index.unwrap_or(0);
        self.http
            .get(&format!(
                "/v1/lightning/channels?public_key={}&status={}&index={}",
                public_key, status, idx
            ))
            .await
    }

    pub async fn get_channels_geodata(&self) -> Result<serde_json::Value, MempoolError> {
        self.http.get("/v1/lightning/channels-geo").await
    }

    pub async fn get_channels_geodata_by_public_key(
        &self,
        public_key: &str,
    ) -> Result<Vec<serde_json::Value>, MempoolError> {
        self.http
            .get(&format!("/v1/lightning/channels-geo/{}", public_key))
            .await
    }
}
