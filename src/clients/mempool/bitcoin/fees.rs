use crate::clients::mempool::bitcoin::types::{FeesMempoolBlocks, FeesRecommended};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Fees {
    http: HttpClient,
}

impl Fees {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_fees_recommended(&self) -> Result<FeesRecommended, MempoolError> {
        self.http.get("/v1/fees/recommended").await
    }

    pub async fn get_fees_mempool_blocks(&self) -> Result<Vec<FeesMempoolBlocks>, MempoolError> {
        self.http.get("/v1/fees/mempool-blocks").await
    }

    pub async fn get_cpfp(&self, txid: &str) -> Result<Vec<FeesMempoolBlocks>, MempoolError> {
        self.http.get(&format!("/v1/cpfp/{}", txid)).await
    }
}
