use crate::clients::mempool::bitcoin::types::{Address, AddressTxsUtxo, Tx};
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::http::HttpClient;

#[derive(Clone)]
pub struct Addresses {
    http: HttpClient,
}

impl Addresses {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_address(&self, address: &str) -> Result<Address, MempoolError> {
        self.http.get(&format!("/address/{}", address)).await
    }

    pub async fn get_address_txs(
        &self,
        address: &str,
        after_txid: Option<&str>,
    ) -> Result<Vec<Tx>, MempoolError> {
        if let Some(after) = after_txid {
            self.http
                .get(&format!("/address/{}/txs?after_txid={}", address, after))
                .await
        } else {
            self.http.get(&format!("/address/{}/txs", address)).await
        }
    }

    pub async fn get_address_txs_chain(&self, address: &str) -> Result<Vec<Tx>, MempoolError> {
        self.http
            .get(&format!("/address/{}/txs/chain", address))
            .await
    }

    pub async fn get_address_txs_mempool(&self, address: &str) -> Result<Vec<Tx>, MempoolError> {
        self.http
            .get(&format!("/address/{}/txs/mempool", address))
            .await
    }

    pub async fn get_address_txs_utxo(
        &self,
        address: &str,
    ) -> Result<Vec<AddressTxsUtxo>, MempoolError> {
        self.http.get(&format!("/address/{}/utxo", address)).await
    }
}
