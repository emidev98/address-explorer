//! Funds Flow screen: interoperable filters, the funds-flow Sankey, data-quality
//! warnings and the aggregated evidence table.
use crate::api::flow::{
    flow_filter_options_api, flow_link_evidence_api, flow_link_table_api, flow_overview_api,
};
use crate::components::format::{format_address, format_btc, format_fiat};
use crate::components::{
    use_fiat_currency, use_latest_price, use_table_state, BalanceKpi, DatePicker, FlowFiatContext,
    FundsFlowChart, Kpi, Modal, MultiSelect, MultiSelectOption, PageHeader, SortableHeader,
    TablePagination, TableState,
};
use crate::domain::civil_time::{self, end_of_day, start_of_day};
use crate::domain::date_selection::{DateSelection, IsoDate};
use crate::domain::flow::{
    FlowAddressOption, FlowEvidenceQuery, FlowEvidenceRow, FlowFilterOption, FlowLinkRow,
    FlowLinkTableQuery, FlowQuery, FlowTimeBounds, FlowView, FlowWarningKind,
};
use crate::domain::table::{SortDirection, DEFAULT_PAGE_SIZE};
use dioxus::prelude::*;
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
pub fn FundsFlow() -> Element {
    let fiat = use_fiat_currency();
    let mut exchanges = use_signal(Vec::<i64>::new);
    let mut addresses = use_signal(Vec::<String>::new);
    let date_selection = use_signal(DateSelection::default);
    let from_time = use_signal(|| Option::<i64>::None);
    let to_time = use_signal(|| Option::<i64>::None);
    let mut date_initialized = use_signal(|| false);
    let date_touched = use_signal(|| false);
    let mut view = use_signal(|| FlowView::Transactions);
    let mut force_transactions = use_signal(|| false);
    let mut link_table = use_table_state(SortDirection::Desc);
    let mut evidence_table = use_table_state(SortDirection::Desc);
    let mut evidence_link = use_signal(|| Option::<FlowLinkRow>::None);
    let date_state = FlowDateState {
        selection: date_selection,
        from_time,
        to_time,
        force_transactions,
        table: link_table,
        evidence_link,
        touched: date_touched,
    };
    let options = use_resource(|| async move {
        flow_filter_options_api()
            .await
            .map_err(|error| error.to_string())
    });
    let latest_price = use_latest_price();
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
        let table = link_table.query(DEFAULT_PAGE_SIZE);
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
        let table = evidence_table.query(DEFAULT_PAGE_SIZE);
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
        let mut selection = date_selection;
        let mut from_time = from_time;
        let mut to_time = to_time;
        selection.set(DateSelection::from_epoch_bounds(Some(first), Some(last)));
        from_time.set(Some(start_of_day(first)));
        to_time.set(Some(end_of_day(last)));
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
    let kpi_values: [Option<String>; 2] = match overview.as_ref() {
        Some(overview) => {
            let kpis = &overview.graph.kpis;
            [
                Some(kpis.address_count.to_string()),
                Some(kpis.transaction_count.to_string()),
            ]
        }
        None => [None, None],
    };
    let balance_sats = overview
        .as_ref()
        .map(|overview| overview.graph.kpis.current_balance_sats);
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
    let date_bounds =
        span.map(|(first, last)| (IsoDate::from_epoch(first), IsoDate::from_epoch(last)));
    let clear = move |_| {
        clear_filters(
            exchanges,
            addresses,
            date_selection,
            from_time,
            to_time,
            span,
            date_initialized,
            date_touched,
            view,
            force_transactions,
            link_table,
        );
        evidence_link.set(None);
    };
    rsx! {
        section { id: "flow", class: "md3-screen",
            PageHeader { title: "Funds Flow"
                                                                        .to_string(),
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
                BalanceKpi {
                    balance_sats,
                    latest_price: latest_value,
                    currency: symbol.clone(),
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
                            link_table.reset_page();
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
                            link_table.reset_page();
                        },
                    }
                    DatePicker {
                        id: "flow-date-range"
                                                                                                        .to_string(),
                        label: "Date range".to_string(),
                        value: date_selection(),
                        min_date: date_bounds.as_ref().map(|(min, _)| *min),
                        max_date: date_bounds
                                                                                                        .as_ref().map(| (_, max) | * max),
                        popup_align_end: true,
                        on_change: move |value: DateSelection| {
                            let mut selection = date_selection;
                            selection.set(value);
                            apply_flow_date_filter(date_state);
                        },
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
                                link_table.reset_page();
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
                                    link_table.reset_page();
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
            if let Some(error) = links_error {
                p { class: "text-body-medium md3-flow-error", "Could not load flow evidence: {error}" }
            } else if links_loading {
                p { class: "text-body-medium text-on-surface-variant", "Loading flow evidence…" }
            } else if let Some(page_data) = links_page.as_ref() {
                if page_data.items.is_empty() {
                    p { class: "text-body-medium text-on-surface-variant",
                        "No flow links match these filters."
                    }
                } else {
                    div { class: "md3-table-container",
                        table {
                            class: "md3-table md3-flow-table",
                            aria_label: "Flow evidence",
                            thead {
                                tr {
                                    SortableHeader {
                                        label: "Source".to_string(),
                                        active_direction: link_table
                                                                                                                                                                        .direction_for("source"),
                                        on_sort: move |_| {
                                            link_table.sort("source", SortDirection::Asc);
                                        },
                                    }
                                    SortableHeader {
                                        label: "Target".to_string(),
                                        active_direction: link_table.direction_for("target"),
                                        on_sort: move |_| {
                                            link_table.sort("target", SortDirection::Asc);
                                        },
                                    }
                                    SortableHeader {
                                        label: "BTC".to_string(),
                                        active_direction: link_table.direction_for("value"),
                                        on_sort: move |_| {
                                            link_table.sort("value", SortDirection::Desc);
                                        },
                                    }
                                    th { "Share" }
                                    SortableHeader {
                                        label: "Txs".to_string(),
                                        active_direction: link_table
                                                                                                                                                                        .direction_for("txs"),
                                        on_sort: move |_| {
                                            link_table.sort("txs", SortDirection::Desc);
                                        },
                                    }
                                    SortableHeader {
                                        label: "First".to_string(),
                                        active_direction: link_table.direction_for("first_tx"),
                                        on_sort: move |_| {
                                            link_table.sort("first_tx", SortDirection::Desc);
                                        },
                                    }
                                    SortableHeader {
                                        label: "Last".to_string(),
                                        active_direction: link_table.direction_for("last_tx"),
                                        on_sort: move |_| {
                                            link_table.sort("last_tx", SortDirection::Desc);
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
                                                    evidence_table.page.set(0);
                                                    evidence_table.sort_by.set(None);
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
                                                td { "{row.first_time.map(civil_time::format_short_datetime).unwrap_or_else(|| \"-\".to_string())}" }
                                                td { "{row.last_time.map(civil_time::format_short_datetime).unwrap_or_else(|| \"-\".to_string())}" }
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
                    TablePagination {
                        page: link_table.page(),
                        page_size: DEFAULT_PAGE_SIZE,
                        total: page_data.total,
                        on_page: move | next | link_table
                                                                                                        .page.set(next),
                    }
                }
            }
        }
        if let Some(link) = evidence_link() {
            FlowEvidenceDialog {
                link,
                evidence: evidence(),
                table: evidence_table,
                on_close: move | _ |
                                                                        evidence_link.set(None),
            }
        }
    }
}
/// Date-filter selection and the reactive state it applies to. Mirrors the
/// Historical Prices date range behaviour: an empty side leaves that end open,
/// the To date includes its whole day, and the picker only emits validated
/// ranges.
#[derive(Clone, Copy)]
struct FlowDateState {
    selection: Signal<DateSelection>,
    from_time: Signal<Option<i64>>,
    to_time: Signal<Option<i64>>,
    force_transactions: Signal<bool>,
    table: TableState,
    evidence_link: Signal<Option<FlowLinkRow>>,
    touched: Signal<bool>,
}
fn apply_flow_date_filter(state: FlowDateState) {
    let FlowDateState {
        selection,
        mut from_time,
        mut to_time,
        mut force_transactions,
        table,
        mut evidence_link,
        mut touched,
    } = state;
    let (from, to) = selection().epoch_bounds();
    touched.set(true);
    from_time.set(from);
    to_time.set(to);
    force_transactions.set(false);
    evidence_link.set(None);
    table.reset_page();
}
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
        .filter(|option| option.address_count > 0 || option.account_count > 0)
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
        .filter(|option| option.confirmed_tx_count > 0)
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
                    },
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
                    option.wallet_name.clone().unwrap_or_default(),
                ),
            }
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
fn clear_filters(
    mut exchanges: Signal<Vec<i64>>,
    mut addresses: Signal<Vec<String>>,
    mut selection: Signal<DateSelection>,
    mut from_time: Signal<Option<i64>>,
    mut to_time: Signal<Option<i64>>,
    span: Option<(i64, i64)>,
    mut date_initialized: Signal<bool>,
    mut date_touched: Signal<bool>,
    mut view: Signal<FlowView>,
    mut force_transactions: Signal<bool>,
    table: TableState,
) {
    exchanges.set(Vec::new());
    addresses.set(Vec::new());
    if let Some((first, last)) = span {
        selection.set(DateSelection::from_epoch_bounds(Some(first), Some(last)));
        from_time.set(Some(start_of_day(first)));
        to_time.set(Some(end_of_day(last)));
        date_initialized.set(true);
    } else {
        selection.set(DateSelection::default());
        from_time.set(None);
        to_time.set(None);
        date_initialized.set(false);
    }
    date_touched.set(false);
    view.set(FlowView::Transactions);
    force_transactions.set(false);
    table.reset_page();
}
#[component]
fn FlowEvidenceDialog(
    link: FlowLinkRow,
    evidence: Option<Option<Result<crate::domain::table::TablePage<FlowEvidenceRow>, String>>>,
    table: TableState,
    on_close: EventHandler<()>,
) -> Element {
    let state = evidence.flatten();
    rsx! {
        Modal {
            aria_label: "Flow evidence".to_string(),
            class: "md3-flow-evidence-dialog".to_string(),
            on_close: move |_| on_close.call(()),
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
                    div { class: "md3-table-container",
                        table { class: "md3-table",
                            thead {
                                tr {
                                    SortableHeader {
                                        label: "Date".to_string(),
                                        active_direction: table.direction_for("date"),
                                        on_sort: move |_| {
                                            table.sort("date", SortDirection::Desc);
                                        },
                                    }
                                    SortableHeader {
                                        label: "BTC"
                                                                                                                                                                        .to_string(),
                                        active_direction: table.direction_for("value"),
                                        on_sort: move |_| {
                                            table.sort("value", SortDirection::Desc);
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
                                            "{row.timestamp.map(civil_time::format_short_datetime).unwrap_or_else(|| \"-\".to_string())}"
                                        }
                                        td { "{format_btc(row.value_sats)}" }
                                        td { "{row.txid.as_deref().unwrap_or(\"-\")}" }
                                        td { "{row.description}" }
                                    }
                                }
                            }
                        }
                    }
                    TablePagination {
                        page: table.page(),
                        page_size: DEFAULT_PAGE_SIZE,
                        total: page_data.total,
                        on_page: move |next| table.page.set(next),
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
        Some(color) => {
            rsx! {
                span {
                    class: "md3-filter-dot",
                    style: "background-color: {color};",
                    aria_hidden: "true",
                }
            }
        }
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::date_selection::{DateBounds, DateSelectionMode};
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
        let start = IsoDate::parse("2026-01-01").unwrap();
        let end = IsoDate::parse("2026-01-31").unwrap();
        assert_eq!(
            DateSelection::new(Some(start), Some(end)).epoch_bounds(),
            (Some(start.epoch_start()), Some(end.epoch_end())),
        );
        assert_eq!(
            DateSelection::new(Some(start), None).epoch_bounds(),
            (Some(start.epoch_start()), None),
        );
        assert_eq!(
            DateSelection::new(None, Some(end)).epoch_bounds(),
            (None, Some(end.epoch_end())),
        );
        assert_eq!(DateSelection::default().epoch_bounds(), (None, None));
    }
    #[test]
    fn input_dates_are_strictly_parsed() {
        assert!(IsoDate::parse("not-a-date").is_err());
        assert!(IsoDate::parse("2026-02-01").is_ok());
        assert_eq!(
            DateSelection::parse_input("2026-02-01 – 2026-01-01", DateSelectionMode::Range,)
                .unwrap()
                .validate(DateSelectionMode::Range, &DateBounds::default()),
            Err(crate::domain::date_selection::DateSelectionError::Order),
        );
    }
    #[test]
    fn share_formatting_handles_non_finite_values() {
        assert_eq!(format_share(12.5), "12.50%");
        assert_eq!(format_share(f64::NAN), "-");
    }
    fn exchange_option(id: i64, address_count: i64, account_count: i64) -> FlowFilterOption {
        FlowFilterOption {
            id,
            name: format!("Exchange {id}"),
            address_count,
            account_count,
            ..Default::default()
        }
    }
    fn address_option(address: &str, confirmed_tx_count: i64) -> FlowAddressOption {
        FlowAddressOption {
            address: address.to_string(),
            confirmed_tx_count,
            ..Default::default()
        }
    }
    fn option_ids(options: &[MultiSelectOption]) -> Vec<&str> {
        options.iter().map(|option| option.id.as_str()).collect()
    }
    #[test]
    fn exchange_options_skip_providers_without_addresses_and_accounts() {
        let options = exchange_select_options(&[
            exchange_option(1, 3, 0),
            exchange_option(2, 0, 4),
            exchange_option(3, 0, 0),
        ]);
        assert_eq!(option_ids(&options), ["1", "2"]);
    }
    #[test]
    fn address_options_skip_addresses_without_confirmed_transactions() {
        let options = address_select_options(&[
            address_option("bc1qactive", 7),
            address_option("bc1qidle", 0),
        ]);
        assert_eq!(option_ids(&options), ["bc1qactive"]);
    }
}
