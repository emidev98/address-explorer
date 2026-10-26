use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, MarketFilter, Page};
use crate::clients::coinmetrics::types::{
    AssetGroupInfo, AssetInfo, ExchangeInfo, IndexInfo, ListResponse, MarketInfo, MetricInfo,
    PairInfo, ProtocolInfo,
};
use serde::Serialize;

/// Filters for `/reference-data/assets`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    /// Extra metadata namespaces to include; currently only `talos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/reference-data/asset-groups`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetGroupsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub groups: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
}

/// Filters for `/reference-data/exchanges`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ExchangesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub exchanges: Vec<String>,
    /// Extra metadata namespaces to include; currently only `talos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/reference-data/markets`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketsQuery {
    #[serde(flatten)]
    pub filter: MarketFilter,
    /// Extra metadata namespaces to include; currently only `talos`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/reference-data/indexes`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub indexes: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/reference-data/protocols`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ProtocolsQuery {
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
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for the `/reference-data/*-metrics` catalogs.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MetricsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub metrics: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/reference-data/asset-metrics`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetMetricsQuery {
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

/// Reference-data endpoints for assets, exchanges, markets, pairs, indexes,
/// protocols and metric catalogs.
#[derive(Clone)]
pub struct ReferenceData {
    http: HttpClient,
}

impl ReferenceData {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_assets(
        &self,
        query: AssetsQuery,
    ) -> Result<ListResponse<AssetInfo>, CoinMetricsError> {
        self.http.get_query("/reference-data/assets", &query).await
    }

    pub async fn get_asset_groups(
        &self,
        query: AssetGroupsQuery,
    ) -> Result<ListResponse<AssetGroupInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/asset-groups", &query)
            .await
    }

    pub async fn get_exchanges(
        &self,
        query: ExchangesQuery,
    ) -> Result<ListResponse<ExchangeInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/exchanges", &query)
            .await
    }

    pub async fn get_markets(
        &self,
        query: MarketsQuery,
    ) -> Result<ListResponse<MarketInfo>, CoinMetricsError> {
        self.http.get_query("/reference-data/markets", &query).await
    }

    pub async fn get_indexes(
        &self,
        query: IndexesQuery,
    ) -> Result<ListResponse<IndexInfo>, CoinMetricsError> {
        self.http.get_query("/reference-data/indexes", &query).await
    }

    pub async fn get_protocols(
        &self,
        query: ProtocolsQuery,
    ) -> Result<ListResponse<ProtocolInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/protocols", &query)
            .await
    }

    pub async fn get_pairs(
        &self,
        query: PairsQuery,
    ) -> Result<ListResponse<PairInfo>, CoinMetricsError> {
        self.http.get_query("/reference-data/pairs", &query).await
    }

    pub async fn get_asset_metrics(
        &self,
        query: AssetMetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/asset-metrics", &query)
            .await
    }

    pub async fn get_exchange_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/exchange-metrics", &query)
            .await
    }

    pub async fn get_exchange_asset_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/exchange-asset-metrics", &query)
            .await
    }

    pub async fn get_exchange_pair_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/exchange-pair-metrics", &query)
            .await
    }

    pub async fn get_pair_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/pair-metrics", &query)
            .await
    }

    pub async fn get_institution_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/institution-metrics", &query)
            .await
    }

    pub async fn get_market_metrics(
        &self,
        query: MetricsQuery,
    ) -> Result<ListResponse<MetricInfo>, CoinMetricsError> {
        self.http
            .get_query("/reference-data/market-metrics", &query)
            .await
    }
}

/// Filters for `/reference-data/pairs`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PairsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub pairs: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_reference_data_filters() {
        let query = AssetsQuery {
            assets: vec!["btc".into(), "eth".into()],
            include: Some("talos".into()),
            page: Page {
                page_size: Some(2),
                ..Default::default()
            },
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({"assets": "btc,eth", "include": "talos", "page_size": 2})
        );
    }

    #[test]
    fn serializes_pair_filters() {
        let query = PairsQuery {
            pairs: vec!["btc-usd".into()],
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            json!({"pairs": "btc-usd"})
        );
    }
}
