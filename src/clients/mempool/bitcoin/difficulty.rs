use crate::clients::mempool::bitcoin::types::{Adjustment, Hashrate};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Difficulty {
    http: HttpClient,
}

impl Difficulty {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_difficulty_adjustment(&self) -> Result<Adjustment, MempoolError> {
        self.http.get("/v1/difficulty-adjustment").await
    }

    pub async fn get_hashrate(&self, interval: &str) -> Result<Hashrate, MempoolError> {
        self.http
            .get(&format!("/v1/mining/hashrate/{}", interval))
            .await
    }
}
