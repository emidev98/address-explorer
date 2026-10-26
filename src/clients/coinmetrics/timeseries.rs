use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{
    comma_separated, Granularity, MetricStatus, OrderBookDataset, Page, TimeRange,
};
use crate::clients::coinmetrics::types::{
    AssetAlert, AssetAlertsResponse, AssetChains, AssetMetricRow, ConstituentSnapshotRow,
    ConstituentTimeframeRow, DefiBalanceSheet, ExchangeAssetMetricRow, ExchangeMetricRow,
    ExchangePairMetricRow, IndexCandle, IndexConstituents, IndexLevel, InstitutionMetricRow,
    ListResponse, MarketCandle, MarketContractPrices, MarketFundingRate,
    MarketFundingRatePredicted, MarketGreeks, MarketImpliedVolatility, MarketLiquidation,
    MarketMetricRow, MarketOpenInterest, MarketOrderBook, MarketQuote, MarketTrade, MempoolFeerate,
    MiningPoolTipsSummary, PairCandle, PairMetricRow, ProtocolMetricRow,
};
use serde::Serialize;

/// Query for `/timeseries/asset-metrics`. `assets` and `metrics` are required.
///
/// `start_time`/`end_time` are mutually exclusive with
/// `start_height`/`end_height` and `start_hash`/`end_hash`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    /// `1b`, `1s`, `1m`, `5m`, `10m`, `1h`, `1d`, ... (default `1d`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    /// Which values to return for reviewable metrics (default `all`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<MetricStatus>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_hash: Option<String>,
    /// How many blocks behind the chain tip block-by-block metrics use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confirmations: Option<u32>,
    /// `asset` (default), `height` or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_asset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub null_as_zero: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_forbidden_errors: Option<bool>,
}

/// Query for `/timeseries/exchange-metrics`. `exchanges` and `metrics` are
/// required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExchangeMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub exchanges: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// `exchange` (default) or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_exchange: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/exchange-asset-metrics`. `exchange_assets` and
/// `metrics` are required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExchangeAssetMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub exchange_assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// `exchange_asset` (default) or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_exchange_asset: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/exchange-pair-metrics`. `exchange_pairs` and
/// `metrics` are required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExchangePairMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub exchange_pairs: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_exchange_pair: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/pair-metrics`. `pairs` and `metrics` are required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PairMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub pairs: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// `pair` (default) or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_pair: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-metrics`. `markets` and `metrics` are
/// required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// `market` (default) or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_forbidden_errors: Option<bool>,
}

/// Query for `/timeseries/institution-metrics`. `institutions` and `metrics`
/// are required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct InstitutionMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub institutions: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// `institution` (default) or `time`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_institution: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/protocol-metrics`. `frequency` defaults to `1d` and
/// is required by the API.
#[derive(Debug, Clone, Serialize)]
pub struct ProtocolMetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub protocols: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub chains: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    /// `1d` or `1m`.
    pub frequency: String,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(flatten)]
    pub page: Page,
}

impl Default for ProtocolMetricsQuery {
    fn default() -> Self {
        Self {
            protocols: Vec::new(),
            chains: Vec::new(),
            markets: Vec::new(),
            metrics: Vec::new(),
            frequency: "1d".to_string(),
            time: TimeRange::default(),
            page: Page::default(),
        }
    }
}

/// Query for `/timeseries/pair-candles`. `pairs` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PairCandlesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub pairs: Vec<String>,
    /// Candle duration such as `5m`, `1h` or `1d`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_pair: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-trades`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketTradesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    /// How many blocks behind the chain tip trades are based on (default `2`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_confirmations: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-openinterest`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketOpenInterestQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-liquidations`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketLiquidationsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-funding-rates` and
/// `/timeseries/market-funding-rates-predicted`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketFundingRatesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-orderbooks`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketOrderBooksQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    /// `1`..`30000`, `10pct_mid_price` or `full_book`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataset: Option<OrderBookDataset>,
    /// Applies only when `dataset=updates`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_with_snapshot: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-quotes`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketQuotesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    /// Include one-sided and empty books.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_one_sided: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-candles`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketCandlesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    /// Candle duration such as `5m`, `1h` or `1d`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_forbidden_errors: Option<bool>,
}

/// Query for `/timeseries/market-contract-prices`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketContractPricesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-implied-volatility`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketImpliedVolatilityQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/market-greeks`. `markets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketGreeksQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<Granularity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_market: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/index-candles`. `indexes` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexCandlesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_index: Option<u32>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/index-levels`. `indexes` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexLevelsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    /// Index frequency such as `1s`, `1h`, `1d` or `1d-ny-close`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub granularity: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_per_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_verification: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/index-constituents`. `indexes` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexConstituentsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/mining-pool-tips-summary`. `assets` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetTimeSeriesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/asset-alerts`. `assets` and `alerts` are required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetAlertsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub alerts: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    /// Include the most recent successful evaluation per alert.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_heartbeats: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/timeseries/defi-balance-sheets`. `defi_protocols` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DefiBalanceSheetsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub defi_protocols: Vec<String>,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_height: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_height: Option<i64>,
    #[serde(flatten)]
    pub page: Page,
}

/// REST time-series endpoints for asset, market, exchange, pair, index,
/// protocol, institution, mining and DeFi data.
#[derive(Clone)]
pub struct Timeseries {
    http: HttpClient,
}

impl Timeseries {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_asset_metrics(
        &self,
        query: AssetMetricsQuery,
    ) -> Result<ListResponse<AssetMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/asset-metrics", &query)
            .await
    }

    pub async fn get_exchange_metrics(
        &self,
        query: ExchangeMetricsQuery,
    ) -> Result<ListResponse<ExchangeMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/exchange-metrics", &query)
            .await
    }

    pub async fn get_exchange_asset_metrics(
        &self,
        query: ExchangeAssetMetricsQuery,
    ) -> Result<ListResponse<ExchangeAssetMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/exchange-asset-metrics", &query)
            .await
    }

    pub async fn get_exchange_pair_metrics(
        &self,
        query: ExchangePairMetricsQuery,
    ) -> Result<ListResponse<ExchangePairMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/exchange-pair-metrics", &query)
            .await
    }

    pub async fn get_pair_metrics(
        &self,
        query: PairMetricsQuery,
    ) -> Result<ListResponse<PairMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/pair-metrics", &query)
            .await
    }

    pub async fn get_market_metrics(
        &self,
        query: MarketMetricsQuery,
    ) -> Result<ListResponse<MarketMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-metrics", &query)
            .await
    }

    pub async fn get_institution_metrics(
        &self,
        query: InstitutionMetricsQuery,
    ) -> Result<ListResponse<InstitutionMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/institution-metrics", &query)
            .await
    }

    pub async fn get_protocol_metrics(
        &self,
        query: ProtocolMetricsQuery,
    ) -> Result<ListResponse<ProtocolMetricRow>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/protocol-metrics", &query)
            .await
    }

    pub async fn get_pair_candles(
        &self,
        query: PairCandlesQuery,
    ) -> Result<ListResponse<PairCandle>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/pair-candles", &query)
            .await
    }

    pub async fn get_market_trades(
        &self,
        query: MarketTradesQuery,
    ) -> Result<ListResponse<MarketTrade>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-trades", &query)
            .await
    }

    pub async fn get_market_openinterest(
        &self,
        query: MarketOpenInterestQuery,
    ) -> Result<ListResponse<MarketOpenInterest>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-openinterest", &query)
            .await
    }

    pub async fn get_market_liquidations(
        &self,
        query: MarketLiquidationsQuery,
    ) -> Result<ListResponse<MarketLiquidation>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-liquidations", &query)
            .await
    }

    pub async fn get_market_funding_rates(
        &self,
        query: MarketFundingRatesQuery,
    ) -> Result<ListResponse<MarketFundingRate>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-funding-rates", &query)
            .await
    }

    pub async fn get_market_funding_rates_predicted(
        &self,
        query: MarketFundingRatesQuery,
    ) -> Result<ListResponse<MarketFundingRatePredicted>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-funding-rates-predicted", &query)
            .await
    }

    pub async fn get_market_orderbooks(
        &self,
        query: MarketOrderBooksQuery,
    ) -> Result<ListResponse<MarketOrderBook>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-orderbooks", &query)
            .await
    }

    pub async fn get_market_quotes(
        &self,
        query: MarketQuotesQuery,
    ) -> Result<ListResponse<MarketQuote>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-quotes", &query)
            .await
    }

    pub async fn get_market_candles(
        &self,
        query: MarketCandlesQuery,
    ) -> Result<ListResponse<MarketCandle>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-candles", &query)
            .await
    }

    pub async fn get_market_contract_prices(
        &self,
        query: MarketContractPricesQuery,
    ) -> Result<ListResponse<MarketContractPrices>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-contract-prices", &query)
            .await
    }

    pub async fn get_market_implied_volatility(
        &self,
        query: MarketImpliedVolatilityQuery,
    ) -> Result<ListResponse<MarketImpliedVolatility>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-implied-volatility", &query)
            .await
    }

    pub async fn get_market_greeks(
        &self,
        query: MarketGreeksQuery,
    ) -> Result<ListResponse<MarketGreeks>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/market-greeks", &query)
            .await
    }

    pub async fn get_index_candles(
        &self,
        query: IndexCandlesQuery,
    ) -> Result<ListResponse<IndexCandle>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/index-candles", &query)
            .await
    }

    pub async fn get_index_levels(
        &self,
        query: IndexLevelsQuery,
    ) -> Result<ListResponse<IndexLevel>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/index-levels", &query)
            .await
    }

    pub async fn get_index_constituents(
        &self,
        query: IndexConstituentsQuery,
    ) -> Result<ListResponse<IndexConstituents>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/index-constituents", &query)
            .await
    }

    pub async fn get_mining_pool_tips_summary(
        &self,
        query: AssetTimeSeriesQuery,
    ) -> Result<ListResponse<MiningPoolTipsSummary>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/mining-pool-tips-summary", &query)
            .await
    }

    pub async fn get_asset_chains(
        &self,
        query: AssetTimeSeriesQuery,
    ) -> Result<ListResponse<AssetChains>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/asset-chains", &query)
            .await
    }

    pub async fn get_mempool_feerates(
        &self,
        query: AssetTimeSeriesQuery,
    ) -> Result<ListResponse<MempoolFeerate>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/mempool-feerates", &query)
            .await
    }

    pub async fn get_asset_alerts(
        &self,
        query: AssetAlertsQuery,
    ) -> Result<AssetAlertsResponse, CoinMetricsError> {
        self.http
            .get_query("/timeseries/asset-alerts", &query)
            .await
    }

    pub async fn get_defi_balance_sheets(
        &self,
        query: DefiBalanceSheetsQuery,
    ) -> Result<ListResponse<DefiBalanceSheet>, CoinMetricsError> {
        self.http
            .get_query("/timeseries/defi-balance-sheets", &query)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_asset_metric_queries() {
        let query = AssetMetricsQuery {
            assets: vec!["btc".into(), "eth".into()],
            metrics: vec!["PriceUSD".into(), "AdrActCnt".into()],
            frequency: Some("1d".into()),
            status: Some(MetricStatus::Reviewed),
            time: TimeRange {
                start_time: Some("2024-01-01".into()),
                end_time: Some("2024-01-31".into()),
                ..Default::default()
            },
            page: Page {
                page_size: Some(100),
                paging_from: Some(crate::clients::coinmetrics::params::PagingFrom::Start),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({
                "assets": "btc,eth",
                "metrics": "PriceUSD,AdrActCnt",
                "frequency": "1d",
                "status": "reviewed",
                "start_time": "2024-01-01",
                "end_time": "2024-01-31",
                "page_size": 100,
                "paging_from": "start"
            })
        );
    }

    #[test]
    fn serializes_market_candle_queries() {
        let query = MarketCandlesQuery {
            markets: vec!["coinbase-btc-usd-spot".into()],
            frequency: Some("5m".into()),
            page: Page {
                page_size: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({
                "markets": "coinbase-btc-usd-spot",
                "frequency": "5m",
                "page_size": 2
            })
        );
    }

    #[test]
    fn protocol_frequency_defaults_to_daily() {
        assert_eq!(
            serde_json::to_value(ProtocolMetricsQuery::default()).unwrap(),
            json!({"frequency": "1d"})
        );
    }

    #[test]
    fn granularity_maps_to_api_values() {
        let query = MarketOpenInterestQuery {
            markets: vec!["binance-btc-usd-future".into()],
            granularity: Some(Granularity::Hour),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({"markets": "binance-btc-usd-future", "granularity": "1h"})
        );
    }

    #[test]
    fn flattened_query_serializes_through_the_url_encoder() {
        let query = AssetMetricsQuery {
            assets: vec!["btc".into()],
            metrics: vec!["PriceUSD".into(), "AdrActCnt".into()],
            frequency: Some("1d".into()),
            time: TimeRange {
                start_time: Some("2024-01-01".into()),
                ..Default::default()
            },
            page: Page {
                page_size: Some(2),
                ..Default::default()
            },
            ..Default::default()
        };
        let request = reqwest::Client::new()
            .get("https://example.com/timeseries/asset-metrics")
            .query(&query)
            .build()
            .unwrap();
        let url = request.url();
        assert_eq!(
            url.query(),
            Some("assets=btc&metrics=PriceUSD%2CAdrActCnt&frequency=1d&start_time=2024-01-01&page_size=2")
        );
    }
}
