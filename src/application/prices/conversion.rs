use super::*;
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
    fetch_fx_series: &FetchSeriesFn<'_>,
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
            civil_time::format_date(to_time),
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
    fetch_fx_series: &FetchSeriesFn<'_>,
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
                    bulk.skipped,
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
