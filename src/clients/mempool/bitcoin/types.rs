use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct StatsInfo {
    pub funded_txo_count: i64,
    pub funded_txo_sum: i64,
    pub spent_txo_count: i64,
    pub spent_txo_sum: i64,
    pub tx_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Address {
    pub address: String,
    pub chain_stats: StatsInfo,
    pub mempool_stats: StatsInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddressTxsUtxo {
    pub txid: String,
    pub vout: i64,
    pub status: TxStatus,
    pub value: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vout {
    pub scriptpubkey: String,
    pub scriptpubkey_asm: String,
    pub scriptpubkey_type: String,
    pub scriptpubkey_address: String,
    pub value: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxStatus {
    pub confirmed: bool,
    pub block_height: i64,
    pub block_hash: String,
    pub block_time: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tx {
    pub txid: String,
    pub version: i64,
    pub locktime: i64,
    pub vin: Vec<TxVin>,
    pub vout: Vec<Vout>,
    pub size: i64,
    pub weight: i64,
    pub fee: i64,
    pub status: TxStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxVin {
    pub txid: String,
    pub vout: i64,
    pub prevout: Vout,
    pub scriptsig: String,
    pub scriptsig_asm: String,
    pub is_coinbase: bool,
    pub sequence: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxMerkleProof {
    pub block_height: i64,
    pub merkle: Vec<String>,
    pub pos: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxOutspend {
    pub spent: bool,
    pub txid: String,
    pub vin: i64,
    pub status: TxStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    pub id: String,
    pub height: i64,
    pub version: i64,
    pub timestamp: i64,
    pub tx_count: i64,
    pub size: i64,
    pub weight: i64,
    pub merkle_root: String,
    pub previousblockhash: String,
    pub mediantime: i64,
    pub nonce: i64,
    pub bits: i64,
    pub difficulty: f64,
    pub extras: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockStatus {
    pub in_best_chain: bool,
    pub height: i64,
    pub next_best: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Adjustment {
    #[serde(rename = "progressPercent")]
    pub progress_percent: f64,
    #[serde(rename = "difficultyChange")]
    pub difficulty_change: f64,
    #[serde(rename = "estimatedRetargetDate")]
    pub estimated_retarget_date: i64,
    #[serde(rename = "remainingBlocks")]
    pub remaining_blocks: i64,
    #[serde(rename = "remainingTime")]
    pub remaining_time: i64,
    #[serde(rename = "previousRetarget")]
    pub previous_retarget: i64,
    #[serde(rename = "nextRetargetHeight")]
    pub next_retarget_height: i64,
    #[serde(rename = "timeAvg")]
    pub time_avg: f64,
    #[serde(rename = "timeOffset")]
    pub time_offset: f64,
    #[serde(rename = "expectedBlocks")]
    pub expected_blocks: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HashRateData {
    pub timestamp: i64,
    #[serde(rename = "avgHashrate")]
    pub avg_hashrate: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DifficultyData {
    pub time: i64,
    pub height: i64,
    pub difficulty: f64,
    pub adjustment: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hashrate {
    pub hashrates: Vec<HashRateData>,
    pub difficulty: Vec<DifficultyData>,
    #[serde(rename = "currentHashrate")]
    pub current_hashrate: f64,
    #[serde(rename = "currentDifficulty")]
    pub current_difficulty: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeesMempoolBlocks {
    #[serde(rename = "blockSize")]
    pub block_size: i64,
    #[serde(rename = "blockVSize")]
    pub block_vsize: i64,
    #[serde(rename = "nTx")]
    pub n_tx: i64,
    #[serde(rename = "totalFees")]
    pub total_fees: i64,
    #[serde(rename = "medianFee")]
    pub median_fee: f64,
    #[serde(rename = "feeRange")]
    pub fee_range: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FeesRecommended {
    #[serde(rename = "fastestFee")]
    pub fastest_fee: i64,
    #[serde(rename = "halfHourFee")]
    pub half_hour_fee: i64,
    #[serde(rename = "hourFee")]
    pub hour_fee: i64,
    #[serde(rename = "minimumFee")]
    pub minimum_fee: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mempool {
    pub count: i64,
    pub vsize: i64,
    pub total_fee: i64,
    pub fee_histogram: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MempoolRecent {
    pub txid: String,
    pub fee: f64,
    pub vsize: i64,
    pub value: i64,
}

// Lightning
#[derive(Debug, Clone, Deserialize)]
pub struct NetworkStatsEntry {
    pub added: String,
    pub avg_base_fee_mtokens: f64,
    pub avg_capacity: f64,
    pub avg_fee_rate: f64,
    pub channel_count: i64,
    pub clearnet_nodes: i64,
    pub clearnet_tor_nodes: i64,
    pub id: i64,
    pub med_base_fee_mtokens: f64,
    pub med_capacity: f64,
    pub med_fee_rate: f64,
    pub node_count: i64,
    pub tor_nodes: i64,
    pub total_capacity: i64,
    pub unannounced_nodes: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkStats {
    pub latest: NetworkStatsEntry,
    pub previous: NetworkStatsEntry,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Node {
    pub alias: String,
    pub public_key: String,
    pub channels: i64,
    pub capacity: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub base_fee_mtokens: Option<f64>,
    pub cltv_delta: Option<i64>,
    pub fee_rate: Option<f64>,
    pub is_disabled: Option<bool>,
    pub max_htlc_mtokens: Option<i64>,
    pub min_htlc_mtokens: Option<i64>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub funding_balance: Option<i64>,
    pub closing_balance: Option<i64>,
    pub city: Option<serde_json::Value>,
    pub country: Option<serde_json::Value>,
    pub iso_code: Option<serde_json::Value>,
    pub subdivision: Option<serde_json::Value>,
    pub first_seen: Option<i64>,
    pub isp: Option<String>,
    pub features: Option<Vec<serde_json::Value>>,
    #[serde(rename = "featuresBit")]
    pub features_bit: Option<String>,
    pub active_channel_count: Option<i64>,
    pub opened_channel_count: Option<i64>,
    pub closed_channel_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeStats {
    pub added: i64,
    pub capacity: i64,
    pub channels: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Channel {
    pub id: i64,
    pub short_id: String,
    pub capacity: i64,
    pub transaction_id: String,
    pub transaction_vout: i64,
    pub closing_transaction_id: String,
    pub closing_reason: i64,
    pub closing_fee: i64,
    pub updated_at: String,
    pub closing_date: Option<String>,
    pub created: String,
    pub status: i64,
    pub funding_ratio: Option<f64>,
    pub closed_by: Option<String>,
    pub single_funded: Option<bool>,
    pub node_left: Node,
    pub node_right: Node,
}
