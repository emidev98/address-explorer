use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;
use crate::clients::mempool::liquid::types::{Tx, TxMerkleProof, TxOutspend, TxStatus};

#[derive(Clone)]
pub struct Transactions {
    http: HttpClient,
}

impl Transactions {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
    pub async fn get_tx(&self, txid: &str) -> Result<Tx, MempoolError> {
        self.http.get(&format!("/tx/{}", txid)).await
    }
    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus, MempoolError> {
        self.http.get(&format!("/tx/{}/status", txid)).await
    }
    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, MempoolError> {
        self.http.get_text(&format!("/tx/{}/hex", txid)).await
    }
    pub async fn get_tx_raw(&self, txid: &str) -> Result<String, MempoolError> {
        self.http.get_text(&format!("/tx/{}/raw", txid)).await
    }
    pub async fn get_tx_merkle_proof(&self, txid: &str) -> Result<TxMerkleProof, MempoolError> {
        self.http.get(&format!("/tx/{}/merkle-proof", txid)).await
    }
    pub async fn get_tx_outspend(&self, txid: &str, vout: i64) -> Result<TxOutspend, MempoolError> {
        self.http
            .get(&format!("/tx/{}/outspend/{}", txid, vout))
            .await
    }
    pub async fn get_tx_outspends(&self, txid: &str) -> Result<Vec<TxOutspend>, MempoolError> {
        self.http.get(&format!("/tx/{}/outspends", txid)).await
    }
    pub async fn post_tx(&self, txhex: &str) -> Result<String, MempoolError> {
        self.http.post_text("/tx", txhex.to_string()).await
    }
}
