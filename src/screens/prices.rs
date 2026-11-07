use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::prices::{
    delete_prices_api, ensure_currency_history_api, list_prices_api, price_conversions_api,
    price_series_api,
};
use crate::api::settings::get_exchange_settings_api;
use crate::components::format::format_fiat;
use crate::components::{
    use_fiat_currency, use_table_selection, use_toast, ConfirmDialog, DatePicker, EditPriceDialog,
    PriceTrendChart, SortableHeader, SyncPricesDialog, TablePagination, TableSelection,
    TableSelectionCell, TableSelectionHeader, Toast,
};
use crate::domain::civil_time::format_datetime;
use crate::domain::date_selection::{DateSelection, IsoDate};
use crate::domain::price::{
    CurrencyHistorySummary, Price, PricePage, PriceSeries, PriceSyncSummary, PriceTableQuery,
    USD_SYMBOL,
};
use crate::domain::table::{SortDirection, TableQuery, DEFAULT_PAGE_SIZE};

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
/// arrived. The table page and row selection are part of the state because
/// every filter or window change restarts the page and clears the selection.
#[derive(Clone, Copy)]
struct DateRangeState {
    selection: Signal<DateSelection>,
    from_time: Signal<i64>,
    to_time: Signal<i64>,
    touched: Signal<bool>,
    page: Signal<u32>,
    chart_window: Signal<(i64, i64)>,
    full_span: Signal<(i64, i64)>,
    table_selection: TableSelection<i64>,
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
/// changed). Opening the screen prepares the stored USD history in that
/// currency first, so the table and chart show the same default everywhere.
///
/// The table follows the shared pattern of Section 5.11 of CONTRIBUTING.md:
/// backend-driven pagination with an editable page index and backend sorting
/// by the Date and Price columns. The date-range calendar and the trend
/// chart are bound bidirectionally: editing the calendar moves the chart to
/// those boundaries, and zooming the chart rewrites the calendar dates,
/// which also filters the table. Row actions modify or delete the stored
/// price or reveal the already stored conversions, and the checkbox column
/// supports deleting several price snapshots at once. The "Sync Prices"
/// action opens the unified dialog that refreshes the BTC/USD history and
/// converts it into the selected currency for a date range.
#[component]
pub fn Prices() -> Element {
    let mut toast = use_toast();
    let fiat = use_fiat_currency();

    // The table and chart only ever show stored rows. Before querying them,
    // make sure the selected currency has been prepared from the stored USD
    // history; the server call is a cheap no-op once that has happened.
    let mut prepared = use_resource(move || {
        let symbol = fiat.code();
        async move {
            if symbol == USD_SYMBOL {
                return Ok(CurrencyHistorySummary {
                    symbol,
                    source_rows: 0,
                    stored: 0,
                    skipped: 0,
                    warning: None,
                });
            }
            ensure_currency_history_api(symbol)
                .await
                .map_err(|error| error.to_string())
        }
    });

    let mut page = use_signal(|| 0_u32);
    let date_selection = use_signal(DateSelection::default);
    let from_time = use_signal(|| 0_i64);
    let to_time = use_signal(|| 0_i64);
    let touched = use_signal(|| false);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Desc);
    let mut conversions = use_signal(BTreeMap::<i64, ConversionState>::new);
    let mut edit_target = use_signal(|| Option::<Price>::None);
    let mut sync_open = use_signal(|| false);
    let mut selection = use_table_selection::<i64>();
    let mut delete_target = use_signal(|| Option::<Vec<i64>>::None);
    let deleting = use_signal(|| false);
    let mut confirm_destructive = use_signal(|| true);

    let settings = use_resource(|| async move { get_exchange_settings_api().await.ok() });
    use_effect(move || {
        if let Some(Some(settings)) = settings() {
            confirm_destructive.set(settings.confirm_destructive_actions);
        }
    });

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
        selection: date_selection,
        from_time,
        to_time,
        touched,
        page,
        chart_window,
        full_span,
        table_selection: selection,
        zoom_reset,
    };

    let mut prices = use_resource(move || {
        // Read the reactive inputs synchronously so the resource tracks them.
        let ready = prepared().is_some();
        let symbol = fiat.code();
        let current_page = page();
        let from = from_time();
        let to = to_time();
        let sort_key = sort_by();
        let direction = sort_dir();
        async move {
            if !ready {
                return Ok(None);
            }
            let query = PriceTableQuery {
                table: TableQuery {
                    page: current_page,
                    page_size: DEFAULT_PAGE_SIZE,
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
        let ready = prepared().is_some();
        let symbol = fiat.code();
        let window = chart_window();
        async move {
            if !ready {
                zoom_in_flight.set(false);
                return Ok(None);
            }
            let result = price_series_api(symbol, window.0, window.1)
                .await
                .map_err(|error| error.to_string());
            if let Ok(Some(series)) = &result {
                applied_window.set(window);
                // `first_close_time`/`last_close_time` describe the full
                // stored span regardless of the requested window.
                full_span.set((series.first_close_time, series.last_close_time));
            } else if matches!(&result, Ok(None)) {
                // Deleting every stored row leaves no span to filter against.
                applied_window.set((0, 0));
                full_span.set((0, 0));
            }
            zoom_in_flight.set(false);
            result
        }
    });

    // Checked rows belong to the currency that was on display when they were
    // selected; switching currencies replaces every row, so drop the stale
    // selection instead of letting it target rows that are no longer shown.
    let selection_symbol = fiat.code();
    use_effect(use_reactive((&selection_symbol,), move |(_symbol,)| {
        selection.clear();
    }));

    // A deletion can empty the current page while earlier pages still have
    // rows; step back until the page has content again.
    use_effect(move || {
        if let Some(Ok(Some(data))) = prices() {
            if data.items.is_empty() && page() > 0 {
                page.set(page() - 1);
                selection.clear();
            }
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
            mut selection,
            mut from_time,
            mut to_time,
            touched,
            mut page,
            chart_window,
            full_span,
            mut table_selection,
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
        table_selection.clear();
        selection.set(DateSelection::from_epoch_bounds(Some(from), Some(to)));
        from_time.set(day_start(from));
        to_time.set(day_end(to));
        page.set(0);
    });

    let symbol = fiat.code();
    let preparation = prepared();
    let preparing = preparation.is_none();
    let preparation_warning = match &preparation {
        Some(Ok(summary)) => summary.warning.clone(),
        Some(Err(error)) => Some(format!(
            "Could not prepare {symbol} price history: {error}. Stored prices are shown; use Sync Prices to retry."
        )),
        None => None,
    };
    let sort_direction_for = |key: &str| (sort_by().as_deref() == Some(key)).then_some(sort_dir());
    let stored_span = full_span();
    let date_bounds = (stored_span.0 > 0).then(|| {
        (
            IsoDate::from_epoch(stored_span.0),
            IsoDate::from_epoch(stored_span.1),
        )
    });

    rsx! {
        section { id: "prices", class: "md3-screen",
            div { class: "md3-page-header",
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Historical Prices"
                }
                section { class: "md3-date-filter-bar",
                    DatePicker {
                        id: "prices-date-range".to_string(),
                        label: "Date range".to_string(),
                        value: date_selection(),
                        min_date: date_bounds.as_ref().map(|(min, _)| *min),
                        max_date: date_bounds.as_ref().map(|(_, max)| *max),
                        popup_align_end: true,
                        on_change: move |value: DateSelection| {
                            let mut selection = date_selection;
                            selection.set(value);
                            apply_date_filter(date_filter);
                        },
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

            if preparing {
                p {
                    class: "text-body-small text-on-surface-variant",
                    role: "status",
                    aria_live: "polite",
                    "Preparing {symbol} price history..."
                }
            }
            if let Some(warning) = preparation_warning {
                p { class: "text-body-small md3-exchange-warning", "{warning}" }
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
                            preparing,
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
                Some(Ok(Some(page_data))) => {
                    let page_ids = page_data
                        .items
                        .iter()
                        .map(|row| row.price.close_time)
                        .collect::<Vec<_>>();
                    let selected_snapshot = selection.snapshot();
                    let selected_count = selection.len();
                    let all_selected = selection.all_selected(&page_ids);
                    let selectable_ids = page_ids.clone();
                    let delete_action_label = if selected_count == 0 {
                        "Select prices to delete".to_string()
                    } else {
                        format!(
                            "Delete {selected_count} {}",
                            if selected_count == 1 {
                                "price snapshot"
                            } else {
                                "price snapshots"
                            },
                        )
                    };
                    rsx! {
                        div { class: "md3-table-container",
                            table { class: "md3-table",
                                thead {
                                    tr {
                                        TableSelectionHeader {
                                            all_selected,
                                            indeterminate: selection.some_selected(&page_ids) && !all_selected,
                                            select_all_label: "Select all prices on this page".to_string(),
                                            disabled: page_ids.is_empty() || deleting(),
                                            on_toggle_all: move |checked| {
                                                selection.set_many(&selectable_ids, checked);
                                            },
                                            button {
                                                r#type: "button",
                                                class: "md3-icon-button md3-icon-button-danger",
                                                disabled: selection.is_empty() || deleting(),
                                                title: "{delete_action_label}",
                                                aria_label: "{delete_action_label}",
                                                onclick: move |_| {
                                                    let close_times = selection.selected_ids();
                                                    if close_times.is_empty() {
                                                        return;
                                                    }
                                                    if confirm_destructive() {
                                                        delete_target.set(Some(close_times));
                                                    } else {
                                                        remove_prices(
                                                            toast,
                                                            deleting,
                                                            selection,
                                                            prices,
                                                            series,
                                                            conversions,
                                                            close_times,
                                                        );
                                                    }
                                                },
                                                {icon(ICON_DELETE)}
                                            }
                                        }
                                        SortableHeader {
                                            label: "Date".to_string(),
                                            active_direction: sort_direction_for("date"),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "date", SortDirection::Desc);
                                                page.set(0);
                                                selection.clear();
                                            },
                                        }
                                        SortableHeader {
                                            label: format!("Price ({symbol})"),
                                            active_direction: sort_direction_for("price"),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "price", SortDirection::Desc);
                                                page.set(0);
                                                selection.clear();
                                            },
                                        }
                                        th { "Other available currencies" }
                                    }
                                }
                                tbody {
                                    for price in &page_data.items {
                                        {
                                            let row = price.clone();
                                            let close_time = row.price.close_time;
                                            let row_selected = selected_snapshot.contains(&close_time);
                                            let state = conversions.read().get(&close_time).cloned();
                                            let hover_symbol = symbol.clone();
                                            let focus_symbol = symbol.clone();
                                            let edit_row = row.price.clone();
                                            let other_exchanges = row.other_symbols.join(", ");
                                            rsx! {
                                                tr { class: if row_selected { "md3-table-row-selected" } else { "" },
                                                    TableSelectionCell {
                                                        checked: row_selected,
                                                        label: format!("Select every stored price at {}", format_datetime(close_time)),
                                                        on_change: move |checked| {
                                                            selection.set(close_time, checked);
                                                        },
                                                    }
                                                    td {
                                                        div { class: "md3-table-actions",
                                                            button {
                                                                r#type: "button",
                                                                class: "md3-icon-button md3-icon-button-danger",
                                                                title: "Delete price snapshot",
                                                                aria_label: "Delete every stored price at {format_datetime(close_time)}",
                                                                disabled: deleting(),
                                                                onclick: move |_| {
                                                                    let close_times = vec![close_time];
                                                                    if confirm_destructive() {
                                                                        delete_target.set(Some(close_times));
                                                                    } else {
                                                                        remove_prices(
                                                                            toast,
                                                                            deleting,
                                                                            selection,
                                                                            prices,
                                                                            series,
                                                                            conversions,
                                                                            close_times,
                                                                        );
                                                                    }
                                                                },
                                                                {icon(ICON_DELETE)}
                                                            }
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
                        TablePagination {
                            page: page(),
                            page_size: DEFAULT_PAGE_SIZE,
                            total: page_data.total,
                            on_page: move |next_page| {
                                page.set(next_page);
                                selection.clear();
                            },
                        }
                    }
                }
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

            if let Some(close_times) = delete_target() {
                {
                    let count = close_times.len();
                    let (title, message) = if count == 1 {
                        (
                            "Delete price snapshot".to_string(),
                            format!(
                                "Delete all stored prices at {}? This removes every currency at that close time and cannot be undone.",
                                format_datetime(close_times[0]),
                            ),
                        )
                    } else {
                        (
                            "Delete price snapshots".to_string(),
                            format!(
                                "Delete all stored prices at {count} selected close times? This removes every stored currency at those times and cannot be undone.",
                            ),
                        )
                    };
                    rsx! {
                        ConfirmDialog {
                            title,
                            message,
                            confirm_text: "Delete".to_string(),
                            cancel_text: "Cancel".to_string(),
                            on_cancel: move |_| delete_target.set(None),
                            on_confirm: move |_| {
                                let close_times = close_times.clone();
                                delete_target.set(None);
                                remove_prices(
                                    toast,
                                    deleting,
                                    selection,
                                    prices,
                                    series,
                                    conversions,
                                    close_times,
                                );
                            },
                        }
                    }
                }
            }

            if sync_open() {
                SyncPricesDialog {
                    on_close: move |_| sync_open.set(false),
                    on_synced: move |summary: PriceSyncSummary| {
                        // The tooltip cache can hold stale conversions for
                        // the synced close times; drop it so hovers refetch.
                        conversions.write().clear();
                        selection.clear();
                        let mut rows = summary.daily_stored + summary.granular_stored;
                        if let Some(conversion) = &summary.conversion {
                            rows += conversion.stored;
                        }
                        toast.success(format!("Synced {rows} price rows for {}.", summary.symbol));
                        prepared.restart();
                        prices.restart();
                        series.restart();
                    },
                }
            }
        }
    }
}

/// Delete the selected price snapshots, refresh the table and chart, and
/// report the outcome in a toast.
///
/// A close time removes every currency stored at that moment, so the tooltip
/// cache entries for those times are dropped as well. The preparation
/// resource is intentionally not restarted: deletion is a direct edit of the
/// stored table, not a gap for the conversion workflow to fill.
fn remove_prices(
    mut toast: Toast,
    mut deleting: Signal<bool>,
    mut selection: TableSelection<i64>,
    mut prices: Resource<Result<Option<PricePage>, String>>,
    mut series: Resource<Result<Option<PriceSeries>, String>>,
    mut conversions: Signal<BTreeMap<i64, ConversionState>>,
    close_times: Vec<i64>,
) {
    if close_times.is_empty() {
        return;
    }
    deleting.set(true);
    spawn(async move {
        let count = close_times.len();
        match delete_prices_api(close_times.clone()).await {
            Ok(deleted) => {
                if deleted == 0 {
                    toast.error("No stored prices were found to delete.");
                } else if count == 1 {
                    toast.success(format!(
                        "Deleted all stored prices at {}.",
                        format_datetime(close_times[0]),
                    ));
                } else {
                    toast.success(format!(
                        "Deleted {deleted} price rows across {count} close times.",
                    ));
                }
                selection.clear();
                {
                    let mut cache = conversions.write();
                    for close_time in &close_times {
                        cache.remove(close_time);
                    }
                }
                prices.restart();
                series.restart();
            }
            Err(error) => toast.error(format!("Could not delete the prices: {error}")),
        }
        deleting.set(false);
    });
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
    preparing: bool,
) -> Element {
    match series() {
        None => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading chart..." }
        },
        Some(Err(error)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Error loading the chart: {error}" }
        },
        Some(Ok(None)) if preparing => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading chart..." }
        },
        Some(Ok(None)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "No prices stored for {symbol} yet."
            }
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

/// Validate and apply a calendar edit: an empty side leaves the range open on
/// that end, and the table restarts from the first page. The chart follows the
/// same boundaries (the bidirectional binding): the window is resolved against
/// the stored span, and a filter covering the whole span reopens the chart
/// instead of zooming it. The picker only emits validated values.
fn apply_date_filter(filter: DateRangeState) {
    let DateRangeState {
        selection,
        mut from_time,
        mut to_time,
        mut touched,
        mut page,
        mut chart_window,
        full_span,
        mut table_selection,
        zoom_reset: _,
    } = filter;
    // The user touched the calendar: the full-range prefill must not
    // override their choice.
    touched.set(true);
    let (from, to) = selection().epoch_bounds();
    // The table query uses 0 for an open side.
    let from = from.unwrap_or(0);
    let to = to.unwrap_or(0);

    from_time.set(from);
    to_time.set(to);
    page.set(0);
    table_selection.clear();

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
        mut selection,
        mut from_time,
        mut to_time,
        mut page,
        mut chart_window,
        full_span,
        mut table_selection,
        mut zoom_reset,
        ..
    } = filter;
    let (span_first, span_last) = full_span();
    if span_first <= 0 {
        return;
    }
    selection.set(DateSelection::from_epoch_bounds(
        Some(span_first),
        Some(span_last),
    ));
    from_time.set(day_start(span_first));
    to_time.set(day_end(span_last));
    page.set(0);
    table_selection.clear();
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

// Material icon paths (24px viewBox): edit and delete.
const ICON_EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";
const ICON_DELETE: &str =
    "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";

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
