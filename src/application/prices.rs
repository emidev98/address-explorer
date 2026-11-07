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

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

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
    BulkSyncSummary, CurrencyHistorySummary, FiatCurrency, Price, PriceConversion, PriceError,
    PricePage, PriceSeries, PriceSyncSummary, SyncGranularity, CSV_COVERAGE_END,
    PRICE_HISTORY_START, USD_SYMBOL,
};
use crate::domain::table::TableQuery;

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
type FetchFuture = Pin<Box<dyn Future<Output = Result<f64, PriceServiceError>> + Send>>;

/// Daily BTC/USD close unavailable in the table: asks `fetch_usd` exactly
/// once for the calendar day of the request and remembers the answer.
async fn usd_close_at(
    database: &Database,
    day_close_time: i64,
    date: &str,
    fetch_usd: &(dyn Fn(String) -> FetchFuture + Send + Sync),
) -> Result<f64, PriceServiceError> {
    if let Some(price) = price_repository::find_price(database, USD_SYMBOL, day_close_time)? {
        return Ok(price.close);
    }
    let close = fetch_usd(date.to_string()).await?;
    validate_price_value(close, USD_SYMBOL, "Coin Metrics PriceUSD")?;
    price_repository::store_price(
        database,
        &Price {
            symbol: USD_SYMBOL.to_string(),
            close_time: day_close_time,
            close,
        },
    )?;
    Ok(close)
}

/// Resolve the BTC price in `symbol` at `timestamp` (UTC seconds).
///
/// Resolution order for the bucketed close time:
///
/// 1. after the imported 15-minute coverage, the freshest stored intraday
///    close within the last hour (the 15-minute/hourly observations written by
///    the unified sync), when one exists;
/// 2. exact row in `historical_prices` — including the imported 15-minute
///    `USD` history;
/// 3. the daily row covering the request (used to answer candles missing from
///    the historical export, e.g. maintenance gaps, without a network call);
/// 4. one API round trip per missing day: Coin Metrics for the BTC/USD close
///    and, for any other symbol, a Frankfurter `USD → symbol` rate applied to
///    the BTC price; every resolved value is stored before returning.
///
/// The storing step materializes an entry at the requested bucket as well, so
/// repeated lookups of the same candle never leave the database.
pub async fn resolve_price(
    database: &Database,
    symbol: &str,
    timestamp: i64,
    fetch_usd: &(dyn Fn(String) -> FetchFuture + Send + Sync),
    fetch_fx: &(dyn Fn(String, String) -> FetchFuture + Send + Sync),
) -> Result<Price, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    if timestamp <= 0 {
        return Err(PriceError::InvalidTimestamp(timestamp).into());
    }

    let bucket = if timestamp <= CSV_COVERAGE_END {
        fifteen_minute_close_time(timestamp)
    } else {
        daily_close_time(timestamp)
    };

    if timestamp > CSV_COVERAGE_END {
        if let Some(price) = price_repository::find_latest_price_between(
            database,
            &symbol,
            timestamp - GRANULAR_RESOLUTION_LOOKBACK,
            timestamp,
        )? {
            // Only a sub-daily observation outranks the daily bucket; a daily
            // close from just before midnight must not replace the close of
            // the requested day.
            if price.close_time != daily_close_time(price.close_time) {
                return Ok(price);
            }
        }
    }

    if let Some(price) = price_repository::find_price(database, &symbol, bucket)? {
        return Ok(price);
    }

    let day_close_time = daily_close_time(timestamp);
    let date = civil_time::format_date(timestamp);

    if bucket != day_close_time {
        if let Some(price) = price_repository::find_price(database, &symbol, day_close_time)? {
            let price = Price {
                close_time: bucket,
                ..price
            };
            price_repository::store_price(database, &price)?;
            return Ok(price);
        }
    }

    let usd_close = usd_close_at(database, day_close_time, &date, fetch_usd).await?;
    let close = if symbol == USD_SYMBOL {
        usd_close
    } else {
        let rate = fetch_fx(date.clone(), symbol.clone()).await?;
        validate_price_value(rate, &symbol, "Frankfurter USD rate")?;
        usd_close * rate
    };

    price_repository::store_price(
        database,
        &Price {
            symbol: symbol.clone(),
            close_time: day_close_time,
            close,
        },
    )?;
    let price = Price {
        symbol,
        close_time: bucket,
        close,
    };
    price_repository::store_price(database, &price)?;
    Ok(price)
}

/// Convert two BTC price quotes anchored to the same moment: the rate always
/// derives from BTC prices, never from a bare fiat pair, which keeps every
/// historical conversion related to the price of BTC.
pub async fn resolve_conversion(
    database: &Database,
    from_symbol: &str,
    to_symbol: &str,
    timestamp: i64,
    fetch_usd: &(dyn Fn(String) -> FetchFuture + Send + Sync),
    fetch_fx: &(dyn Fn(String, String) -> FetchFuture + Send + Sync),
) -> Result<PriceConversion, PriceServiceError> {
    let from = resolve_price(database, from_symbol, timestamp, fetch_usd, fetch_fx).await?;
    let to = resolve_price(database, to_symbol, timestamp, fetch_usd, fetch_fx).await?;
    Ok(PriceConversion {
        from_symbol: from.symbol,
        to_symbol: to.symbol,
        close_time: from.close_time,
        rate: from.close / to.close,
    })
}

/// Largest page the listing endpoint serves; bigger requests are clamped
/// instead of rejected so a misbehaving client cannot force a full-table
/// scan into one response.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Read one page of the stored history of `symbol` matching `query` — the
/// shared table contract of pagination and sorting — bounded to
/// `[from_time, to_time]` (a bound of `0` or less leaves that side open).
///
/// This is a pure read of `historical_prices`: listing never resolves prices
/// over the network, it only reports what the table already holds. Sorting
/// runs in the database; the sort key is validated against the repository's
/// allowlist before any SQL is built.
pub fn list_price_page(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
    query: &TableQuery,
) -> Result<PricePage, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    let page_size = query.page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = u64::from(query.page) * u64::from(page_size);
    let sort = match query.sort_by.as_deref() {
        None => None,
        Some(key) => Some((
            PriceSortColumn::from_key(key)
                .ok_or_else(|| PriceError::InvalidSortColumn(key.to_string()))?,
            query.sort_dir,
        )),
    };

    let items = price_repository::list_prices(
        database,
        &price_repository::PriceListQuery {
            symbol: &symbol,
            limit: page_size,
            offset,
            from_time,
            to_time,
            sort,
        },
    )?;
    let total = price_repository::count_prices(database, &symbol, from_time, to_time)?;
    Ok(PricePage { items, total })
}

/// Owned future producing the FX series of a bulk sync: `(date, rate)`
/// pairs covering the requested range.
type FetchSeriesFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(String, f64)>, PriceServiceError>> + Send>>;

/// Owned future producing daily BTC/USD closes for a range.
type FetchDailyFuture = Pin<Box<dyn Future<Output = Result<Vec<Price>, PriceServiceError>> + Send>>;

/// Owned future producing reference-rate observations as `(interval start,
/// value)` pairs for a frequency and range.
type FetchReferenceFuture =
    Pin<Box<dyn Future<Output = Result<Vec<(i64, f64)>, PriceServiceError>> + Send>>;

/// Convert the stored USD history of `[from_time, to_time]` into `symbol`
/// and persist it, in one transaction.
///
/// The sync matches the granularity of the table: it only ever populates
/// close times that already exist as stored USD rows — the Frankfurter
/// series is fetched once for the whole range and applied locally, never
/// used to invent timestamps. Each USD row takes the latest rate published
/// on or before its calendar day (weekends and holidays use the previous
/// business day's rate); days before the first available rate are skipped.
/// Existing `(symbol, close_time)` rows are left untouched, so re-running a
/// sync only fills gaps.
pub async fn sync_prices_bulk(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
    fetch_fx_series: &(dyn Fn(String, String) -> FetchSeriesFuture + Send + Sync),
) -> Result<BulkSyncSummary, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    if symbol == USD_SYMBOL {
        return Err(PriceError::InvalidData(
            "bulk sync converts the stored USD history into another currency; \
             choose a target other than USD"
                .to_string(),
        )
        .into());
    }
    if from_time <= 0 {
        return Err(PriceError::InvalidTimestamp(from_time).into());
    }
    if to_time <= 0 {
        return Err(PriceError::InvalidTimestamp(to_time).into());
    }
    if from_time > to_time {
        return Err(PriceError::InvalidData(format!(
            "the start of the sync range ({}) must not be after its end ({})",
            civil_time::format_date(from_time),
            civil_time::format_date(to_time)
        ))
        .into());
    }

    let source = price_repository::list_prices_in_range(database, USD_SYMBOL, from_time, to_time)?;
    let source_rows = source.len() as u64;
    let mut summary = BulkSyncSummary {
        symbol: symbol.clone(),
        from_time,
        to_time,
        source_rows,
        stored: 0,
        skipped: 0,
    };
    if source.is_empty() {
        return Ok(summary);
    }

    // One network round trip for the whole range; the series comes back as
    // ISO `YYYY-MM-DD` dates, which sort lexicographically.
    let mut rates = fetch_fx_series(
        civil_time::format_date(from_time),
        civil_time::format_date(to_time),
    )
    .await?;
    rates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut prices = Vec::with_capacity(source.len());
    for row in &source {
        let day = civil_time::format_date(row.close_time);
        let index = rates.partition_point(|(date, _)| date.as_str() <= day.as_str());
        let Some(rate) = index.checked_sub(1).map(|last| rates[last].1) else {
            summary.skipped += 1;
            continue;
        };
        let close = validate_price_value(row.close * rate, &symbol, "Frankfurter USD rate")?;
        prices.push(Price {
            symbol: symbol.clone(),
            close_time: row.close_time,
            close,
        });
    }

    summary.stored = price_repository::store_prices_bulk(database, &prices)?;
    Ok(summary)
}

/// Ensure the stored USD price history also exists in `symbol`.
///
/// This is the automatic preparation behind the default fiat currency
/// setting. It is idempotent: a currency that already covers every stored USD
/// close time returns immediately without a network call, and existing target
/// rows — including manually edited prices — are never overwritten.
///
/// Network and rate-source failures are reported through the summary warning
/// instead of failing the caller: the preference is already saved and the
/// operation can be retried later.
pub async fn ensure_currency_history(
    database: &Database,
    symbol: &str,
    fetch_fx_series: &(dyn Fn(String, String) -> FetchSeriesFuture + Send + Sync),
) -> Result<CurrencyHistorySummary, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    let mut summary = CurrencyHistorySummary {
        symbol: symbol.clone(),
        source_rows: 0,
        stored: 0,
        skipped: 0,
        warning: None,
    };
    if symbol == USD_SYMBOL {
        return Ok(summary);
    }

    let Some((source_first, source_last)) =
        price_repository::price_series_range(database, USD_SYMBOL)?
    else {
        summary.warning = Some("There is no stored USD price history to convert.".to_string());
        return Ok(summary);
    };
    let Some((missing_first, missing_last)) = price_repository::missing_price_range(
        database,
        USD_SYMBOL,
        &symbol,
        source_first,
        source_last,
    )?
    else {
        return Ok(summary);
    };

    match sync_prices_bulk(
        database,
        &symbol,
        missing_first,
        missing_last,
        fetch_fx_series,
    )
    .await
    {
        Ok(bulk) => {
            summary.source_rows = bulk.source_rows;
            summary.stored = bulk.stored;
            summary.skipped = bulk.skipped;
            if bulk.skipped > 0 {
                summary.warning = Some(format!(
                    "{} source prices had no FX rate for their day and were skipped.",
                    bulk.skipped
                ));
            }
        }
        Err(error @ PriceServiceError::Database(_)) => return Err(error),
        Err(error) => {
            summary.warning = Some(format!("Could not convert historical prices: {error}"));
        }
    }

    Ok(summary)
}

/// Availability of the BTC/USD series reported by the Coin Metrics catalogue.
#[derive(Debug, Clone, Default)]
pub struct BtcUsdCoverage {
    /// First day the `PriceUSD` metric serves; `None` when the catalogue did
    /// not say, in which case [`PRICE_HISTORY_START`] is assumed.
    pub daily_start: Option<i64>,
    /// Finer reference-rate frequencies the source serves.
    pub reference: Vec<ReferenceCoverage>,
}

/// Time span of one finer reference-rate frequency.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCoverage {
    /// Coin Metrics frequency, `1m` or `1h`.
    pub frequency: String,
    pub min_time: i64,
    pub max_time: i64,
}

impl BtcUsdCoverage {
    /// Assumed coverage when the catalogue cannot be reached: the documented
    /// first daily day and the last seven days at both finer frequencies.
    pub fn fallback(now: i64) -> Self {
        let start = now - 7 * DAY_SECONDS;
        Self {
            daily_start: Some(PRICE_HISTORY_START),
            reference: vec![
                ReferenceCoverage {
                    frequency: "1m".to_string(),
                    min_time: start,
                    max_time: now,
                },
                ReferenceCoverage {
                    frequency: "1h".to_string(),
                    min_time: start,
                    max_time: now,
                },
            ],
        }
    }

    /// Intersection of `[from_time, to_time]` with the coverage of the
    /// frequency behind `granularity`, when one exists.
    fn reference_window(
        &self,
        granularity: SyncGranularity,
        from_time: i64,
        to_time: i64,
    ) -> Option<(i64, i64)> {
        let frequency = granularity.source_frequency()?;
        let coverage = self
            .reference
            .iter()
            .find(|entry| entry.frequency == frequency)?;
        let start = from_time.max(coverage.min_time);
        let end = to_time.min(coverage.max_time);
        (start <= end).then_some((start, end))
    }
}

/// Intraday stage resolved for one run of the unified sync.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceSyncPlan {
    /// Resolution actually applied; `Daily` skips the intraday stage.
    pub granularity: SyncGranularity,
    /// First day the daily source serves.
    pub first_day: i64,
    /// Inclusive intersection of the range with the finer source coverage.
    pub granular_from: Option<i64>,
    pub granular_to: Option<i64>,
}

impl PriceSyncPlan {
    fn daily(first_day: i64) -> Self {
        Self {
            granularity: SyncGranularity::Daily,
            first_day,
            granular_from: None,
            granular_to: None,
        }
    }

    fn granular_window(self) -> Option<(i64, i64)> {
        self.granular_from.zip(self.granular_to)
    }
}

/// Resolve the requested granularity against the source coverage.
///
/// `Auto` walks the options from the finest to the coarsest and keeps the
/// first one whose window intersects the range; when no finer window does, it
/// degrades to daily closes only. A manual intraday choice without coverage
/// keeps its label so the run can report why it was skipped.
pub fn plan_price_sync(
    requested: SyncGranularity,
    coverage: &BtcUsdCoverage,
    from_time: i64,
    to_time: i64,
    now: i64,
) -> PriceSyncPlan {
    let first_day = coverage.daily_start.unwrap_or(PRICE_HISTORY_START);
    let from = from_time.max(first_day);
    let to = to_time.min(now);
    if from > to || requested == SyncGranularity::Daily {
        return PriceSyncPlan::daily(first_day);
    }

    let candidates: Vec<SyncGranularity> = match requested {
        SyncGranularity::Auto => {
            vec![SyncGranularity::FifteenMinutes, SyncGranularity::Hourly]
        }
        other => vec![other],
    };
    for candidate in candidates {
        if let Some((start, end)) = coverage.reference_window(candidate, from, to) {
            return PriceSyncPlan {
                granularity: candidate,
                first_day,
                granular_from: Some(start),
                granular_to: Some(end),
            };
        }
    }

    if requested.is_intraday() {
        PriceSyncPlan {
            granularity: requested,
            first_day,
            granular_from: None,
            granular_to: None,
        }
    } else {
        PriceSyncPlan::daily(first_day)
    }
}

/// Sources the unified sync calls. Closures keep the network out of the
/// orchestration so the workflow is testable without HTTP.
pub struct PriceSyncSources<'a> {
    pub daily: &'a (dyn Fn(i64, i64) -> FetchDailyFuture + Send + Sync),
    pub reference: &'a (dyn Fn(&str, i64, i64) -> FetchReferenceFuture + Send + Sync),
    pub fx_series: &'a (dyn Fn(String, String) -> FetchSeriesFuture + Send + Sync),
}

/// Synchronize the stored BTC price history of `symbol` for `[from_time,
/// to_time]`.
///
/// Every run first fills the BTC/USD daily closes of the range from Coin
/// Metrics `PriceUSD`, so the reference series stays complete even on an empty
/// database. When the resolved plan carries an intraday window, the matching
/// `ReferenceRateUSD` observations are stored as well; they are keyed at their
/// source timestamps so their values are never shifted and cannot collide with
/// the daily `23:59:59` close. A non-USD `symbol` then converts every stored
/// USD close time of the range through [`sync_prices_bulk`], reusing the same
/// Frankfurter series for the whole span.
///
/// Existing rows, including manual edits, are never overwritten. The daily
/// stage is mandatory; a failing intraday or conversion stage is reported as a
/// warning while the rest of the run stays stored.
pub async fn sync_prices_range(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
    now: i64,
    plan: PriceSyncPlan,
    sources: &PriceSyncSources<'_>,
) -> Result<PriceSyncSummary, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    if now <= 0 {
        return Err(PriceError::InvalidTimestamp(now).into());
    }
    if from_time <= 0 {
        return Err(PriceError::InvalidTimestamp(from_time).into());
    }
    if to_time <= 0 {
        return Err(PriceError::InvalidTimestamp(to_time).into());
    }
    if from_time > to_time {
        return Err(PriceError::InvalidData(format!(
            "the start of the sync range ({}) must not be after its end ({})",
            civil_time::format_date(from_time),
            civil_time::format_date(to_time)
        ))
        .into());
    }

    let range_start = from_time.max(plan.first_day);
    let range_end = to_time.min(now);
    if range_start > range_end {
        return Err(PriceError::InvalidData(format!(
            "the sync range ends before the first day the source serves ({})",
            civil_time::format_date(plan.first_day)
        ))
        .into());
    }

    let mut summary = PriceSyncSummary {
        symbol: symbol.clone(),
        from_time: range_start,
        to_time: range_end,
        granularity: plan.granularity,
        granular_from_time: None,
        granular_to_time: None,
        daily_fetched: 0,
        daily_stored: 0,
        granular_fetched: 0,
        granular_stored: 0,
        unavailable_days: 0,
        conversion: None,
        warnings: Vec::new(),
    };

    // Daily stage: the reference series and the floor every other value
    // derives from.
    let mut daily = (sources.daily)(range_start, range_end).await?;
    daily.retain(|price| {
        price.close_time >= daily_close_time(range_start)
            && price.close_time <= daily_close_time(range_end)
    });
    for price in &mut daily {
        price.symbol = USD_SYMBOL.to_string();
        price.close = validate_price_value(price.close, USD_SYMBOL, "Coin Metrics PriceUSD 1d")?;
    }
    daily.sort_by_key(|price| price.close_time);
    daily.dedup_by_key(|price| price.close_time);
    summary.daily_fetched = daily.len() as u64;
    summary.daily_stored = price_repository::store_prices_bulk(database, &daily)?;

    // The in-progress day has no published daily close yet, so it is not
    // counted as unavailable.
    let complete_through = range_end.min(day_start(now) - 1);
    let fetched_complete = daily
        .iter()
        .filter(|price| price.close_time <= complete_through)
        .count() as u64;
    summary.unavailable_days =
        count_daily_closes(range_start, complete_through).saturating_sub(fetched_complete);

    // Intraday stage: only the newest slice of the range has a finer source.
    if let Some((granular_from, granular_to)) = plan.granular_window() {
        let frequency = plan
            .granularity
            .source_frequency()
            .expect("an intraday plan carries a source frequency");
        match (sources.reference)(frequency, granular_from, granular_to).await {
            Ok(rows) => {
                let mut granular = reference_prices(plan.granularity, rows);
                granular.retain(|price| {
                    price.close_time >= granular_from && price.close_time <= granular_to
                });
                for price in &mut granular {
                    price.close = validate_price_value(
                        price.close,
                        USD_SYMBOL,
                        "Coin Metrics ReferenceRateUSD",
                    )?;
                }
                granular.sort_by_key(|price| price.close_time);
                granular.dedup_by_key(|price| price.close_time);
                summary.granular_fetched = granular.len() as u64;
                summary.granular_stored = price_repository::store_prices_bulk(database, &granular)?;
                summary.granular_from_time = Some(granular_from);
                summary.granular_to_time = Some(granular_to);
            }
            Err(error @ PriceServiceError::Database(_)) => return Err(error),
            Err(error) => summary.warnings.push(format!(
                "Could not fetch {} closes: {error}",
                plan.granularity.resolution_label()
            )),
        }
    } else if plan.granularity.is_intraday() {
        summary.warnings.push(format!(
            "No {} closes are available for the selected range; only daily closes were synced.",
            plan.granularity.resolution_label()
        ));
    }

    // Conversion stage: every stored USD close time of the range, including
    // the rows the stages above just inserted.
    if symbol != USD_SYMBOL {
        match sync_prices_bulk(database, &symbol, range_start, range_end, sources.fx_series).await {
            Ok(bulk) => {
                if bulk.skipped > 0 {
                    summary.warnings.push(format!(
                        "{} source prices had no FX rate for their day and were skipped.",
                        bulk.skipped
                    ));
                }
                summary.conversion = Some(bulk);
            }
            Err(error @ PriceServiceError::Database(_)) => return Err(error),
            Err(error) => summary.warnings.push(format!(
                "Could not convert the stored prices into {symbol}: {error}"
            )),
        }
    }

    Ok(summary)
}

/// Map reference-rate rows to stored USD observations.
///
/// The source timestamps each observation at the start of its interval; the
/// rows are kept at that timestamp so the value is not shifted. The minute
/// feed is sampled every 15 minutes, which is the finest resolution the source
/// offers and matches the imported history's spacing.
fn reference_prices(granularity: SyncGranularity, rows: Vec<(i64, f64)>) -> Vec<Price> {
    let unit = match granularity {
        SyncGranularity::FifteenMinutes => FIFTEEN_MINUTES,
        SyncGranularity::Hourly => 3_600,
        _ => return Vec::new(),
    };
    rows.into_iter()
        .filter(|(start, _)| start.rem_euclid(unit) == 0)
        .map(|(start, close)| Price {
            symbol: USD_SYMBOL.to_string(),
            close_time: start,
            close,
        })
        .collect()
}

/// Start of the UTC day containing `timestamp`.
fn day_start(timestamp: i64) -> i64 {
    timestamp.div_euclid(DAY_SECONDS) * DAY_SECONDS
}

/// Number of daily close times (`23:59:59` UTC) within `[from_time,
/// through]`, counting the close of the day containing `from_time`.
fn count_daily_closes(from_time: i64, through: i64) -> u64 {
    if through < from_time {
        return 0;
    }
    let first = daily_close_time(from_time);
    if first > through {
        return 0;
    }
    ((through - first) / DAY_SECONDS + 1) as u64
}

/// Number of time buckets a series request is downsampled into at most, so a
/// chart never receives more points than it can meaningfully draw.
pub const SERIES_MAX_POINTS: i64 = 1000;

/// Read the stored price series of `symbol` for the trend chart: the full
/// span of the history plus the closes of `[from_time, to_time]` (bounds of
/// `0` or less fall back to the full span), downsampled to at most
/// [`SERIES_MAX_POINTS`] bucketed averages.
///
/// A narrower window yields proportionally finer buckets, so re-requesting
/// after a zoom narrows down the data instead of loading the raw history.
/// Returns `None` when the symbol has no stored rows. Pure read, like
/// [`list_price_page`].
pub fn price_series(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
) -> Result<Option<PriceSeries>, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    let Some((first, last)) = price_repository::price_series_range(database, &symbol)? else {
        return Ok(None);
    };

    let from = if from_time > 0 {
        from_time.max(first)
    } else {
        first
    };
    let to = if to_time > 0 { to_time.min(last) } else { last };

    let points = if from <= to {
        let span = to - from + 1;
        let bucket_width = (span + SERIES_MAX_POINTS - 1) / SERIES_MAX_POINTS;
        price_repository::price_series(database, &symbol, from, to, bucket_width)?
    } else {
        Vec::new()
    };

    Ok(Some(PriceSeries {
        first_close_time: first,
        last_close_time: last,
        points,
    }))
}

/// Update the close of a stored price after validating the input like any
/// external value. A missing `(symbol, close_time)` row is a not-found
/// failure, never an insert: editing must not create history.
pub fn update_stored_price(
    database: &Database,
    symbol: &str,
    close_time: i64,
    close: f64,
) -> Result<Price, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    if close_time <= 0 {
        return Err(PriceError::InvalidTimestamp(close_time).into());
    }
    let close = validate_price_value(close, &symbol, "manual price edit")?;

    let price = Price {
        symbol,
        close_time,
        close,
    };
    if !price_repository::update_price(database, &price)? {
        return Err(PriceError::NotFound {
            symbol: price.symbol.clone(),
            date: civil_time::format_datetime(close_time),
        }
        .into());
    }
    Ok(price)
}

/// Delete every stored price at the given close times in one batch.
///
/// A close time identifies a whole price snapshot: every symbol stored at that
/// moment is removed, including the conversions other currencies expose in the
/// table's tooltip. Other close times are left untouched. The selection is
/// validated and deduplicated before the repository runs its single
/// transaction.
pub fn delete_stored_prices(
    database: &Database,
    close_times: &[i64],
) -> Result<usize, PriceServiceError> {
    let mut close_times = close_times.to_vec();
    close_times.sort_unstable();
    close_times.dedup();
    if close_times.is_empty() {
        return Err(PriceServiceError::Validation(
            "Select at least one price to delete.".to_string(),
        ));
    }
    if let Some(invalid) = close_times.iter().find(|&&close_time| close_time <= 0) {
        return Err(PriceError::InvalidTimestamp(*invalid).into());
    }
    Ok(price_repository::delete_prices(database, &close_times)?)
}

/// Read the stored prices of the other symbols at the same close time: the
/// conversions displayed when hovering a row of the historical prices table.
/// Like [`list_price_page`], this is a pure read of the table.
pub fn list_price_conversions(
    database: &Database,
    symbol: &str,
    close_time: i64,
) -> Result<Vec<Price>, PriceServiceError> {
    let symbol = normalize_symbol(symbol)?;
    if close_time <= 0 {
        return Err(PriceError::InvalidTimestamp(close_time).into());
    }
    Ok(price_repository::list_conversions(
        database, close_time, &symbol,
    )?)
}

/// One day in seconds: the spacing of the daily price series.
const DAY_SECONDS: i64 = 86_400;

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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .map_err(|_| PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
}

/// Resolve the most recent BTC price in `symbol`.
///
/// The daily close of the requested day is published after the day ends, so
/// a request for the current day may legitimately find nothing. Only that
/// "not published yet" case falls back one day; every other failure (invalid
/// input, infrastructure errors) is returned unchanged.
pub async fn latest_price_at(
    database: &Database,
    symbol: &str,
    now: i64,
    fetch_usd: &(dyn Fn(String) -> FetchFuture + Send + Sync),
    fetch_fx: &(dyn Fn(String, String) -> FetchFuture + Send + Sync),
) -> Result<Price, PriceServiceError> {
    match resolve_price(database, symbol, now, fetch_usd, fetch_fx).await {
        Ok(price) => Ok(price),
        Err(PriceServiceError::Price(PriceError::NotFound { .. })) => {
            resolve_price(
                database,
                symbol,
                now - LATEST_PRICE_FALLBACK_SECONDS,
                fetch_usd,
                fetch_fx,
            )
            .await
        }
        Err(error) => Err(error),
    }
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

    /// Convert the BTC price quoted in `from_symbol` into `to_symbol` at the
    /// same moment; see [`resolve_conversion`].
    pub async fn convert_price(
        &self,
        from_symbol: &str,
        to_symbol: &str,
        timestamp: i64,
    ) -> Result<PriceConversion, PriceServiceError> {
        let fetch_usd = |date: String| -> FetchFuture {
            let timeseries = self.coinmetrics.timeseries.clone();
            Box::pin(async move { fetch_btc_usd_close(&timeseries, &date).await })
        };
        let fetch_fx = |date: String, quote: String| -> FetchFuture {
            let rates = self.frankfurter.rates.clone();
            Box::pin(async move { fetch_usd_fx_rate(&rates, &date, &quote).await })
        };
        resolve_conversion(
            &self.database,
            from_symbol,
            to_symbol,
            timestamp,
            &fetch_usd,
            &fetch_fx,
        )
        .await
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
                    Some(format!(
                        "Could not read the Coin Metrics catalogue ({error}); assuming the documented coverage."
                    )),
                )
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
    // A range starting on a weekend has no rate of its own. Fetch a short
    // lookback so the conversion can use the prior business day's rate.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Drive a future to completion without an async runtime; test futures
    /// complete immediately and must never yield pending.
    fn block_on<F: Future>(future: F) -> F::Output {
        use std::task::{Context, RawWaker, RawWakerVTable, Waker};

        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn noop(_: *const ()) {}
            // SAFETY: the vtable never dereferences the null data pointer:
            // cloning manufactures another null waker and the remaining
            // operations are no-ops, so no requirement of `RawWaker` is
            // violated.
            static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
            RawWaker::new(std::ptr::null(), &VTABLE)
        }

        // SAFETY: `raw_waker` builds a `RawWaker` whose vtable satisfies the
        // contract documented above; the data pointer is never dereferenced.
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => output,
            std::task::Poll::Pending => panic!("test future must not pend"),
        }
    }

    /// Fetcher stubs with call counters, in place of HTTP clients.
    struct StubFetchers {
        usd_calls: std::sync::Arc<AtomicU32>,
        fx_calls: std::sync::Arc<AtomicU32>,
        usd: std::sync::Arc<dyn Fn(String) -> FetchFuture + Send + Sync>,
        fx: std::sync::Arc<dyn Fn(String, String) -> FetchFuture + Send + Sync>,
    }

    fn stub_fetchers(usd_price: f64, fx_rate: f64) -> StubFetchers {
        let usd_calls = std::sync::Arc::new(AtomicU32::new(0));
        let fx_calls = std::sync::Arc::new(AtomicU32::new(0));
        let usd = {
            let calls = std::sync::Arc::clone(&usd_calls);
            move |_date: String| -> FetchFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move { Ok(usd_price) })
            }
        };
        let fx = {
            let calls = std::sync::Arc::clone(&fx_calls);
            move |_date: String, _quote: String| -> FetchFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move { Ok(fx_rate) })
            }
        };
        StubFetchers {
            usd_calls,
            fx_calls,
            usd: std::sync::Arc::new(usd),
            fx: std::sync::Arc::new(fx),
        }
    }

    const INSIDE_COVERAGE: i64 = civil_time::epoch_seconds(2020, 1, 1, 12, 34, 0);
    const BEYOND_COVERAGE: i64 = civil_time::epoch_seconds(2026, 7, 1, 10, 0, 0);

    #[test]
    fn stored_price_is_returned_without_network() {
        let database = Database::open_in_memory().unwrap();
        let bucket = fifteen_minute_close_time(INSIDE_COVERAGE);
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: bucket,
                close: 12345.0,
            },
        )
        .unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let price = block_on(resolve_price(
            &database,
            "USD",
            INSIDE_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 12345.0);
        assert_eq!(price.close_time, bucket);
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
        assert_eq!(stubs.fx_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn usd_miss_fetches_once_and_stores_day_and_bucket_rows() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);
        let bucket = fifteen_minute_close_time(INSIDE_COVERAGE);
        let day = daily_close_time(INSIDE_COVERAGE);

        let price = block_on(resolve_price(
            &database,
            "USD",
            INSIDE_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 66_000.0);
        assert_eq!(price.close_time, bucket);
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            price_repository::find_price(&database, "USD", day)
                .unwrap()
                .map(|stored| stored.close),
            Some(66_000.0)
        );
        assert_eq!(
            price_repository::find_price(&database, "USD", bucket)
                .unwrap()
                .map(|stored| stored.close),
            Some(66_000.0)
        );

        // A repeated request of the same candle is served from the database.
        block_on(resolve_price(
            &database,
            "USD",
            INSIDE_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn eur_miss_resolves_usd_first_then_converts_and_stores() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);
        let bucket = fifteen_minute_close_time(INSIDE_COVERAGE);
        let day = daily_close_time(INSIDE_COVERAGE);

        let price = block_on(resolve_price(
            &database,
            "EUR",
            INSIDE_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 66_000.0 * 0.9);
        assert_eq!(price.close_time, bucket);
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
        assert_eq!(stubs.fx_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            price_repository::find_price(&database, "EUR", day)
                .unwrap()
                .map(|stored| stored.close),
            Some(66_000.0 * 0.9)
        );
        assert_eq!(
            price_repository::find_price(&database, "EUR", bucket)
                .unwrap()
                .map(|stored| stored.close),
            Some(66_000.0 * 0.9)
        );

        // The USD reference row for the day was stored as well, so a later
        // same-day lookup hits the database only.
        let second = block_on(resolve_price(
            &database,
            "EUR",
            INSIDE_COVERAGE + 900,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();
        assert_eq!(
            second.close_time,
            fifteen_minute_close_time(INSIDE_COVERAGE + 900)
        );
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
        assert_eq!(stubs.fx_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn beyond_coverage_resolves_at_daily_granularity() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);
        let day = daily_close_time(BEYOND_COVERAGE);

        let price = block_on(resolve_price(
            &database,
            "USD",
            BEYOND_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close_time, day);
        let rows: i64 = database
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn latest_price_uses_the_requested_day_without_fallback() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let price = block_on(latest_price_at(
            &database,
            "USD",
            BEYOND_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close_time, daily_close_time(BEYOND_COVERAGE));
        assert_eq!(price.close, 66_000.0);
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn latest_price_falls_back_one_day_when_today_is_not_published() {
        let database = Database::open_in_memory().unwrap();
        let fx_calls = std::sync::Arc::new(AtomicU32::new(0));
        let today = civil_time::format_date(BEYOND_COVERAGE);

        // The current day's close is not published yet; any earlier day is.
        let usd = move |date: String| -> FetchFuture {
            let not_found = date == today;
            Box::pin(async move {
                if not_found {
                    Err(PriceServiceError::Price(PriceError::NotFound {
                        symbol: USD_SYMBOL.to_string(),
                        date,
                    }))
                } else {
                    Ok(64_000.0)
                }
            })
        };
        let fx = {
            let calls = std::sync::Arc::clone(&fx_calls);
            move |_date: String, _quote: String| -> FetchFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move { Ok(0.9) })
            }
        };

        let price = block_on(latest_price_at(
            &database,
            "USD",
            BEYOND_COVERAGE,
            &usd,
            &fx,
        ))
        .unwrap();

        assert_eq!(
            price.close_time,
            daily_close_time(BEYOND_COVERAGE - LATEST_PRICE_FALLBACK_SECONDS)
        );
        assert_eq!(price.close, 64_000.0);
        assert_eq!(fx_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn latest_price_propagates_non_not_found_errors() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        assert!(matches!(
            block_on(latest_price_at(
                &database,
                "us",
                BEYOND_COVERAGE,
                stubs.usd.as_ref(),
                stubs.fx.as_ref()
            )),
            Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
        ));
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn conversion_is_anchored_to_btc_prices() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let conversion = block_on(resolve_conversion(
            &database,
            "EUR",
            "USD",
            INSIDE_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(conversion.rate, 0.9);
        assert_eq!(
            conversion.close_time,
            fifteen_minute_close_time(INSIDE_COVERAGE)
        );
        assert_eq!(conversion.from_symbol, "EUR");
        assert_eq!(conversion.to_symbol, "USD");
        // One API round trip per missing day total, not per converted symbol.
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 1);
        assert_eq!(stubs.fx_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn invalid_input_is_rejected_before_storage() {
        let database = Database::open_in_memory().unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        assert!(matches!(
            block_on(resolve_price(
                &database,
                "us",
                INSIDE_COVERAGE,
                stubs.usd.as_ref(),
                stubs.fx.as_ref()
            )),
            Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
        ));
        assert!(matches!(
            block_on(resolve_price(
                &database,
                "USD",
                0,
                stubs.usd.as_ref(),
                stubs.fx.as_ref()
            )),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);

        let rows: i64 = database
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM historical_prices", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(rows, 0);
    }

    fn query(page: u32, page_size: u32) -> TableQuery {
        TableQuery {
            page,
            page_size,
            ..Default::default()
        }
    }

    #[test]
    fn list_price_page_normalizes_the_symbol_and_clamps_the_page_size() {
        let database = Database::open_in_memory().unwrap();
        for index in 0..3 {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: "EUR".to_string(),
                    close_time: 100 + index,
                    close: index as f64,
                },
            )
            .unwrap();
        }

        let page =
            list_price_page(&database, " eur ", 0, 0, &query(0, MAX_PAGE_SIZE + 50)).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 3);
        // Newest close first.
        assert_eq!(page.items[0].price.close_time, 102);

        let second = list_price_page(&database, "EUR", 0, 0, &query(1, 2)).unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.items[0].price.close_time, 100);

        // Time bounds narrow both the page and the total.
        let bounded = list_price_page(&database, "EUR", 101, 102, &query(0, 10)).unwrap();
        assert_eq!(bounded.total, 2);

        assert!(matches!(
            list_price_page(&database, "u$", 0, 0, &query(0, 10)),
            Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
        ));
    }

    #[test]
    fn list_price_page_sorts_in_the_backend() {
        let database = Database::open_in_memory().unwrap();
        for (close_time, close) in [(200, 10.0), (100, 2.0), (300, 100.0)] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: "USD".to_string(),
                    close_time,
                    close,
                },
            )
            .unwrap();
        }

        let sorted = list_price_page(
            &database,
            "USD",
            0,
            0,
            &TableQuery {
                sort_by: Some("price".to_string()),
                sort_dir: crate::domain::table::SortDirection::Desc,
                ..query(0, 10)
            },
        )
        .unwrap();
        assert_eq!(
            sorted
                .items
                .iter()
                .map(|price| price.price.close)
                .collect::<Vec<_>>(),
            [100.0, 10.0, 2.0]
        );

        // An unknown sort key is rejected at the service boundary.
        assert!(matches!(
            list_price_page(
                &database,
                "USD",
                0,
                0,
                &TableQuery {
                    sort_by: Some("volume".to_string()),
                    ..query(0, 10)
                },
            ),
            Err(PriceServiceError::Price(PriceError::InvalidSortColumn(_)))
        ));
    }

    /// Stub FX series fetcher keyed by date, counting its calls.
    fn stub_fx_series(
        rates: Vec<(String, f64)>,
    ) -> (
        std::sync::Arc<AtomicU32>,
        impl Fn(String, String) -> FetchSeriesFuture + Send + Sync,
    ) {
        let calls = std::sync::Arc::new(AtomicU32::new(0));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |_from: String, _to: String| -> FetchSeriesFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                let rates = rates.clone();
                Box::pin(async move { Ok(rates) })
            }
        };
        (calls, fetch)
    }

    #[test]
    fn sync_prices_bulk_converts_only_existing_usd_close_times() {
        let database = Database::open_in_memory().unwrap();
        // Wednesday 2020-01-01 and Saturday 2020-01-04; the series publishes
        // business days only.
        let wednesday = civil_time::epoch_seconds(2020, 1, 1, 12, 0, 0);
        let saturday = civil_time::epoch_seconds(2020, 1, 4, 12, 0, 0);
        for close_time in [wednesday, saturday] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time,
                    close: 100.0,
                },
            )
            .unwrap();
        }
        let (calls, fetch) = stub_fx_series(vec![
            ("2020-01-01".to_string(), 0.9),
            ("2020-01-03".to_string(), 0.8),
        ]);

        let summary = block_on(sync_prices_bulk(
            &database, "eur", wednesday, saturday, &fetch,
        ))
        .unwrap();

        assert_eq!(summary.symbol, "EUR");
        assert_eq!(summary.source_rows, 2);
        assert_eq!(summary.stored, 2);
        assert_eq!(summary.skipped, 0);
        // One series request for the whole range, not one per row.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Wednesday takes its own rate; Saturday falls back to Friday's.
        assert_eq!(
            price_repository::find_price(&database, "EUR", wednesday)
                .unwrap()
                .map(|price| price.close),
            Some(90.0)
        );
        assert_eq!(
            price_repository::find_price(&database, "EUR", saturday)
                .unwrap()
                .map(|price| price.close),
            Some(80.0)
        );

        // A second run only fills gaps: nothing is inserted twice.
        let second = block_on(sync_prices_bulk(
            &database, "EUR", wednesday, saturday, &fetch,
        ))
        .unwrap();
        assert_eq!(second.stored, 0);
        assert_eq!(second.source_rows, 2);
    }

    #[test]
    fn sync_prices_bulk_skips_days_before_the_first_available_rate() {
        let database = Database::open_in_memory().unwrap();
        let before_coverage = civil_time::epoch_seconds(1998, 12, 31, 12, 0, 0);
        let covered = civil_time::epoch_seconds(1999, 1, 4, 12, 0, 0);
        for close_time in [before_coverage, covered] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time,
                    close: 100.0,
                },
            )
            .unwrap();
        }
        let (_, fetch) = stub_fx_series(vec![("1999-01-04".to_string(), 0.9)]);

        let summary = block_on(sync_prices_bulk(
            &database,
            "EUR",
            before_coverage,
            covered,
            &fetch,
        ))
        .unwrap();

        assert_eq!(summary.stored, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(
            price_repository::find_price(&database, "EUR", before_coverage).unwrap(),
            None
        );
    }

    #[test]
    fn fx_series_start_date_pads_the_range_for_prior_business_days() {
        assert_eq!(fx_series_start_date("2020-01-04"), "2019-12-28");
        // An unparseable value is passed through instead of panicking.
        assert_eq!(fx_series_start_date("not-a-date"), "not-a-date");
    }

    #[test]
    fn ensure_currency_history_converts_the_full_stored_history_once() {
        let database = Database::open_in_memory().unwrap();
        let first = civil_time::epoch_seconds(2020, 1, 1, 12, 0, 0);
        let second = civil_time::epoch_seconds(2020, 1, 2, 12, 0, 0);
        for (close_time, close) in [(first, 100.0), (second, 200.0)] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time,
                    close,
                },
            )
            .unwrap();
        }
        let (calls, fetch) = stub_fx_series(vec![
            ("2020-01-01".to_string(), 0.5),
            ("2020-01-02".to_string(), 0.4),
        ]);

        let summary = block_on(ensure_currency_history(&database, "eur", &fetch)).unwrap();

        assert_eq!(summary.symbol, "EUR");
        assert_eq!(summary.source_rows, 2);
        assert_eq!(summary.stored, 2);
        assert_eq!(summary.skipped, 0);
        assert_eq!(summary.warning, None);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            price_repository::find_price(&database, "EUR", first)
                .unwrap()
                .map(|price| price.close),
            Some(50.0)
        );
        assert_eq!(
            price_repository::find_price(&database, "EUR", second)
                .unwrap()
                .map(|price| price.close),
            Some(80.0)
        );

        // An already prepared currency is a pure database check.
        let second_run = block_on(ensure_currency_history(&database, "EUR", &fetch)).unwrap();
        assert_eq!(second_run.source_rows, 0);
        assert_eq!(second_run.stored, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ensure_currency_history_fills_gaps_without_overwriting_existing_rows() {
        let database = Database::open_in_memory().unwrap();
        let first = civil_time::epoch_seconds(2020, 1, 1, 12, 0, 0);
        let second = civil_time::epoch_seconds(2020, 1, 2, 12, 0, 0);
        for close_time in [first, second] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time,
                    close: 100.0,
                },
            )
            .unwrap();
        }
        // A manually edited target row must survive the automatic pass.
        price_repository::store_price(
            &database,
            &Price {
                symbol: "EUR".to_string(),
                close_time: first,
                close: 999.0,
            },
        )
        .unwrap();
        let (calls, fetch) = stub_fx_series(vec![("2020-01-02".to_string(), 0.4)]);

        let summary = block_on(ensure_currency_history(&database, "EUR", &fetch)).unwrap();

        assert_eq!(summary.source_rows, 1);
        assert_eq!(summary.stored, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            price_repository::find_price(&database, "EUR", first)
                .unwrap()
                .map(|price| price.close),
            Some(999.0)
        );
        assert_eq!(
            price_repository::find_price(&database, "EUR", second)
                .unwrap()
                .map(|price| price.close),
            Some(40.0)
        );
    }

    #[test]
    fn ensure_currency_history_noops_for_usd_and_without_source_rows() {
        let database = Database::open_in_memory().unwrap();
        let (calls, fetch) = stub_fx_series(vec![("2020-01-01".to_string(), 0.5)]);

        let usd = block_on(ensure_currency_history(&database, "usd", &fetch)).unwrap();
        assert_eq!(usd.stored, 0);
        assert_eq!(usd.warning, None);

        let empty = block_on(ensure_currency_history(&database, "EUR", &fetch)).unwrap();
        assert_eq!(empty.stored, 0);
        assert!(empty.warning.is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn ensure_currency_history_reports_network_failures_as_warnings() {
        let database = Database::open_in_memory().unwrap();
        let close_time = civil_time::epoch_seconds(2020, 1, 1, 12, 0, 0);
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time,
                close: 100.0,
            },
        )
        .unwrap();
        let fetch = |_from: String, _to: String| -> FetchSeriesFuture {
            Box::pin(async move {
                Err(PriceServiceError::Price(PriceError::InvalidData(
                    "fx offline".to_string(),
                )))
            })
        };

        let summary = block_on(ensure_currency_history(&database, "EUR", &fetch)).unwrap();

        assert_eq!(summary.stored, 0);
        assert!(summary
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("fx offline")));
    }

    #[test]
    fn sync_prices_bulk_validates_input_before_touching_the_network() {
        let database = Database::open_in_memory().unwrap();
        let (calls, fetch) = stub_fx_series(vec![]);

        // USD is the source currency, never a sync target.
        assert!(matches!(
            block_on(sync_prices_bulk(&database, "USD", 100, 200, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        // Open bounds and inverted ranges are rejected.
        assert!(matches!(
            block_on(sync_prices_bulk(&database, "EUR", 0, 200, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
        assert!(matches!(
            block_on(sync_prices_bulk(&database, "EUR", 300, 200, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        // An empty USD range short-circuits without a network call.
        let summary = block_on(sync_prices_bulk(&database, "EUR", 100, 200, &fetch)).unwrap();
        assert_eq!(summary.source_rows, 0);
        assert_eq!(summary.stored, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn list_price_conversions_validates_input_and_reads_the_table() {
        let database = Database::open_in_memory().unwrap();
        for (symbol, close) in [("USD", 2.0), ("EUR", 1.8), ("AUD", 3.0)] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: symbol.to_string(),
                    close_time: 200,
                    close,
                },
            )
            .unwrap();
        }

        let conversions = list_price_conversions(&database, "usd", 200).unwrap();
        assert_eq!(
            conversions
                .iter()
                .map(|price| price.symbol.as_str())
                .collect::<Vec<_>>(),
            ["AUD", "EUR"]
        );

        assert!(matches!(
            list_price_conversions(&database, "USD", 0),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
        assert!(matches!(
            list_price_conversions(&database, "u$", 200),
            Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
        ));
    }

    #[test]
    fn price_series_reports_the_full_span_and_downsamples_the_window() {
        let database = Database::open_in_memory().unwrap();
        assert!(price_series(&database, "USD", 0, 0).unwrap().is_none());

        // 10_000 rows one second apart: far more than SERIES_MAX_POINTS.
        for index in 0..10_000_i64 {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: "USD".to_string(),
                    close_time: 1_000_000 + index,
                    close: index as f64,
                },
            )
            .unwrap();
        }

        let series = price_series(&database, "usd", 0, 0).unwrap().unwrap();
        assert_eq!(series.first_close_time, 1_000_000);
        assert_eq!(series.last_close_time, 1_009_999);
        assert!(series.points.len() <= SERIES_MAX_POINTS as usize);
        assert!(series.points.len() > 1);
        // Oldest first.
        assert!(series.points[0].close_time < series.points[1].close_time);

        // A narrow window yields finer buckets over the same span.
        let zoomed = price_series(&database, "USD", 1_000_000, 1_000_099)
            .unwrap()
            .unwrap();
        assert_eq!(zoomed.points.len(), 100);
        assert_eq!(zoomed.points[0].close_time, 1_000_000);

        // Windows outside the history clamp to the stored span.
        let clamped = price_series(&database, "USD", 0, i64::MAX)
            .unwrap()
            .unwrap();
        assert_eq!(clamped.points.len(), series.points.len());
    }

    #[test]
    fn update_stored_price_validates_and_updates_in_place() {
        let database = Database::open_in_memory().unwrap();
        price_repository::store_price(
            &database,
            &Price {
                symbol: "USD".to_string(),
                close_time: 200,
                close: 2.0,
            },
        )
        .unwrap();

        let updated = update_stored_price(&database, " usd ", 200, 2.5).unwrap();
        assert_eq!(
            updated,
            Price {
                symbol: "USD".to_string(),
                close_time: 200,
                close: 2.5
            }
        );
        assert_eq!(
            price_repository::find_price(&database, "USD", 200)
                .unwrap()
                .map(|price| price.close),
            Some(2.5)
        );

        assert!(matches!(
            update_stored_price(&database, "USD", 200, -1.0),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert!(matches!(
            update_stored_price(&database, "USD", 999, 2.5),
            Err(PriceServiceError::Price(PriceError::NotFound { .. }))
        ));
        assert!(matches!(
            update_stored_price(&database, "USD", 0, 2.5),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
        // Failed edits leave the stored value untouched.
        assert_eq!(
            price_repository::find_price(&database, "USD", 200)
                .unwrap()
                .map(|price| price.close),
            Some(2.5)
        );
    }

    /// Stub daily fetcher returning `prices` and counting its calls.
    fn stub_daily(
        prices: Vec<Price>,
    ) -> (
        std::sync::Arc<AtomicU32>,
        impl Fn(i64, i64) -> FetchDailyFuture + Send + Sync,
    ) {
        let calls = std::sync::Arc::new(AtomicU32::new(0));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |_from: i64, _to: i64| -> FetchDailyFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                let prices = prices.clone();
                Box::pin(async move { Ok(prices) })
            }
        };
        (calls, fetch)
    }

    /// Stub reference-rate fetcher returning `rows` and counting its calls.
    fn stub_reference(
        rows: Vec<(i64, f64)>,
    ) -> (
        std::sync::Arc<AtomicU32>,
        impl Fn(&str, i64, i64) -> FetchReferenceFuture + Send + Sync,
    ) {
        let calls = std::sync::Arc::new(AtomicU32::new(0));
        let fetch = {
            let calls = std::sync::Arc::clone(&calls);
            move |_frequency: &str, _from: i64, _to: i64| -> FetchReferenceFuture {
                calls.fetch_add(1, Ordering::SeqCst);
                let rows = rows.clone();
                Box::pin(async move { Ok(rows) })
            }
        };
        (calls, fetch)
    }

    /// Drive [`sync_prices_range`] with the three stub sources.
    #[allow(clippy::too_many_arguments)]
    fn run_sync(
        database: &Database,
        symbol: &str,
        from_time: i64,
        to_time: i64,
        now: i64,
        plan: PriceSyncPlan,
        daily: &(dyn Fn(i64, i64) -> FetchDailyFuture + Send + Sync),
        reference: &(dyn Fn(&str, i64, i64) -> FetchReferenceFuture + Send + Sync),
        fx_series: &(dyn Fn(String, String) -> FetchSeriesFuture + Send + Sync),
    ) -> Result<PriceSyncSummary, PriceServiceError> {
        let sources = PriceSyncSources {
            daily,
            reference,
            fx_series,
        };
        block_on(sync_prices_range(
            database, symbol, from_time, to_time, now, plan, &sources,
        ))
    }

    /// Noon of a November 2026 day, the month the sync tests use.
    fn november_day(day: i64) -> i64 {
        civil_time::epoch_seconds(2026, 11, day, 12, 0, 0)
    }

    fn daily_row(day: i64, close: f64) -> Price {
        Price {
            symbol: USD_SYMBOL.to_string(),
            close_time: daily_close_time(november_day(day)),
            close,
        }
    }

    fn empty_reference() -> impl Fn(&str, i64, i64) -> FetchReferenceFuture + Send + Sync {
        |_frequency: &str, _from: i64, _to: i64| -> FetchReferenceFuture {
            Box::pin(async move { Ok(Vec::new()) })
        }
    }

    fn empty_fx_series() -> impl Fn(String, String) -> FetchSeriesFuture + Send + Sync {
        |_from: String, _to: String| -> FetchSeriesFuture {
            Box::pin(async move { Ok(Vec::new()) })
        }
    }

    #[test]
    fn asset_metric_rows_map_to_stored_daily_closes() {
        let price = daily_price("2026-06-16T00:00:00.000000000Z", 66_000.0).unwrap();
        assert_eq!(price.symbol, USD_SYMBOL);
        assert_eq!(
            price.close_time,
            civil_time::epoch_seconds(2026, 6, 16, 23, 59, 59)
        );
        assert_eq!(price.close, 66_000.0);

        assert_eq!(daily_price("not a timestamp", 66_000.0), None);
    }

    #[test]
    fn plan_price_sync_picks_the_finest_available_window() {
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let week_ago = now - 7 * DAY_SECONDS;
        let coverage = BtcUsdCoverage {
            daily_start: Some(PRICE_HISTORY_START),
            reference: vec![
                ReferenceCoverage {
                    frequency: "1m".to_string(),
                    min_time: week_ago,
                    max_time: now,
                },
                ReferenceCoverage {
                    frequency: "1h".to_string(),
                    min_time: week_ago,
                    max_time: now,
                },
            ],
        };

        let plan = plan_price_sync(
            SyncGranularity::Auto,
            &coverage,
            PRICE_HISTORY_START,
            now,
            now,
        );
        assert_eq!(plan.granularity, SyncGranularity::FifteenMinutes);
        assert_eq!(plan.granular_from, Some(week_ago));
        assert_eq!(plan.granular_to, Some(now));

        // A manual choice is honored when its window intersects the range.
        let plan = plan_price_sync(SyncGranularity::Hourly, &coverage, week_ago, now, now);
        assert_eq!(plan.granularity, SyncGranularity::Hourly);
        assert_eq!(plan.granular_from, Some(week_ago));

        // A range older than the reference window falls back to daily only.
        let old = civil_time::epoch_seconds(2020, 1, 1, 0, 0, 0);
        let plan = plan_price_sync(
            SyncGranularity::Auto,
            &coverage,
            old,
            old + DAY_SECONDS,
            now,
        );
        assert_eq!(plan.granularity, SyncGranularity::Daily);
        assert_eq!(plan.granular_from, None);

        // A manual intraday choice without coverage keeps its label so the
        // run can explain why the stage was skipped.
        let empty = BtcUsdCoverage {
            daily_start: Some(PRICE_HISTORY_START),
            reference: Vec::new(),
        };
        let plan = plan_price_sync(SyncGranularity::FifteenMinutes, &empty, old, now, now);
        assert_eq!(plan.granularity, SyncGranularity::FifteenMinutes);
        assert_eq!(plan.granular_from, None);
    }

    #[test]
    fn reference_prices_sample_the_requested_frequency() {
        let start = civil_time::epoch_seconds(2026, 11, 6, 18, 0, 0);
        let rows = vec![
            (start, 1.0),
            (start + 60, 2.0),
            (start + 15 * 60, 3.0),
            (start + 3_600, 4.0),
        ];

        let quarter = reference_prices(SyncGranularity::FifteenMinutes, rows.clone());
        assert_eq!(
            quarter,
            [
                Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time: start,
                    close: 1.0,
                },
                Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time: start + 15 * 60,
                    close: 3.0,
                },
                Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time: start + 3_600,
                    close: 4.0,
                },
            ]
        );

        let hourly = reference_prices(SyncGranularity::Hourly, rows);
        assert_eq!(hourly.len(), 2);
        assert_eq!(hourly[0].close_time, start);
        assert_eq!(hourly[1].close_time, start + 3_600);

        assert!(reference_prices(SyncGranularity::Daily, vec![(start, 1.0)]).is_empty());
    }

    #[test]
    fn sync_prices_range_fills_daily_and_granular_rows() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let granular_start = civil_time::epoch_seconds(2026, 11, 6, 18, 0, 0);
        let (daily_calls, daily) = stub_daily(
            (1..=5)
                .map(|day| daily_row(day, 100.0 + day as f64))
                .collect(),
        );
        let (reference_calls, reference) = stub_reference(vec![
            (granular_start, 1.0),
            (granular_start + 15 * 60, 2.0),
            (granular_start + 30 * 60, 3.0),
        ]);
        let fx = empty_fx_series();
        let plan = PriceSyncPlan {
            granularity: SyncGranularity::FifteenMinutes,
            first_day: PRICE_HISTORY_START,
            granular_from: Some(granular_start),
            granular_to: Some(now),
        };

        let summary = run_sync(
            &database, USD_SYMBOL, from, now, now, plan, &daily, &reference, &fx,
        )
        .unwrap();

        assert_eq!(summary.from_time, from);
        assert_eq!(summary.to_time, now);
        assert_eq!(summary.granularity, SyncGranularity::FifteenMinutes);
        assert_eq!(summary.daily_fetched, 5);
        assert_eq!(summary.daily_stored, 5);
        assert_eq!(summary.granular_fetched, 3);
        assert_eq!(summary.granular_stored, 3);
        assert_eq!(summary.unavailable_days, 0);
        assert!(summary.warnings.is_empty());
        assert_eq!(daily_calls.load(Ordering::SeqCst), 1);
        assert_eq!(reference_calls.load(Ordering::SeqCst), 1);

        // The daily closes live at 23:59:59; the reference observations keep
        // their source timestamps.
        assert_eq!(
            price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(3)))
                .unwrap()
                .map(|price| price.close),
            Some(103.0)
        );
        assert_eq!(
            price_repository::find_latest_price_between(
                &database,
                USD_SYMBOL,
                now - GRANULAR_RESOLUTION_LOOKBACK,
                now,
            )
            .unwrap()
            .map(|price| price.close),
            Some(3.0)
        );

        // A second run inserts nothing new.
        let second = run_sync(
            &database, USD_SYMBOL, from, now, now, plan, &daily, &reference, &fx,
        )
        .unwrap();
        assert_eq!(second.daily_stored, 0);
        assert_eq!(second.granular_stored, 0);
    }

    #[test]
    fn sync_prices_range_preserves_existing_rows_and_counts_unavailable_days() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        // A manually edited row on day 2 must survive the sync.
        price_repository::store_price(&database, &daily_row(2, 999.0)).unwrap();
        // The source only knows days 1 and 3 of the expected five days.
        let (_, daily) = stub_daily(vec![daily_row(1, 101.0), daily_row(3, 103.0)]);
        let reference = empty_reference();
        let fx = empty_fx_series();

        let summary = run_sync(
            &database,
            USD_SYMBOL,
            from,
            now,
            now,
            PriceSyncPlan::daily(PRICE_HISTORY_START),
            &daily,
            &reference,
            &fx,
        )
        .unwrap();

        assert_eq!(summary.daily_fetched, 2);
        assert_eq!(summary.daily_stored, 2);
        assert_eq!(summary.unavailable_days, 3);
        assert_eq!(summary.granular_fetched, 0);
        assert_eq!(
            price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(2)))
                .unwrap()
                .map(|price| price.close),
            Some(999.0)
        );
    }

    #[test]
    fn sync_prices_range_keeps_daily_rows_when_the_intraday_stage_fails() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let granular_start = civil_time::epoch_seconds(2026, 11, 6, 18, 0, 0);
        let (_, daily) = stub_daily(vec![daily_row(5, 105.0)]);
        let reference = |_frequency: &str, _from: i64, _to: i64| -> FetchReferenceFuture {
            Box::pin(async move {
                Err(PriceServiceError::CoinMetrics(CoinMetricsError::Url(
                    "offline".to_string(),
                )))
            })
        };
        let fx = empty_fx_series();
        let plan = PriceSyncPlan {
            granularity: SyncGranularity::Hourly,
            first_day: PRICE_HISTORY_START,
            granular_from: Some(granular_start),
            granular_to: Some(now),
        };

        let summary = run_sync(
            &database, USD_SYMBOL, from, now, now, plan, &daily, &reference, &fx,
        )
        .unwrap();

        assert_eq!(summary.daily_stored, 1);
        assert_eq!(summary.granular_stored, 0);
        assert_eq!(summary.warnings.len(), 1);
        assert!(summary.warnings[0].contains("Could not fetch hourly closes"));
    }

    #[test]
    fn sync_prices_range_fails_when_the_daily_source_fails() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let daily = |_from: i64, _to: i64| -> FetchDailyFuture {
            Box::pin(async move {
                Err(PriceServiceError::CoinMetrics(CoinMetricsError::Url(
                    "offline".to_string(),
                )))
            })
        };
        let reference = empty_reference();
        let fx = empty_fx_series();

        assert!(matches!(
            run_sync(
                &database,
                USD_SYMBOL,
                from,
                now,
                now,
                PriceSyncPlan::daily(PRICE_HISTORY_START),
                &daily,
                &reference,
                &fx,
            ),
            Err(PriceServiceError::CoinMetrics(_))
        ));
    }

    #[test]
    fn sync_prices_range_rejects_invalid_source_values_and_bad_ranges() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let (_, daily) = stub_daily(vec![Price {
            symbol: USD_SYMBOL.to_string(),
            close_time: daily_close_time(november_day(5)),
            close: -1.0,
        }]);
        let reference = empty_reference();
        let fx = empty_fx_series();

        assert!(matches!(
            run_sync(
                &database,
                USD_SYMBOL,
                from,
                now,
                now,
                PriceSyncPlan::daily(PRICE_HISTORY_START),
                &daily,
                &reference,
                &fx,
            ),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert!(matches!(
            run_sync(
                &database,
                USD_SYMBOL,
                now,
                from,
                now,
                PriceSyncPlan::daily(PRICE_HISTORY_START),
                &daily,
                &reference,
                &fx,
            ),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert!(matches!(
            run_sync(
                &database,
                USD_SYMBOL,
                from,
                now,
                0,
                PriceSyncPlan::daily(PRICE_HISTORY_START),
                &daily,
                &reference,
                &fx,
            ),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
    }

    #[test]
    fn sync_prices_range_converts_every_stored_usd_close_time() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let quarter_hour = civil_time::epoch_seconds(2026, 11, 1, 12, 14, 59);
        let observation = civil_time::epoch_seconds(2026, 11, 6, 18, 0, 0);
        for close_time in [quarter_hour, observation] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: USD_SYMBOL.to_string(),
                    close_time,
                    close: 100.0,
                },
            )
            .unwrap();
        }
        let (_, daily) = stub_daily((1..=5).map(|day| daily_row(day, 100.0)).collect());
        let reference = empty_reference();
        let (fx_calls, fx) = stub_fx_series(vec![("2010-01-01".to_string(), 0.5)]);

        let summary = run_sync(
            &database,
            "eur",
            from,
            now,
            now,
            PriceSyncPlan::daily(PRICE_HISTORY_START),
            &daily,
            &reference,
            &fx,
        )
        .unwrap();

        let conversion = summary.conversion.expect("EUR run converts");
        assert_eq!(conversion.symbol, "EUR");
        assert_eq!(conversion.source_rows, 7);
        assert_eq!(conversion.stored, 7);
        assert_eq!(conversion.skipped, 0);
        assert_eq!(fx_calls.load(Ordering::SeqCst), 1);
        for close_time in [quarter_hour, observation, daily_close_time(november_day(5))] {
            assert_eq!(
                price_repository::find_price(&database, "EUR", close_time)
                    .unwrap()
                    .map(|price| price.close),
                Some(50.0)
            );
        }

        // A second run keeps the target rows and stores nothing new.
        let second = run_sync(
            &database,
            "EUR",
            from,
            now,
            now,
            PriceSyncPlan::daily(PRICE_HISTORY_START),
            &daily,
            &reference,
            &fx,
        )
        .unwrap();
        let second = second.conversion.expect("EUR run converts");
        assert_eq!(second.source_rows, 7);
        assert_eq!(second.stored, 0);
    }

    #[test]
    fn sync_prices_range_reports_a_failed_conversion_as_a_warning() {
        let database = Database::open_in_memory().unwrap();
        let now = civil_time::epoch_seconds(2026, 11, 6, 19, 0, 0);
        let from = civil_time::epoch_seconds(2026, 11, 1, 0, 0, 0);
        let (_, daily) = stub_daily(vec![daily_row(5, 100.0)]);
        let reference = empty_reference();
        let fx = |_from: String, _to: String| -> FetchSeriesFuture {
            Box::pin(async move {
                Err(PriceServiceError::Price(PriceError::InvalidData(
                    "offline".to_string(),
                )))
            })
        };

        let summary = run_sync(
            &database,
            "EUR",
            from,
            now,
            now,
            PriceSyncPlan::daily(PRICE_HISTORY_START),
            &daily,
            &reference,
            &fx,
        )
        .unwrap();

        assert_eq!(summary.daily_stored, 1);
        assert!(summary.conversion.is_none());
        assert_eq!(summary.warnings.len(), 1);
        assert!(summary.warnings[0].contains("Could not convert"));
        assert_eq!(
            price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(5)))
                .unwrap()
                .map(|price| price.close),
            Some(100.0)
        );
    }

    #[test]
    fn resolution_prefers_a_recent_intraday_row() {
        let database = Database::open_in_memory().unwrap();
        let observation = BEYOND_COVERAGE - 30 * 60;
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: observation,
                close: 123.0,
            },
        )
        .unwrap();
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: daily_close_time(BEYOND_COVERAGE),
                close: 456.0,
            },
        )
        .unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let price = block_on(resolve_price(
            &database,
            USD_SYMBOL,
            BEYOND_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 123.0);
        assert_eq!(price.close_time, observation);
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
        assert_eq!(stubs.fx_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn resolution_falls_back_to_the_daily_row_without_intraday_data() {
        let database = Database::open_in_memory().unwrap();
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: daily_close_time(BEYOND_COVERAGE),
                close: 456.0,
            },
        )
        .unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let price = block_on(resolve_price(
            &database,
            USD_SYMBOL,
            BEYOND_COVERAGE,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 456.0);
        assert_eq!(price.close_time, daily_close_time(BEYOND_COVERAGE));
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_daily_close_just_before_midnight_does_not_outrank_the_daily_bucket() {
        let database = Database::open_in_memory().unwrap();
        let just_after_midnight = civil_time::epoch_seconds(2026, 7, 1, 0, 30, 0);
        let previous_close = daily_close_time(just_after_midnight - DAY_SECONDS);
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: previous_close,
                close: 456.0,
            },
        )
        .unwrap();
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: daily_close_time(just_after_midnight),
                close: 789.0,
            },
        )
        .unwrap();
        let stubs = stub_fetchers(66_000.0, 0.9);

        let price = block_on(resolve_price(
            &database,
            USD_SYMBOL,
            just_after_midnight,
            stubs.usd.as_ref(),
            stubs.fx.as_ref(),
        ))
        .unwrap();

        assert_eq!(price.close, 789.0);
        assert_eq!(price.close_time, daily_close_time(just_after_midnight));
        assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn delete_stored_prices_validates_and_deduplicates_the_snapshot_batch() {
        let database = Database::open_in_memory().unwrap();
        for (symbol, close_time, close) in [
            ("USD", 100, 1.0),
            ("EUR", 100, 0.9),
            ("USD", 200, 2.0),
            ("GBP", 300, 3.0),
        ] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: symbol.to_string(),
                    close_time,
                    close,
                },
            )
            .unwrap();
        }

        assert!(matches!(
            delete_stored_prices(&database, &[]),
            Err(PriceServiceError::Validation(_))
        ));
        assert!(matches!(
            delete_stored_prices(&database, &[0]),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));

        // Duplicate close times collapse into one snapshot deletion.
        assert_eq!(
            delete_stored_prices(&database, &[100, 100, 300]).unwrap(),
            3
        );
        assert_eq!(
            price_repository::count_prices(&database, "USD", 0, 0).unwrap(),
            1
        );
        assert_eq!(
            price_repository::count_prices(&database, "EUR", 0, 0).unwrap(),
            0
        );
        assert_eq!(
            price_repository::count_prices(&database, "GBP", 0, 0).unwrap(),
            0
        );
        assert_eq!(
            price_repository::find_price(&database, "USD", 200).unwrap(),
            Some(Price {
                symbol: "USD".to_string(),
                close_time: 200,
                close: 2.0,
            })
        );
    }
}
