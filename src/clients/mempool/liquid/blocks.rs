use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;
use crate::clients::mempool::liquid::types::{Block, BlockStatus, Tx};

#[derive(Clone)]
pub struct Blocks {
    http: HttpClient,
}

impl Blocks {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
    pub async fn get_block(&self, hash: &str) -> Result<Block, MempoolError> {
        self.http.get(&format!("/v1/block/{}", hash)).await
    }
    pub async fn get_block_status(&self, hash: &str) -> Result<BlockStatus, MempoolError> {
        self.http.get(&format!("/block/{}/status", hash)).await
    }
    pub async fn get_block_txs(
        &self,
        hash: &str,
        start_index: Option<i64>,
    ) -> Result<Tx, MempoolError> {
        match start_index {
            Some(i) => self.http.get(&format!("/block/{}/txs/{}", hash, i)).await,
            None => self.http.get(&format!("/block/{}/txs", hash)).await,
        }
    }
    pub async fn get_block_txids(&self, hash: &str) -> Result<Vec<String>, MempoolError> {
        self.http.get(&format!("/block/{}/txids", hash)).await
    }
    pub async fn get_block_txid(&self, hash: &str, index: i64) -> Result<String, MempoolError> {
        self.http
            .get_text(&format!("/block/{}/txid/{}", hash, index))
            .await
    }
    pub async fn get_block_raw(&self, hash: &str) -> Result<String, MempoolError> {
        self.http.get_text(&format!("/block/{}/raw", hash)).await
    }
    pub async fn get_block_height(&self, height: i64) -> Result<String, MempoolError> {
        self.http
            .get_text(&format!("/block-height/{}", height))
            .await
    }
    pub async fn get_blocks(&self, start_height: Option<i64>) -> Result<Vec<Block>, MempoolError> {
        match start_height {
            Some(h) => self.http.get(&format!("/v1/blocks/{}", h)).await,
            None => self.http.get("/v1/blocks").await,
        }
    }
    pub async fn get_blocks_tip_height(&self) -> Result<i64, MempoolError> {
        self.http.get("/blocks/tip/height").await
    }
    pub async fn get_blocks_tip_hash(&self) -> Result<String, MempoolError> {
        self.http.get_text("/blocks/tip/hash").await
    }
}
