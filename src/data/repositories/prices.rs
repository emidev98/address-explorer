//! SQL access to `historical_prices`. Callers run these synchronous
//! functions away from the UI path; the guard inside `Database` keeps
//! them serialized.

use rusqlite::{params, OptionalExtension};

use crate::data::{Database, DatabaseError};
use crate::domain::price::{Price, PriceRow};
use crate::domain::table::SortDirection;

/// Sortable columns of the historical prices table: the fixed allowlist the
/// dynamic `ORDER BY` identifier is selected from (Section 6.3 of
/// CONTRIBUTING.md). Client sort keys never reach SQL directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceSortColumn {
    /// Close time, API key `date`.
    CloseTime,
    /// Close price, API key `price`.
    Close,
}

impl PriceSortColumn {
    /// Parse the client-facing sort key; unknown keys are rejected by the
    /// caller at the service boundary.
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "date" => Some(Self::CloseTime),
            "price" => Some(Self::Close),
            _ => None,
        }
    }

    /// `ORDER BY` expression of the column. Dates are stored as epoch
    /// seconds and closes as `REAL`, so ordering follows the typed storage
    /// class: dates compare as dates and numbers as numbers, never as
    /// formatted strings (Section 5.11 of CONTRIBUTING.md).
    fn order_expression(self) -> &'static str {
        match self {
            Self::CloseTime => "close_time",
            Self::Close => "close",
        }
    }
}

/// The `WHERE` fragment shared by [`list_prices`] and [`count_prices`]:
/// the symbol and the optional close-time bounds.
const LIST_WHERE: &str = "symbol = ?1
       AND (?2 <= 0 OR close_time >= ?2)
       AND (?3 <= 0 OR close_time <= ?3)";

/// Look up the stored price of a symbol at an exact close time.
pub fn find_price(
    database: &Database,
    symbol: &str,
    close_time: i64,
) -> Result<Option<Price>, DatabaseError> {
    let connection = database.connection()?;
    connection
        .query_row(
            "SELECT close_time, close, symbol FROM historical_prices
             WHERE symbol = ?1 AND close_time = ?2",
            params![symbol, close_time],
            |row| {
                Ok(Price {
                    close_time: row.get(0)?,
                    close: row.get(1)?,
                    symbol: row.get(2)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

/// Persist a price. The primary key keeps the first value for a
/// `(symbol, close_time)` pair; `Ok(true)` reports an actual insert.
pub fn store_price(database: &Database, price: &Price) -> Result<bool, DatabaseError> {
    let connection = database.connection()?;
    let inserted = connection.execute(
        "INSERT INTO historical_prices (close_time, close, symbol)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(symbol, close_time) DO NOTHING",
        params![price.close_time, price.close, price.symbol],
    )?;
    Ok(inserted == 1)
}

/// Filters, paging, and sorting of a [`list_prices`] call.
#[derive(Debug, Clone, Default)]
pub struct PriceListQuery<'a> {
    pub symbol: &'a str,
    pub limit: u32,
    pub offset: u64,
    /// Inclusive close-time bounds; a bound of `0` or less leaves that side
    /// of the range open.
    pub from_time: i64,
    pub to_time: i64,
    /// Column of the [`PriceSortColumn`] allowlist and direction; `None` is
    /// the default order, newest close first.
    pub sort: Option<(PriceSortColumn, SortDirection)>,
}

/// Read one page of the stored history matching `query`. Each row carries
/// the other symbols stored at the same close time (sorted by code), read
/// through the `idx_historical_prices_close_time` index.
pub fn list_prices(
    database: &Database,
    query: &PriceListQuery<'_>,
) -> Result<Vec<PriceRow>, DatabaseError> {
    // SQLite integers are signed; offsets beyond i64::MAX are empty pages.
    let offset = i64::try_from(query.offset).unwrap_or(i64::MAX);
    let (column, direction) = query
        .sort
        .map(|(column, direction)| (column.order_expression(), direction.as_sql()))
        .unwrap_or(("close_time", "DESC"));
    // The secondary key keeps rows with equal sort values in a deterministic
    // order, so pagination never overlaps or skips rows between pages.
    let sql = format!(
        "SELECT close_time, close, symbol,
                (SELECT GROUP_CONCAT(other_symbol) FROM (
                    SELECT other.symbol AS other_symbol
                    FROM historical_prices AS other
                    WHERE other.close_time = historical_prices.close_time
                      AND other.symbol != historical_prices.symbol
                    ORDER BY other.symbol))
         FROM historical_prices
         WHERE {LIST_WHERE}
         ORDER BY {column} {direction}, close_time {direction}
         LIMIT ?4 OFFSET ?5"
    );
    let connection = database.connection()?;
    let mut statement = connection.prepare(&sql)?;
    let rows = statement
        .query_map(
            params![
                query.symbol,
                query.from_time,
                query.to_time,
                query.limit,
                offset
            ],
            |row| {
                let other_symbols: Option<String> = row.get(3)?;
                Ok(PriceRow {
                    price: Price {
                        close_time: row.get(0)?,
                        close: row.get(1)?,
                        symbol: row.get(2)?,
                    },
                    other_symbols: other_symbols
                        .map(|joined| joined.split(',').map(str::to_string).collect())
                        .unwrap_or_default(),
                })
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Count the stored rows of `symbol`, across all pages. The bounds follow
/// the same conventions as [`list_prices`].
pub fn count_prices(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
) -> Result<u64, DatabaseError> {
    let sql = format!(
        "SELECT COUNT(*) FROM historical_prices
         WHERE {LIST_WHERE}"
    );
    let connection = database.connection()?;
    let count: i64 =
        connection.query_row(&sql, params![symbol, from_time, to_time], |row| row.get(0))?;
    Ok(count.max(0) as u64)
}

/// All stored rows of `symbol` within `[from_time, to_time]`, oldest first:
/// the source rows of a bulk conversion sync.
pub fn list_prices_in_range(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
) -> Result<Vec<Price>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT close_time, close, symbol FROM historical_prices
         WHERE symbol = ?1 AND close_time >= ?2 AND close_time <= ?3
         ORDER BY close_time",
    )?;
    let rows = statement
        .query_map(params![symbol, from_time, to_time], |row| {
            Ok(Price {
                close_time: row.get(0)?,
                close: row.get(1)?,
                symbol: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Store many prices in a single transaction: the unit of work of a bulk
/// sync (Section 6.4 of CONTRIBUTING.md). Returns the number of rows
/// actually inserted; existing `(symbol, close_time)` rows are left
/// untouched, like [`store_price`].
pub fn store_prices_bulk(database: &Database, prices: &[Price]) -> Result<u64, DatabaseError> {
    let mut connection = database.connection()?;
    let transaction = connection.transaction()?;
    let mut inserted = 0_u64;
    {
        let mut statement = transaction.prepare(
            "INSERT INTO historical_prices (close_time, close, symbol)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(symbol, close_time) DO NOTHING",
        )?;
        for price in prices {
            inserted +=
                statement.execute(params![price.close_time, price.close, price.symbol])? as u64;
        }
    }
    transaction.commit()?;
    Ok(inserted)
}

/// Update the close of a stored price; `Ok(true)` reports the row existed.
pub fn update_price(database: &Database, price: &Price) -> Result<bool, DatabaseError> {
    let connection = database.connection()?;
    let updated = connection.execute(
        "UPDATE historical_prices SET close = ?1
         WHERE symbol = ?2 AND close_time = ?3",
        params![price.close, price.symbol, price.close_time],
    )?;
    Ok(updated == 1)
}

/// Time span of the stored history of `symbol`: the oldest and newest close
/// time, or `None` when the symbol has no rows.
pub fn price_series_range(
    database: &Database,
    symbol: &str,
) -> Result<Option<(i64, i64)>, DatabaseError> {
    let connection = database.connection()?;
    let (first, last) = connection.query_row(
        "SELECT MIN(close_time), MAX(close_time) FROM historical_prices
         WHERE symbol = ?1",
        [symbol],
        |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
    )?;
    Ok(first.zip(last))
}

/// Bucketed average closes of `symbol` within `[from_time, to_time]`, oldest
/// first. Rows are grouped into fixed-width time buckets of
/// `bucket_width_seconds` (at least 1) and averaged, which bounds the number
/// of returned points regardless of how many raw rows the window holds.
pub fn price_series(
    database: &Database,
    symbol: &str,
    from_time: i64,
    to_time: i64,
    bucket_width_seconds: i64,
) -> Result<Vec<Price>, DatabaseError> {
    let bucket_width = bucket_width_seconds.max(1);
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT (close_time / ?4) * ?4 AS bucket, AVG(close)
         FROM historical_prices
         WHERE symbol = ?1 AND close_time >= ?2 AND close_time <= ?3
         GROUP BY bucket
         ORDER BY bucket",
    )?;
    let rows = statement
        .query_map(params![symbol, from_time, to_time, bucket_width], |row| {
            Ok(Price {
                close_time: row.get(0)?,
                close: row.get(1)?,
                symbol: symbol.to_string(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Read the stored prices of the other symbols at the same close time: the
/// conversions shown by the historical prices table for one row.
pub fn list_conversions(
    database: &Database,
    close_time: i64,
    exclude_symbol: &str,
) -> Result<Vec<Price>, DatabaseError> {
    let connection = database.connection()?;
    let mut statement = connection.prepare(
        "SELECT close_time, close, symbol FROM historical_prices
         WHERE close_time = ?1 AND symbol != ?2
         ORDER BY symbol",
    )?;
    let rows = statement
        .query_map(params![close_time, exclude_symbol], |row| {
            Ok(Price {
                close_time: row.get(0)?,
                close: row.get(1)?,
                symbol: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_then_find_round_trips_and_store_is_idempotent() {
        let database = Database::open_in_memory().unwrap();
        let price = Price {
            symbol: "USD".to_string(),
            close_time: 1_514_768_099,
            close: 13556.15,
        };

        assert!(store_price(&database, &price).unwrap());
        assert!(!store_price(&database, &price).unwrap());

        assert_eq!(
            find_price(&database, "USD", 1_514_768_099).unwrap(),
            Some(price)
        );
        assert_eq!(find_price(&database, "EUR", 1_514_768_099).unwrap(), None);
    }

    fn store(symbol: &str, close_time: i64, close: f64) -> Price {
        Price {
            symbol: symbol.to_string(),
            close_time,
            close,
        }
    }

    fn row(symbol: &str, close_time: i64, close: f64, other_symbols: &[&str]) -> PriceRow {
        PriceRow {
            price: store(symbol, close_time, close),
            other_symbols: other_symbols
                .iter()
                .map(|symbol| symbol.to_string())
                .collect(),
        }
    }

    /// Unsorted, unfiltered list call: the defaults of the shared table
    /// contract (newest close first).
    fn list(database: &Database, symbol: &str, limit: u32, offset: u64) -> Vec<PriceRow> {
        list_prices(
            database,
            &PriceListQuery {
                symbol,
                limit,
                offset,
                ..Default::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn list_prices_pages_newest_first_within_one_symbol() {
        let database = Database::open_in_memory().unwrap();
        for (close_time, close) in [(100, 1.0), (200, 2.0), (300, 3.0), (400, 4.0)] {
            store_price(&database, &store("USD", close_time, close)).unwrap();
        }
        // Another symbol at the same close time must not leak into the page.
        store_price(&database, &store("EUR", 400, 5.0)).unwrap();

        let first = list(&database, "USD", 2, 0);
        let second = list(&database, "USD", 2, 2);
        let beyond = list(&database, "USD", 2, 4);

        assert_eq!(
            first,
            [row("USD", 400, 4.0, &["EUR"]), row("USD", 300, 3.0, &[])]
        );
        assert_eq!(
            second,
            [row("USD", 200, 2.0, &[]), row("USD", 100, 1.0, &[])]
        );
        assert!(beyond.is_empty());
        assert_eq!(count_prices(&database, "USD", 0, 0).unwrap(), 4);
        assert_eq!(count_prices(&database, "EUR", 0, 0).unwrap(), 1);
        assert_eq!(count_prices(&database, "RON", 0, 0).unwrap(), 0);
    }

    #[test]
    fn list_prices_reports_the_other_symbols_at_each_close_time_sorted() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("USD", 100, 1.0)).unwrap();
        store_price(&database, &store("EUR", 100, 0.9)).unwrap();
        store_price(&database, &store("GBP", 100, 0.8)).unwrap();
        // A close time stored in the listed symbol only: no other symbols.
        store_price(&database, &store("USD", 200, 2.0)).unwrap();

        let page = list(&database, "USD", 10, 0);

        assert_eq!(
            page,
            [
                row("USD", 200, 2.0, &[]),
                row("USD", 100, 1.0, &["EUR", "GBP"])
            ]
        );
        // The lookup is symmetric from the other symbols' pages.
        assert_eq!(
            list(&database, "EUR", 10, 0),
            [row("EUR", 100, 0.9, &["GBP", "USD"])]
        );
    }

    #[test]
    fn list_and_count_respect_the_time_bounds() {
        let database = Database::open_in_memory().unwrap();
        for (close_time, close) in [(100, 1.0), (200, 2.0), (300, 3.0), (400, 4.0)] {
            store_price(&database, &store("USD", close_time, close)).unwrap();
        }

        let bounded = list_prices(
            &database,
            &PriceListQuery {
                symbol: "USD",
                limit: 10,
                from_time: 200,
                to_time: 300,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            bounded,
            [row("USD", 300, 3.0, &[]), row("USD", 200, 2.0, &[])]
        );
        assert_eq!(count_prices(&database, "USD", 200, 300).unwrap(), 2);

        // One-sided bounds stay open on the other side.
        assert_eq!(count_prices(&database, "USD", 300, 0).unwrap(), 2);
        assert_eq!(count_prices(&database, "USD", 0, 200).unwrap(), 2);
        // An inverted range matches nothing.
        assert!(list_prices(
            &database,
            &PriceListQuery {
                symbol: "USD",
                limit: 10,
                from_time: 300,
                to_time: 200,
                ..Default::default()
            },
        )
        .unwrap()
        .is_empty());
    }

    #[test]
    fn list_prices_sorts_by_the_allowlisted_columns_in_typed_order() {
        let database = Database::open_in_memory().unwrap();
        // Unsorted storage order on purpose.
        store_price(&database, &store("USD", 200, 10.0)).unwrap();
        store_price(&database, &store("USD", 100, 2.0)).unwrap();
        store_price(&database, &store("USD", 300, 100.0)).unwrap();

        let by_date_asc = list_prices(
            &database,
            &PriceListQuery {
                symbol: "USD",
                limit: 10,
                sort: Some((PriceSortColumn::CloseTime, SortDirection::Asc)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            by_date_asc,
            [
                row("USD", 100, 2.0, &[]),
                row("USD", 200, 10.0, &[]),
                row("USD", 300, 100.0, &[])
            ]
        );

        // Numeric order, not lexicographic: 100.0 > 10.0 > 2.0.
        let by_price_desc = list_prices(
            &database,
            &PriceListQuery {
                symbol: "USD",
                limit: 10,
                sort: Some((PriceSortColumn::Close, SortDirection::Desc)),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            by_price_desc,
            [
                row("USD", 300, 100.0, &[]),
                row("USD", 200, 10.0, &[]),
                row("USD", 100, 2.0, &[])
            ]
        );
    }

    #[test]
    fn sort_keys_parse_from_the_client_facing_allowlist() {
        assert_eq!(
            PriceSortColumn::from_key("date"),
            Some(PriceSortColumn::CloseTime)
        );
        assert_eq!(
            PriceSortColumn::from_key("price"),
            Some(PriceSortColumn::Close)
        );
        assert_eq!(PriceSortColumn::from_key("symbol"), None);
        assert_eq!(PriceSortColumn::from_key("close; DROP TABLE"), None);
    }

    #[test]
    fn list_prices_in_range_reads_oldest_first_for_bulk_sync() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("USD", 300, 3.0)).unwrap();
        store_price(&database, &store("USD", 100, 1.0)).unwrap();
        store_price(&database, &store("USD", 200, 2.0)).unwrap();
        store_price(&database, &store("USD", 900, 9.0)).unwrap();
        store_price(&database, &store("EUR", 200, 1.8)).unwrap();

        assert_eq!(
            list_prices_in_range(&database, "USD", 100, 300).unwrap(),
            [
                store("USD", 100, 1.0),
                store("USD", 200, 2.0),
                store("USD", 300, 3.0)
            ]
        );
        assert!(list_prices_in_range(&database, "USD", 400, 500)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn store_prices_bulk_inserts_in_one_transaction_and_skips_conflicts() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("EUR", 200, 1.8)).unwrap();

        let batch = [
            store("EUR", 100, 1.5),
            store("EUR", 200, 9.9), // conflict: kept as 1.8
            store("EUR", 300, 1.7),
        ];
        let inserted = store_prices_bulk(&database, &batch).unwrap();

        assert_eq!(inserted, 2);
        assert_eq!(
            find_price(&database, "EUR", 200).unwrap(),
            Some(store("EUR", 200, 1.8))
        );
        assert_eq!(count_prices(&database, "EUR", 0, 0).unwrap(), 3);
        // An empty batch commits nothing and succeeds.
        assert_eq!(store_prices_bulk(&database, &[]).unwrap(), 0);
    }

    #[test]
    fn update_price_changes_only_the_target_row() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("USD", 200, 2.0)).unwrap();
        store_price(&database, &store("EUR", 200, 1.8)).unwrap();

        assert!(update_price(&database, &store("USD", 200, 2.5)).unwrap());
        assert_eq!(
            find_price(&database, "USD", 200).unwrap(),
            Some(store("USD", 200, 2.5))
        );
        // The sibling row is untouched.
        assert_eq!(
            find_price(&database, "EUR", 200).unwrap(),
            Some(store("EUR", 200, 1.8))
        );

        // Updating a missing row reports false and inserts nothing.
        assert!(!update_price(&database, &store("USD", 999, 2.5)).unwrap());
        assert_eq!(count_prices(&database, "USD", 0, 0).unwrap(), 1);
    }

    #[test]
    fn price_series_range_spans_the_history_of_one_symbol() {
        let database = Database::open_in_memory().unwrap();
        assert_eq!(price_series_range(&database, "USD").unwrap(), None);

        store_price(&database, &store("USD", 200, 2.0)).unwrap();
        store_price(&database, &store("USD", 400, 4.0)).unwrap();
        store_price(&database, &store("USD", 300, 3.0)).unwrap();
        store_price(&database, &store("EUR", 100, 1.0)).unwrap();

        assert_eq!(
            price_series_range(&database, "USD").unwrap(),
            Some((200, 400))
        );
    }

    #[test]
    fn price_series_averages_fixed_width_buckets_oldest_first() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("USD", 100, 1.0)).unwrap();
        store_price(&database, &store("USD", 140, 3.0)).unwrap();
        store_price(&database, &store("USD", 260, 5.0)).unwrap();
        // Outside the window or the symbol: excluded.
        store_price(&database, &store("USD", 900, 9.0)).unwrap();
        store_price(&database, &store("EUR", 120, 2.0)).unwrap();

        // Bucket width 100: [100,200) averages 1.0 and 3.0, [200,300) holds 5.0.
        let series = price_series(&database, "USD", 100, 300, 100).unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].close_time, 100);
        assert_eq!(series[0].close, 2.0);
        assert_eq!(series[1].close_time, 200);
        assert_eq!(series[1].close, 5.0);
        assert!(series.iter().all(|price| price.symbol == "USD"));

        // A single wide bucket collapses the window into one average.
        let collapsed = price_series(&database, "USD", 100, 300, 10_000).unwrap();
        assert_eq!(collapsed.len(), 1);
        assert_eq!(collapsed[0].close, 3.0);
    }

    #[test]
    fn list_conversions_reads_other_symbols_of_the_same_close_time() {
        let database = Database::open_in_memory().unwrap();
        store_price(&database, &store("USD", 200, 2.0)).unwrap();
        store_price(&database, &store("EUR", 200, 1.8)).unwrap();
        store_price(&database, &store("AUD", 200, 3.0)).unwrap();
        store_price(&database, &store("EUR", 300, 1.9)).unwrap();

        assert_eq!(
            list_conversions(&database, 200, "USD").unwrap(),
            [store("AUD", 200, 3.0), store("EUR", 200, 1.8)]
        );
        assert_eq!(
            list_conversions(&database, 200, "EUR").unwrap(),
            [store("AUD", 200, 3.0), store("USD", 200, 2.0)]
        );
        assert!(list_conversions(&database, 999, "USD").unwrap().is_empty());
    }
}
