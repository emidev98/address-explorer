use super::*;

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

/// Largest page the listing endpoint serves; bigger requests are clamped
/// instead of rejected so a misbehaving client cannot force a full-table
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
