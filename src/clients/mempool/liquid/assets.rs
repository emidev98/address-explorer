use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;
use crate::clients::mempool::liquid::types::Asset;

#[derive(Clone)]
pub struct Assets {
    http: HttpClient,
}

impl Assets {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
    pub async fn get_asset(&self, asset_id: &str) -> Result<Asset, MempoolError> {
        self.http.get(&format!("/asset/{}", asset_id)).await
    }
    pub async fn get_asset_icon(&self, asset_id: &str) -> Result<serde_json::Value, MempoolError> {
        self.http.get(&format!("/v1/asset/{}/icon", asset_id)).await
    }
    pub async fn get_asset_txs(
        &self,
        asset_id: &str,
        is_mempool: bool,
    ) -> Result<Asset, MempoolError> {
        let suffix = if is_mempool { "/mempool" } else { "/chain" };
        self.http
            .get(&format!("/asset/{}/txs{}", asset_id, suffix))
            .await
    }
    pub async fn get_asset_supply(
        &self,
        asset_id: &str,
        decimal: bool,
    ) -> Result<Asset, MempoolError> {
        let suffix = if decimal { "/decimal" } else { "" };
        self.http
            .get(&format!("/asset/{}/supply{}", asset_id, suffix))
            .await
    }
    pub async fn get_assets_icons(&self) -> Result<Vec<String>, MempoolError> {
        self.http.get("/v1/assets/icons").await
    }
}
