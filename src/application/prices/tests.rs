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
        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
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
        Some(66_000.0),
    );
    assert_eq!(
        price_repository::find_price(&database, "USD", bucket)
            .unwrap()
            .map(|stored| stored.close),
        Some(66_000.0),
    );
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
        Some(66_000.0 * 0.9),
    );
    assert_eq!(
        price_repository::find_price(&database, "EUR", bucket)
            .unwrap()
            .map(|stored| stored.close),
        Some(66_000.0 * 0.9),
    );
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
        daily_close_time(BEYOND_COVERAGE - LATEST_PRICE_FALLBACK_SECONDS),
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
            stubs.fx.as_ref(),
        ),),
        Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
    ),);
    assert_eq!(stubs.usd_calls.load(Ordering::SeqCst), 0);
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
            stubs.fx.as_ref(),
        ),),
        Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
    ),);
    assert!(matches!(
        block_on(resolve_price(
            &database,
            "USD",
            0,
            stubs.usd.as_ref(),
            stubs.fx.as_ref()
        ),),
        Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
    ),);
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
    let page = list_price_page(&database, " eur ", 0, 0, &query(0, MAX_PAGE_SIZE + 50)).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);
    assert_eq!(page.items[0].price.close_time, 102);
    let second = list_price_page(&database, "EUR", 0, 0, &query(1, 2)).unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].price.close_time, 100);
    let bounded = list_price_page(&database, "EUR", 101, 102, &query(0, 10)).unwrap();
    assert_eq!(bounded.total, 2);
    assert!(matches!(
        list_price_page(&database, "u$", 0, 0, &query(0, 10)),
        Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
    ),);
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
        [100.0, 10.0, 2.0],
    );
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
    ),);
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
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        price_repository::find_price(&database, "EUR", wednesday)
            .unwrap()
            .map(|price| price.close),
        Some(90.0),
    );
    assert_eq!(
        price_repository::find_price(&database, "EUR", saturday)
            .unwrap()
            .map(|price| price.close),
        Some(80.0),
    );
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
        None,
    );
}
#[test]
fn fx_series_start_date_pads_the_range_for_prior_business_days() {
    assert_eq!(fx_series_start_date("2020-01-04"), "2019-12-28");
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
        Some(50.0),
    );
    assert_eq!(
        price_repository::find_price(&database, "EUR", second)
            .unwrap()
            .map(|price| price.close),
        Some(80.0),
    );
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
        Some(999.0),
    );
    assert_eq!(
        price_repository::find_price(&database, "EUR", second)
            .unwrap()
            .map(|price| price.close),
        Some(40.0),
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
        .is_some_and(|warning| warning.contains("fx offline")),);
}
#[test]
fn sync_prices_bulk_validates_input_before_touching_the_network() {
    let database = Database::open_in_memory().unwrap();
    let (calls, fetch) = stub_fx_series(vec![]);
    assert!(matches!(
        block_on(sync_prices_bulk(&database, "USD", 100, 200, &fetch)),
        Err(PriceServiceError::Price(PriceError::InvalidData(_)))
    ),);
    assert!(matches!(
        block_on(sync_prices_bulk(&database, "EUR", 0, 200, &fetch)),
        Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
    ),);
    assert!(matches!(
        block_on(sync_prices_bulk(&database, "EUR", 300, 200, &fetch)),
        Err(PriceServiceError::Price(PriceError::InvalidData(_)))
    ),);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
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
        ["AUD", "EUR"],
    );
    assert!(matches!(
        list_price_conversions(&database, "USD", 0),
        Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
    ),);
    assert!(matches!(
        list_price_conversions(&database, "u$", 200),
        Err(PriceServiceError::Price(PriceError::InvalidSymbol(_)))
    ),);
}
#[test]
fn price_series_reports_the_full_span_and_downsamples_the_window() {
    let database = Database::open_in_memory().unwrap();
    assert!(price_series(&database, "USD", 0, 0).unwrap().is_none());
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
    assert!(series.points[0].close_time < series.points[1].close_time);
    let zoomed = price_series(&database, "USD", 1_000_000, 1_000_099)
        .unwrap()
        .unwrap();
    assert_eq!(zoomed.points.len(), 100);
    assert_eq!(zoomed.points[0].close_time, 1_000_000);
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
            close: 2.5,
        },
    );
    assert_eq!(
        price_repository::find_price(&database, "USD", 200)
            .unwrap()
            .map(|price| price.close),
        Some(2.5),
    );
    assert!(matches!(
        update_stored_price(&database, "USD", 200, -1.0),
        Err(PriceServiceError::Price(PriceError::InvalidData(_)))
    ),);
    assert!(matches!(
        update_stored_price(&database, "USD", 999, 2.5),
        Err(PriceServiceError::Price(PriceError::NotFound { .. }))
    ),);
    assert!(matches!(
        update_stored_price(&database, "USD", 0, 2.5),
        Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
    ),);
    assert_eq!(
        price_repository::find_price(&database, "USD", 200)
            .unwrap()
            .map(|price| price.close),
        Some(2.5),
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
    |_from: String, _to: String| -> FetchSeriesFuture { Box::pin(async move { Ok(Vec::new()) }) }
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
    let week_ago = now - 7 * civil_time::SECONDS_PER_DAY;
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
    let plan = plan_price_sync(SyncGranularity::Hourly, &coverage, week_ago, now, now);
    assert_eq!(plan.granularity, SyncGranularity::Hourly);
    assert_eq!(plan.granular_from, Some(week_ago));
    let old = civil_time::epoch_seconds(2020, 1, 1, 0, 0, 0);
    let plan = plan_price_sync(
        SyncGranularity::Auto,
        &coverage,
        old,
        old + civil_time::SECONDS_PER_DAY,
        now,
    );
    assert_eq!(plan.granularity, SyncGranularity::Daily);
    assert_eq!(plan.granular_from, None);
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
    let quarter = super::sync::reference_prices(SyncGranularity::FifteenMinutes, rows.clone());
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
        ],
    );
    let hourly = super::sync::reference_prices(SyncGranularity::Hourly, rows);
    assert_eq!(hourly.len(), 2);
    assert_eq!(hourly[0].close_time, start);
    assert_eq!(hourly[1].close_time, start + 3_600);
    assert!(super::sync::reference_prices(SyncGranularity::Daily, vec![(start, 1.0)]).is_empty(),);
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
    assert_eq!(
        price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(3)),)
            .unwrap()
            .map(|price| price.close),
        Some(103.0),
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
        Some(3.0),
    );
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
    price_repository::store_price(&database, &daily_row(2, 999.0)).unwrap();
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
        price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(2)),)
            .unwrap()
            .map(|price| price.close),
        Some(999.0),
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
    ),);
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
    ),);
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
    ),);
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
    ),);
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
            Some(50.0),
        );
    }
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
        price_repository::find_price(&database, USD_SYMBOL, daily_close_time(november_day(5)),)
            .unwrap()
            .map(|price| price.close),
        Some(100.0),
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
    let previous_close = daily_close_time(just_after_midnight - civil_time::SECONDS_PER_DAY);
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
    ),);
    assert!(matches!(
        delete_stored_prices(&database, &[0]),
        Err(PriceServiceError::Price(PriceError::InvalidTimestamp(0)))
    ),);
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
        }),
    );
}
