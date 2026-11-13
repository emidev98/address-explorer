use crate::clients::mempool::bitcoin::types::{Tx, TxMerkleProof, TxOutspend, TxStatus};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::{encode_txid, HttpClient};
#[derive(Clone)]
pub struct Transactions {
    http: HttpClient,
}
impl Transactions {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
    pub async fn get_tx(&self, txid: &str) -> Result<Tx, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get(&format!("/tx/{txid}")).await
    }
    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get(&format!("/tx/{txid}/status")).await
    }
    pub async fn get_tx_hex(&self, txid: &str) -> Result<String, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get_text(&format!("/tx/{txid}/hex")).await
    }
    pub async fn get_tx_raw(&self, txid: &str) -> Result<String, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get_text(&format!("/tx/{txid}/raw")).await
    }
    pub async fn get_tx_merkle_block_proof(&self, txid: &str) -> Result<String, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http
            .get_text(&format!("/tx/{txid}/merkleblock-proof"))
            .await
    }
    pub async fn get_tx_merkle_proof(&self, txid: &str) -> Result<TxMerkleProof, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get(&format!("/tx/{txid}/merkle-proof")).await
    }
    pub async fn get_tx_outspend(&self, txid: &str, vout: i64) -> Result<TxOutspend, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get(&format!("/tx/{txid}/outspend/{vout}")).await
    }
    pub async fn get_tx_outspends(&self, txid: &str) -> Result<Vec<TxOutspend>, MempoolError> {
        let txid = encode_txid(txid)?;
        self.http.get(&format!("/tx/{txid}/outspends")).await
    }
    pub async fn post_tx(&self, txhex: &str) -> Result<String, MempoolError> {
        self.http.post_text("/tx", txhex.to_string()).await
    }
}
