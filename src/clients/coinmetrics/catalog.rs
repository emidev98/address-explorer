use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page, TimeRange};
use crate::clients::coinmetrics::types::{
    AssetMetricsCatalogInfo, AssetTimeRangeCatalogInfo, ExchangeAssetMetricsCatalogInfo,
    ExchangeMetricsCatalogInfo, ExchangePairMetricsCatalogInfo, IndexCatalogInfo,
    InstitutionMetricsCatalogInfo, ListResponse, MarketCandlesCatalogInfo,
    MarketMetricsCatalogInfo, MarketOrderBooksCatalogInfo, MarketTimeRangeCatalogInfo,
    PairCandlesCatalogInfo, PairMetricsCatalogInfo, ProtocolMetricsCatalogInfo,
};
use serde::Serialize;

/// Catalog variant to query. `V2` lists items available with the current
/// credentials; `AllV2` also lists historical and otherwise restricted items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogScope {
    V2,
    AllV2,
}

impl CatalogScope {
    fn prefix(&self) -> &'static str {
        match self {
            Self::V2 => "catalog-v2",
            Self::AllV2 => "catalog-all-v2",
        }
    }
}

/// Filters for `/catalog-*/asset-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogAssetMetricsQuery {
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
    /// Limit to human-reviewable metrics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewable: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/exchange-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogExchangeMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/exchange-asset-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogExchangeAssetMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/exchange-pair-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogExchangePairMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/pair-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogPairMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/institution-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogInstitutionMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Market selectors accepted by the `/catalog-*/market-*` endpoints. This is
/// narrower than [`MarketFilter`], which also serves the reference-data
/// endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogMarketFilter {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub market_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// Filters for the `/catalog-*/market-*` coverage endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogMarketDataQuery {
    #[serde(flatten)]
    pub filter: CatalogMarketFilter,
    #[serde(flatten)]
    pub time: TimeRange,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/market-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogMarketMetricsQuery {
    #[serde(flatten)]
    pub filter: CatalogMarketFilter,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for asset-scoped catalogs such as `/catalog-*/asset-chains`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogAssetsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/pair-candles`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogPairsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub pairs: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/index-levels` and `/catalog-*/index-candles`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogIndexesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/catalog-*/protocol-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CatalogProtocolMetricsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Coverage endpoints describing which metrics, markets and time ranges are
/// available for each entity.
#[derive(Clone)]
pub struct Catalog {
    http: HttpClient,
}

impl Catalog {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    fn path(&self, scope: CatalogScope, resource: &str) -> String {
        format!("/{}/{}", scope.prefix(), resource)
    }

    pub async fn get_asset_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogAssetMetricsQuery,
    ) -> Result<ListResponse<AssetMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "asset-metrics"), &query)
            .await
    }

    pub async fn get_exchange_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogExchangeMetricsQuery,
    ) -> Result<ListResponse<ExchangeMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "exchange-metrics"), &query)
            .await
    }

    pub async fn get_exchange_asset_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogExchangeAssetMetricsQuery,
    ) -> Result<ListResponse<ExchangeAssetMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "exchange-asset-metrics"), &query)
            .await
    }

    pub async fn get_exchange_pair_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogExchangePairMetricsQuery,
    ) -> Result<ListResponse<ExchangePairMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "exchange-pair-metrics"), &query)
            .await
    }

    pub async fn get_pair_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogPairMetricsQuery,
    ) -> Result<ListResponse<PairMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "pair-metrics"), &query)
            .await
    }

    pub async fn get_institution_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogInstitutionMetricsQuery,
    ) -> Result<ListResponse<InstitutionMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "institution-metrics"), &query)
            .await
    }

    pub async fn get_market_trades(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-trades"), &query)
            .await
    }

    pub async fn get_market_candles(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketCandlesCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-candles"), &query)
            .await
    }

    pub async fn get_market_orderbooks(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketOrderBooksCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-orderbooks"), &query)
            .await
    }

    pub async fn get_market_quotes(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-quotes"), &query)
            .await
    }

    pub async fn get_market_funding_rates(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-funding-rates"), &query)
            .await
    }

    pub async fn get_market_funding_rates_predicted(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-funding-rates-predicted"), &query)
            .await
    }

    pub async fn get_market_contract_prices(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-contract-prices"), &query)
            .await
    }

    pub async fn get_market_implied_volatility(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-implied-volatility"), &query)
            .await
    }

    pub async fn get_market_greeks(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-greeks"), &query)
            .await
    }

    pub async fn get_market_openinterest(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-openinterest"), &query)
            .await
    }

    pub async fn get_market_liquidations(
        &self,
        scope: CatalogScope,
        query: CatalogMarketDataQuery,
    ) -> Result<ListResponse<MarketTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-liquidations"), &query)
            .await
    }

    pub async fn get_market_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogMarketMetricsQuery,
    ) -> Result<ListResponse<MarketMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "market-metrics"), &query)
            .await
    }

    pub async fn get_pair_candles(
        &self,
        scope: CatalogScope,
        query: CatalogPairsQuery,
    ) -> Result<ListResponse<PairCandlesCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "pair-candles"), &query)
            .await
    }

    pub async fn get_index_levels(
        &self,
        scope: CatalogScope,
        query: CatalogIndexesQuery,
    ) -> Result<ListResponse<IndexCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "index-levels"), &query)
            .await
    }

    pub async fn get_index_candles(
        &self,
        scope: CatalogScope,
        query: CatalogIndexesQuery,
    ) -> Result<ListResponse<IndexCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "index-candles"), &query)
            .await
    }

    pub async fn get_asset_chains(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "asset-chains"), &query)
            .await
    }

    pub async fn get_mempool_feerates(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "mempool-feerates"), &query)
            .await
    }

    pub async fn get_mining_pool_tips_summary(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "mining-pool-tips-summary"), &query)
            .await
    }

    pub async fn get_transaction_tracker(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "transaction-tracker"), &query)
            .await
    }

    pub async fn get_blockchain_accounts(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "blockchain-v2/accounts"), &query)
            .await
    }

    pub async fn get_blockchain_balance_updates(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "blockchain-v2/balance-updates"), &query)
            .await
    }

    pub async fn get_blockchain_rebasing_changes(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "blockchain-v2/rebasing-changes"), &query)
            .await
    }

    pub async fn get_blockchain_blocks(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "blockchain-v2/blocks"), &query)
            .await
    }

    pub async fn get_blockchain_transactions(
        &self,
        scope: CatalogScope,
        query: CatalogAssetsQuery,
    ) -> Result<ListResponse<AssetTimeRangeCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "blockchain-v2/transactions"), &query)
            .await
    }

    pub async fn get_protocol_metrics(
        &self,
        scope: CatalogScope,
        query: CatalogProtocolMetricsQuery,
    ) -> Result<ListResponse<ProtocolMetricsCatalogInfo>, CoinMetricsError> {
        self.http
            .get_query(&self.path(scope, "protocol-metrics"), &query)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_selects_the_catalog_prefix() {
        assert_eq!(CatalogScope::V2.prefix(), "catalog-v2");
        assert_eq!(CatalogScope::AllV2.prefix(), "catalog-all-v2");
    }

    #[test]
    fn serializes_market_coverage_filters() {
        let query = CatalogMarketDataQuery {
            filter: CatalogMarketFilter {
                markets: vec!["binance-btc-usd-spot".into()],
                market_type: Some("spot".into()),
                ..Default::default()
            },
            time: TimeRange {
                start_time: Some("2024-01-01".into()),
                ..Default::default()
            },
            page: Page {
                page_size: Some(5),
                ..Default::default()
            },
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({
                "markets": "binance-btc-usd-spot",
                "type": "spot",
                "start_time": "2024-01-01",
                "page_size": 5
            })
        );
    }
}
