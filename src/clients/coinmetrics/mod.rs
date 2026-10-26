//! Client for the [Coin Metrics API v4](https://docs.coinmetrics.io/api/v4).
//!
//! The client mirrors the API's documentation groups and returns typed models
//! from [`types`]. The default configuration targets the free Community API;
//! use [`CoinMetricsConfig::pro`] with an API key for the paid API.
//!
//! ```text
//! let client = CoinMetrics::new(CoinMetricsConfig::pro(api_key));
//!
//! // Daily price and active-address series for BTC.
//! let rows = client
//!     .timeseries
//!     .get_asset_metrics(AssetMetricsQuery {
//!         assets: vec!["btc".into()],
//!         metrics: vec!["PriceUSD".into(), "AdrActCnt".into()],
//!         time: TimeRange {
//!             start_time: Some("2024-01-01".into()),
//!             ..Default::default()
//!         },
//!         ..Default::default()
//!     })
//!     .await?;
//!
//! // Discover which metrics and time ranges exist for BTC.
//! let catalog = client
//!     .catalog
//!     .get_asset_metrics(CatalogScope::V2, CatalogAssetMetricsQuery::default())
//!     .await?;
//! ```
//!
//! Metric rows keep the requested metric columns in [`types::AssetMetricRow::values`]
//! because the API returns an arbitrary, requested subset of columns, usually
//! as numeric strings. Market data endpoints return typed records such as
//! [`types::MarketTrade`] and [`types::MarketCandle`].
//!
//! WebSocket endpoints are exposed as authenticated URL builders on
//! [`Streams`]; the caller owns the connection and decodes messages with the
//! `Streaming*` models in [`types`].
#![allow(dead_code, unused_imports)]
pub mod blockchain;
pub mod blockchain_metadata;
pub mod catalog;
pub mod config;
pub mod constituents;
pub mod error;
pub mod http;
pub mod jobs;
pub mod params;
pub mod profiles;
pub mod reference_data;
pub mod security_master;
pub mod streams;
pub mod taxonomy;
pub mod timeseries;
pub mod types;

pub use blockchain::Blockchain;
pub use blockchain_metadata::BlockchainMetadata;
pub use catalog::{Catalog, CatalogScope};
pub use config::CoinMetricsConfig;
pub use constituents::Constituents;
pub use error::CoinMetricsError;
pub use jobs::Jobs;
pub use profiles::Profiles;
pub use reference_data::ReferenceData;
pub use security_master::SecurityMaster;
pub use streams::Streams;
pub use taxonomy::Taxonomy;
pub use timeseries::Timeseries;

use http::HttpClient;

/// Client for the Coin Metrics API v4, grouped like the API's documentation:
/// reference data, catalogs, time series, profiles, taxonomy, blockchain
/// entities, jobs and WebSocket stream URL builders.
#[derive(Clone)]
pub struct CoinMetrics {
    pub blockchain: Blockchain,
    pub blockchain_metadata: BlockchainMetadata,
    pub catalog: Catalog,
    pub constituents: Constituents,
    pub jobs: Jobs,
    pub profiles: Profiles,
    pub reference_data: ReferenceData,
    pub security_master: SecurityMaster,
    pub streams: Streams,
    pub taxonomy: Taxonomy,
    pub timeseries: Timeseries,
}

impl CoinMetrics {
    /// Creates a client for the API described by `config`; the default targets
    /// the free Community API and needs no API key.
    pub fn new(config: CoinMetricsConfig) -> Self {
        let http = HttpClient::new(&config);
        Self {
            blockchain: Blockchain::new(http.clone()),
            blockchain_metadata: BlockchainMetadata::new(http.clone()),
            catalog: Catalog::new(http.clone()),
            constituents: Constituents::new(http.clone()),
            jobs: Jobs::new(http.clone()),
            profiles: Profiles::new(http.clone()),
            reference_data: ReferenceData::new(http.clone()),
            security_master: SecurityMaster::new(http.clone()),
            streams: Streams::new(http.clone()),
            taxonomy: Taxonomy::new(http.clone()),
            timeseries: Timeseries::new(http),
        }
    }
}
