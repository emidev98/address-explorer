use crate::clients::coinmetrics::error::CoinMetricsError;
use crate::clients::coinmetrics::http::HttpClient;
use crate::clients::coinmetrics::params::{comma_separated, CursorPage};
use crate::clients::coinmetrics::types::{
    BlockchainMetadataLocation, BlockchainMetadataOwner, BlockchainMetadataTag,
    BlockchainMetadataTaggedEntity, ListResponse,
};
use serde::Serialize;

/// Filters for `/blockchain-metadata/tags`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TagsQuery {
    /// Tag type; currently only `ENTITY` is available.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tag_type: Option<String>,
    #[serde(flatten)]
    pub page: CursorPage,
}

/// Filters for `/blockchain-metadata/tagged-entities`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TaggedEntitiesQuery {
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub tags: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub entities: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub locations: Vec<String>,
    #[serde(
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "comma_separated"
    )]
    pub owner_names: Vec<String>,
    #[serde(flatten)]
    pub page: CursorPage,
}

/// Blockchain entity metadata: locations, owners, tags and tagged entities.
#[derive(Clone)]
pub struct BlockchainMetadata {
    http: HttpClient,
}

impl BlockchainMetadata {
    pub fn new(http: HttpClient) -> Self {
        Self { http }
    }

    pub async fn get_locations(
        &self,
        page: CursorPage,
    ) -> Result<ListResponse<BlockchainMetadataLocation>, CoinMetricsError> {
        self.http
            .get_query("/blockchain-metadata/locations", &page)
            .await
    }

    pub async fn get_owners(
        &self,
        page: CursorPage,
    ) -> Result<ListResponse<BlockchainMetadataOwner>, CoinMetricsError> {
        self.http
            .get_query("/blockchain-metadata/owners", &page)
            .await
    }

    pub async fn get_tags(
        &self,
        query: TagsQuery,
    ) -> Result<ListResponse<BlockchainMetadataTag>, CoinMetricsError> {
        self.http
            .get_query("/blockchain-metadata/tags", &query)
            .await
    }

    pub async fn get_tagged_entities(
        &self,
        query: TaggedEntitiesQuery,
    ) -> Result<ListResponse<BlockchainMetadataTaggedEntity>, CoinMetricsError> {
        self.http
            .get_query("/blockchain-metadata/tagged-entities", &query)
            .await
    }
}
