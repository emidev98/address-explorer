use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page};
use crate::clients::coinmetrics::types::{
    ConstituentSnapshotRow, ConstituentTimeframeRow, ListResponse,
};
use serde::Serialize;

/// Query for `/constituent-snapshots/asset-metrics`. `metric` is required and
/// `at_time` is mutually exclusive with `start_time`/`end_time`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConstituentSnapshotsQuery {
    pub metric: String,
    /// A date or `now`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Query for `/constituent-timeframes/asset-metrics`. `metric` is required.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ConstituentTimeframesQuery {
    pub metric: String,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub constituents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Constituent snapshots and timeframes for asset metrics.
#[derive(Clone)]
pub struct Constituents {
    http: HttpClient,
}

impl Constituents {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns the constituents active at each observation of `metric`.
    pub async fn get_snapshots(
        &self,
        query: ConstituentSnapshotsQuery,
    ) -> Result<ListResponse<ConstituentSnapshotRow>, CoinMetricsError> {
        self.http
            .get_query("/constituent-snapshots/asset-metrics", &query)
            .await
    }

    /// Returns the timeframes during which each constituent contributed to
    /// `metric`.
    pub async fn get_timeframes(
        &self,
        query: ConstituentTimeframesQuery,
    ) -> Result<ListResponse<ConstituentTimeframeRow>, CoinMetricsError> {
        self.http
            .get_query("/constituent-timeframes/asset-metrics", &query)
            .await
    }
}
