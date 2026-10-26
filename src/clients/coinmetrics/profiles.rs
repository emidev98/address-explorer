use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, Page};
use crate::clients::coinmetrics::types::{AssetProfile, ListResponse, NetworkProfile};
use serde::Serialize;

/// Filters for `/profile/assets`. `assets` and `full_names` are mutually
/// exclusive.
#[derive(Debug, Clone, Default, Serialize)]
pub struct AssetProfilesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub assets: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub full_names: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Filters for `/profile/networks`. `networks` and `full_names` are mutually
/// exclusive.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NetworkProfilesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub networks: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub full_names: Vec<String>,
    #[serde(flatten)]
    pub page: Page,
}

/// Asset and network profile endpoints.
#[derive(Clone)]
pub struct Profiles {
    http: HttpClient,
}

impl Profiles {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_assets(
        &self,
        query: AssetProfilesQuery,
    ) -> Result<ListResponse<AssetProfile>, CoinMetricsError> {
        self.http.get_query("/profile/assets", &query).await
    }

    pub async fn get_networks(
        &self,
        query: NetworkProfilesQuery,
    ) -> Result<ListResponse<NetworkProfile>, CoinMetricsError> {
        self.http.get_query("/profile/networks", &query).await
    }
}
