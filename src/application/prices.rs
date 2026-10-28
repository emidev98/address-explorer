//! Price service: keeps the `historical_prices` table populated and resolves
//! BTC prices in fiat.
//!
//! Resolution checks the database first, asks the external APIs exactly once
//! per missing day, and stores every resolved price back into the table so
//! the same lookup never touches the network twice. SQL stays in
//! `data::repositories::prices`; the external sources stay behind the
//! `CoinMetrics` and `Frankfurter` clients.

use std::future::Future;
use std::pin::Pin;

use crate::clients::coinmetrics::params::{Page, TimeRange};
use crate::clients::coinmetrics::timeseries::AssetMetricsQuery;
use crate::clients::coinmetrics::{CoinMetrics, CoinMetricsConfig, CoinMetricsError, Timeseries};
use crate::clients::frankfurter::{
    Frankfurter, FrankfurterConfig, FrankfurterError, Rates, RatesQuery,
};
use crate::data::repositories::prices::{self as price_repository, PriceSortColumn};
use crate::data::{Database, DatabaseError};
use crate::domain::civil_time;
use crate::domain::price::{
    daily_close_time, fifteen_minute_close_time, normalize_symbol, validate_price_value,
    BulkSyncSummary, FiatCurrency, Price, PriceConversion, PriceError, PricePage, PriceSeries,
    SyncToTodaySummary, CSV_COVERAGE_END, USD_SYMBOL,
};
use crate::domain::table::TableQuery;

/// Failures of the price service: domain validation and resolution failures,
/// plus the infrastructure errors of the database and the external sources.
#[derive(Debug, thiserror::Error)]
pub enum PriceServiceError {
    #[error(transparent)]
    Price(#[from] PriceError),
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
/// 1. exact row in `historical_prices` — including the imported 15-minute
///    `USD` history;
/// 2. the daily row covering the request (used to answer candles missing from
///    the historical export, e.g. maintenance gaps, without a network call);
/// 3. one API round trip per missing day: Coin Metrics for the BTC/USD close
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

/// Fetch BTC/USD daily closes from Coin Metrics from the latest stored USD
/// close through the current day and persist them, in one transaction.
///
/// The daily series supplies the granularity of the table beyond the imported
/// history. Coin Metrics timestamps a daily row at the start of its interval,
/// so every row is stored at the matching candle close time (`23:59:59`). Only
/// days after the latest stored close are inserted; existing rows are left
/// untouched, so re-running the sync only fills gaps.
pub async fn sync_prices_to_today(
    database: &Database,
    now: i64,
    fetch_daily: &(dyn Fn(i64, i64) -> FetchDailyFuture + Send + Sync),
) -> Result<SyncToTodaySummary, PriceServiceError> {
    if now <= 0 {
        return Err(PriceError::InvalidTimestamp(now).into());
    }
    let Some((_, latest)) = price_repository::price_series_range(database, USD_SYMBOL)? else {
        return Err(PriceError::InvalidData(
            "there is no stored USD price history to sync from".to_string(),
        )
        .into());
    };

    let mut summary = SyncToTodaySummary {
        from_time: latest,
        to_time: now,
        fetched: 0,
        stored: 0,
    };
    if latest >= now {
        return Ok(summary);
    }

    let mut prices = fetch_daily(latest, now).await?;
    // The source starts at the day containing the latest stored close; the
    // range filter drops that overlapping day when it is already stored.
    prices.retain(|price| price.close_time > latest && price.close_time <= daily_close_time(now));
    prices.sort_by_key(|price| price.close_time);
    prices.dedup_by_key(|price| price.close_time);
    for price in &mut prices {
        price.symbol = USD_SYMBOL.to_string();
        price.close = validate_price_value(price.close, USD_SYMBOL, "Coin Metrics PriceUSD 1d")?;
    }

    summary.fetched = prices.len() as u64;
    summary.stored = price_repository::store_prices_bulk(database, &prices)?;
    Ok(summary)
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

/// One day in seconds: the fallback distance of [`latest_price_at`].
const LATEST_PRICE_FALLBACK_SECONDS: i64 = 86_400;

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
            coinmetrics: CoinMetrics::new(CoinMetricsConfig::default()),
            frankfurter: Frankfurter::new(FrankfurterConfig::default()),
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

    /// Convert the stored USD history of `[from_time, to_time]` into
    /// `symbol` and persist it; see [`sync_prices_bulk`]. The Frankfurter
    /// `USD → symbol` series for the range is fetched in a single request.
    pub async fn sync_prices(
        &self,
        symbol: &str,
        from_time: i64,
        to_time: i64,
    ) -> Result<BulkSyncSummary, PriceServiceError> {
        let fetch_fx_series = |from_date: String, to_date: String| -> FetchSeriesFuture {
            let rates = self.frankfurter.rates.clone();
            let quote = symbol.to_string();
            Box::pin(async move { fetch_usd_fx_series(&rates, &from_date, &to_date, &quote).await })
        };
        sync_prices_bulk(&self.database, symbol, from_time, to_time, &fetch_fx_series).await
    }

    /// Fetch the BTC/USD daily closes from the latest stored close through
    /// the current day and persist them; see [`sync_prices_to_today`].
    pub async fn sync_prices_to_today(&self) -> Result<SyncToTodaySummary, PriceServiceError> {
        let now = current_epoch_seconds()?;
        let fetch_daily = |from_time: i64, to_time: i64| -> FetchDailyFuture {
            let timeseries = self.coinmetrics.timeseries.clone();
            Box::pin(async move { fetch_btc_usd_daily(&timeseries, from_time, to_time).await })
        };
        sync_prices_to_today(&self.database, now, &fetch_daily).await
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
/// `next_page_token` so a long gap between the latest stored price and the
/// current day is fetched completely. Rows are keyed by their interval start
/// and mapped to the close time stored in the table. Rows without a
/// `PriceUSD` value are skipped: the API reports gaps that way.
async fn fetch_btc_usd_daily(
    timeseries: &Timeseries,
    from_time: i64,
    to_time: i64,
) -> Result<Vec<Price>, PriceServiceError> {
    // Include the day containing the latest stored close; the service drops
    // it when that close is already stored.
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

/// Fetch the `USD → quote` blended rate series of `[from_date, to_date]`
/// (ISO `YYYY-MM-DD`) from Frankfurter, as `(date, rate)` pairs.
async fn fetch_usd_fx_series(
    rates: &Rates,
    from_date: &str,
    to_date: &str,
    quote: &str,
) -> Result<Vec<(String, f64)>, PriceServiceError> {
    let query = RatesQuery {
        from: Some(from_date.to_string()),
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
    fn sync_prices_to_today_stores_daily_closes_through_the_current_day() {
        let database = Database::open_in_memory().unwrap();
        let latest = civil_time::epoch_seconds(2026, 6, 15, 23, 29, 59);
        let now = civil_time::epoch_seconds(2026, 6, 18, 3, 30, 0);
        for (symbol, close) in [(USD_SYMBOL, 100.0), ("EUR", 90.0)] {
            price_repository::store_price(
                &database,
                &Price {
                    symbol: symbol.to_string(),
                    close_time: latest,
                    close,
                },
            )
            .unwrap();
        }

        let first_day = civil_time::epoch_seconds(2026, 6, 15, 0, 0, 0);
        let mut rows = (0..5)
            .map(|index| Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: daily_close_time(first_day + index * 86_400),
                close: 100.0 + index as f64,
            })
            .collect::<Vec<_>>();
        // A duplicate is collapsed, and the day after `now` is dropped. The
        // current day's row is kept: "to today" includes the day in progress.
        rows.push(rows[1].clone());
        let (calls, fetch) = stub_daily(rows);

        let summary = block_on(sync_prices_to_today(&database, now, &fetch)).unwrap();

        assert_eq!(summary.from_time, latest);
        assert_eq!(summary.to_time, now);
        assert_eq!(summary.fetched, 4);
        assert_eq!(summary.stored, 4);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        for index in 0..4 {
            let close_time = daily_close_time(first_day + index * 86_400);
            assert_eq!(
                price_repository::find_price(&database, USD_SYMBOL, close_time)
                    .unwrap()
                    .map(|price| price.close),
                Some(100.0 + index as f64)
            );
        }
        assert_eq!(
            price_repository::find_price(
                &database,
                USD_SYMBOL,
                daily_close_time(first_day + 4 * 86_400)
            )
            .unwrap(),
            None
        );
        // Other symbols keep their values.
        assert_eq!(
            price_repository::find_price(&database, "EUR", latest)
                .unwrap()
                .map(|price| price.close),
            Some(90.0)
        );

        // A re-run after the current day's close is stored has nothing to do.
        let second = block_on(sync_prices_to_today(&database, now, &fetch)).unwrap();
        assert_eq!(second.fetched, 0);
        assert_eq!(second.stored, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sync_prices_to_today_requires_stored_history_and_a_positive_clock() {
        let database = Database::open_in_memory().unwrap();
        let (calls, fetch) = stub_daily(vec![]);

        assert!(matches!(
            block_on(sync_prices_to_today(&database, 0, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
        ));
        assert!(matches!(
            block_on(sync_prices_to_today(&database, 1_000, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        let latest = civil_time::epoch_seconds(2026, 6, 15, 23, 29, 59);
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: latest,
                close: 100.0,
            },
        )
        .unwrap();

        // Nothing to fetch when the history already reaches the clock.
        let summary = block_on(sync_prices_to_today(&database, latest, &fetch)).unwrap();
        assert_eq!(summary.from_time, latest);
        assert_eq!(summary.to_time, latest);
        assert_eq!(summary.fetched, 0);
        assert_eq!(summary.stored, 0);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn sync_prices_to_today_rejects_invalid_source_values() {
        let database = Database::open_in_memory().unwrap();
        let latest = civil_time::epoch_seconds(2026, 6, 15, 23, 29, 59);
        let now = civil_time::epoch_seconds(2026, 6, 18, 3, 30, 0);
        price_repository::store_price(
            &database,
            &Price {
                symbol: USD_SYMBOL.to_string(),
                close_time: latest,
                close: 100.0,
            },
        )
        .unwrap();
        let (_, fetch) = stub_daily(vec![Price {
            symbol: USD_SYMBOL.to_string(),
            close_time: daily_close_time(civil_time::epoch_seconds(2026, 6, 16, 0, 0, 0)),
            close: -1.0,
        }]);

        assert!(matches!(
            block_on(sync_prices_to_today(&database, now, &fetch)),
            Err(PriceServiceError::Price(PriceError::InvalidData(_)))
        ));
        assert_eq!(
            price_repository::count_prices(&database, USD_SYMBOL, 0, 0).unwrap(),
            1
        );
    }
}
