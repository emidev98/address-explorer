//! Browser wrappers for the price endpoints.

use crate::domain::price::{
    CurrencyHistorySummary, FiatCurrency, Price, PricePage, PriceSeries, PriceSyncSummary,
    PriceTableQuery, SyncGranularity,
};
use dioxus_fullstack_core::ServerFnError;

pub async fn latest_price_api(symbol: String) -> Result<Price, ServerFnError> {
    crate::web_get_query!("prices.latest", "/api/prices/latest", { "symbol": symbol })
}

pub async fn list_prices_api(query: PriceTableQuery) -> Result<PricePage, ServerFnError> {
    crate::web_post_body!("prices.list", "/api/prices/list", { "query": query })
}

pub async fn sync_prices_api(
    symbol: String,
    from_time: i64,
    to_time: i64,
    granularity: SyncGranularity,
) -> Result<PriceSyncSummary, ServerFnError> {
    crate::web_post_body!(
        "prices.sync",
        "/api/prices/sync",
        {
            "symbol": symbol,
            "from_time": from_time,
            "to_time": to_time,
            "granularity": granularity
        }
    )
}

pub async fn ensure_currency_history_api(
    symbol: String,
) -> Result<CurrencyHistorySummary, ServerFnError> {
    crate::web_post_body!(
        "prices.ensure_currency_history",
        "/api/prices/ensure-currency-history",
        { "symbol": symbol }
    )
}

pub async fn price_conversions_api(
    symbol: String,
    close_time: i64,
) -> Result<Vec<Price>, ServerFnError> {
    crate::web_get_query!(
        "prices.conversions",
        "/api/prices/conversions",
        { "symbol": symbol, "close_time": close_time }
    )
}

pub async fn update_price_api(
    symbol: String,
    close_time: i64,
    close: f64,
) -> Result<Price, ServerFnError> {
    crate::web_post_body!(
        "prices.update",
        "/api/prices/update",
        { "symbol": symbol, "close_time": close_time, "close": close }
    )
}

pub async fn delete_prices_api(close_times: Vec<i64>) -> Result<usize, ServerFnError> {
    crate::web_post_body!("prices.delete", "/api/prices/delete", { "close_times": close_times })
}

pub async fn price_series_api(
    symbol: String,
    from_time: i64,
    to_time: i64,
) -> Result<Option<PriceSeries>, ServerFnError> {
    crate::web_get_query!(
        "prices.series",
        "/api/prices/series",
        { "symbol": symbol, "from_time": from_time, "to_time": to_time }
    )
}

pub async fn conversion_currencies_api() -> Result<Vec<FiatCurrency>, ServerFnError> {
    crate::web_get_query!("prices.currencies", "/api/prices/currencies", {})
}
