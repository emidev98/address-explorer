//! Price endpoints.

use dioxus::prelude::*;

use crate::domain::price::{
    CurrencyHistorySummary, FiatCurrency, Price, PricePage, PriceSeries, PriceSyncSummary,
    PriceTableQuery, SyncGranularity,
};

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::prices::{self, PriceService};

/// Resolve the most recent BTC price in `symbol`: the freshest stored close
/// (an intraday observation when one was synced), falling back to the previous
/// day when today's daily close is not published yet.
#[server]
#[get("/api/prices/latest?symbol")]
pub async fn latest_price_api(symbol: String) -> Result<Price, ServerFnError> {
    PriceService::connect(open_database()?)
        .latest_price(&symbol)
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

/// Synchronize the stored BTC price history of `[from_time, to_time]`.
///
/// The run always fills the BTC/USD daily closes of the range and adds the
/// finest intraday closes the free source still serves for its recent days.
/// A non-USD `symbol` is then converted from every stored USD close time of
/// the range with Frankfurter's daily rates. Existing rows are left untouched.
#[server]
#[post("/api/prices/sync")]
pub async fn sync_prices_api(
    symbol: String,
    from_time: i64,
    to_time: i64,
    granularity: SyncGranularity,
) -> Result<PriceSyncSummary, ServerFnError> {
    PriceService::connect(open_database()?)
        .sync_prices(&symbol, from_time, to_time, granularity)
        .await
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Prepare the stored USD price history in `symbol`.
///
/// This is idempotent: an already prepared currency returns immediately
/// without a network call. Existing target rows, including manually edited
/// prices, are never overwritten. Non-fatal conversion problems are reported
/// in the summary warning so the saved preference is not rolled back.
#[server]
#[post("/api/prices/ensure-currency-history")]
pub async fn ensure_currency_history_api(
    symbol: String,
) -> Result<CurrencyHistorySummary, ServerFnError> {
    PriceService::connect(open_database()?)
        .ensure_currency_history(&symbol)
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

/// Delete every stored price at the given close times in one transaction.
///
/// A close time removes the whole snapshot, including every currency stored
/// at that moment. Returns the number of deleted rows.
#[server]
#[post("/api/prices/delete")]
pub async fn delete_prices_api(close_times: Vec<i64>) -> Result<usize, ServerFnError> {
    prices::delete_stored_prices(&open_database()?, &close_times)
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
