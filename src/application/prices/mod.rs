//! Price service: keeps the `historical_prices` table populated and resolves
//! BTC prices in fiat.
//!
//! Resolution checks the database first, asks the external APIs exactly once
//! per missing day, and stores every resolved price back into the table so
//! the same lookup never touches the network twice. The unified sync fills the
//! BTC/USD daily closes of a range, adds the finer recent reference-rate
//! observations when the free tier serves them, and converts the stored USD
//! close times into another currency on request. The default-currency
//! preparation reuses the same bulk conversion to materialize the stored USD
//! history in whichever currency Settings selects. SQL stays in
//! `data::repositories::prices`; the external sources stay behind the
//! `CoinMetrics` and `Frankfurter` clients.
use crate::clients::coinmetrics::catalog::CatalogAssetMetricsQuery;
use crate::clients::coinmetrics::params::{Page, TimeRange};
use crate::clients::coinmetrics::timeseries::AssetMetricsQuery;
use crate::clients::coinmetrics::{
    CatalogScope, CoinMetrics, CoinMetricsConfig, CoinMetricsError, Timeseries,
};
use crate::clients::frankfurter::{
    Frankfurter, FrankfurterConfig, FrankfurterError, Rates, RatesQuery,
};
use crate::data::repositories::prices::{self as price_repository, PriceSortColumn};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::price::{
    daily_close_time, fifteen_minute_close_time, normalize_symbol, validate_price_value,
    BulkSyncSummary, CurrencyHistorySummary, FiatCurrency, Price, PriceError, PricePage,
    PriceSeries, PriceSyncSummary, SyncGranularity, CSV_COVERAGE_END, PRICE_HISTORY_START,
    USD_SYMBOL,
};
use crate::domain::table::TableQuery;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
/// Failures of the price service: domain validation and resolution failures,
/// plus the infrastructure errors of the database and the external sources.
#[derive(Debug, thiserror::Error)]
pub enum PriceServiceError {
    #[error(transparent)]
    Price(#[from] PriceError),
    #[error("{0}")]
    Validation(String),
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("coin metrics request failed: {0}")]
    CoinMetrics(#[from] CoinMetricsError),
    #[error("exchange-rate request failed: {0}")]
    Frankfurter(#[from] FrankfurterError),
}
/// Owned future produced by a fetcher closure.
#[cfg(not(target_arch = "wasm32"))]
type FetchFuture = Pin<Box<dyn Future<Output = Result<f64, PriceServiceError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type FetchFuture = Pin<Box<dyn Future<Output = Result<f64, PriceServiceError>>>>;
/// Owned future producing the FX series of a bulk sync: `(date, rate)`
/// pairs covering the requested range.
#[cfg(not(target_arch = "wasm32"))]
type FetchSeriesFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(String, f64)>, PriceServiceError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type FetchSeriesFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(String, f64)>, PriceServiceError>>>>;
/// Owned future producing daily BTC/USD closes for a range.
#[cfg(not(target_arch = "wasm32"))]
type FetchDailyFuture = Pin<Box<dyn Future<Output = Result<Vec<Price>, PriceServiceError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type FetchDailyFuture = Pin<Box<dyn Future<Output = Result<Vec<Price>, PriceServiceError>>>>;
/// Owned future producing reference-rate observations as `(interval start,
/// value)` pairs for a frequency and range.
#[cfg(not(target_arch = "wasm32"))]
type FetchReferenceFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(i64, f64)>, PriceServiceError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type FetchReferenceFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(i64, f64)>, PriceServiceError>>>>;

#[cfg(not(target_arch = "wasm32"))]
type FetchUsdFn<'a> = dyn Fn(String) -> FetchFuture + Send + Sync + 'a;
#[cfg(target_arch = "wasm32")]
type FetchUsdFn<'a> = dyn Fn(String) -> FetchFuture + 'a;
#[cfg(not(target_arch = "wasm32"))]
type FetchFxFn<'a> = dyn Fn(String, String) -> FetchFuture + Send + Sync + 'a;
#[cfg(target_arch = "wasm32")]
type FetchFxFn<'a> = dyn Fn(String, String) -> FetchFuture + 'a;
#[cfg(not(target_arch = "wasm32"))]
type FetchDailyFn<'a> = dyn Fn(i64, i64) -> FetchDailyFuture + Send + Sync + 'a;
#[cfg(target_arch = "wasm32")]
type FetchDailyFn<'a> = dyn Fn(i64, i64) -> FetchDailyFuture + 'a;
#[cfg(not(target_arch = "wasm32"))]
type FetchReferenceFn<'a> = dyn Fn(&str, i64, i64) -> FetchReferenceFuture + Send + Sync + 'a;
#[cfg(target_arch = "wasm32")]
type FetchReferenceFn<'a> = dyn Fn(&str, i64, i64) -> FetchReferenceFuture + 'a;
#[cfg(not(target_arch = "wasm32"))]
type FetchSeriesFn<'a> = dyn Fn(String, String) -> FetchSeriesFuture + Send + Sync + 'a;
#[cfg(target_arch = "wasm32")]
type FetchSeriesFn<'a> = dyn Fn(String, String) -> FetchSeriesFuture + 'a;
/// One 15-minute interval in seconds: the spacing of the imported history and
/// of the finest intraday closes the sync stores.
const FIFTEEN_MINUTES: i64 = 15 * 60;
/// How far back price resolution looks for a stored intraday close after the
/// imported 15-minute coverage. Within the synchronized granular window every
/// interval has a row, so one hour always finds the newest one.
const GRANULAR_RESOLUTION_LOOKBACK: i64 = 3_600;
/// One day in seconds: the fallback distance of [`latest_price_at`].
const LATEST_PRICE_FALLBACK_SECONDS: i64 = 86_400;
/// Extra days fetched before a conversion range. The latest rate on or before
/// a close time is used, so a range that starts on a weekend or holiday needs
/// the preceding business day's rate from before the range boundary.
const FX_RATE_LOOKBACK_DAYS: i64 = 7;
/// Current UTC epoch seconds from the server clock.
fn current_epoch_seconds() -> Result<i64, PriceServiceError> {
    Ok(civil_time::now_epoch_seconds())
}
/// Facade over the database and the external price sources. Every price
/// requested through this service is checked against `historical_prices`
/// first; API answers are stored before being returned.
pub struct PriceService {
    database: Database,
    coinmetrics: CoinMetrics,
    frankfurter: Frankfurter,
}
impl PriceService {
    /// Connect the clients with their default configurations (Coin Metrics
    /// Community API and the public Frankfurter instance).
    pub fn connect(database: Database) -> Self {
        Self {
            database,
            coinmetrics: CoinMetrics::new(CoinMetricsConfig {
                timeout: Some(Duration::from_secs(20)),
                ..CoinMetricsConfig::default()
            }),
            frankfurter: Frankfurter::new(FrankfurterConfig {
                timeout: Some(Duration::from_secs(20)),
                ..FrankfurterConfig::default()
            }),
        }
    }
    /// Resolve the BTC price in `symbol` at `timestamp`; see
    /// [`resolve_price`] for the storage behaviour.
    pub async fn get_price(
        &self,
        symbol: &str,
        timestamp: i64,
    ) -> Result<Price, PriceServiceError> {
        let fetch_usd = |date: String| -> FetchFuture {
            let timeseries = self.coinmetrics.timeseries.clone();
            Box::pin(async move { fetch_btc_usd_close(&timeseries, &date).await })
        };
        let fetch_fx = |date: String, quote: String| -> FetchFuture {
            let rates = self.frankfurter.rates.clone();
            Box::pin(async move { fetch_usd_fx_rate(&rates, &date, &quote).await })
        };
        resolve_price(&self.database, symbol, timestamp, &fetch_usd, &fetch_fx).await
    }
    /// Resolve the most recent BTC price in `symbol` at the server's clock,
    /// falling back one day when today's close is not published yet; see
    /// [`latest_price_at`].
    pub async fn latest_price(&self, symbol: &str) -> Result<Price, PriceServiceError> {
        let now = current_epoch_seconds()?;
        let fetch_usd = |date: String| -> FetchFuture {
            let timeseries = self.coinmetrics.timeseries.clone();
            Box::pin(async move { fetch_btc_usd_close(&timeseries, &date).await })
        };
        let fetch_fx = |date: String, quote: String| -> FetchFuture {
            let rates = self.frankfurter.rates.clone();
            Box::pin(async move { fetch_usd_fx_rate(&rates, &date, &quote).await })
        };
        latest_price_at(&self.database, symbol, now, &fetch_usd, &fetch_fx).await
    }
    /// Synchronize the BTC/USD history of `[from_time, to_time]` and, when
    /// `symbol` is not USD, convert every stored USD close time of the range
    /// into it; see [`sync_prices_range`]. The Coin Metrics catalogue is read
    /// once to discover the daily start and the finer reference-rate window.
    pub async fn sync_prices(
        &self,
        symbol: &str,
        from_time: i64,
        to_time: i64,
        granularity: SyncGranularity,
    ) -> Result<PriceSyncSummary, PriceServiceError> {
        let symbol = normalize_symbol(symbol)?;
        let now = current_epoch_seconds()?;
        let (coverage, warning) = self.btc_usd_coverage(now).await;
        let plan = plan_price_sync(granularity, &coverage, from_time, to_time, now);
        let fetch_daily = |from_time: i64, to_time: i64| -> FetchDailyFuture {
            let timeseries = self.coinmetrics.timeseries.clone();
            Box::pin(async move { fetch_btc_usd_daily(&timeseries, from_time, to_time).await })
        };
        let fetch_reference =
            |frequency: &str, from_time: i64, to_time: i64| -> FetchReferenceFuture {
                let timeseries = self.coinmetrics.timeseries.clone();
                let frequency = frequency.to_string();
                Box::pin(async move {
                    fetch_btc_usd_reference(&timeseries, &frequency, from_time, to_time).await
                })
            };
        let quote = symbol.clone();
        let fetch_fx_series = |from_date: String, to_date: String| -> FetchSeriesFuture {
            let rates = self.frankfurter.rates.clone();
            let quote = quote.clone();
            Box::pin(async move { fetch_usd_fx_series(&rates, &from_date, &to_date, &quote).await })
        };
        let sources = PriceSyncSources {
            daily: &fetch_daily,
            reference: &fetch_reference,
            fx_series: &fetch_fx_series,
        };
        let mut summary = sync_prices_range(
            &self.database,
            &symbol,
            from_time,
            to_time,
            now,
            plan,
            &sources,
        )
        .await?;
        if let Some(warning) = warning {
            summary.warnings.insert(0, warning);
        }
        Ok(summary)
    }
    /// Read the BTC/USD availability from the Coin Metrics catalogue.
    ///
    /// A failure is not fatal: the documented coverage is assumed and the
    /// reason is returned so the run can report it as a warning.
    async fn btc_usd_coverage(&self, now: i64) -> (BtcUsdCoverage, Option<String>) {
        let query = CatalogAssetMetricsQuery {
            assets: vec!["btc".to_string()],
            metrics: vec!["PriceUSD".to_string(), "ReferenceRateUSD".to_string()],
            ..Default::default()
        };
        let response = match self
            .coinmetrics
            .catalog
            .get_asset_metrics(CatalogScope::V2, query)
            .await
        {
            Ok(response) => response,
            Err(error) => {
                return (
                    BtcUsdCoverage::fallback(now),
                    Some(
                        format!(
                            "Could not read the Coin Metrics catalogue ({error}); assuming the documented coverage.",
                        ),
                    ),
                );
            }
        };
        let mut coverage = BtcUsdCoverage::default();
        for asset in response.data {
            for metric in asset.metrics {
                match metric.metric.as_str() {
                    "PriceUSD" => {
                        if let Some(start) = metric
                            .frequencies
                            .iter()
                            .find(|frequency| frequency.frequency == "1d")
                            .and_then(|frequency| frequency.min_time.as_deref())
                            .and_then(civil_time::parse_iso_timestamp)
                        {
                            coverage.daily_start = Some(start);
                        }
                    }
                    "ReferenceRateUSD" => {
                        for frequency in ["1m", "1h"] {
                            let Some(entry) = metric
                                .frequencies
                                .iter()
                                .find(|entry| entry.frequency == frequency)
                            else {
                                continue;
                            };
                            let min_time = entry
                                .min_time
                                .as_deref()
                                .and_then(civil_time::parse_iso_timestamp);
                            let max_time = entry
                                .max_time
                                .as_deref()
                                .and_then(civil_time::parse_iso_timestamp);
                            if let (Some(min_time), Some(max_time)) = (min_time, max_time) {
                                coverage.reference.push(ReferenceCoverage {
                                    frequency: frequency.to_string(),
                                    min_time,
                                    max_time,
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        (coverage, None)
    }
    /// Prepare the stored USD history in the currently selected currency;
    /// see [`ensure_currency_history`]. The automatic preparation behind the
    /// default fiat currency setting reuses the same conversion path as
    /// [`sync_prices_bulk`].
    pub async fn ensure_currency_history(
        &self,
        symbol: &str,
    ) -> Result<CurrencyHistorySummary, PriceServiceError> {
        let symbol = normalize_symbol(symbol)?;
        let quote = symbol.clone();
        let fetch_fx_series = |from_date: String, to_date: String| -> FetchSeriesFuture {
            let rates = self.frankfurter.rates.clone();
            let quote = quote.clone();
            Box::pin(async move { fetch_usd_fx_series(&rates, &from_date, &to_date, &quote).await })
        };
        ensure_currency_history(&self.database, &symbol, &fetch_fx_series).await
    }
    /// List the currencies the Frankfurter catalogue offers for conversions,
    /// sorted by code. This is the catalogue the "add conversion" action of
    /// the historical prices table chooses from.
    pub async fn conversion_currencies(&self) -> Result<Vec<FiatCurrency>, PriceServiceError> {
        let mut currencies = self
            .frankfurter
            .currencies
            .get_currencies(None, &[])
            .await?
            .into_iter()
            .map(|currency| FiatCurrency {
                code: currency.iso_code,
                name: currency.name,
            })
            .collect::<Vec<_>>();
        currencies.sort_by(|left, right| left.code.cmp(&right.code));
        Ok(currencies)
    }
}
/// Fetch the BTC `PriceUSD` metric row for one calendar day from Coin
/// Metrics.
async fn fetch_btc_usd_close(
    timeseries: &Timeseries,
    date: &str,
) -> Result<f64, PriceServiceError> {
    let query = AssetMetricsQuery {
        assets: vec!["btc".to_string()],
        metrics: vec!["PriceUSD".to_string()],
        frequency: Some("1d".to_string()),
        time: TimeRange {
            start_time: Some(date.to_string()),
            end_time: Some(date.to_string()),
            ..Default::default()
        },
        page: Page {
            page_size: Some(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let response = timeseries.get_asset_metrics(query).await?;
    response
        .data
        .first()
        .and_then(|row| row.values.get("PriceUSD"))
        .and_then(json_number_f64)
        .ok_or_else(|| {
            PriceServiceError::Price(PriceError::NotFound {
                symbol: USD_SYMBOL.to_string(),
                date: date.to_string(),
            })
        })
}
/// Largest page Coin Metrics serves for one asset-metrics request.
const DAILY_PAGE_SIZE: u32 = 10_000;
/// Fetch the BTC/USD daily closes of `[from_time, to_time]` from Coin
/// Metrics.
///
/// The API returns the rows of one page at a time; the loop follows
/// `next_page_token` so a range longer than one page is fetched completely.
/// Rows are keyed by their interval start and mapped to the close time stored
/// in the table. Rows without a `PriceUSD` value are skipped: the API reports
/// gaps that way.
async fn fetch_btc_usd_daily(
    timeseries: &Timeseries,
    from_time: i64,
    to_time: i64,
) -> Result<Vec<Price>, PriceServiceError> {
    let start_date = civil_time::format_date(from_time);
    let end_date = civil_time::format_date(to_time);
    let mut prices = Vec::new();
    let mut next_page_token = None;
    loop {
        let query = AssetMetricsQuery {
            assets: vec!["btc".to_string()],
            metrics: vec!["PriceUSD".to_string()],
            frequency: Some("1d".to_string()),
            time: TimeRange {
                start_time: Some(start_date.clone()),
                end_time: Some(end_date.clone()),
                ..Default::default()
            },
            page: Page {
                page_size: Some(DAILY_PAGE_SIZE),
                next_page_token: next_page_token.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let response = timeseries.get_asset_metrics(query).await?;
        for row in &response.data {
            let Some(close) = row.values.get("PriceUSD").and_then(json_number_f64) else {
                continue;
            };
            if let Some(price) = daily_price(&row.time, close) {
                prices.push(price);
            }
        }
        match response.next_page_token {
            Some(token) if !token.is_empty() && Some(&token) != next_page_token.as_ref() => {
                next_page_token = Some(token);
            }
            _ => break,
        }
    }
    Ok(prices)
}
/// Fetch `ReferenceRateUSD` observations of `[from_time, to_time]` at
/// `frequency` (`1m` or `1h`) from Coin Metrics, as `(interval start, value)`
/// pairs.
///
/// The Community API serves finer frequencies only for a short rolling window;
/// the caller intersects the request with the window the catalogue reports.
/// Rows without a value are skipped, and the loop follows `next_page_token` so
/// a full window is fetched completely.
async fn fetch_btc_usd_reference(
    timeseries: &Timeseries,
    frequency: &str,
    from_time: i64,
    to_time: i64,
) -> Result<Vec<(i64, f64)>, PriceServiceError> {
    let start_time = civil_time::format_iso_timestamp(from_time);
    let end_time = civil_time::format_iso_timestamp(to_time);
    let mut observations = Vec::new();
    let mut next_page_token = None;
    loop {
        let query = AssetMetricsQuery {
            assets: vec!["btc".to_string()],
            metrics: vec!["ReferenceRateUSD".to_string()],
            frequency: Some(frequency.to_string()),
            time: TimeRange {
                start_time: Some(start_time.clone()),
                end_time: Some(end_time.clone()),
                ..Default::default()
            },
            page: Page {
                page_size: Some(DAILY_PAGE_SIZE),
                next_page_token: next_page_token.clone(),
                ..Default::default()
            },
            ..Default::default()
        };
        let response = timeseries.get_asset_metrics(query).await?;
        for row in &response.data {
            let Some(close) = row.values.get("ReferenceRateUSD").and_then(json_number_f64) else {
                continue;
            };
            if let Some(start) = civil_time::parse_iso_timestamp(&row.time) {
                observations.push((start, close));
            }
        }
        match response.next_page_token {
            Some(token) if !token.is_empty() && Some(&token) != next_page_token.as_ref() => {
                next_page_token = Some(token);
            }
            _ => break,
        }
    }
    Ok(observations)
}
/// Map one Coin Metrics asset-metric row to the stored daily close.
///
/// The API row time is the start of the interval; the table stores the close
/// time of the day. Rows whose timestamp cannot be parsed are skipped by the
/// caller.
fn daily_price(interval_start: &str, close: f64) -> Option<Price> {
    let interval_start = civil_time::parse_iso_timestamp(interval_start)?;
    Some(Price {
        symbol: USD_SYMBOL.to_string(),
        close_time: daily_close_time(interval_start),
        close,
    })
}
/// Fetch the `USD → quote` blended rate for one calendar day from
/// Frankfurter.
async fn fetch_usd_fx_rate(
    rates: &Rates,
    date: &str,
    quote: &str,
) -> Result<f64, PriceServiceError> {
    let rate = rates.get_rate(USD_SYMBOL, quote, Some(date), &[]).await?;
    Ok(rate.rate)
}
/// Start date of an FX series request, padded so a range beginning on a
/// weekend or holiday can use the preceding business day's rate.
fn fx_series_start_date(from_date: &str) -> String {
    civil_time::parse_date(from_date)
        .map(|day| civil_time::format_date(day - FX_RATE_LOOKBACK_DAYS * 86_400))
        .unwrap_or_else(|| from_date.to_string())
}
/// Fetch the `USD → quote` blended rate series of `[from_date, to_date]`
/// (ISO `YYYY-MM-DD`) from Frankfurter, as `(date, rate)` pairs.
async fn fetch_usd_fx_series(
    rates: &Rates,
    from_date: &str,
    to_date: &str,
    quote: &str,
) -> Result<Vec<(String, f64)>, PriceServiceError> {
    let from_date = fx_series_start_date(from_date);
    let query = RatesQuery {
        from: Some(from_date),
        to: Some(to_date.to_string()),
        base: Some(USD_SYMBOL.to_string()),
        quotes: vec![quote.to_string()],
        ..Default::default()
    };
    let series = rates.get_rates(query).await?;
    Ok(series
        .into_iter()
        .map(|rate| (rate.date, rate.rate))
        .collect())
}
/// Parse a Coin Metrics metric value, returned as a numeric string.
fn json_number_f64(value: &serde_json::Value) -> Option<f64> {
    value
        .as_str()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .or_else(|| value.as_f64())
        .or_else(|| value.as_i64().map(|number| number as f64))
}
mod conversion;
mod listing;
mod resolution;
mod sync;
#[cfg(test)]
mod tests;
pub use conversion::{ensure_currency_history, sync_prices_bulk};
pub use listing::{
    delete_stored_prices, list_price_conversions, list_price_page, price_series,
    update_stored_price,
};
#[cfg(test)]
pub use listing::{MAX_PAGE_SIZE, SERIES_MAX_POINTS};
pub use resolution::{latest_price_at, resolve_price};
#[cfg(test)]
pub use sync::PriceSyncPlan;
pub use sync::{
    plan_price_sync, sync_prices_range, BtcUsdCoverage, PriceSyncSources, ReferenceCoverage,
};
