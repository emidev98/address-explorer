//! Flow screen: interoperable filters, the funds-flow Sankey, data-quality
//! warnings and the aggregated evidence table.

use dioxus::prelude::*;

use crate::api::flow::{
    flow_filter_options_api, flow_link_evidence_api, flow_link_table_api, flow_overview_api,
};
use crate::api::prices::latest_price_api;
use crate::components::format::{format_address, format_btc, format_fiat};
use crate::components::{
    use_fiat_currency, use_toast, DatePicker, FlowFiatContext, FundsFlowChart, Kpi, MultiSelect,
    MultiSelectOption, SortableHeader, TablePagination,
};
use crate::domain::civil_time::{format_date, format_datetime, parse_date};
use crate::domain::flow::{
    FlowAddressOption, FlowEvidenceQuery, FlowEvidenceRow, FlowFilterOption, FlowLinkRow,
    FlowLinkTableQuery, FlowQuery, FlowTimeBounds, FlowView, FlowWarningKind,
};
use crate::domain::table::{SortDirection, TableQuery};

const PAGE_SIZE: u32 = 25;
const EVIDENCE_PAGE_SIZE: u32 = 25;
const DAY_SECONDS: i64 = 86_400;

fn day_start(time: i64) -> i64 {
    time - time.rem_euclid(DAY_SECONDS)
}

fn day_end(time: i64) -> i64 {
    day_start(time) + DAY_SECONDS - 1
}

fn flow_span(bounds: &FlowTimeBounds) -> Option<(i64, i64)> {
    let first = [bounds.first_tx_time, bounds.first_ledger_time]
        .into_iter()
        .flatten()
        .min()?;
    let last = [bounds.last_tx_time, bounds.last_ledger_time]
        .into_iter()
        .flatten()
        .max()?;
    Some((first, last))
}

#[component]
pub fn Flow() -> Element {
    let fiat = use_fiat_currency();
    let mut toast = use_toast();
    let mut exchanges = use_signal(Vec::<i64>::new);
    let mut addresses = use_signal(Vec::<String>::new);
    let mut from_input = use_signal(String::new);
    let mut to_input = use_signal(String::new);
    let from_time = use_signal(|| Option::<i64>::None);
    let to_time = use_signal(|| Option::<i64>::None);
    let mut date_initialized = use_signal(|| false);
    let date_touched = use_signal(|| false);
    let mut view = use_signal(|| FlowView::Transactions);
    let mut force_transactions = use_signal(|| false);
    let mut page = use_signal(|| 0_u32);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Desc);
    let mut evidence_link = use_signal(|| Option::<FlowLinkRow>::None);
    let mut evidence_page = use_signal(|| 0_u32);
    let mut evidence_sort_by = use_signal(|| Option::<String>::None);
    let evidence_sort_dir = use_signal(|| SortDirection::Desc);

    let date_state = FlowDateState {
        from_input,
        to_input,
        from_time,
        to_time,
        force_transactions,
        page,
        evidence_link,
        touched: date_touched,
    };

    let options = use_resource(|| async move {
        flow_filter_options_api()
            .await
            .map_err(|error| error.to_string())
    });
    let latest_price = use_resource(move || {
        let symbol = fiat.code();
        async move { latest_price_api(symbol).await.ok().map(|price| price.close) }
    });

    let graph = use_resource(move || {
        let query = current_query(
            exchanges(),
            addresses(),
            from_time(),
            to_time(),
            view(),
            force_transactions(),
        );
        async move {
            flow_overview_api(query)
                .await
                .map_err(|error| error.to_string())
        }
    });

    let links = use_resource(move || {
        let query = current_query(
            exchanges(),
            addresses(),
            from_time(),
            to_time(),
            view(),
            force_transactions(),
        );
        let table = TableQuery {
            page: page(),
            page_size: PAGE_SIZE,
            sort_by: sort_by(),
            sort_dir: sort_dir(),
        };
        async move {
            flow_link_table_api(FlowLinkTableQuery { flow: query, table })
                .await
                .map_err(|error| error.to_string())
        }
    });

    let evidence = use_resource(move || {
        let link = evidence_link();
        let query = current_query(
            exchanges(),
            addresses(),
            from_time(),
            to_time(),
            view(),
            force_transactions(),
        );
        let table = TableQuery {
            page: evidence_page(),
            page_size: EVIDENCE_PAGE_SIZE,
            sort_by: evidence_sort_by(),
            sort_dir: evidence_sort_dir(),
        };
        async move {
            let link = link?;
            Some(
                flow_link_evidence_api(FlowEvidenceQuery {
                    flow: query,
                    source_id: link.source_id.clone(),
                    target_id: link.target_id.clone(),
                    table,
                })
                .await
                .map_err(|error| error.to_string()),
            )
        }
    });

    let options_state = options();
    let options_loading = options_state.is_none();
    let options_error = options_state
        .as_ref()
        .and_then(|state| state.as_ref().err().cloned());
    let filter_options = options_state
        .as_ref()
        .and_then(|state| state.as_ref().ok())
        .cloned()
        .unwrap_or_default();

    let exchange_options = exchange_select_options(&filter_options.exchanges);
    let address_options = address_select_options(&filter_options.addresses);
    let span = flow_span(&filter_options.time_bounds);

    // Prefill the date selectors with the dataset's full available span once
    // the filter options arrive. A user edit or a Clear filters action wins.
    use_effect(move || {
        let Some(Ok(options)) = options() else {
            return;
        };
        if date_initialized() || date_touched() {
            return;
        }
        let Some((first, last)) = flow_span(&options.time_bounds) else {
            return;
        };
        let mut from_input = from_input;
        let mut to_input = to_input;
        let mut from_time = from_time;
        let mut to_time = to_time;
        from_input.set(format_date(first));
        to_input.set(format_date(last));
        from_time.set(Some(day_start(first)));
        to_time.set(Some(day_end(last)));
        date_initialized.set(true);
    });

    let graph_state = graph();
    let graph_loading = graph_state.is_none();
    let graph_error = graph_state
        .as_ref()
        .and_then(|state| state.as_ref().err().cloned());
    let overview = graph_state
        .as_ref()
        .and_then(|state| state.as_ref().ok())
        .cloned();
    let requested_view = view();
    let rendered_view = overview
        .as_ref()
        .map(|overview| overview.rendered_view)
        .unwrap_or(requested_view);
    let symbol = fiat.code();
    let latest_value = latest_price().flatten();
    let fiat_context = latest_value.map(|btc_price| FlowFiatContext {
        symbol: symbol.clone(),
        btc_price,
    });
    let kpi_values: [Option<String>; 3] = match overview.as_ref() {
        Some(overview) => {
            let kpis = &overview.graph.kpis;
            [
                Some(kpis.address_count.to_string()),
                Some(kpis.transaction_count.to_string()),
                Some(format_btc(kpis.current_balance_sats)),
            ]
        }
        None => [None, None, None],
    };
    let circular_excluded_sats = overview.as_ref().and_then(|overview| {
        overview
            .graph
            .warnings
            .iter()
            .find(|warning| warning.kind == FlowWarningKind::ExcludedCircular)
            .map(|warning| warning.value_sats)
    });

    let links_state = links();
    let links_loading = links_state.is_none();
    let links_error = links_state
        .as_ref()
        .and_then(|state| state.as_ref().err().cloned());
    let links_page = links_state
        .as_ref()
        .and_then(|state| state.as_ref().ok())
        .cloned();

    let clear = move |_| {
        clear_filters(
            exchanges,
            addresses,
            from_input,
            to_input,
            from_time,
            to_time,
            span,
            date_initialized,
            date_touched,
            view,
            force_transactions,
            page,
        );
        evidence_link.set(None);
    };

    rsx! {
        section { id: "flow", class: "md3-screen",
            div { class: "md3-page-header",
                div {
                    h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                        "Flow of coins"
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    onclick: clear,
                    "Clear filters"
                }
            }

            div { class: "md3-kpi-grid-three",
                Kpi {
                    title: "Addresses".to_string(),
                    value: kpi_values[0].clone(),
                    loading: graph_loading,
                }
                Kpi {
                    title: "Transactions".to_string(),
                    value: kpi_values[1].clone(),
                    loading: graph_loading,
                }
                Kpi {
                    title: "Balance".to_string(),
                    value: kpi_values[2].clone(),
                    loading: graph_loading,
                }
            }

            section {
                class: "md3-card md3-card-elevated md3-flow-filters",
                aria_label: "Flow filters",
                div { class: "md3-flow-filter-grid",
                    MultiSelect {
                        label: "Exchanges".to_string(),
                        placeholder: "All exchanges".to_string(),
                        options: exchange_options,
                        selected: exchanges().iter().map(i64::to_string).collect(),
                        loading: options_loading,
                        on_change: move |values: Vec<String>| {
                            exchanges.set(values.iter().filter_map(|value| value.parse().ok()).collect());
                            force_transactions.set(false);
                            evidence_link.set(None);
                            page.set(0);
                        },
                    }
                    MultiSelect {
                        label: "Addresses".to_string(),
                        placeholder: "All stored addresses".to_string(),
                        options: address_options,
                        selected: addresses(),
                        loading: options_loading,
                        on_change: move |values: Vec<String>| {
                            addresses.set(values);
                            force_transactions.set(false);
                            evidence_link.set(None);
                            page.set(0);
                        },
                    }
                    div { class: "md3-filter-field",
                        label {
                            class: "text-body-small text-on-surface-variant",
                            r#for: "flow-from",
                            "From"
                        }
                        DatePicker {
                            id: "flow-from".to_string(),
                            value: from_input(),
                            aria_label: "From date".to_string(),
                            on_change: move |value: String| {
                                from_input.set(value);
                                if let Err(error) = apply_flow_date_filter(date_state) {
                                    toast.error(error);
                                }
                            },
                        }
                    }
                    div { class: "md3-filter-field",
                        label {
                            class: "text-body-small text-on-surface-variant",
                            r#for: "flow-to",
                            "To"
                        }
                        DatePicker {
                            id: "flow-to".to_string(),
                            value: to_input(),
                            aria_label: "To date".to_string(),
                            on_change: move |value: String| {
                                to_input.set(value);
                                if let Err(error) = apply_flow_date_filter(date_state) {
                                    toast.error(error);
                                }
                            },
                        }
                    }
                }

                if let Some(error) = options_error {
                    p { class: "text-body-small md3-settings-error",
                        "Could not load filters: {error}"
                    }
                }
            }

            if let Some(overview) = overview.as_ref() {
                if overview.auto_fallback {
                    div { class: "md3-flow-banner md3-flow-banner-warning",
                        div {
                            strong { "Showing the address summary" }
                            p {
                                "This selection is larger than the transaction render budget. The chart is aggregated to stay responsive."
                            }
                        }
                        button {
                            r#type: "button",
                            class: "md3-button-tonal",
                            onclick: move |_| {
                                view.set(FlowView::Transactions);
                                force_transactions.set(true);
                                evidence_link.set(None);
                                page.set(0);
                            },
                            "Show transaction detail anyway"
                        }
                    }
                }
                for warning in overview
                    .graph
                    .warnings
                    .iter()
                    .filter(|warning| {
                        !matches!(
                            warning.kind,
                            FlowWarningKind::AutoFallback
                                | FlowWarningKind::UnmatchedLedger
                                | FlowWarningKind::ExcludedCircular
                        )
                    })
                {
                    div { class: if warning.kind == FlowWarningKind::LargeGraph { "md3-flow-banner md3-flow-banner-warning" } else { "md3-flow-banner" },
                        div {
                            strong { "{warning_title(warning.kind)}" }
                            p { "{warning.message}" }
                        }
                    }
                }
            }

            section {
                class: "md3-chart-card md3-flow-chart-card",
                aria_label: "Funds flow chart",
                div { class: "md3-chart-header",
                    div {
                        h3 { class: "text-title-large text-on-surface", "Funds flow" }
                        p { class: "text-body-small text-on-surface-variant",
                            "{rendered_view.label()} view · origin → destination · BTC"
                            if let Some(price) = latest_value {
                                " · current fiat at {format_fiat(price, &symbol)}/BTC"
                            }
                        }
                        if let Some(excluded_sats) = circular_excluded_sats {
                            p { class: "text-body-small text-on-surface-variant",
                                "{format_btc(excluded_sats)} of circular value was removed from this summary"
                            }
                        }
                    }
                    div {
                        class: "md3-flow-view-toggle",
                        role: "group",
                        aria_label: "Flow node grouping",
                        for candidate in [FlowView::Transactions, FlowView::Addresses] {
                            button {
                                r#type: "button",
                                class: if rendered_view == candidate { "md3-chip md3-chip-selected" } else { "md3-chip" },
                                aria_pressed: (rendered_view == candidate).to_string(),
                                onclick: move |_| {
                                    view.set(candidate);
                                    force_transactions.set(candidate == FlowView::Transactions);
                                    evidence_link.set(None);
                                    page.set(0);
                                },
                                "{candidate.label()}"
                            }
                        }
                    }
                }
                div { class: "md3-flow-legend", aria_label: "Chart legend",
                    {legend_item("Transaction", "md3-flow-legend-transaction")}
                    {legend_item("Exchange", "md3-flow-legend-exchange")}
                    {legend_item("Wallet", "md3-flow-legend-wallet")}
                    {legend_item("Address", "md3-flow-legend-address")}
                    {legend_item("External", "md3-flow-legend-external")}
                    {legend_item("Unlinked", "md3-flow-legend-unlinked")}
                }
                div { class: "md3-chart-body md3-flow-chart-body",
                    if graph_loading {
                        p { class: "text-body-medium text-on-surface-variant",
                            "Building funds flow…"
                        }
                    } else if let Some(error) = graph_error {
                        p { class: "text-body-medium md3-flow-error",
                            "Could not build funds flow: {error}"
                        }
                    } else if let Some(overview) = overview.as_ref() {
                        if overview.graph.nodes.is_empty() || overview.graph.links.is_empty() {
                            p { class: "text-body-medium text-on-surface-variant",
                                "No funds flow matches these filters."
                            }
                        } else {
                            FundsFlowChart {
                                graph: overview.graph.clone(),
                                fiat: fiat_context.clone(),
                                aria_label: format!(
                                    "Funds flow: {} transactions, {} links, {} BTC moved.",
                                    overview.graph.stats.transaction_count,
                                    overview.graph.stats.link_count,
                                    format_btc(overview.graph.kpis.gross_output_sats),
                                ),
                            }
                        }
                    }
                }
            }

            if let Some(overview) = overview.as_ref() {
                section {
                    class: "md3-card md3-card-elevated md3-flow-quality-card",
                    aria_label: "Flow quality",
                    h3 { class: "text-title-large text-on-surface", "Flow quality" }
                    div { class: "md3-flow-quality-list",
                        {
                            quality_item(
                                "Origins / destinations",
                                format!(
                                    "{} / {}",
                                    overview.graph.kpis.origin_count,
                                    overview.graph.kpis.destination_count,
                                ),
                            )
                        }
                        {quality_item("Fees", format_btc(overview.graph.kpis.fee_sats))}
                        {
                            quality_item(
                                "Incomplete addresses",
                                overview.graph.kpis.incomplete_address_count.to_string(),
                            )
                        }
                    }
                }
            }

            section { class: "md3-flow-evidence-card", aria_label: "Flow evidence",
                div { class: "md3-chart-header",
                    div {
                        h3 { class: "text-title-large text-on-surface", "Flow evidence" }
                        p { class: "text-body-small text-on-surface-variant",
                            "Aggregated links for the {rendered_view.label().to_lowercase()} view. Select a row to inspect its contributing transactions."
                        }
                    }
                }
                if let Some(error) = links_error {
                    p { class: "text-body-medium md3-flow-error",
                        "Could not load flow evidence: {error}"
                    }
                } else if links_loading {
                    p { class: "text-body-medium text-on-surface-variant", "Loading flow evidence…" }
                } else if let Some(page_data) = links_page.as_ref() {
                    if page_data.items.is_empty() {
                        p { class: "text-body-medium text-on-surface-variant",
                            "No flow links match these filters."
                        }
                    } else {
                        TablePagination {
                            page: page(),
                            page_size: PAGE_SIZE,
                            total: page_data.total,
                            on_page: move |next| page.set(next),
                        }
                        div { class: "md3-table-container",
                            table { class: "md3-table md3-flow-table",
                                thead {
                                    tr {
                                        SortableHeader {
                                            label: "Source".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("source")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "source", SortDirection::Asc);
                                                page.set(0);
                                            },
                                        }
                                        SortableHeader {
                                            label: "Target".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("target")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "target", SortDirection::Asc);
                                                page.set(0);
                                            },
                                        }
                                        SortableHeader {
                                            label: "BTC".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("value")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "value", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        th { "Share" }
                                        SortableHeader {
                                            label: "Txs".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("txs")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "txs", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        SortableHeader {
                                            label: "First".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("first_tx")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "first_tx", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        SortableHeader {
                                            label: "Last".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("last_tx")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "last_tx", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        th { "Type" }
                                    }
                                }
                                tbody {
                                    for row in &page_data.items {
                                        {
                                            let evidence_row = row.clone();
                                            rsx! {
                                                tr {
                                                    class: "md3-flow-link-row",
                                                    onclick: move |_| {
                                                        evidence_link.set(Some(evidence_row.clone()));
                                                        evidence_page.set(0);
                                                        evidence_sort_by.set(None);
                                                    },
                                                    td {
                                                        div { class: "md3-flow-label-cell",
                                                            {flow_dot(row.source_color.as_deref())}
                                                            span { "{row.source_label}" }
                                                        }
                                                    }
                                                    td {
                                                        div { class: "md3-flow-label-cell",
                                                            {flow_dot(row.target_color.as_deref())}
                                                            span { "{row.target_label}" }
                                                        }
                                                    }
                                                    td { "{format_btc(row.value_sats)}" }
                                                    td { "{format_share(row.share)}" }
                                                    td { "{row.event_count}" }
                                                    td { "{row.first_time.map(format_short_datetime).unwrap_or_else(|| \"-\".to_string())}" }
                                                    td { "{row.last_time.map(format_short_datetime).unwrap_or_else(|| \"-\".to_string())}" }
                                                    td {
                                                        span { class: "md3-flow-kind-chip", "{link_kind_label(row.kind)}" }
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
        }

        if let Some(link) = evidence_link() {
            FlowEvidenceDialog {
                link,
                evidence: evidence(),
                page: evidence_page,
                sort_by: evidence_sort_by,
                sort_dir: evidence_sort_dir,
                on_close: move |_| evidence_link.set(None),
            }
        }
    }
}

/// Date-filter inputs and the reactive state they apply to. Mirrors the
/// Historical Prices date range behaviour: an empty side leaves that end open,
/// the To date includes its whole day, and invalid input is rejected before
/// the graph or table is refreshed.
#[derive(Clone, Copy)]
struct FlowDateState {
    from_input: Signal<String>,
    to_input: Signal<String>,
    from_time: Signal<Option<i64>>,
    to_time: Signal<Option<i64>>,
    force_transactions: Signal<bool>,
    page: Signal<u32>,
    evidence_link: Signal<Option<FlowLinkRow>>,
    touched: Signal<bool>,
}

fn parse_flow_date_bounds(
    from_raw: &str,
    to_raw: &str,
) -> Result<(Option<i64>, Option<i64>), String> {
    let from = if from_raw.trim().is_empty() {
        Some(None)
    } else {
        parse_date(from_raw).map(Some)
    };
    let to = if to_raw.trim().is_empty() {
        Some(None)
    } else {
        parse_date(to_raw).map(|value| Some(day_end(value)))
    };
    let (Some(from), Some(to)) = (from, to) else {
        return Err("Enter dates as YYYY-MM-DD.".to_string());
    };
    if let (Some(from), Some(to)) = (from, to) {
        if from > to {
            return Err("The start date must not be after the end date.".to_string());
        }
    }
    Ok((from, to))
}

fn apply_flow_date_filter(state: FlowDateState) -> Result<(), String> {
    let FlowDateState {
        from_input,
        to_input,
        mut from_time,
        mut to_time,
        mut force_transactions,
        mut page,
        mut evidence_link,
        mut touched,
    } = state;
    let (from, to) = parse_flow_date_bounds(&from_input(), &to_input())?;

    touched.set(true);
    from_time.set(from);
    to_time.set(to);
    force_transactions.set(false);
    evidence_link.set(None);
    page.set(0);
    Ok(())
}

// The query is a flat projection of the filter signals; keeping the
// arguments explicit makes every signal dependency visible to `use_resource`.
fn current_query(
    exchange_ids: Vec<i64>,
    addresses: Vec<String>,
    from_time: Option<i64>,
    to_time: Option<i64>,
    view: FlowView,
    force_transactions: bool,
) -> FlowQuery {
    FlowQuery {
        exchange_ids,
        addresses,
        from_time,
        to_time,
        view,
        force_transactions,
    }
}

fn exchange_select_options(options: &[FlowFilterOption]) -> Vec<MultiSelectOption> {
    options
        .iter()
        .map(|option| MultiSelectOption {
            id: option.id.to_string(),
            label: option.name.clone(),
            description: Some(format!(
                "{} address{} · {} ledger account{}",
                option.address_count,
                if option.address_count == 1 { "" } else { "es" },
                option.account_count,
                if option.account_count == 1 { "" } else { "s" },
            )),
            group: "Exchanges".to_string(),
            color: option.brand_color.clone(),
            count: Some(option.account_count),
            search_text: option.name.clone(),
        })
        .collect()
}

fn address_select_options(options: &[FlowAddressOption]) -> Vec<MultiSelectOption> {
    options
        .iter()
        .map(|option| {
            let owner = option
                .exchange_name
                .as_deref()
                .map(|name| format!("Exchange · {name}"))
                .or_else(|| {
                    option
                        .wallet_name
                        .as_deref()
                        .map(|name| format!("Wallet · {name}"))
                })
                .unwrap_or_else(|| "Unclassified".to_string());
            MultiSelectOption {
                id: option.address.clone(),
                label: option
                    .description
                    .clone()
                    .unwrap_or_else(|| format_address(&option.address)),
                description: Some(format!(
                    "{} · {} · {} tx{} · {owner}",
                    option.address,
                    format_btc(option.balance_sats),
                    option.confirmed_tx_count,
                    if option.confirmed_tx_count == 1 {
                        ""
                    } else {
                        "s"
                    }
                )),
                group: "Addresses".to_string(),
                color: option
                    .exchange_color
                    .clone()
                    .or_else(|| option.wallet_color.clone()),
                count: Some(option.confirmed_tx_count),
                search_text: format!(
                    "{} {} {} {}",
                    option.address,
                    option.description.clone().unwrap_or_default(),
                    option.exchange_name.clone().unwrap_or_default(),
                    option.wallet_name.clone().unwrap_or_default()
                ),
            }
        })
        .collect()
}

// Clearing is one user action touching every independent filter signal; the
// explicit arguments keep ownership and reset semantics obvious.
#[allow(clippy::too_many_arguments)]
fn clear_filters(
    mut exchanges: Signal<Vec<i64>>,
    mut addresses: Signal<Vec<String>>,
    mut from_input: Signal<String>,
    mut to_input: Signal<String>,
    mut from_time: Signal<Option<i64>>,
    mut to_time: Signal<Option<i64>>,
    span: Option<(i64, i64)>,
    mut date_initialized: Signal<bool>,
    mut date_touched: Signal<bool>,
    mut view: Signal<FlowView>,
    mut force_transactions: Signal<bool>,
    mut page: Signal<u32>,
) {
    exchanges.set(Vec::new());
    addresses.set(Vec::new());
    if let Some((first, last)) = span {
        from_input.set(format_date(first));
        to_input.set(format_date(last));
        from_time.set(Some(day_start(first)));
        to_time.set(Some(day_end(last)));
        date_initialized.set(true);
    } else {
        from_input.set(String::new());
        to_input.set(String::new());
        from_time.set(None);
        to_time.set(None);
        date_initialized.set(false);
    }
    date_touched.set(false);
    view.set(FlowView::Transactions);
    force_transactions.set(false);
    page.set(0);
}

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

#[component]
fn FlowEvidenceDialog(
    link: FlowLinkRow,
    evidence: Option<Option<Result<crate::domain::table::TablePage<FlowEvidenceRow>, String>>>,
    page: Signal<u32>,
    sort_by: Signal<Option<String>>,
    sort_dir: Signal<SortDirection>,
    on_close: EventHandler<()>,
) -> Element {
    let mut page = page;
    let state = evidence.flatten();
    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| on_close.call(()),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog md3-flow-evidence-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_label: "Flow evidence",
                onclick: move |event| event.stop_propagation(),
                div { class: "md3-flow-evidence-header",
                    div {
                        h2 { class: "text-headline-small text-on-surface", "Flow evidence" }
                        p { class: "text-body-small text-on-surface-variant",
                            "{link.source_label} → {link.target_label} · {format_btc(link.value_sats)}"
                        }
                    }
                    button {
                        r#type: "button",
                        class: "md3-icon-button",
                        aria_label: "Close evidence",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }
                match state {
                    None => rsx! {
                        p { class: "text-body-medium text-on-surface-variant", "Loading evidence…" }
                    },
                    Some(Err(error)) => rsx! {
                        p { class: "text-body-medium md3-flow-error", "Could not load evidence: {error}" }
                    },
                    Some(Ok(page_data)) if page_data.items.is_empty() => rsx! {
                        p { class: "text-body-medium text-on-surface-variant", "No evidence rows available." }
                    },
                    Some(Ok(page_data)) => rsx! {
                        TablePagination {
                            page: page(),
                            page_size: EVIDENCE_PAGE_SIZE,
                            total: page_data.total,
                            on_page: move |next| page.set(next),
                        }
                        div { class: "md3-table-container",
                            table { class: "md3-table",
                                thead {
                                    tr {
                                        SortableHeader {
                                            label: "Date".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("date")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "date", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        SortableHeader {
                                            label: "BTC".to_string(),
                                            active_direction: (sort_by().as_deref() == Some("value")).then_some(sort_dir()),
                                            on_sort: move |_| {
                                                toggle_sort(sort_by, sort_dir, "value", SortDirection::Desc);
                                                page.set(0);
                                            },
                                        }
                                        th { "Transaction / ledger" }
                                        th { "Evidence" }
                                    }
                                }
                                tbody {
                                    for row in &page_data.items {
                                        tr {
                                            td {
                                                "{row.timestamp.map(format_short_datetime).unwrap_or_else(|| \"-\".to_string())}"
                                            }
                                            td { "{format_btc(row.value_sats)}" }
                                            td { "{row.txid.as_deref().unwrap_or(\"-\")}" }
                                            td { "{row.description}" }
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                }
            }
        }
    }
}

fn warning_title(kind: FlowWarningKind) -> &'static str {
    match kind {
        FlowWarningKind::IncompleteHistory => "Incomplete history",
        FlowWarningKind::UnmatchedLedger => "Unmatched exchange ledger",
        FlowWarningKind::AutoFallback => "Address summary",
        FlowWarningKind::ExcludedCircular => "Circular value excluded",
        FlowWarningKind::LargeGraph => "Large transaction graph",
    }
}

fn quality_item(label: &str, value: String) -> Element {
    rsx! {
        div { class: "md3-flow-quality-item",
            span { class: "text-body-medium text-on-surface-variant", "{label}" }
            strong { class: "text-title-small text-on-surface", "{value}" }
        }
    }
}

fn legend_item(label: &str, class: &str) -> Element {
    rsx! {
        span { class: "md3-flow-legend-item",
            span { class: "md3-flow-legend-swatch {class}", aria_hidden: "true" }
            span { "{label}" }
        }
    }
}

fn flow_dot(color: Option<&str>) -> Element {
    match color {
        Some(color) => rsx! {
            span {
                class: "md3-filter-dot",
                style: "background-color: {color};",
                aria_hidden: "true",
            }
        },
        None => rsx! {},
    }
}

fn link_kind_label(kind: crate::domain::flow::FlowLinkKind) -> &'static str {
    match kind {
        crate::domain::flow::FlowLinkKind::OnChain => "On-chain",
        crate::domain::flow::FlowLinkKind::LedgerUnlinked => "Ledger",
        crate::domain::flow::FlowLinkKind::Aggregated => "Aggregated",
    }
}

fn format_share(share: f64) -> String {
    if share.is_finite() {
        format!("{share:.2}%")
    } else {
        "-".to_string()
    }
}

fn format_short_datetime(timestamp: i64) -> String {
    let formatted = format_datetime(timestamp);
    formatted.get(..16).unwrap_or(&formatted).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_span_uses_the_outer_bounds_of_transactions_and_ledger() {
        let bounds = FlowTimeBounds {
            first_tx_time: Some(200),
            last_tx_time: Some(400),
            first_ledger_time: Some(100),
            last_ledger_time: Some(300),
        };
        assert_eq!(flow_span(&bounds), Some((100, 400)));
        assert_eq!(flow_span(&FlowTimeBounds::default()), None);
    }

    #[test]
    fn date_bounds_include_the_whole_to_day_and_allow_open_sides() {
        let start = parse_date("2026-01-01").unwrap();
        let end = parse_date("2026-01-31").unwrap();
        assert_eq!(
            parse_flow_date_bounds("2026-01-01", "2026-01-31").unwrap(),
            (Some(start), Some(end + 86_399))
        );
        assert_eq!(
            parse_flow_date_bounds("2026-01-01", "").unwrap(),
            (Some(start), None)
        );
        assert_eq!(
            parse_flow_date_bounds("", "2026-01-31").unwrap(),
            (None, Some(end + 86_399))
        );
        assert_eq!(parse_flow_date_bounds("", "").unwrap(), (None, None));
    }

    #[test]
    fn date_bounds_reject_invalid_and_inverted_ranges() {
        assert!(parse_flow_date_bounds("not-a-date", "").is_err());
        assert!(parse_flow_date_bounds("2026-02-01", "2026-01-01").is_err());
    }

    #[test]
    fn share_formatting_handles_non_finite_values() {
        assert_eq!(format_share(12.5), "12.50%");
        assert_eq!(format_share(f64::NAN), "-");
    }
}
