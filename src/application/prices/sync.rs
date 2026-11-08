use super::*;
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
        let start = now - 7 * civil_time::SECONDS_PER_DAY;
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
    pub(crate) fn daily(first_day: i64) -> Self {
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
            civil_time::format_date(to_time),
        ))
        .into());
    }
    let range_start = from_time.max(plan.first_day);
    let range_end = to_time.min(now);
    if range_start > range_end {
        return Err(PriceError::InvalidData(format!(
            "the sync range ends before the first day the source serves ({})",
            civil_time::format_date(plan.first_day),
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
    let complete_through = range_end.min(civil_time::start_of_day(now) - 1);
    let fetched_complete = daily
        .iter()
        .filter(|price| price.close_time <= complete_through)
        .count() as u64;
    summary.unavailable_days =
        count_daily_closes(range_start, complete_through).saturating_sub(fetched_complete);
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
                plan.granularity.resolution_label(),
            )),
        }
    } else if plan.granularity.is_intraday() {
        summary.warnings.push(format!(
            "No {} closes are available for the selected range; only daily closes were synced.",
            plan.granularity.resolution_label(),
        ));
    }
    if symbol != USD_SYMBOL {
        match sync_prices_bulk(database, &symbol, range_start, range_end, sources.fx_series).await {
            Ok(bulk) => {
                if bulk.skipped > 0 {
                    summary.warnings.push(format!(
                        "{} source prices had no FX rate for their day and were skipped.",
                        bulk.skipped,
                    ));
                }
                summary.conversion = Some(bulk);
            }
            Err(error @ PriceServiceError::Database(_)) => return Err(error),
            Err(error) => summary.warnings.push(format!(
                "Could not convert the stored prices into {symbol}: {error}",
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
pub(crate) fn reference_prices(granularity: SyncGranularity, rows: Vec<(i64, f64)>) -> Vec<Price> {
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
    ((through - first) / civil_time::SECONDS_PER_DAY + 1) as u64
}
