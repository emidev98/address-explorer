use crate::clients::mempool::bitcoin::types::{Mempool as MempoolInfo, MempoolRecent};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct MempoolEndpoints {
    http: HttpClient,
}

impl MempoolEndpoints {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_mempool(&self) -> Result<Vec<MempoolInfo>, MempoolError> {
        self.http.get("/mempool").await
    }

    pub async fn get_mempool_txids(&self) -> Result<Vec<String>, MempoolError> {
        self.http.get("/mempool/txids").await
    }

    pub async fn get_mempool_recent(&self) -> Result<Vec<MempoolRecent>, MempoolError> {
        self.http.get("/mempool/recent").await
    }
}
