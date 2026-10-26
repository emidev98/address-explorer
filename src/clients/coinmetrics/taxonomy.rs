use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page};
use crate::clients::coinmetrics::types::{ListResponse, TaxonomyAsset, TaxonomyMetadataAsset};
use serde::Serialize;

/// Filters for `/taxonomy/assets`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TaxonomyAssetsQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub class_ids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub sector_ids: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub subsector_ids: Vec<String>,
    /// Taxonomy version; `*` returns every version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification_end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_inclusive: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/taxonomy-metadata/assets`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TaxonomyMetadataQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_inclusive: Option<bool>,
    #[serde(flatten)]
    pub page: Page,
}

/// Asset taxonomy classification and taxonomy metadata endpoints.
#[derive(Clone)]
pub struct Taxonomy {
    http: HttpClient,
}

impl Taxonomy {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    /// Returns the taxonomy classification of assets.
    pub async fn get_assets(
        &self,
        query: TaxonomyAssetsQuery,
    ) -> Result<ListResponse<TaxonomyAsset>, CoinMetricsError> {
        self.http.get_query("/taxonomy/assets", &query).await
    }

    /// Returns taxonomy versions and their subsector metadata.
    pub async fn get_metadata(
        &self,
        query: TaxonomyMetadataQuery,
    ) -> Result<ListResponse<TaxonomyMetadataAsset>, CoinMetricsError> {
        self.http
            .get_query("/taxonomy-metadata/assets", &query)
            .await
    }
}
