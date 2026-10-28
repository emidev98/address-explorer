use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::prices::{list_prices_api, price_conversions_api, price_series_api};
use crate::api::settings::get_default_fiat_currency_api;
use crate::components::format::format_fiat;
use crate::components::{
    use_toast, DatePicker, EditPriceDialog, PriceTrendChart, SortableHeader, SyncPricesDialog,
    TablePagination, Toast,
};
use crate::domain::civil_time::{format_date, format_datetime, parse_date};
use crate::domain::price::{Price, PriceSeries, PriceSyncOutcome, PriceTableQuery, USD_SYMBOL};
use crate::domain::table::{SortDirection, TableQuery, DEFAULT_PAGE_SIZE};

/// Rows per page of the historical prices table.
const PAGE_SIZE: u32 = DEFAULT_PAGE_SIZE;

/// Seconds in a day; the inclusive end bound of the date-range filter.
const DAY_SECONDS: i64 = 86_400;

/// Start of the UTC day containing `time`.
fn day_start(time: i64) -> i64 {
    time - time.rem_euclid(DAY_SECONDS)
}

/// End of the UTC day containing `time` (inclusive).
fn day_end(time: i64) -> i64 {
    day_start(time) + DAY_SECONDS - 1
}

/// Reactive state of the bidirectional binding between the date-range
/// calendar and the trend chart: the raw input texts, the applied bounds
/// (a bound of `0` leaves that side open), the requested chart window
/// (`(0, 0)` = the full stored span), the stored span itself, and whether
/// the user has edited the calendar — the guard that keeps the full-range
/// prefill from clobbering dates typed before the first series response
/// arrived. The table page is part of the state because every filter or
/// window change restarts it.
#[derive(Clone, Copy)]
struct DateRangeState {
    from_input: Signal<String>,
    to_input: Signal<String>,
    from_time: Signal<i64>,
    to_time: Signal<i64>,
    touched: Signal<bool>,
    page: Signal<u32>,
    chart_window: Signal<(i64, i64)>,
    full_span: Signal<(i64, i64)>,
    /// One-shot token the Reset action bumps so the chart restores its
    /// visible window even when the requested window does not change.
    zoom_reset: Signal<u64>,
}

/// Fetch state of the other-symbol conversions of one close time, loaded the
/// first time a row's info button is hovered or focused.
#[derive(Debug, Clone, PartialEq)]
enum ConversionState {
    Loading,
    Loaded(Vec<Price>),
    Failed(String),
}

/// Historical Prices screen: the zoomable trend chart of the stored history
/// at the top, and below it the paginated content of the `historical_prices`
/// table for the default fiat currency configured in Settings (USD until
/// changed).
///
/// The table follows the shared pattern of Section 5.11 of CONTRIBUTING.md:
/// backend-driven pagination with an editable page index and backend sorting
/// by the Date and Price columns. The date-range calendar and the trend
/// chart are bound bidirectionally: editing the calendar moves the chart to
/// those boundaries, and zooming the chart rewrites the calendar dates,
/// which also filters the table. Row actions modify the stored price or
/// reveal the already stored conversions, and the "Sync Prices" action opens
/// the dialog that converts a range into another currency or fetches the
/// daily history up to the current day.
#[component]
pub fn Prices() -> Element {
    let mut toast = use_toast();
    let currency = use_resource(move || async move {
        get_default_fiat_currency_api()
            .await
            .map_err(|error| error.to_string())
    });

    let mut page = use_signal(|| 0_u32);
    let mut from_input = use_signal(String::new);
    let mut to_input = use_signal(String::new);
    let from_time = use_signal(|| 0_i64);
    let to_time = use_signal(|| 0_i64);
    let touched = use_signal(|| false);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Desc);
    let mut conversions = use_signal(BTreeMap::<i64, ConversionState>::new);
    let mut edit_target = use_signal(|| Option::<Price>::None);
    let mut sync_open = use_signal(|| false);

    // (0, 0) requests the full stored span. `chart_window` is the requested
    // window driving both the series fetch and — through the mirror effect
    // below — the calendar and the table filter. `applied_window` is the
    // window of the series currently on screen, so the chart never mixes
    // the axis of a new window with the points of the previous one.
    // `full_span` is the stored history's own span, learned from the first
    // series response.
    let chart_window = use_signal(|| (0_i64, 0_i64));
    let mut applied_window = use_signal(|| (0_i64, 0_i64));
    let mut zoom_in_flight = use_signal(|| false);
    let mut full_span = use_signal(|| (0_i64, 0_i64));
    let zoom_reset = use_signal(|| 0_u64);

    let date_filter = DateRangeState {
        from_input,
        to_input,
        from_time,
        to_time,
        touched,
        page,
        chart_window,
        full_span,
        zoom_reset,
    };

    let mut prices = use_resource(move || {
        // Read the reactive inputs synchronously so the resource tracks them.
        let currency_state = currency();
        let current_page = page();
        let from = from_time();
        let to = to_time();
        let sort_key = sort_by();
        let direction = sort_dir();
        async move {
            let symbol = match currency_state {
                Some(Ok(symbol)) => symbol,
                Some(Err(error)) => return Err(error),
                None => return Ok(None),
            };
            let query = PriceTableQuery {
                table: TableQuery {
                    page: current_page,
                    page_size: PAGE_SIZE,
                    sort_by: sort_key,
                    sort_dir: direction,
                },
                symbol,
                from_time: from,
                to_time: to,
            };
            list_prices_api(query)
                .await
                .map(Some)
                .map_err(|error| error.to_string())
        }
    });

    let mut series = use_resource(move || {
        let currency_state = currency();
        let window = chart_window();
        async move {
            let symbol = match currency_state {
                Some(Ok(symbol)) => symbol,
                Some(Err(error)) => return Err(error),
                None => return Ok(None),
            };
            let result = price_series_api(symbol, window.0, window.1)
                .await
                .map_err(|error| error.to_string());
            if let Ok(Some(series)) = &result {
                applied_window.set(window);
                // `first_close_time`/`last_close_time` describe the full
                // stored span regardless of the requested window.
                full_span.set((series.first_close_time, series.last_close_time));
            }
            zoom_in_flight.set(false);
            result
        }
    });

    // The bidirectional binding: the calendar mirrors the chart window, and
    // the table filter follows the same day-aligned boundaries. The effect
    // never reads what it writes, so editing the calendar never re-triggers
    // it; calendar edits flow back in through `apply_date_filter`, which
    // sets the chart window. A (0, 0) window prefills the calendar with the
    // full range — but never over a filter the user already applied.
    use_effect(move || {
        let DateRangeState {
            mut from_input,
            mut to_input,
            mut from_time,
            mut to_time,
            touched,
            mut page,
            chart_window,
            full_span,
            zoom_reset: _,
        } = date_filter;
        let window = chart_window();
        let (span_first, span_last) = full_span();
        if span_first <= 0 {
            return;
        }
        if window == (0, 0) && touched() {
            return;
        }
        let (from, to) = if window == (0, 0) {
            (span_first, span_last)
        } else {
            window
        };
        from_input.set(format_date(from));
        to_input.set(format_date(to));
        from_time.set(day_start(from));
        to_time.set(day_end(to));
        page.set(0);
    });

    let symbol = match currency() {
        Some(Ok(symbol)) => symbol,
        _ => USD_SYMBOL.to_string(),
    };

    rsx! {
        section { id: "prices", class: "md3-screen",
            div { class: "md3-page-header",
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Historical Prices"
                }
                section { class: "md3-date-range md3-date-range-compact",
                    div {
                        label {
                            class: "text-body-small text-on-surface-variant",
                            r#for: "prices-filter-from",
                            "From"
                        }
                        DatePicker {
                            id: "prices-filter-from".to_string(),
                            value: from_input(),
                            aria_label: "From date".to_string(),
                            on_change: move |value: String| {
                                from_input.set(value);
                                apply_date_filter(date_filter, toast);
                            },
                        }
                    }
                    div {
                        label {
                            class: "text-body-small text-on-surface-variant",
                            r#for: "prices-filter-to",
                            "To"
                        }
                        DatePicker {
                            id: "prices-filter-to".to_string(),
                            value: to_input(),
                            aria_label: "To date".to_string(),
                            on_change: move |value: String| {
                                to_input.set(value);
                                apply_date_filter(date_filter, toast);
                            },
                        }
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-tonal",
                        disabled: full_span().0 <= 0,
                        onclick: move |_| reset_date_filter(date_filter),
                        "Reset"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-tonal",
                        onclick: move |_| sync_open.set(true),
                        "Sync Prices"
                    }
                }
            }

            section { class: "md3-chart-card",
                div { class: "md3-chart-header",
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "BTC price trend ({symbol})"
                    }
                }
                div { class: "md3-chart-body",
                    {
                        chart_content(
                            symbol.clone(),
                            series,
                            applied_window,
                            chart_window,
                            zoom_in_flight,
                            zoom_reset,
                        )
                    }
                }
            }

            match prices() {
                None => rsx! {
                    p { class: "text-body-medium text-on-surface-variant", "Loading prices..." }
                },
                Some(Err(error)) => rsx! {
                    p { class: "text-body-medium text-on-surface-variant", "Error loading prices: {error}" }
                },
                Some(Ok(None)) => rsx! {
                    p { class: "text-body-medium text-on-surface-variant", "Loading prices..." }
                },
                Some(Ok(Some(page_data))) if page_data.items.is_empty() => rsx! {
                    p { class: "text-body-medium text-on-surface-variant",
                        "No prices stored for {symbol} matching this filter. Prices in other currencies appear here once they have been resolved or synced."
                    }
                },
                Some(Ok(Some(page_data))) => rsx! {
                    TablePagination {
                        page: page(),
                        page_size: PAGE_SIZE,
                        total: page_data.total,
                        on_page: move |next_page| page.set(next_page),
                    }
                    div { class: "md3-table-container",
                        table { class: "md3-table",
                            thead {
                                tr {
                                    th {}
                                    SortableHeader {
                                        label: "Date".to_string(),
                                        active_direction: (sort_by().as_deref() == Some("date"))
                                            .then_some(sort_dir()),
                                        on_sort: move |_| {
                                            toggle_sort(sort_by, sort_dir, "date", SortDirection::Desc);
                                            page.set(0);
                                        },
                                    }
                                    SortableHeader {
                                        label: "Price".to_string(),
                                        active_direction: (sort_by().as_deref() == Some("price"))
                                            .then_some(sort_dir()),
                                        on_sort: move |_| {
                                            toggle_sort(sort_by, sort_dir, "price", SortDirection::Desc);
                                            page.set(0);
                                        },
                                    }
                                    th { "Other available exchanges" }
                                }
                            }
                            tbody {
                                for price in &page_data.items {
                                    {
                                        let row = price.clone();
                                        let close_time = row.price.close_time;
                                        let state = conversions.read().get(&close_time).cloned();
                                        let hover_symbol = symbol.clone();
                                        let focus_symbol = symbol.clone();
                                        let edit_row = row.price.clone();
                                        let other_exchanges = row.other_symbols.join(", ");
                                        rsx! {
                                            tr {
                                                td {
                                                    div { class: "md3-table-actions",
                                                        button {
                                                            r#type: "button",
                                                            class: "md3-icon-button",
                                                            title: "Modify price",
                                                            aria_label: "Modify the price at {format_datetime(close_time)}",
                                                            onclick: move |_| edit_target.set(Some(edit_row.clone())),
                                                            {icon(ICON_EDIT)}
                                                        }
                                                    }
                                                }
                                                td { "{format_datetime(close_time)}" }
                                                td { "{format_fiat(row.price.close, &row.price.symbol)}" }
                                                td {
                                                    if row.other_symbols.is_empty() {
                                                        "-"
                                                    } else {
                                                        div {
                                                            class: "md3-price-info",
                                                            tabindex: "0",
                                                            onmouseenter: move |_| {
                                                                ensure_conversions(conversions, hover_symbol.clone(), close_time);
                                                            },
                                                            onfocus: move |_| {
                                                                ensure_conversions(conversions, focus_symbol.clone(), close_time);
                                                            },
                                                            span { "{other_exchanges}" }
                                                            div { class: "md3-price-tooltip", role: "tooltip",
                                                                span { class: "text-label-large text-on-surface-variant", "Other conversions" }
                                                                {conversion_tooltip(state)}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }

            if let Some(target) = edit_target() {
                EditPriceDialog {
                    price: target,
                    on_close: move |_| edit_target.set(None),
                    on_saved: move |updated: Price| {
                        edit_target.set(None);
                        toast
                            .success(
                                format!(
                                    "Price updated to {}.",
                                    format_fiat(updated.close, &updated.symbol),
                                ),
                            );
                        prices.restart();
                        series.restart();
                    },
                }
            }

            if sync_open() {
                SyncPricesDialog {
                    initial_from: from_input(),
                    initial_to: to_input(),
                    on_close: move |_| sync_open.set(false),
                    on_synced: move |outcome: PriceSyncOutcome| {
                        // The tooltip cache can hold stale conversions for
                        // the synced close times; drop it so hovers refetch.
                        conversions.write().clear();
                        match &outcome {
                            PriceSyncOutcome::Historical(summary) => {
                                toast
                                    .success(
                                        format!(
                                            "Synced {} prices: {} stored, {} skipped.",
                                            summary.symbol,
                                            summary.stored,
                                            summary.skipped,
                                        ),
                                    );
                            }
                            PriceSyncOutcome::ToToday(summary) => {
                                toast
                                    .success(
                                        format!(
                                            "Synced BTC/USD daily prices: {} stored ({} fetched).",
                                            summary.stored,
                                            summary.fetched,
                                        ),
                                    );
                            }
                        }
                        prices.restart();
                        series.restart();
                    },
                }
            }
        }
    }
}

/// Apply a header click to the sort state: clicking the active column
/// toggles its direction; clicking another column selects it with its
/// default direction (dates and prices read newest/highest first).
fn toggle_sort(
    mut sort_by: Signal<Option<String>>,
    mut sort_dir: Signal<SortDirection>,
    key: &str,
    default_direction: SortDirection,
) {
    if sort_by().as_deref() == Some(key) {
        sort_dir.set(sort_dir().toggled());
    } else {
        sort_by.set(Some(key.to_string()));
        sort_dir.set(default_direction);
    }
}

/// Chart card body: the trend chart itself plus its loading, error, and
/// empty states (the chart component only renders data).
fn chart_content(
    symbol: String,
    series: Resource<Result<Option<PriceSeries>, String>>,
    applied_window: Signal<(i64, i64)>,
    chart_window: Signal<(i64, i64)>,
    zoom_in_flight: Signal<bool>,
    zoom_reset: Signal<u64>,
) -> Element {
    match series() {
        None => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading chart..." }
        },
        Some(Err(error)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Error loading the chart: {error}" }
        },
        Some(Ok(None)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading chart..." }
        },
        Some(Ok(Some(series))) if series.points.is_empty() => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "No prices stored for {symbol} yet."
            }
        },
        Some(Ok(Some(series))) => {
            let (window_start, window_end) = if applied_window() == (0, 0) {
                (series.first_close_time, series.last_close_time)
            } else {
                applied_window()
            };
            rsx! {
                PriceTrendChart {
                    points: series.points.clone(),
                    window_start,
                    window_end,
                    symbol,
                    reset_zoom: zoom_reset(),
                    on_zoom: move |(start_pct, end_pct)| {
                        handle_chart_zoom(
                            chart_window,
                            applied_window,
                            zoom_in_flight,
                            &series,
                            start_pct,
                            end_pct,
                        );
                    },
                    aria_label: "BTC price trend chart".to_string(),
                }
            }
        }
    }
}

/// Map a zoom gesture (axis percentages) onto the window the chart currently
/// displays and request the finer series for it. Events describing the full
/// span are ignored: they fire programmatically after every option replace,
/// and zooming all the way back out is what the calendar's Reset button
/// does.
fn handle_chart_zoom(
    mut chart_window: Signal<(i64, i64)>,
    applied_window: Signal<(i64, i64)>,
    mut zoom_in_flight: Signal<bool>,
    series: &PriceSeries,
    start_pct: f64,
    end_pct: f64,
) {
    if (start_pct <= 0.0 && end_pct >= 100.0) || end_pct <= start_pct || *zoom_in_flight.read() {
        return;
    }

    let current = if applied_window() == (0, 0) {
        (series.first_close_time, series.last_close_time)
    } else {
        applied_window()
    };
    let span = (current.1 - current.0) as f64;
    if span <= 0.0 {
        return;
    }

    let from = current.0 + (span * start_pct / 100.0) as i64;
    let to = current.0 + (span * end_pct / 100.0) as i64;
    if to <= from || (from == current.0 && to == current.1) {
        return;
    }

    zoom_in_flight.set(true);
    chart_window.set((from, to));
}

/// Validate and apply a calendar edit: each side parses independently, an
/// empty side leaves the range open on that end, and the table restarts
/// from the first page. The chart follows the same boundaries (the
/// bidirectional binding): the window is resolved against the stored span,
/// and a filter covering the whole span reopens the chart instead of
/// zooming it. Invalid input is reported in a toast.
fn apply_date_filter(filter: DateRangeState, mut toast: Toast) {
    let DateRangeState {
        from_input,
        to_input,
        mut from_time,
        mut to_time,
        mut touched,
        mut page,
        mut chart_window,
        full_span,
        zoom_reset: _,
    } = filter;
    // The user touched the calendar: the full-range prefill must not
    // override their choice.
    touched.set(true);
    let from_raw = from_input();
    let to_raw = to_input();

    let from = if from_raw.is_empty() {
        Some(0)
    } else {
        parse_date(&from_raw)
    };
    let to = if to_raw.is_empty() {
        Some(0)
    } else {
        // The To date includes its whole day.
        parse_date(&to_raw).map(day_end)
    };

    let (Some(from), Some(to)) = (from, to) else {
        toast.error("Enter dates as YYYY-MM-DD.");
        return;
    };
    if from > 0 && to > 0 && from > to {
        toast.error("The start date must not be after the end date.");
        return;
    }

    from_time.set(from);
    to_time.set(to);
    page.set(0);

    // Move the chart to the same boundaries. An open side resolves against
    // the stored span; until the span is known the chart keeps its current
    // window (it shows the loading or empty state anyway).
    let (span_first, span_last) = full_span();
    if span_first > 0 {
        let window_from = if from > 0 {
            from
        } else {
            day_start(span_first)
        };
        let window_to = if to > 0 { to } else { day_end(span_last) };
        if window_from <= day_start(span_first) && window_to >= day_end(span_last) {
            chart_window.set((0, 0));
        } else {
            chart_window.set((window_from, window_to));
        }
    }
}

/// Restore the full range: the calendar shows the first and last stored
/// prices again, the table restarts from the first page, and the chart
/// reopens to the full span. A no-op until the span is known. The
/// `zoom_reset` bump matters when the requested window already is the full
/// span: a zoom ECharts applied client-side may never have reached Rust, so
/// only the chart itself can restore the visible window.
fn reset_date_filter(filter: DateRangeState) {
    let DateRangeState {
        mut from_input,
        mut to_input,
        mut from_time,
        mut to_time,
        mut page,
        mut chart_window,
        full_span,
        mut zoom_reset,
        ..
    } = filter;
    let (span_first, span_last) = full_span();
    if span_first <= 0 {
        return;
    }
    from_input.set(format_date(span_first));
    to_input.set(format_date(span_last));
    from_time.set(day_start(span_first));
    to_time.set(day_end(span_last));
    page.set(0);
    chart_window.set((0, 0));
    zoom_reset.set(zoom_reset() + 1);
}

/// Tooltip body for the current fetch state of one close time's conversions.
fn conversion_tooltip(state: Option<ConversionState>) -> Element {
    match state {
        Some(ConversionState::Loaded(list)) if !list.is_empty() => rsx! {
            for conversion in &list {
                span { class: "md3-price-tooltip-row text-body-small",
                    "{format_fiat(conversion.close, &conversion.symbol)}"
                }
            }
        },
        Some(ConversionState::Loaded(_)) => rsx! {
            span { class: "text-body-small text-on-surface-variant",
                "No other conversions stored for this close time."
            }
        },
        Some(ConversionState::Failed(error)) => rsx! {
            span { class: "text-body-small text-on-surface-variant",
                "Could not load conversions: {error}"
            }
        },
        _ => rsx! {
            span { class: "text-body-small text-on-surface-variant", "Loading conversions..." }
        },
    }
}

/// Fetch the conversions of one close time the first time they are requested;
/// the result is cached for the lifetime of the screen.
fn ensure_conversions(
    mut conversions: Signal<BTreeMap<i64, ConversionState>>,
    symbol: String,
    close_time: i64,
) {
    if conversions.read().contains_key(&close_time) {
        return;
    }
    conversions
        .write()
        .insert(close_time, ConversionState::Loading);

    spawn(async move {
        let state = match price_conversions_api(symbol, close_time).await {
            Ok(list) => ConversionState::Loaded(list),
            Err(error) => ConversionState::Failed(error.to_string()),
        };
        conversions.write().insert(close_time, state);
    });
}

// Material icon path (24px viewBox): edit.
const ICON_EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";

/// Render one Material icon path as an inline SVG.
fn icon(path: &str) -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "20",
            height: "20",
            fill: "currentColor",
            path { d: "{path}" }
        }
    }
}
