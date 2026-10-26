//! Shared query-parameter structures and serialization helpers.
//!
//! Coin Metrics serializes list parameters as comma-separated values rather
//! than repeated keys, so list fields use [`comma_separated`].

use serde::{Serialize, Serializer};

/// Serializes a list as `a,b,c`, matching the API's `explode: false` style.
pub(crate) fn comma_separated<S>(values: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&values.join(","))
}

/// Where the first page of results starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PagingFrom {
    Start,
    End,
}

/// Cursor pagination parameters shared by the list endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Page {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paging_from: Option<PagingFrom>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Cursor pagination without `paging_from`, for endpoints that only accept
/// `page_size` and `next_page_token`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CursorPage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Time interval filters shared by the time-series endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TimeRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_inclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

/// Filters shared by the reference-data and catalog market endpoints.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MarketFilter {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cusip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub figi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isin: Option<String>,
}

/// Which metric values to return for reviewable metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricStatus {
    All,
    Flash,
    Reviewed,
    Revised,
}

/// Downsampling granularity for market derivative data and order books.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Granularity {
    Raw,
    #[serde(rename = "1m")]
    Minute,
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "1d")]
    Day,
}

/// Logical order-book dataset to return.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderBookDataset {
    Snapshots,
    Updates,
}

/// Whether a stream sends the latest values on connection (`latest`) or only
/// real-time data (`none`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Backfill {
    Latest,
    #[serde(rename = "none")]
    Skip,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn page_omits_unset_fields() {
        let page = Page {
            page_size: Some(10),
            paging_from: Some(PagingFrom::Start),
            next_page_token: None,
        };
        assert_eq!(
            serde_json::to_value(page).unwrap(),
            json!({"page_size": 10, "paging_from": "start"})
        );
    }

    #[test]
    fn time_range_omits_unset_fields() {
        let range = TimeRange {
            start_time: Some("2024-01-01".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(range).unwrap(),
            json!({"start_time": "2024-01-01"})
        );
    }

    #[test]
    fn market_filter_joins_list_parameters() {
        let filter = MarketFilter {
            markets: vec![
                "binance-btc-usd-spot".into(),
                "coinbase-btc-usd-spot".into(),
            ],
            exchange: Some("binance".into()),
            market_type: Some("spot".into()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(filter).unwrap(),
            json!({
                "markets": "binance-btc-usd-spot,coinbase-btc-usd-spot",
                "exchange": "binance",
                "type": "spot"
            })
        );
    }

    #[test]
    fn enums_serialize_to_api_values() {
        assert_eq!(
            serde_json::to_value(Granularity::Minute).unwrap(),
            json!("1m")
        );
        assert_eq!(
            serde_json::to_value(OrderBookDataset::Snapshots).unwrap(),
            json!("snapshots")
        );
        assert_eq!(serde_json::to_value(Backfill::Skip).unwrap(), json!("none"));
        assert_eq!(
            serde_json::to_value(MetricStatus::Reviewed).unwrap(),
            json!("reviewed")
        );
    }
}
