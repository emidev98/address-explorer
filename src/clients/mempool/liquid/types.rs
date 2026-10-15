pub use crate::clients::mempool::bitcoin::types::{
    Block, BlockStatus, FeesMempoolBlocks, FeesRecommended, Mempool, MempoolRecent, Tx,
    TxMerkleProof, TxOutspend, TxStatus,
};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AssetStats {
    pub tx_count: i64,
    pub peg_in_count: i64,
    pub peg_in_amount: i64,
    pub peg_out_count: i64,
    pub peg_out_amount: i64,
    pub burn_count: i64,
    pub burned_amount: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Asset {
    pub asset_id: String,
    pub chain_stats: AssetStats,
    pub mempool_stats: AssetStats,
}
