//! Domain types and pure business rules for BTC prices in fiat.
//!
//! The 15-minute `USD` history comes from the data migration in
//! `data::seeds`. For anything missing — other symbols, days beyond the
//! imported coverage, or candles lost to the historical exchange outages —
//! `application::prices` checks the database first, asks the external APIs
//! exactly once, and stores every resolved price back into the table so the
//! same lookup never touches the network twice.

// Price resolution runs behind the server feature; web builds only need the
// API-facing types, so the server-only items are allowed to look dead there.
#![cfg_attr(not(feature = "server"), allow(dead_code))]

use serde::{Deserialize, Serialize};

use super::civil_time;
use super::table::{TablePage, TableQuery};

/// Quote symbol of the imported 15-minute history and the reference currency
/// all other symbols derive from. It is also the fallback of the
/// `default_fiat_currency` setting.
pub const USD_SYMBOL: &str = "USD";

/// Fiat currencies offered as the default display currency in Settings and
/// resolved by `application::prices` conversions. The codes match the
/// catalogue of the Frankfurter instance that supplies the `USD → symbol`
/// rates.
pub const FIAT_CURRENCIES: &[&str] = &[
    "AUD", "BGN", "BRL", "CAD", "CHF", "CNY", "CZK", "DKK", "EUR", "GBP", "HKD", "HUF", "IDR",
    "ILS", "INR", "ISK", "JPY", "KRW", "MXN", "MYR", "NOK", "NZD", "PHP", "PLN", "RON", "SEK",
    "SGD", "THB", "TRY", "USD", "ZAR",
];

const FIFTEEN_MINUTES: i64 = 15 * 60;
const DAY_SECONDS: i64 = 24 * 60 * 60;

/// Close time of the last complete 15-minute candle imported from
/// `assets/pricing`. Requests beyond it resolve at daily granularity because
/// the live sources publish daily data.
pub const CSV_COVERAGE_END: i64 = civil_time::epoch_seconds(2026, 6, 15, 23, 29, 59);

/// A BTC price in `symbol` at a point in time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Price {
    /// Quote currency code, e.g. `USD` or `EUR`.
    pub symbol: String,
    /// UTC close time of the quoted candle, in epoch seconds.
    pub close_time: i64,
    /// BTC close price in `symbol` at `close_time`.
    pub close: f64,
}

/// Conversion between two BTC price quotes anchored to the same moment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceConversion {
    pub from_symbol: String,
    pub to_symbol: String,
    /// UTC close time of the candle both quotes resolve to.
    pub close_time: i64,
    /// Units of `to_symbol` per unit of `from_symbol` of the BTC price.
    pub rate: f64,
}

/// One row of the historical prices table listing: a stored price plus the
/// other symbols stored at the same close time — the "other available
/// exchanges" of the row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceRow {
    #[serde(flatten)]
    pub price: Price,
    /// Other symbols stored at the same close time, sorted by code; empty
    /// when the close time exists only in the listed symbol.
    pub other_symbols: Vec<String>,
}

/// One page of the stored price history of a symbol.
pub type PricePage = TablePage<PriceRow>;

/// Query of the historical prices endpoint: the shared table contract
/// (pagination, sorting) plus the price-specific filters — the
/// symbol whose history is listed and an optional close-time range (a bound
/// of `0` or less leaves that side open).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceTableQuery {
    #[serde(flatten)]
    pub table: TableQuery,
    /// Quote currency whose history is listed, e.g. `USD`.
    pub symbol: String,
    /// Inclusive lower close-time bound in epoch seconds; `0` or less is open.
    pub from_time: i64,
    /// Inclusive upper close-time bound in epoch seconds; `0` or less is open.
    pub to_time: i64,
}

/// Outcome of a bulk price sync: the stored USD history of
/// `[from_time, to_time]` converted into `symbol` and persisted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BulkSyncSummary {
    /// Target currency the prices were converted into.
    pub symbol: String,
    pub from_time: i64,
    pub to_time: i64,
    /// Stored USD rows found in the range; the sync only ever populates
    /// close times that already exist in USD.
    pub source_rows: u64,
    /// Rows actually inserted; existing `(symbol, close_time)` rows are left
    /// untouched and do not count.
    pub stored: u64,
    /// Source rows skipped because no FX rate was available for their day.
    pub skipped: u64,
}

/// Outcome of a "sync to today" run: BTC/USD daily closes fetched from Coin
/// Metrics between the latest stored close time and the current day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncToTodaySummary {
    /// Latest stored BTC/USD close time when the sync started.
    pub from_time: i64,
    /// Current time the sync ran up to.
    pub to_time: i64,
    /// Daily closes returned by Coin Metrics within the window.
    pub fetched: u64,
    /// Rows actually inserted; existing `close_time` rows are left untouched
    /// and do not count.
    pub stored: u64,
}

/// Outcome of one run of the "Sync Prices" dialog, whichever mode was used.
#[derive(Debug, Clone, PartialEq)]
pub enum PriceSyncOutcome {
    /// The stored USD history of a range was converted into another currency.
    Historical(BulkSyncSummary),
    /// BTC/USD daily closes were fetched up to the current day.
    ToToday(SyncToTodaySummary),
}

/// The stored price series of a symbol for the trend chart: the full span of
/// the history plus the closes of the requested window, downsampled to a
/// bounded number of time buckets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceSeries {
    /// Close time of the oldest stored row of the symbol.
    pub first_close_time: i64,
    /// Close time of the newest stored row of the symbol.
    pub last_close_time: i64,
    /// Bucketed average closes of the requested window, oldest first.
    pub points: Vec<Price>,
}

/// One entry of the currency catalogue offered for price conversions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiatCurrency {
    /// Currency code, e.g. `EUR`.
    pub code: String,
    /// Full currency name, e.g. `Euro`.
    pub name: String,
}

/// Failures of price resolution that belong to the domain itself.
#[derive(Debug, thiserror::Error)]
pub enum PriceError {
    #[error("invalid symbol {0:?}: use an uppercase currency code such as USD or EUR")]
    InvalidSymbol(String),
    #[error("invalid timestamp {0}")]
    InvalidTimestamp(i64),
    #[error("no price available for {symbol} on {date}")]
    NotFound { symbol: String, date: String },
    #[error("price source returned an invalid value: {0}")]
    InvalidData(String),
    #[error(
        "invalid sort column {0:?}: the historical prices table sorts by \"date\" or \"price\""
    )]
    InvalidSortColumn(String),
}

/// Normalize and validate a currency code for storage and API queries.
pub fn normalize_symbol(symbol: &str) -> Result<String, PriceError> {
    let symbol = symbol.trim().to_ascii_uppercase();
    if (3..=8).contains(&symbol.len())
        && symbol
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
    {
        Ok(symbol)
    } else {
        Err(PriceError::InvalidSymbol(symbol))
    }
}

/// Close time of the 15-minute candle containing `timestamp`.
pub fn fifteen_minute_close_time(timestamp: i64) -> i64 {
    timestamp.div_euclid(FIFTEEN_MINUTES) * FIFTEEN_MINUTES + FIFTEEN_MINUTES - 1
}

/// Close time of the UTC day containing `timestamp`.
pub fn daily_close_time(timestamp: i64) -> i64 {
    timestamp.div_euclid(DAY_SECONDS) * DAY_SECONDS + DAY_SECONDS - 1
}

/// Reject non-finite or non-positive values coming from external sources.
pub fn validate_price_value(value: f64, symbol: &str, source: &str) -> Result<f64, PriceError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(PriceError::InvalidData(format!(
            "{source} returned {value} for {symbol}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_currency_codes() {
        assert_eq!(normalize_symbol("usd").unwrap(), "USD");
        assert_eq!(normalize_symbol(" eur ").unwrap(), "EUR");
        assert_eq!(normalize_symbol("USDT").unwrap(), "USDT");

        for invalid in ["", "us", "longsymbol", "U$D", "usd."] {
            assert!(matches!(
                normalize_symbol(invalid),
                Err(PriceError::InvalidSymbol(_))
            ));
        }
    }

    #[test]
    fn fifteen_minute_bucket_maps_to_candle_close_time() {
        let candle_open = civil_time::epoch_seconds(2018, 1, 1, 0, 15, 0);
        assert_eq!(
            fifteen_minute_close_time(candle_open),
            civil_time::epoch_seconds(2018, 1, 1, 0, 29, 59)
        );
        // The close time itself stays in its own candle.
        assert_eq!(
            fifteen_minute_close_time(civil_time::epoch_seconds(2018, 1, 1, 0, 14, 59)),
            civil_time::epoch_seconds(2018, 1, 1, 0, 14, 59)
        );
    }

    #[test]
    fn daily_bucket_maps_to_day_end() {
        let noon = civil_time::epoch_seconds(2026, 7, 1, 12, 0, 0);
        assert_eq!(
            daily_close_time(noon),
            civil_time::epoch_seconds(2026, 7, 1, 23, 59, 59)
        );
    }

    #[test]
    fn fiat_currencies_include_the_default_and_are_uppercase_codes() {
        assert!(FIAT_CURRENCIES.contains(&USD_SYMBOL));
        for code in FIAT_CURRENCIES {
            assert_eq!(normalize_symbol(code).unwrap(), *code);
        }
    }

    #[test]
    fn rejects_invalid_price_values() {
        assert!(validate_price_value(f64::NAN, "USD", "test").is_err());
        assert!(validate_price_value(0.0, "USD", "test").is_err());
        assert!(validate_price_value(-1.0, "USD", "test").is_err());
        assert!(validate_price_value(42.0, "USD", "test").is_ok());
    }
}
