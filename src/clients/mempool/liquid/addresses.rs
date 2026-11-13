use crate::clients::mempool::bitcoin::types::{Address, AddressTxsUtxo, Tx};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::{encode_path_segment, encode_txid, HttpClient};
#[derive(Clone)]
pub struct Addresses {
    http: HttpClient,
}
impl Addresses {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }
    pub async fn get_address(&self, address: &str) -> Result<Address, MempoolError> {
        let address = encode_path_segment(address);
        self.http.get(&format!("/address/{address}")).await
    }
    pub async fn get_address_txs(
        &self,
        address: &str,
        after_txid: Option<&str>,
    ) -> Result<Vec<Tx>, MempoolError> {
        let address = encode_path_segment(address);
        if let Some(after) = after_txid {
            let after = encode_txid(after)?;
            self.http
                .get(&format!("/address/{address}/txs?after_txid={after}"))
                .await
        } else {
            self.http.get(&format!("/address/{address}/txs")).await
        }
    }
    pub async fn get_address_txs_chain(&self, address: &str) -> Result<Vec<Tx>, MempoolError> {
        let address = encode_path_segment(address);
        self.http
            .get(&format!("/address/{address}/txs/chain"))
            .await
    }
    pub async fn get_address_txs_mempool(&self, address: &str) -> Result<Vec<Tx>, MempoolError> {
        let address = encode_path_segment(address);
        self.http
            .get(&format!("/address/{address}/txs/mempool"))
            .await
    }
    pub async fn get_address_txs_utxo(
        &self,
        address: &str,
    ) -> Result<Vec<AddressTxsUtxo>, MempoolError> {
        let address = encode_path_segment(address);
        self.http.get(&format!("/address/{address}/utxo")).await
    }
}
