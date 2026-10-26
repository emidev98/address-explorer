use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Backfill};
use serde::Serialize;

/// Query for the `/timeseries-stream/asset-metrics` WebSocket endpoint.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamAssetMetricsQuery {
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
    /// `1b`, `1s` or `200ms`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_unsupported_errors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore_forbidden_errors: Option<bool>,
}

/// Query for the market WebSocket streams that only select markets.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamMarketsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/market-orderbooks`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamOrderBooksQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    /// `100` or `full_book`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_limit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/market-quotes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamQuotesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_one_sided: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/asset-quotes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamAssetQuotesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    /// Aggregation method used to combine exchange quotes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/pair-quotes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamPairQuotesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub pairs: Vec<String>,
    /// Aggregation method used to combine exchange quotes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregation_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/market-candles`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamMarketCandlesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    /// `1m`, `5m`, `10m`, `15m`, `30m`, `1h`, `4h` or `1d`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Query for `/timeseries-stream/index-levels`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct StreamIndexLevelsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_verification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backfill: Option<Backfill>,
}

/// Builders for the Coin Metrics WebSocket stream URLs.
///
/// The client returns authenticated `wss://` URLs rather than opening a
/// connection, matching the mempool WebSocket helper design. Message payloads
/// are modelled in [`crate::clients::coinmetrics::types`].
#[derive(Clone)]
pub struct Streams {
    http: HttpClient,
}

impl Streams {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub fn asset_metrics_url(
        &self,
        query: StreamAssetMetricsQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/asset-metrics", &query)
    }

    pub fn market_trades_url(&self, query: StreamMarketsQuery) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-trades", &query)
    }

    pub fn market_liquidations_url(
        &self,
        query: StreamMarketsQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-liquidations", &query)
    }

    pub fn market_openinterest_url(
        &self,
        query: StreamMarketsQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-openinterest", &query)
    }

    pub fn market_contract_prices_url(
        &self,
        query: StreamMarketsQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-contract-prices", &query)
    }

    pub fn market_orderbooks_url(
        &self,
        query: StreamOrderBooksQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-orderbooks", &query)
    }

    pub fn market_quotes_url(&self, query: StreamQuotesQuery) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-quotes", &query)
    }

    pub fn asset_quotes_url(
        &self,
        query: StreamAssetQuotesQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/asset-quotes", &query)
    }

    pub fn pair_quotes_url(
        &self,
        query: StreamPairQuotesQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/pair-quotes", &query)
    }

    pub fn market_candles_url(
        &self,
        query: StreamMarketCandlesQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/market-candles", &query)
    }

    pub fn index_levels_url(
        &self,
        query: StreamIndexLevelsQuery,
    ) -> Result<String, CoinMetricsError> {
        self.http
            .stream_url("/timeseries-stream/index-levels", &query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::coinmetrics::config::CoinMetricsConfig;

    fn streams() -> Streams {
        Streams::new(HttpClient::new(&CoinMetricsConfig::pro("secret key")))
    }

    #[test]
    fn builds_an_authenticated_wss_url_with_comma_separated_lists() {
        let url = streams()
            .asset_metrics_url(StreamAssetMetricsQuery {
                assets: vec!["btc".into(), "eth".into()],
                metrics: vec!["ReferenceRate".into()],
                frequency: Some("1s".into()),
                backfill: Some(Backfill::Latest),
                ..Default::default()
            })
            .unwrap();
        assert!(url.starts_with("wss://api.coinmetrics.io/v4/timeseries-stream/asset-metrics?"));
        assert!(url.contains("assets=btc%2Ceth"));
        assert!(url.contains("metrics=ReferenceRate"));
        assert!(url.contains("frequency=1s"));
        assert!(url.contains("api_key=secret+key"));
    }

    #[test]
    fn community_streams_use_the_community_host() {
        let streams = Streams::new(HttpClient::new(&CoinMetricsConfig::default()));
        let url = streams
            .market_trades_url(StreamMarketsQuery {
                markets: vec!["coinbase-btc-usd-spot".into()],
                backfill: Some(Backfill::Skip),
            })
            .unwrap();
        assert!(url
            .starts_with("wss://community-api.coinmetrics.io/v4/timeseries-stream/market-trades?"));
        assert!(url.contains("backfill=none"));
    }
}
