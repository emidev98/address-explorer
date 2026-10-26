use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page};
use crate::clients::coinmetrics::types::{ListResponse, SecurityMasterAsset, SecurityMasterMarket};
use serde::Serialize;

/// Filters for `/security-master/assets`. `assets` and `codes` are mutually
/// exclusive.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SecurityMasterAssetsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub codes: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/security-master/markets`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SecurityMasterMarketsQuery {
    /// `spot`, `future` or `option`.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub market_type: Option<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub markets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Security master asset and market metadata endpoints.
#[derive(Clone)]
pub struct SecurityMaster {
    http: HttpClient,
}

impl SecurityMaster {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_assets(
        &self,
        query: SecurityMasterAssetsQuery,
    ) -> Result<ListResponse<SecurityMasterAsset>, CoinMetricsError> {
        self.http.get_query("/security-master/assets", &query).await
    }

    pub async fn get_markets(
        &self,
        query: SecurityMasterMarketsQuery,
    ) -> Result<ListResponse<SecurityMasterMarket>, CoinMetricsError> {
        self.http
            .get_query("/security-master/markets", &query)
            .await
    }
}
