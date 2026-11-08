use super::*;
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
