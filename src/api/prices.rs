//! Price endpoints.

use dioxus::prelude::*;

use crate::domain::price::{
    BulkSyncSummary, FiatCurrency, Price, PriceConversion, PricePage, PriceSeries, PriceTableQuery,
    SyncToTodaySummary,
};

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::prices::{self, PriceService};

/// Resolve the BTC price in `symbol` at `timestamp` (UTC seconds). The
/// database is checked first; an API answer is stored before returning.
#[server]
#[post("/api/prices/get")]
pub async fn get_price_api(symbol: String, timestamp: i64) -> Result<Price, ServerFnError> {
    PriceService::connect(open_database()?)
        .get_price(&symbol, timestamp)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Resolve the most recent BTC price in `symbol`: today's daily close when
/// published, otherwise yesterday's.
#[server]
#[get("/api/prices/latest?symbol")]
pub async fn latest_price_api(symbol: String) -> Result<Price, ServerFnError> {
    PriceService::connect(open_database()?)
        .latest_price(&symbol)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Convert the BTC price quoted in `from_symbol` into `to_symbol` at the same
/// moment, e.g. a historical EUR → USD conversion anchored to the BTC price.
#[server]
#[post("/api/prices/convert")]
pub async fn convert_price_api(
    from_symbol: String,
    to_symbol: String,
    timestamp: i64,
) -> Result<PriceConversion, ServerFnError> {
    PriceService::connect(open_database()?)
        .convert_price(&from_symbol, &to_symbol, timestamp)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Read one page of the stored history matching `query`: the shared table
/// contract (page, page size, backend sorting) plus the
/// price-specific symbol and close-time bounds (a bound of `0` or less
/// leaves that side open).
#[server]
#[post("/api/prices/list")]
pub async fn list_prices_api(query: PriceTableQuery) -> Result<PricePage, ServerFnError> {
    prices::list_price_page(
        &open_database()?,
        &query.symbol,
        query.from_time,
        query.to_time,
        &query.table,
    )
    .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Convert the stored USD history of `[from_time, to_time]` into `symbol`
/// and persist it. The sync only populates close times that already exist
/// in USD, matching the granularity of the table; existing rows are left
/// untouched.
#[server]
#[post("/api/prices/sync")]
pub async fn sync_prices_api(
    symbol: String,
    from_time: i64,
    to_time: i64,
) -> Result<BulkSyncSummary, ServerFnError> {
    PriceService::connect(open_database()?)
        .sync_prices(&symbol, from_time, to_time)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Fetch BTC/USD daily closes from the latest stored close through the
/// current day and persist them. Existing rows are left untouched.
#[server]
#[post("/api/prices/sync-to-today")]
pub async fn sync_prices_to_today_api() -> Result<SyncToTodaySummary, ServerFnError> {
    PriceService::connect(open_database()?)
        .sync_prices_to_today()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Read the stored prices of the other symbols at `close_time`: the
/// conversions displayed when hovering a row of the historical prices table.
#[server]
#[get("/api/prices/conversions?symbol&close_time")]
pub async fn price_conversions_api(
    symbol: String,
    close_time: i64,
) -> Result<Vec<Price>, ServerFnError> {
    prices::list_price_conversions(&open_database()?, &symbol, close_time)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Update the close of a stored price. Editing never creates rows: a missing
/// `(symbol, close_time)` pair is a not-found failure.
#[server]
#[post("/api/prices/update")]
pub async fn update_price_api(
    symbol: String,
    close_time: i64,
    close: f64,
) -> Result<Price, ServerFnError> {
    prices::update_stored_price(&open_database()?, &symbol, close_time, close)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Read the stored price series of `symbol` for the trend chart, downsampled
/// to a bounded number of bucketed averages for `[from_time, to_time]`
/// (bounds of `0` or less fall back to the full stored span). `None` when the
/// symbol has no stored rows.
#[server]
#[get("/api/prices/series?symbol&from_time&to_time")]
pub async fn price_series_api(
    symbol: String,
    from_time: i64,
    to_time: i64,
) -> Result<Option<PriceSeries>, ServerFnError> {
    prices::price_series(&open_database()?, &symbol, from_time, to_time)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// List the currencies the Frankfurter catalogue offers for conversions, the
/// choices of the "add conversion" action of the historical prices table.
#[server]
#[get("/api/prices/currencies")]
pub async fn conversion_currencies_api() -> Result<Vec<FiatCurrency>, ServerFnError> {
    PriceService::connect(open_database()?)
        .conversion_currencies()
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}
