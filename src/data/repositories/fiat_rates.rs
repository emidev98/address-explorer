//! SQL access to the historical fiat-rate cache used by exchange imports.

use rusqlite::{params, OptionalExtension};

use crate::data::{Database, DatabaseError};

/// Cached rate for a currency pair on a calendar date.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedFiatRate {
    pub date: String,
    pub rate: f64,
}

/// Find the latest cached rate at or before `date` for a pair.
pub fn latest_rate(
    database: &Database,
    base: &str,
    quote: &str,
    date: &str,
) -> Result<Option<CachedFiatRate>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT rate_date, rate FROM fiat_exchange_rates
             WHERE base = ?1 AND quote = ?2 AND rate_date <= ?3
             ORDER BY rate_date DESC LIMIT 1",
            params![base, quote, date],
            |row| {
                Ok(CachedFiatRate {
                    date: row.get(0)?,
                    rate: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Insert or refresh one cached rate.
pub fn upsert_rate(
    database: &Database,
    base: &str,
    quote: &str,
    date: &str,
    rate: f64,
    fetched_at: i64,
) -> Result<(), DatabaseError> {
    let connection = database.connection()?;
    connection.execute(
        "INSERT INTO fiat_exchange_rates (base, quote, rate_date, rate, fetched_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(base, quote, rate_date) DO UPDATE SET
             rate = excluded.rate, fetched_at = excluded.fetched_at",
        params![base, quote, date, rate, fetched_at],
    )?;
    Ok(())
}

/// Insert several rates atomically.
pub fn upsert_rates(
    database: &Database,
    rates: &[(String, String, String, f64)],
    fetched_at: i64,
) -> Result<(), DatabaseError> {
    if rates.is_empty() {
        return Ok(());
    }
    let mut connection = database.connection()?;
    let transaction = connection.transaction()?;
    for (base, quote, date, rate) in rates {
        transaction.execute(
            "INSERT INTO fiat_exchange_rates (base, quote, rate_date, rate, fetched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(base, quote, rate_date) DO UPDATE SET
                 rate = excluded.rate, fetched_at = excluded.fetched_at",
            params![base, quote, date, rate, fetched_at],
        )?;
    }
    transaction.commit()?;
    Ok(())
}
