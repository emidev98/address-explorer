use serde::Deserialize;
use serde_json::{Map, Value};

/// Deserializers that accept either JSON numbers or numeric strings. Coin
/// Metrics serializes some numeric fields as strings in metric responses and
/// uses numbers in market responses, so typed models accept both.
pub(crate) mod number {
    use serde::de::{self, Deserializer};
    use serde::Deserialize;
    use serde_json::Value;

    fn value_to_f64<E: de::Error>(value: Value) -> Result<f64, E> {
        match value {
            Value::Number(number) => number
                .as_f64()
                .ok_or_else(|| E::custom("number is not representable as f64")),
            Value::String(text) => text.parse::<f64>().map_err(E::custom),
            other => Err(E::custom(format!(
                "expected a number or numeric string, got {other}"
            ))),
        }
    }

    fn value_to_i64<E: de::Error>(value: Value) -> Result<i64, E> {
        match value {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_f64().map(|value| value as i64))
                .ok_or_else(|| E::custom("number is not representable as i64")),
            Value::String(text) => text.parse::<i64>().map_err(E::custom),
            other => Err(E::custom(format!(
                "expected an integer or numeric string, got {other}"
            ))),
        }
    }

    pub fn f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        value_to_f64(Value::deserialize(deserializer)?)
    }

    pub fn opt_f64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
        match Option::<Value>::deserialize(deserializer)? {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value_to_f64(value).map(Some),
        }
    }

    pub fn i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
        value_to_i64(Value::deserialize(deserializer)?)
    }

    pub fn opt_i64<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<i64>, D::Error> {
        match Option::<Value>::deserialize(deserializer)? {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value_to_i64(value).map(Some),
        }
    }
}

/// Standard `{ "data": [...], "next_page_token": ..., "next_page_url": ... }`
/// envelope used by nearly every list endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub next_page_token: Option<String>,
    pub next_page_url: Option<String>,
    /// Non-fatal warning returned by some blockchain and reference-data
    /// endpoints.
    pub warning: Option<Warning>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Warning {
    pub r#type: String,
    pub message: String,
}

/// API error envelope. Used by the HTTP client to build
/// [`crate::clients::coinmetrics::CoinMetricsError::Api`].
#[derive(Debug, Clone, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorObject,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorObject {
    pub r#type: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Reference data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct AssetInfo {
    pub asset: String,
    pub full_name: String,
    /// Talos metadata, returned only with `include=talos`.
    pub talos: Option<Vec<TalosAssetMetadata>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TalosAssetMetadata {
    pub cm_ext_asset: Option<String>,
    pub cm_ext_asset_by_id: Option<String>,
    pub symbol: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub currency_id: Option<i64>,
    pub description: Option<String>,
    pub alias_for_currency: Option<String>,
    pub alias_for_chain_symbol: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetGroupInfo {
    pub group: String,
    pub constituents: AssetGroupConstituents,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetGroupConstituents {
    #[serde(default)]
    pub assets: Vec<String>,
    #[serde(default)]
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeInfo {
    pub exchange: String,
    pub full_name: String,
    /// Talos metadata, returned only with `include=talos`.
    pub talos: Option<Vec<TalosExchangeMetadata>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TalosExchangeMetadata {
    pub cm_ext_exchange: Option<String>,
    pub cm_ext_exchange_by_id: Option<String>,
    pub market: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub market_id: Option<i64>,
    pub display_name: Option<String>,
    pub short_name: Option<String>,
    #[serde(rename = "type")]
    pub exchange_type: Option<String>,
    pub color: Option<String>,
    pub depth_type: Option<String>,
    #[serde(default)]
    pub capabilities: Option<Map<String, Value>>,
    #[serde(default)]
    pub flags: Option<Map<String, Value>>,
    #[serde(default)]
    pub supported_time_in_force: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketInfo {
    pub market: String,
    pub exchange: String,
    #[serde(rename = "type")]
    pub market_type: String,
    pub base: Option<String>,
    pub quote: Option<String>,
    pub pair: Option<String>,
    pub symbol: Option<String>,
    pub asset_class: Option<String>,
    pub source_type: Option<String>,
    pub size_asset: Option<String>,
    pub margin_asset: Option<String>,
    pub strike: Option<String>,
    pub option_contract_type: Option<String>,
    pub is_european: Option<bool>,
    pub contract_size: Option<String>,
    pub tick_size: Option<String>,
    pub multiplier_size: Option<String>,
    pub listing: Option<String>,
    pub expiration: Option<String>,
    pub settlement_price: Option<String>,
    pub pool_config_id: Option<String>,
    pub contract_address: Option<String>,
    pub fee: Option<String>,
    pub price_includes_fee: Option<bool>,
    pub variable_fee: Option<bool>,
    pub base_address: Option<String>,
    pub quote_address: Option<String>,
    pub status: Option<String>,
    pub order_amount_increment: Option<String>,
    pub order_amount_min: Option<String>,
    pub order_amount_max: Option<String>,
    pub order_price_increment: Option<String>,
    pub order_price_min: Option<String>,
    pub order_price_max: Option<String>,
    pub order_size_min: Option<String>,
    pub order_taker_fee: Option<String>,
    pub order_maker_fee: Option<String>,
    pub margin_trading_enabled: Option<bool>,
    pub experimental: Option<bool>,
    pub base_native: Option<String>,
    pub quote_native: Option<String>,
    pub publisher: Option<String>,
    pub cusip: Option<String>,
    pub figi: Option<String>,
    pub isin: Option<String>,
    pub lei: Option<String>,
    pub sedol: Option<String>,
    pub security_type: Option<String>,
    pub security_term: Option<String>,
    pub description: Option<String>,
    pub coupon_rate: Option<String>,
    pub issue_date: Option<String>,
    pub maturity_date: Option<String>,
    pub price_per100: Option<String>,
    /// Talos metadata, returned only with `include=talos`.
    pub talos: Option<Vec<TalosMarketMetadata>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TalosMarketMetadata {
    pub cm_ext_market: Option<String>,
    pub cm_ext_market_by_id: Option<String>,
    pub market: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub market_id: Option<i64>,
    pub symbol: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub security_id: Option<i64>,
    pub market_symbol: Option<String>,
    pub product_type: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub base_currency_id: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub quote_currency_id: Option<i64>,
    pub end_time: Option<String>,
    pub display_name: Option<String>,
    pub normal_size: Option<String>,
    pub description: Option<String>,
    pub base_currency: Option<String>,
    pub quote_currency: Option<String>,
    pub counter_currency: Option<String>,
    pub option_type: Option<String>,
    pub strike_price: Option<String>,
    pub settlement_currency: Option<String>,
    pub notional_multiplier: Option<String>,
    pub expiration: Option<String>,
    pub settle_value_type: Option<String>,
    #[serde(default)]
    pub size_buckets: Vec<TalosMarketSizeBucket>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub position_currency_id: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub underlying_security_id: Option<i64>,
    pub underlying_quote_currency: Option<String>,
    pub underlying_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TalosMarketSizeBucket {
    pub size: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexInfo {
    pub index: String,
    pub full_name: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub index_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairInfo {
    pub pair: String,
    pub full_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolInfo {
    pub market: String,
    pub full_name: String,
    pub protocol: String,
    #[serde(rename = "type")]
    pub protocol_type: String,
    #[serde(default)]
    pub chains: Vec<ProtocolChain>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolChain {
    pub chain: String,
    pub address: Option<String>,
    pub asset: Option<String>,
    pub asset_address: Option<String>,
    pub asset_decimals: Option<String>,
    pub configurator_address: Option<String>,
    pub atoken_address: Option<String>,
    pub atoken_asset: Option<String>,
    pub variable_debt_token_address: Option<String>,
    pub variable_debt_asset: Option<String>,
    pub stable_debt_token_address: Option<String>,
    pub curator_address: Option<String>,
    pub curator_name: Option<String>,
    pub guardian_address: Option<String>,
    pub guardian_name: Option<String>,
    pub collateral_asset: Option<String>,
    pub collateral_asset_address: Option<String>,
    pub oracle_address: Option<String>,
    pub irm_address: Option<String>,
    pub lltv: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricInfo {
    pub metric: String,
    pub full_name: String,
    pub description: String,
    pub product: String,
    pub category: String,
    pub subcategory: String,
    pub unit: String,
    pub data_type: String,
    #[serde(rename = "type")]
    pub metric_type: String,
    pub reviewable: Option<bool>,
    pub display_name: Option<String>,
    pub experimental: Option<bool>,
    pub constituent_snapshots_url: Option<String>,
    pub constituent_timeframes_url: Option<String>,
    pub docs_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Catalog v2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct AssetMetricsCatalogInfo {
    pub asset: String,
    #[serde(default)]
    pub metrics: Vec<AssetMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetMetricInfo {
    pub metric: String,
    #[serde(default)]
    pub frequencies: Vec<AssetMetricFrequency>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetMetricFrequency {
    pub frequency: String,
    pub min_time: Option<String>,
    pub max_time: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub min_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub max_height: Option<i64>,
    pub min_hash: Option<String>,
    pub max_hash: Option<String>,
    pub community: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeMetricsCatalogInfo {
    pub exchange: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeAssetMetricsCatalogInfo {
    pub exchange_asset: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangePairMetricsCatalogInfo {
    pub exchange_pair: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairMetricsCatalogInfo {
    pub pair: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstitutionMetricsCatalogInfo {
    pub institution: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketMetricsCatalogInfo {
    pub market: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EntityMetricInfo {
    pub metric: String,
    #[serde(default)]
    pub frequencies: Vec<FrequencyRange>,
}

/// A frequency with its covered time range.
#[derive(Debug, Clone, Deserialize)]
pub struct FrequencyRange {
    pub frequency: String,
    pub min_time: Option<String>,
    pub max_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketTimeRangeCatalogInfo {
    pub market: String,
    pub min_time: String,
    pub max_time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetTimeRangeCatalogInfo {
    pub asset: String,
    pub min_time: String,
    pub max_time: String,
    /// Present on `/catalog-v2/blockchain-v2/*` entries.
    pub experimental: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketCandlesCatalogInfo {
    pub market: String,
    #[serde(default)]
    pub frequencies: Vec<FrequencyRange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketOrderBooksCatalogInfo {
    pub market: String,
    #[serde(default)]
    pub depths: Vec<OrderBookDepthInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookDepthInfo {
    pub depth: String,
    pub min_time: String,
    pub max_time: String,
    #[serde(default)]
    pub datasets: Vec<OrderBookDatasetInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookDatasetInfo {
    pub dataset: String,
    pub min_time: String,
    pub max_time: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairCandlesCatalogInfo {
    pub pair: String,
    #[serde(default)]
    pub frequencies: Vec<FrequencyRange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexCatalogInfo {
    pub index: String,
    #[serde(default)]
    pub frequencies: Vec<FrequencyRange>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolMetricsCatalogInfo {
    pub protocol: String,
    pub market: String,
    pub chain: String,
    #[serde(default)]
    pub metrics: Vec<EntityMetricInfo>,
}

// ---------------------------------------------------------------------------
// Time series
// ---------------------------------------------------------------------------

/// A metric row keyed by entity and time. Metric columns are kept in `values`
/// because the API returns an arbitrary subset of requested metrics as extra
/// JSON fields, usually as numeric strings.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetMetricRow {
    pub asset: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeMetricRow {
    pub exchange: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeAssetMetricRow {
    pub exchange_asset: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExchangePairMetricRow {
    pub exchange_pair: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairMetricRow {
    pub pair: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketMetricRow {
    pub market: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstitutionMetricRow {
    pub institution: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProtocolMetricRow {
    pub protocol: String,
    pub market: String,
    pub chain: String,
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConstituentSnapshotRow {
    pub time: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConstituentTimeframeRow {
    pub start_time: String,
    pub end_time: Option<String>,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketTrade {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price: f64,
    pub side: Option<String>,
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub block_height: Option<i64>,
    pub txid: Option<String>,
    pub initiator: Option<String>,
    pub sender: Option<String>,
    pub beneficiary: Option<String>,
    pub database_time: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mark_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub index_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_trade: Option<f64>,
    pub liquidation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketOpenInterest {
    pub market: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub contract_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub value_usd: f64,
    pub database_time: Option<String>,
    pub exchange_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketLiquidation {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price: f64,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub liquidation_type: String,
    pub database_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketFundingRate {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub rate: Option<f64>,
    pub period: Option<String>,
    pub interval: Option<String>,
    pub database_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketFundingRatePredicted {
    pub market: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub rate_predicted: f64,
    pub rate_time: Option<String>,
    pub database_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketOrderBook {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(default)]
    pub asks: Vec<OrderBookEntry>,
    #[serde(default)]
    pub bids: Vec<OrderBookEntry>,
    pub database_time: Option<String>,
    pub collect_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookEntry {
    #[serde(deserialize_with = "number::f64")]
    pub price: f64,
    #[serde(deserialize_with = "number::f64")]
    pub size: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketQuote {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub ask_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub ask_size: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub bid_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub bid_size: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketCandle {
    pub market: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub price_open: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_close: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_high: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_low: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub vwap: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub volume: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub candle_usd_volume: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub candle_trades_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketContractPrices {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mark_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub index_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub settlement_price_estimated: Option<f64>,
    pub database_time: Option<String>,
    pub exchange_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketImpliedVolatility {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_trade: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_bid: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_ask: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_mark: Option<f64>,
    pub database_time: Option<String>,
    pub exchange_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MarketGreeks {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub vega: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub theta: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub rho: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub delta: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub gamma: Option<f64>,
    pub database_time: Option<String>,
    pub exchange_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PairCandle {
    pub pair: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub price_open: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_close: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_high: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_low: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub vwap: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub volume: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub candle_usd_volume: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub candle_trades_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexCandle {
    pub index: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub price_open: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_close: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_high: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price_low: f64,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub candle_trades_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexLevel {
    pub index: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub level: Option<f64>,
    pub verification: Option<Verification>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Verification {
    pub timestamp: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub level: Option<f64>,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexConstituents {
    pub index: String,
    pub time: String,
    #[serde(default)]
    pub constituents: Vec<IndexConstituent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IndexConstituent {
    pub asset: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub weight: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub quantity: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub weight_basis_value: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MiningPoolTipsSummary {
    pub asset: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub tips_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub block_hashes_at_tip: f64,
    #[serde(default)]
    pub tips: Vec<MiningPoolTip>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MiningPoolTip {
    pub last_time: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub height: Option<i64>,
    pub hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub pool_count: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetChains {
    pub asset: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub chains_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub blocks_count_at_tip: f64,
    pub reorg: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub reorg_depth: Option<f64>,
    #[serde(default)]
    pub chains: Vec<Vec<AssetChainBlock>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetChainBlock {
    pub time: String,
    pub hash: String,
    #[serde(deserialize_with = "number::i64")]
    pub height: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MempoolFeerate {
    pub asset: String,
    pub time: String,
    #[serde(default)]
    pub feerates: Vec<MempoolFeerateBand>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MempoolFeerateBand {
    #[serde(deserialize_with = "number::f64")]
    pub feerate: f64,
    #[serde(deserialize_with = "number::f64")]
    pub count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub consensus_size: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub physical_size: Option<f64>,
    #[serde(deserialize_with = "number::f64")]
    pub fees: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetAlert {
    pub asset: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub block_height: Option<i64>,
    pub alert: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub value: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub threshold: Option<f64>,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetAlertHeartbeat {
    pub asset: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub block_height: Option<i64>,
    pub alert: String,
}

/// `/timeseries/asset-alerts` includes recent evaluation heartbeats next to
/// the alert rows.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetAlertsResponse {
    #[serde(default)]
    pub heartbeats: Vec<AssetAlertHeartbeat>,
    #[serde(default)]
    pub data: Vec<AssetAlert>,
    pub next_page_token: Option<String>,
    pub next_page_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefiBalanceSheet {
    pub defi_protocol: String,
    #[serde(deserialize_with = "number::i64")]
    pub block_height: i64,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub assets_total_usd: f64,
    #[serde(deserialize_with = "number::f64")]
    pub assets_total_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub liabilities_total_usd: f64,
    #[serde(deserialize_with = "number::f64")]
    pub liabilities_total_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub loans_lent_total_usd: f64,
    #[serde(deserialize_with = "number::f64")]
    pub tvl_total_usd: f64,
    #[serde(deserialize_with = "number::f64")]
    pub net_working_capital_usd: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub protocol_utilization_ratio: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub liquid_supply_ratio: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub current_ratio: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub debt_to_assets_ratio: Option<f64>,
    #[serde(default)]
    pub assets: Vec<DefiBalanceSheetAsset>,
    #[serde(default)]
    pub liabilities: Vec<DefiBalanceSheetLiability>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefiBalanceSheetAsset {
    pub asset: String,
    #[serde(deserialize_with = "number::f64")]
    pub total_units: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub loans_lent_units: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub tvl_units: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_usd: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub loans_lent_usd: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub tvl_usd: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_share: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub loans_lent_share: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub tvl_share: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DefiBalanceSheetLiability {
    pub asset: String,
    #[serde(deserialize_with = "number::f64")]
    pub total_units: f64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_usd: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_share: Option<f64>,
}

// ---------------------------------------------------------------------------
// Taxonomy and profiles
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct TaxonomyAsset {
    pub asset: String,
    pub full_name: String,
    pub taxonomy_version: String,
    pub updated_at_taxonomy_version: String,
    pub classification_start_time: String,
    pub classification_end_time: Option<String>,
    pub class_id: String,
    pub class: String,
    pub sector_id: String,
    pub sector: String,
    pub subsector_id: String,
    pub subsector: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaxonomyMetadataAsset {
    pub taxonomy_version: String,
    pub taxonomy_start_time: String,
    pub taxonomy_end_time: Option<String>,
    #[serde(default)]
    pub subsectors: Vec<TaxonomyMetadataSubsector>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaxonomyMetadataSubsector {
    pub class_id: Option<String>,
    pub class: Option<String>,
    pub sector_id: Option<String>,
    pub sector: Option<String>,
    pub subsector_id: Option<String>,
    pub subsector: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetProfile {
    pub asset: String,
    pub full_name: String,
    pub description: String,
    pub overview: String,
    #[serde(default)]
    pub token_purpose: Vec<String>,
    pub project_team: Option<String>,
    pub foundation: Option<String>,
    pub website: Option<String>,
    pub whitepaper_url: Option<String>,
    pub creation_date: String,
    pub token_generation_event_supply: Option<String>,
    pub supply_cap: Option<String>,
    #[serde(default)]
    pub initial_supply_token_distribution: Vec<InitialSupplyDistribution>,
    #[serde(default)]
    pub vesting_schedule: Vec<VestingSchedule>,
    #[serde(default)]
    pub new_token_issuance_recipient: Vec<String>,
    #[serde(default)]
    pub issuing_networks: Vec<String>,
    #[serde(default)]
    pub bridged_networks: Vec<String>,
    pub issuance_schedule_changes: Option<String>,
    #[serde(default)]
    pub significant_historical_changes: Vec<DatedDetails>,
    #[serde(default)]
    pub upgrade_history: Vec<DatedDetails>,
    pub blog_updates: Option<String>,
    #[serde(default)]
    pub project_github_repository: Vec<Repository>,
    pub asset_regulation: Option<String>,
    #[serde(default)]
    pub asset_regulated_products: Vec<String>,
    #[serde(default)]
    pub etp_custodians: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InitialSupplyDistribution {
    pub initial_supply_allocation: Option<String>,
    pub initial_supply_distribution: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VestingSchedule {
    pub vesting_schedule_allocation: Option<String>,
    pub vesting_schedule_cliff: Option<String>,
    pub vesting_schedule_total_unlock_time: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatedDetails {
    pub date: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Repository {
    pub github_repository: Option<String>,
    pub github_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkProfile {
    pub full_name: String,
    pub network: String,
    pub overview: Option<String>,
    pub consensus_mechanism: Option<String>,
    pub hashing_algorithm: Option<String>,
    pub transaction_finality: Option<String>,
    #[serde(default)]
    pub available_clients: Vec<String>,
    pub smart_contract_deployment: Option<String>,
    #[serde(default)]
    pub foundation: Vec<String>,
    pub founding_team_identity: Option<String>,
    #[serde(default)]
    pub repository: Vec<Repository>,
    pub transaction_visibility: Option<String>,
    pub native_fee_token: Option<String>,
    #[serde(default)]
    pub significant_historical_changes: Vec<DatedDetails>,
    #[serde(default)]
    pub upgrade_history: Vec<DatedDetails>,
    #[serde(default)]
    pub rollup_info: Vec<RollupInfo>,
    pub modular_or_monolithic: String,
    pub network_scaling_solution: Option<String>,
    pub type_of_network: Option<String>,
    #[serde(default)]
    pub chain_explorers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RollupInfo {
    pub rollup_type: Option<String>,
    pub state_validation: Option<String>,
}

// ---------------------------------------------------------------------------
// Security master
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityMasterAsset {
    pub asset: String,
    pub code: Option<String>,
    pub description: Option<String>,
    pub overview: Option<String>,
    pub website: Option<String>,
    pub whitepaper: Option<String>,
    pub decimals: Option<String>,
    pub creation_date: Option<String>,
    #[serde(rename = "type")]
    pub asset_type: Option<String>,
    pub parent_asset: Option<String>,
    pub pricing_asset: Option<String>,
    pub erc20_token_contract: Option<String>,
    pub fiat: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SecurityMasterMarket {
    pub market: String,
    #[serde(rename = "type")]
    pub market_type: String,
    pub exchange: String,
    pub code: Option<String>,
    pub pair: Option<String>,
    pub trades_min_time: Option<String>,
    pub trades_max_time: Option<String>,
    pub orderbooks_min_time: Option<String>,
    pub orderbooks_max_time: Option<String>,
    pub quotes_min_time: Option<String>,
    pub quotes_max_time: Option<String>,
    pub funding_rates_min_time: Option<String>,
    pub funding_rates_max_time: Option<String>,
    pub openinterest_min_time: Option<String>,
    pub openinterest_max_time: Option<String>,
    pub liquidations_min_time: Option<String>,
    pub liquidations_max_time: Option<String>,
    pub base: Option<String>,
    pub quote: Option<String>,
    pub symbol: Option<String>,
    pub size_asset: Option<String>,
    pub margin_asset: Option<String>,
    pub strike: Option<String>,
    pub option_contract_type: Option<String>,
    pub is_european: Option<bool>,
    pub contract_size: Option<String>,
    pub tick_size: Option<String>,
    pub multiplier_size: Option<String>,
    pub listing: Option<String>,
    pub expiration: Option<String>,
    pub settlement_price: Option<String>,
    pub pool_config_id: Option<String>,
    pub contract_address: Option<String>,
    pub fee: Option<String>,
    pub price_includes_fee: Option<bool>,
    pub variable_fee: Option<bool>,
    pub base_address: Option<String>,
    pub quote_address: Option<String>,
    pub status: Option<String>,
    pub order_amount_increment: Option<String>,
    pub order_amount_min: Option<String>,
    pub order_amount_max: Option<String>,
    pub order_price_increment: Option<String>,
    pub order_price_min: Option<String>,
    pub order_price_max: Option<String>,
    pub order_size_min: Option<String>,
    pub order_taker_fee: Option<String>,
    pub order_maker_fee: Option<String>,
    pub margin_trading_enabled: Option<bool>,
    pub experimental: Option<bool>,
    pub base_native: Option<String>,
    pub quote_native: Option<String>,
}

// ---------------------------------------------------------------------------
// Blockchain metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainMetadataLocation {
    pub location: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainMetadataOwner {
    pub owner_name: String,
    #[serde(rename = "type")]
    pub owner_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainMetadataTag {
    pub tag: String,
    #[serde(rename = "type")]
    pub tag_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainMetadataTaggedEntity {
    pub entity: String,
    pub owner_name: Option<String>,
    pub tag: String,
    pub location: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    pub tagger_type: String,
    pub tagger_method: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub start_block_height: Option<String>,
    pub end_block_height: Option<String>,
    pub start_block_hash: Option<String>,
    pub end_block_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Blockchain v2
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainBlock {
    pub block_hash: String,
    pub parent_block_hash: Option<String>,
    #[serde(deserialize_with = "number::i64")]
    pub height: i64,
    pub consensus_time: String,
    pub miner_time: Option<String>,
    pub nonce: Option<String>,
    pub extra_data: Option<String>,
    #[serde(deserialize_with = "number::f64")]
    pub n_transactions: f64,
    #[serde(deserialize_with = "number::f64")]
    pub n_balance_updates: f64,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub version: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub difficulty: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub physical_size: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub consensus_size: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub consensus_size_limit: Option<i64>,
    pub stale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainAccount {
    pub account: String,
    #[serde(rename = "type")]
    pub account_type: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_debits: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_credits: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub creation_height: Option<i64>,
    pub creation_block_hash: Option<String>,
    pub creation_time: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub creation_chain_sequence_number: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub last_chain_sequence_number: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub last_debit_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub last_credit_height: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainSubAccount {
    pub sub_account: String,
    pub account: String,
    #[serde(rename = "type")]
    pub account_type: String,
    #[serde(deserialize_with = "number::i64")]
    pub creation_height: i64,
    pub creation_time: String,
    #[serde(deserialize_with = "number::i64")]
    pub creation_chain_sequence_number: i64,
    pub creation_block_hash: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub balance: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainTransaction {
    pub txid: String,
    pub consensus_time: String,
    pub miner_time: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub tx_position: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub min_chain_sequence_number: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub max_chain_sequence_number: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub n_balance_updates: f64,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub version: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub physical_size: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub consensus_size: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub fee: Option<f64>,
    pub stale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainBalanceUpdate {
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub height: Option<i64>,
    pub consensus_time: Option<String>,
    pub txid: Option<String>,
    pub credit: Option<bool>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_received: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub total_sent: Option<f64>,
    #[serde(deserialize_with = "number::i64")]
    pub chain_sequence_number: i64,
    pub account: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub account_creation_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub change: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub rebasing_numerator: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub rebasing_denominator: Option<i64>,
    pub rebasing_rounding_mode: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub previous_balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub new_balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub transaction_sequence_number: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_debits: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_credits: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_debit_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_credit_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_chain_sequence_number: Option<i64>,
    pub sub_account: Option<BlockchainSubAccountChange>,
    pub stale: Option<String>,
    pub denomination: Option<String>,
}

/// Sub-account state carried by a balance update.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainSubAccountChange {
    pub sub_account: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub previous_balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub new_balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_debits: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_credits: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainRebasingChange {
    #[serde(deserialize_with = "number::i64")]
    pub chain_sequence_number: i64,
    pub block_hash: String,
    #[serde(deserialize_with = "number::i64")]
    pub height: i64,
    pub consensus_time: String,
    pub txid: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub transaction_sequence_number: Option<i64>,
    #[serde(deserialize_with = "number::i64")]
    pub previous_rebasing_numerator: i64,
    #[serde(deserialize_with = "number::i64")]
    pub previous_rebasing_denominator: i64,
    pub previous_rebasing_rounding_mode: String,
    #[serde(deserialize_with = "number::i64")]
    pub new_rebasing_numerator: i64,
    #[serde(deserialize_with = "number::i64")]
    pub new_rebasing_denominator: i64,
    pub new_rebasing_rounding_mode: String,
    pub stale: Option<String>,
}

/// Transaction balance update embedded in a full transaction or block.
#[derive(Debug, Clone, Deserialize)]
pub struct TransactionBalanceUpdate {
    #[serde(deserialize_with = "number::i64")]
    pub chain_sequence_number: i64,
    pub account: String,
    #[serde(deserialize_with = "number::i64")]
    pub account_creation_height: i64,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub change: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub rebasing_numerator: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub rebasing_denominator: Option<i64>,
    pub rebasing_rounding_mode: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub previous_balance: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub new_balance: Option<f64>,
    #[serde(deserialize_with = "number::i64")]
    pub transaction_sequence_number: i64,
    #[serde(deserialize_with = "number::i64")]
    pub n_debits: i64,
    #[serde(deserialize_with = "number::i64")]
    pub n_credits: i64,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_debit_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_credit_height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub previous_chain_sequence_number: Option<i64>,
    pub sub_account: Option<BlockchainSubAccountChange>,
    pub stale: Option<String>,
    pub denomination: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainBlockTransaction {
    pub txid: String,
    pub consensus_time: String,
    pub miner_time: Option<String>,
    #[serde(deserialize_with = "number::i64")]
    pub tx_position: i64,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub min_chain_sequence_number: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub max_chain_sequence_number: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub n_balance_updates: f64,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    pub stale: Option<String>,
}

/// A full block: block metadata plus its transactions and balance updates.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainFullBlock {
    #[serde(flatten)]
    pub block: BlockchainBlock,
    #[serde(default)]
    pub transactions: Vec<BlockchainFullTransaction>,
    #[serde(default)]
    pub balance_updates: Vec<TransactionBalanceUpdate>,
}

/// A transaction with its balance updates, as returned by the full-transaction
/// endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainFullTransaction {
    #[serde(flatten)]
    pub transaction: BlockchainBlockTransaction,
    #[serde(default)]
    pub balance_updates: Vec<TransactionBalanceUpdate>,
}

/// A single transaction with its balance updates, as returned by
/// `/blockchain-v2/{asset}/transactions/{txid}`.
#[derive(Debug, Clone, Deserialize)]
pub struct BlockchainFullSingleTransaction {
    #[serde(flatten)]
    pub transaction: BlockchainTransaction,
    #[serde(default)]
    pub balance_updates: Vec<TransactionBalanceUpdate>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxTrackerTransaction {
    pub txid: String,
    pub time: String,
    pub first_seen_time: String,
    pub status: String,
    pub status_update_time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mempool_approx_queue_position: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub next_block_approx_settlement_probability_pct: Option<f64>,
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub height: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub n_confirmations: Option<i64>,
    #[serde(default)]
    pub status_updates: Vec<TxTrackerStatusUpdate>,
    pub details: TxTrackerDetails,
    #[serde(default)]
    pub geo: Vec<NodeLocationSeenTime>,
    pub replacement_for_txid: Option<String>,
    #[serde(default)]
    pub inputs: Vec<TxTrackerOutput>,
    #[serde(default)]
    pub outputs: Vec<TxTrackerOutput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxTrackerStatusUpdate {
    pub time: String,
    pub status: String,
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub height: Option<i64>,
    pub removal_reason: Option<String>,
    pub replacement_txid: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxTrackerDetails {
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub version: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    pub replace_by_fee_supported: Option<bool>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub fee: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub feerate: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mempool_feerate_mean_at_first_seen_time: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mempool_feerate_min_at_first_seen_time: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub consensus_size: Option<i64>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub physical_size: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxTrackerOutput {
    pub address: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NodeLocationSeenTime {
    pub location: String,
    pub seen_time: String,
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct JobDetails {
    pub id: String,
    pub status: String,
    pub creation_time: String,
    pub completion_time: Option<String>,
    pub expiration_time: Option<String>,
    pub error: Option<ErrorObject>,
    #[serde(default)]
    pub results: Vec<JobResult>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobResult {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatedJob {
    pub job_id: String,
    pub job_url: String,
}

/// Response of `/jobs` (no pagination).
#[derive(Debug, Clone, Deserialize)]
pub struct JobsResponse {
    #[serde(default)]
    pub data: Vec<JobDetails>,
}

// ---------------------------------------------------------------------------
// WebSocket stream messages
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingAssetMetric {
    pub asset: String,
    pub time: String,
    pub cm_sequence_id: String,
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingMarketTrade {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price: f64,
    pub side: Option<String>,
    pub cm_sequence_id: String,
    pub collect_time: String,
    pub block_hash: Option<String>,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub block_height: Option<i64>,
    pub txid: Option<String>,
    pub initiator: Option<String>,
    pub sender: Option<String>,
    pub beneficiary: Option<String>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mark_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub index_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub iv_trade: Option<f64>,
    pub liquidation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingMarketLiquidation {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_i64")]
    pub coin_metrics_id: Option<i64>,
    #[serde(deserialize_with = "number::f64")]
    pub amount: f64,
    #[serde(deserialize_with = "number::f64")]
    pub price: f64,
    pub side: Option<String>,
    #[serde(rename = "type")]
    pub liquidation_type: String,
    pub cm_sequence_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingMarketOpenInterest {
    pub market: String,
    pub time: String,
    #[serde(deserialize_with = "number::f64")]
    pub contract_count: f64,
    #[serde(deserialize_with = "number::f64")]
    pub value_usd: f64,
    pub exchange_time: Option<String>,
    pub cm_sequence_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingMarketContractPrice {
    pub market: String,
    pub time: String,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub mark_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub index_price: Option<f64>,
    #[serde(default, deserialize_with = "number::opt_f64")]
    pub settlement_price_estimated: Option<f64>,
    pub exchange_time: Option<String>,
    pub cm_sequence_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_the_list_envelope() {
        let response: ListResponse<AssetInfo> = serde_json::from_str(
            r#"{
                "data": [{"asset": "btc", "full_name": "Bitcoin"}],
                "next_page_token": "MHgw",
                "next_page_url": "https://community-api.coinmetrics.io/v4/reference-data/assets?page_size=2&next_page_token=MHgw"
            }"#,
        )
        .unwrap();
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].asset, "btc");
        assert_eq!(response.next_page_token.as_deref(), Some("MHgw"));
        assert!(response.warning.is_none());
    }

    #[test]
    fn deserializes_asset_metric_rows_with_dynamic_string_values() {
        let response: ListResponse<AssetMetricRow> = serde_json::from_str(
            r#"{"data":[
                {"asset":"btc","time":"2026-10-24T00:00:00.000000000Z","AdrActCnt":"579621","PriceUSD":"80699.3131272238"},
                {"asset":"btc","time":"2026-10-25T00:00:00.000000000Z","AdrActCnt":null,"PriceUSD":"80038.9058032613"}
            ]}"#,
        )
        .unwrap();
        assert_eq!(response.data[0].asset, "btc");
        assert_eq!(
            response.data[0].values.get("PriceUSD"),
            Some(&Value::String("80699.3131272238".into()))
        );
        assert!(response.data[1].values.get("AdrActCnt") == Some(&Value::Null));
    }

    #[test]
    fn deserializes_catalog_asset_metrics() {
        let response: ListResponse<AssetMetricsCatalogInfo> = serde_json::from_str(
            r#"{"data":[{"asset":"btc","metrics":[{"metric":"AdrActCnt","frequencies":[
                {"frequency":"1d","min_time":"2009-01-03T00:00:00.000000000Z","max_time":"2026-10-24T00:00:00.000000000Z","community":true}
            ]}]}]}"#,
        )
        .unwrap();
        let metric = &response.data[0].metrics[0];
        assert_eq!(metric.metric, "AdrActCnt");
        assert_eq!(metric.frequencies[0].frequency, "1d");
        assert_eq!(metric.frequencies[0].community, Some(true));
    }

    #[test]
    fn deserializes_numeric_fields_returned_as_strings() {
        let trade: MarketTrade = serde_json::from_str(
            r#"{"market":"coinbase-btc-usd-spot","time":"2021-06-08 20:55:00+00:00",
                "coin_metrics_id":"200000000","amount":"1000.01","price":"10.61","database_time":"2021-06-08 20:55:01+00:00"}"#,
        )
        .unwrap();
        assert_eq!(trade.coin_metrics_id, Some(200_000_000));
        assert_eq!(trade.amount, 1000.01);
        assert_eq!(trade.price, 10.61);
    }

    #[test]
    fn deserializes_api_errors() {
        let error: ErrorResponse = serde_json::from_str(
            r#"{"error":{"type":"forbidden","message":"Requested metric is not available."}}"#,
        )
        .unwrap();
        assert_eq!(error.error.r#type, "forbidden");
    }

    #[test]
    fn deserializes_a_captured_market_reference_record() {
        let market: MarketInfo = serde_json::from_str(
            r#"{
                "market":"aerodrome_slipstream_base-1-aero_base.eth-cbbtc_base.eth-spot",
                "exchange":"aerodrome_slipstream_base",
                "type":"spot",
                "base":"aero_base.eth",
                "quote":"cbbtc_base.eth",
                "pair":"aero_base.eth-cbbtc_base.eth",
                "asset_class":"digital",
                "source_type":"exchange",
                "pool_config_id":"1",
                "contract_address":"dfe5f275020def30993f042174fc2d335678b626",
                "price_includes_fee":true,
                "variable_fee":true,
                "base_address":"940181a94a35a4569e4529a3cdfb74e38fd98631",
                "quote_address":"cbb7c0000ab88b473b1f5afd9ef808440eed33bf",
                "experimental":true
            }"#,
        )
        .unwrap();
        assert_eq!(market.market_type, "spot");
        assert_eq!(market.exchange, "aerodrome_slipstream_base");
        assert_eq!(market.price_includes_fee, Some(true));
        assert_eq!(market.experimental, Some(true));
        assert!(market.talos.is_none());
    }

    #[test]
    fn deserializes_a_captured_metric_reference_record() {
        let metric: MetricInfo = serde_json::from_str(
            r#"{
                "metric":"AdrAct30dCnt",
                "full_name":"Addresses, active, monthly, count",
                "description":"The sum count of unique addresses that were active in the network.",
                "product":"Network Data",
                "category":"Addresses",
                "subcategory":"Active",
                "unit":"Addresses",
                "data_type":"bigint",
                "type":"Sum",
                "display_name":"Active Monthly Addr Cnt",
                "docs_url":"https://docs.coinmetrics.io/network-data/network-data-overview/addresses/active-addresses"
            }"#,
        )
        .unwrap();
        assert_eq!(metric.metric, "AdrAct30dCnt");
        assert_eq!(metric.metric_type, "Sum");
        assert_eq!(metric.data_type, "bigint");
    }

    #[test]
    fn deserializes_a_blockchain_account_record() {
        let account: BlockchainAccount = serde_json::from_str(
            r#"{
                "account":"112jmDkNGHSbhhY17JGpxU3sMA9ZExG7b2",
                "type":"UTXO",
                "balance":5.83e-05,
                "n_debits":3,
                "n_credits":4,
                "creation_height":470920,
                "creation_block_hash":"0000000000000000002c7505ef2272e0677fa53d68d633f8e076ed42dd3380e6",
                "creation_time":"2017-06-12 06:50:36+00:00",
                "creation_chain_sequence_number":2022585999040108,
                "last_chain_sequence_number":2054987232324222,
                "last_debit_height":474496,
                "last_credit_height":478464
            }"#,
        )
        .unwrap();
        assert_eq!(account.account, "112jmDkNGHSbhhY17JGpxU3sMA9ZExG7b2");
        assert_eq!(account.balance, Some(5.83e-05));
        assert_eq!(account.creation_height, Some(470_920));
    }

    #[test]
    fn deserializes_block_by_block_metric_rows_with_numeric_values() {
        let row: AssetMetricRow = serde_json::from_str(
            r#"{
                "block_hash":"0000000000000000000b7ee9ded84d26167a13c8377cd003fb344584368ba7e9",
                "parent_block_hash":"0000000000000000000fd3f882c12258b3c604c724fe2e9747e250b094b8b026",
                "height":635276,
                "asset":"btc",
                "time":"2020-06-18 10:37:23+00:00",
                "FlowTfrToExCnt":374,
                "FlowTfrToExCnt-status":"flash"
            }"#,
        )
        .unwrap();
        assert_eq!(row.asset, "btc");
        assert_eq!(row.values.get("FlowTfrToExCnt"), Some(&Value::from(374)));
        assert_eq!(
            row.values.get("FlowTfrToExCnt-status"),
            Some(&Value::String("flash".into()))
        );
    }

    #[test]
    fn deserializes_asset_chain_fragments() {
        let chains: AssetChains = serde_json::from_str(
            r#"{
                "asset":"btc","time":"2021-01-01 15:30:00+00:00",
                "chains_count":2,"blocks_count_at_tip":1,
                "reorg":"true","reorg_depth":1,
                "chains":[[{"hash":"0000a","height":679917,"time":"2021-01-01 15:00:00+00:00"}],
                          [{"hash":"0000b","height":679918,"time":"2021-01-01 15:20:00+00:00"}]]
            }"#,
        )
        .unwrap();
        assert_eq!(chains.reorg.as_deref(), Some("true"));
        assert_eq!(chains.reorg_depth, Some(1.0));
        assert_eq!(chains.chains.len(), 2);
        assert_eq!(chains.chains[1][0].height, 679_918);
    }

    #[test]
    fn deserializes_mempool_feerate_bands() {
        let feerates: MempoolFeerate = serde_json::from_str(
            r#"{
                "asset":"btc","time":"2021-06-08 11:48:21+00:00",
                "feerates":[{"feerate":1,"count":123,"consensus_size":512,"physical_size":128,"fees":0.09}]
            }"#,
        )
        .unwrap();
        assert_eq!(feerates.feerates.len(), 1);
        assert_eq!(feerates.feerates[0].count, 123.0);
        assert_eq!(feerates.feerates[0].fees, 0.09);
    }
}
