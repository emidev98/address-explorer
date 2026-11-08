//! Domain types and pure business rules for BTC prices in fiat.
//!
//! The 15-minute `USD` history comes from the data migration in
//! `data::seeds`. For anything missing — other symbols, days beyond the
//! imported coverage, or candles lost to the historical exchange outages —
//! `application::prices` checks the database first, asks the external APIs
//! exactly once, and stores every resolved price back into the table so the
//! same lookup never touches the network twice.
#![cfg_attr(not(feature = "server"), allow(dead_code))]
use super::civil_time;
use super::table::{TablePage, TableQuery};
use serde::{Deserialize, Serialize};
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
/// First day Coin Metrics serves the `PriceUSD` daily metric (2010-07-18).
/// It is the historical floor of every sync and the default lower bound of the
/// sync dialog; the live catalogue remains authoritative when it is reachable.
pub const PRICE_HISTORY_START: i64 = civil_time::epoch_seconds(2010, 7, 18, 0, 0, 0);
/// Convert a satoshi balance into its fiat value at `btc_price`.
///
/// `btc_price` is the BTC unit price in the target currency, so the result is
/// quoted in that same currency.
pub fn sats_to_fiat(satoshis: i64, btc_price: f64) -> f64 {
    satoshis as f64 / 100_000_000.0 * btc_price
}
/// Convert a fiat amount into its BTC-equivalent satoshis at `btc_price`.
///
/// Returns `None` when the price is not finite or not positive, because a
/// non-positive BTC price cannot express a meaningful conversion.
pub fn fiat_to_sats(amount: f64, btc_price: f64) -> Option<i64> {
    if !amount.is_finite() || !btc_price.is_finite() || btc_price <= 0.0 {
        return None;
    }
    Some((amount / btc_price * 100_000_000.0).round() as i64)
}
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
/// Outcome of preparing the stored price history of the default currency.
///
/// The source is always the stored USD history. `stored` counts the missing
/// `(symbol, close_time)` rows this run inserted; existing target rows, such
/// as manually edited prices, are never overwritten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrencyHistorySummary {
    pub symbol: String,
    /// USD source rows found in the range that needed conversion.
    pub source_rows: u64,
    /// Missing target rows actually inserted.
    pub stored: u64,
    /// Source rows skipped because no FX rate was available for their day.
    pub skipped: u64,
    /// Non-fatal problem (network failure, missing source history, skipped
    /// rates); the preference stays saved and the operation can be retried.
    pub warning: Option<String>,
}
/// Resolution of the closes fetched for the most recent days of a sync.
///
/// Coin Metrics serves BTC/USD daily closes for the whole history and a
/// reference rate at finer frequencies only for a short rolling window, so the
/// granularity of a sync only ever applies to the newest slice of the range;
/// older days always come from the daily series.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncGranularity {
    /// Pick the finest resolution the source can serve for the selected range.
    #[default]
    Auto,
    /// Daily closes only, from the `PriceUSD` metric.
    Daily,
    /// One observation every 15 minutes, derived from the 1-minute reference
    /// rate (Coin Metrics has no `15m` frequency of its own).
    FifteenMinutes,
    /// Hourly reference-rate observations.
    Hourly,
}
impl SyncGranularity {
    /// Human-readable label of the option.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Daily => "Daily",
            Self::FifteenMinutes => "15 minutes",
            Self::Hourly => "Hourly",
        }
    }
    /// Adjective used in status and warning sentences.
    pub fn resolution_label(self) -> &'static str {
        match self {
            Self::Auto => "intraday",
            Self::Daily => "daily",
            Self::FifteenMinutes => "15-minute",
            Self::Hourly => "hourly",
        }
    }
    /// Stable key used by the dialog's selector.
    pub fn key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Daily => "1d",
            Self::FifteenMinutes => "15m",
            Self::Hourly => "1h",
        }
    }
    /// Parse the selector key.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "auto" => Some(Self::Auto),
            "1d" => Some(Self::Daily),
            "15m" => Some(Self::FifteenMinutes),
            "1h" => Some(Self::Hourly),
            _ => None,
        }
    }
    /// Coin Metrics frequency the intraday stage requests; `None` when the
    /// option needs no intraday fetch.
    pub fn source_frequency(self) -> Option<&'static str> {
        match self {
            Self::Auto | Self::Daily => None,
            Self::FifteenMinutes => Some("1m"),
            Self::Hourly => Some("1h"),
        }
    }
    /// Whether the option fetches closes finer than one day.
    pub fn is_intraday(self) -> bool {
        matches!(self, Self::FifteenMinutes | Self::Hourly)
    }
}
/// Outcome of one run of the unified "Sync Prices" action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceSyncSummary {
    /// Currency the run prepared, e.g. `USD` or `EUR`.
    pub symbol: String,
    /// Inclusive bounds of the requested range (clamped to the source span).
    pub from_time: i64,
    pub to_time: i64,
    /// Resolution actually applied to the recent window; `Auto` never appears.
    pub granularity: SyncGranularity,
    /// Inclusive bounds of the intraday window, when one was synced.
    pub granular_from_time: Option<i64>,
    pub granular_to_time: Option<i64>,
    /// BTC/USD daily closes returned by the source and inserted.
    pub daily_fetched: u64,
    pub daily_stored: u64,
    /// Intraday closes returned by the source and inserted.
    pub granular_fetched: u64,
    pub granular_stored: u64,
    /// Complete days of the range the daily source had no price for.
    pub unavailable_days: u64,
    /// Conversion into `symbol` when it is not USD; `None` for USD runs.
    pub conversion: Option<BulkSyncSummary>,
    /// Non-fatal problems (catalogue, intraday stage, conversion). The run
    /// stored everything else it could.
    pub warnings: Vec<String>,
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
/// Normalize a stored default-currency setting, falling back to USD when empty.
pub fn normalize_default_symbol(symbol: &str) -> String {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        USD_SYMBOL.to_string()
    } else {
        symbol.to_ascii_uppercase()
    }
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
/// Close time of the interval of `unit_seconds` containing `timestamp`.
///
/// Coin Metrics timestamps rows at the start of their interval while the table
/// stores closes, so the last second of the interval is the stored key.
pub fn unit_close_time(timestamp: i64, unit_seconds: i64) -> i64 {
    let unit = unit_seconds.max(1);
    timestamp.div_euclid(unit) * unit + unit - 1
}
/// Close time of the 15-minute candle containing `timestamp`.
pub fn fifteen_minute_close_time(timestamp: i64) -> i64 {
    unit_close_time(timestamp, FIFTEEN_MINUTES)
}
/// Close time of the UTC day containing `timestamp`.
pub fn daily_close_time(timestamp: i64) -> i64 {
    unit_close_time(timestamp, DAY_SECONDS)
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
            ),);
        }
    }
    #[test]
    fn fifteen_minute_bucket_maps_to_candle_close_time() {
        let candle_open = civil_time::epoch_seconds(2018, 1, 1, 0, 15, 0);
        assert_eq!(
            fifteen_minute_close_time(candle_open),
            civil_time::epoch_seconds(2018, 1, 1, 0, 29, 59),
        );
        assert_eq!(
            fifteen_minute_close_time(civil_time::epoch_seconds(2018, 1, 1, 0, 14, 59)),
            civil_time::epoch_seconds(2018, 1, 1, 0, 14, 59),
        );
    }
    #[test]
    fn daily_bucket_maps_to_day_end() {
        let noon = civil_time::epoch_seconds(2026, 7, 1, 12, 0, 0);
        assert_eq!(
            daily_close_time(noon),
            civil_time::epoch_seconds(2026, 7, 1, 23, 59, 59),
        );
    }
    #[test]
    fn unit_close_time_lands_on_the_last_second_of_the_interval() {
        let start = civil_time::epoch_seconds(2026, 7, 1, 12, 0, 0);
        assert_eq!(
            unit_close_time(start, 60),
            civil_time::epoch_seconds(2026, 7, 1, 12, 0, 59),
        );
        assert_eq!(
            unit_close_time(start, 15 * 60),
            civil_time::epoch_seconds(2026, 7, 1, 12, 14, 59),
        );
        assert_eq!(
            unit_close_time(start, 3_600),
            civil_time::epoch_seconds(2026, 7, 1, 12, 59, 59),
        );
        assert_eq!(
            unit_close_time(start, 86_400),
            civil_time::epoch_seconds(2026, 7, 1, 23, 59, 59),
        );
        assert_eq!(
            unit_close_time(civil_time::epoch_seconds(2026, 7, 1, 12, 59, 59), 3_600),
            civil_time::epoch_seconds(2026, 7, 1, 12, 59, 59),
        );
        assert_eq!(
            unit_close_time(start, 0),
            civil_time::epoch_seconds(2026, 7, 1, 12, 0, 0),
        );
    }
    #[test]
    fn sync_granularity_keys_round_trip_and_never_source_auto() {
        for granularity in [
            SyncGranularity::Auto,
            SyncGranularity::Daily,
            SyncGranularity::FifteenMinutes,
            SyncGranularity::Hourly,
        ] {
            assert_eq!(
                SyncGranularity::from_key(granularity.key()),
                Some(granularity)
            );
            assert!(!granularity.label().is_empty());
            assert!(!granularity.resolution_label().is_empty());
        }
        assert_eq!(SyncGranularity::from_key("1m"), None);
        assert_eq!(SyncGranularity::from_key("5m"), None);
        assert_eq!(SyncGranularity::Auto.source_frequency(), None);
        assert_eq!(SyncGranularity::Daily.source_frequency(), None);
        assert_eq!(
            SyncGranularity::FifteenMinutes.source_frequency(),
            Some("1m")
        );
        assert_eq!(SyncGranularity::Hourly.source_frequency(), Some("1h"));
        assert!(!SyncGranularity::Daily.is_intraday());
        assert!(SyncGranularity::FifteenMinutes.is_intraday());
    }
    #[test]
    fn price_history_starts_on_the_first_coin_metrics_day() {
        assert_eq!(
            civil_time::format_date(PRICE_HISTORY_START),
            "2010-07-18".to_string(),
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
    fn satoshi_balances_convert_to_the_quoted_fiat_value() {
        assert_eq!(sats_to_fiat(100_000_000, 66_000.0), 66_000.0);
        assert_eq!(sats_to_fiat(1, 100_000_000.0), 1.0);
        assert_eq!(sats_to_fiat(0, 66_000.0), 0.0);
        assert!((sats_to_fiat(50_000, 0.5) - 0.00025).abs() < f64::EPSILON);
    }
    #[test]
    fn fiat_fees_convert_to_btc_equivalent_satoshis() {
        assert_eq!(fiat_to_sats(60_000.0, 60_000.0), Some(100_000_000));
        assert_eq!(fiat_to_sats(2.75, 60_000.0), Some(4_583));
        assert_eq!(fiat_to_sats(0.0, 60_000.0), Some(0));
        assert_eq!(fiat_to_sats(10.0, 0.0), None);
        assert_eq!(fiat_to_sats(10.0, -1.0), None);
        assert_eq!(fiat_to_sats(f64::NAN, 60_000.0), None);
    }
    #[test]
    fn rejects_invalid_price_values() {
        assert!(validate_price_value(f64::NAN, "USD", "test").is_err());
        assert!(validate_price_value(0.0, "USD", "test").is_err());
        assert!(validate_price_value(-1.0, "USD", "test").is_err());
        assert!(validate_price_value(42.0, "USD", "test").is_ok());
    }
}
