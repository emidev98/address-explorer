//! Exchange detail screen: ledger transaction table, inspector, reconciliation,
//! CSV append, and manual on-chain linking.

use dioxus::prelude::*;

use crate::api::exchanges::{
    delete_exchange_transaction_api, get_exchange_api, get_exchange_transaction_api,
    list_exchange_transactions_api, reconcile_exchange_api,
};
use crate::api::settings::get_exchange_settings_api;
use crate::app::Route;
use crate::components::format::{format_btc, format_fiat};
use crate::components::{
    use_toast, ConfirmDialog, ExchangeInspector, ExchangeLinkDialog, ImportDataDialog, Kpi,
    SortableHeader, TablePagination, Toast,
};
use crate::domain::civil_time;
use crate::domain::exchange::csv::is_bitcoin_currency;
use crate::domain::exchange::{
    ExchangeDetail as ExchangeDetailData, ExchangeTransaction, ExchangeTransactionTableQuery,
    ImportDataSummary,
};
use crate::domain::table::{SortDirection, TableQuery};

const PAGE_SIZE: u32 = 25;

#[component]
pub fn ExchangeDetail(
    exchange_id: i64,
    #[props(default)] transaction_id: Option<String>,
) -> Element {
    let mut toast = use_toast();
    let navigator = use_navigator();
    let mut page = use_signal(|| 0_u32);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Desc);
    let mut editor_open = use_signal(|| false);
    let mut editor_transaction = use_signal(|| Option::<ExchangeTransaction>::None);
    let mut loaded_query = use_signal(|| Option::<String>::None);
    let mut show_import_data = use_signal(|| false);
    let mut link_target = use_signal(|| Option::<ExchangeTransaction>::None);
    let mut delete_target = use_signal(|| Option::<ExchangeTransaction>::None);
    let mut reconciling = use_signal(|| false);
    let mut confirm_destructive = use_signal(|| true);

    let settings = use_resource(|| async move { get_exchange_settings_api().await.ok() });
    use_effect(move || {
        if let Some(Some(settings)) = settings() {
            confirm_destructive.set(settings.confirm_destructive_actions);
        }
    });

    let mut detail = use_resource(move || async move {
        get_exchange_api(exchange_id)
            .await
            .map_err(|error| error.to_string())
    });
    let mut transactions = use_resource(move || {
        let current_page = page();
        let sort_key = sort_by();
        let direction = sort_dir();
        async move {
            list_exchange_transactions_api(ExchangeTransactionTableQuery {
                exchange_id,
                table: TableQuery {
                    page: current_page,
                    page_size: PAGE_SIZE,
                    sort_by: sort_key,
                    sort_dir: direction,
                },
            })
            .await
            .map_err(|error| error.to_string())
        }
    });

    // Open a deep-linked transaction once per query value. The prop is
    // bridged through use_reactive so a route change reruns this effect,
    // while editor state changes do not close the panel.
    use_effect(use_reactive((&transaction_id,), move |(query,)| {
        let Some(raw_id) = query else {
            if editor_transaction.peek().is_some() {
                editor_open.set(false);
                editor_transaction.set(None);
                loaded_query.set(None);
            }
            return;
        };
        if loaded_query
            .peek()
            .as_ref()
            .is_some_and(|value| value == &raw_id)
        {
            return;
        }
        let Ok(id) = raw_id.parse::<i64>() else {
            return;
        };
        loaded_query.set(Some(raw_id.clone()));
        spawn(async move {
            match get_exchange_transaction_api(exchange_id, id).await {
                Ok(transaction) => {
                    editor_transaction.set(Some(transaction));
                    editor_open.set(true);
                }
                Err(error) => {
                    toast.error(format!("Could not load the selected transaction: {error}"));
                    navigate_to_detail(navigator, exchange_id, None);
                }
            }
        });
    }));

    let state = detail();
    let account = match &state {
        Some(Ok(detail)) => Some(detail.account.clone()),
        _ => None,
    };
    let kpis = match &state {
        Some(Ok(detail)) => Some(detail.kpis.clone()),
        _ => None,
    };
    let conversion_warning = state
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|detail| detail.conversion_warning.clone());
    let loading = state.is_none();

    let close_editor = use_callback(move |_: ()| {
        editor_open.set(false);
        editor_transaction.set(None);
        loaded_query.set(None);
        navigate_to_detail(navigator, exchange_id, None);
    });

    let table_content = match &transactions() {
        None => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading transactions..." }
        },
        Some(Err(error)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "Error loading transactions: {error}"
            }
        },
        Some(Ok(page_data)) if page_data.items.is_empty() => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "No transactions imported yet. Import a CSV or add a transaction manually."
            }
        },
        Some(Ok(page_data)) => rsx! {
            TablePagination {
                page: page(),
                page_size: PAGE_SIZE,
                total: page_data.total,
                on_page: move |next| page.set(next),
            }
            div { class: "md3-table-container",
                table { class: "md3-table md3-exchange-transactions-table",
                    thead {
                        tr {
                            th {}
                            SortableHeader {
                                label: "Date".to_string(),
                                active_direction: (sort_by().as_deref() == Some("date")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "date", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Type".to_string(),
                                active_direction: (sort_by().as_deref() == Some("type")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "type", SortDirection::Asc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Amount".to_string(),
                                active_direction: (sort_by().as_deref() == Some("amount")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "amount", SortDirection::Desc); page.set(0); },
                            }
                            th { "Fiat" }
                            SortableHeader {
                                label: "Fee".to_string(),
                                active_direction: (sort_by().as_deref() == Some("fee")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "fee", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "On-chain".to_string(),
                                active_direction: (sort_by().as_deref() == Some("on_chain")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "on_chain", SortDirection::Asc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Exchange TX Hash".to_string(),
                                active_direction: (sort_by().as_deref() == Some("tx_hash")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "tx_hash", SortDirection::Asc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Description".to_string(),
                                active_direction: (sort_by().as_deref() == Some("description")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "description", SortDirection::Asc); page.set(0); },
                            }
                        }
                    }
                    tbody {
                        for transaction in &page_data.items {
                            {
                                let row = transaction.clone();
                                let edit_row = row.clone();
                                let edit_action_row = row.clone();
                                let delete_row = row.clone();
                                let link_row = row.clone();
                                let selected = editor_transaction().as_ref().map(|row| row.id) == Some(row.id);
                                let linkable = matches!(
                                    row.tx_type,
                                    crate::domain::exchange::ExchangeTxType::Deposit
                                    | crate::domain::exchange::ExchangeTxType::Withdrawal
                                );
                                let linked = row.on_chain_link.is_some() || row.matched_tx_id.is_some();
                                let on_chain = row
                                    .on_chain_link
                                    .as_ref()
                                    .map(|link| link.txid.clone())
                                    .or_else(|| row.matched_tx_id.clone());
                                rsx! {
                                    tr {
                                        class: if selected { "md3-exchange-selected-row" } else { "" },
                                        onclick: move |_| {
                                            editor_transaction.set(Some(edit_row.clone()));
                                            editor_open.set(true);
                                            loaded_query.set(Some(edit_row.id.to_string()));
                                            navigate_to_detail(navigator, exchange_id, Some(edit_row.id));
                                        },
                                        td {
                                            div { class: "md3-table-actions",
                                                button {
                                                    r#type: "button",
                                                    class: "md3-icon-button",
                                                    title: "Edit transaction",
                                                    aria_label: "Edit transaction",
                                                    onclick: move |event| {
                                                        event.stop_propagation();
                                                        editor_transaction.set(Some(edit_action_row.clone()));
                                                        editor_open.set(true);
                                                        loaded_query.set(Some(edit_action_row.id.to_string()));
                                                        navigate_to_detail(navigator, exchange_id, Some(edit_action_row.id));
                                                    },
                                                    {icon(ICON_EDIT)}
                                                }
                                                button {
                                                    r#type: "button",
                                                    class: "md3-icon-button md3-icon-button-danger",
                                                    title: "Remove transaction",
                                                    aria_label: "Remove transaction",
                                                    onclick: move |event| {
                                                        event.stop_propagation();
                                                        if editor_transaction()
                                                            .as_ref()
                                                            .is_some_and(|row| row.id == delete_row.id)
                                                        {
                                                            close_editor.call(());
                                                        }
                                                        if confirm_destructive() {
                                                            delete_target.set(Some(delete_row.clone()));
                                                        } else {
                                                            remove_transaction(toast, detail, transactions, exchange_id, delete_row.id);
                                                        }
                                                    },
                                                    {icon(ICON_DELETE)}
                                                }
                                            }
                                        }
                                        td { "{civil_time::format_datetime(row.timestamp)}" }
                                        td {
                                            button {
                                                r#type: "button",
                                                class: "md3-exchange-type-cell",
                                                disabled: !linkable,
                                                title: if linkable { if linked { "View on-chain link" } else { "Link on-chain transaction" } } else { "Only deposits and withdrawals can be linked" },
                                                onclick: move |event| {
                                                    event.stop_propagation();
                                                    if linkable {
                                                        link_target.set(Some(link_row.clone()));
                                                    }
                                                },
                                                span { class: if linked { "md3-exchange-chip md3-exchange-chip-success" } else { "md3-exchange-chip" },
                                                    "{row.tx_type.as_str()}"
                                                }
                                                if linkable {
                                                    span { class: "md3-exchange-link-icon", "↗" }
                                                }
                                            }
                                        }
                                        td {
                                            span { class: if row.amount_sats < 0 { "md3-exchange-amount-negative" } else { "md3-exchange-amount-positive" },
                                                "{format_btc(row.amount_sats)}"
                                            }
                                        }
                                        td {
                                            if let Some(amount) = row.converted_fiat_amount {
                                                "{format_fiat(amount, row.converted_fiat_currency.as_deref().unwrap_or(\"\"))}"
                                            } else {
                                                "-"
                                            }
                                        }
                                        td { "{format_fee(row.fee, row.fee_currency.as_deref())}" }
                                        td { "{on_chain.as_deref().map(format_tx_hash).unwrap_or_else(|| \"-\".to_string())}" }
                                        td { "{row.tx_hash.as_deref().map(format_tx_hash).unwrap_or_else(|| \"-\".to_string())}" }
                                        td { "{row.description.as_deref().unwrap_or(\"-\")}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
    };

    if loading {
        return rsx! {
            section { class: "md3-screen",
                p { class: "text-body-medium text-on-surface-variant", "Loading exchange..." }
            }
        };
    }
    let Some(account) = account else {
        return rsx! {
            section { class: "md3-screen",
                Link { to: Route::Exchanges {}, class: "md3-button-text", "← Back to exchanges" }
                section { class: "md3-card md3-card-elevated",
                    h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                        "Exchange not found"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "The selected exchange account is not available locally."
                    }
                }
            }
        };
    };

    let kpi = kpis.unwrap_or_default();
    let currency = kpi.fiat_currency.clone();
    let add_open = editor_open() && editor_transaction().is_none();
    let inspector_key = editor_transaction()
        .as_ref()
        .map(|row| row.id.to_string())
        .unwrap_or_else(|| "new".to_string());

    rsx! {
        section { id: "exchange-detail", class: "md3-screen md3-exchange-detail",
            div { class: "md3-exchange-detail-header",
                div {
                    Link {
                        to: Route::Exchanges {},
                        class: "md3-exchange-back-link",
                        "← Exchanges"
                    }
                    div { class: "md3-exchange-title-row",
                        h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                            "{account.name}"
                        }
                        if let Some(provider) = &account.provider_name {
                            span { class: "md3-exchange-provider", "{provider}" }
                        }
                    }
                }
                div { class: "md3-exchange-toolbar",
                    button {
                        r#type: "button",
                        class: "md3-button-tonal",
                        onclick: move |_| {
                            editor_transaction.set(None);
                            editor_open.set(true);
                            loaded_query.set(None);
                            navigate_to_detail(navigator, exchange_id, None);
                        },
                        "+ Add transaction"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-tonal",
                        onclick: move |_| show_import_data.set(true),
                        "Import Data"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *reconciling.read(),
                        onclick: move |_| {
                            reconciling.set(true);
                            spawn(async move {
                                match reconcile_exchange_api(exchange_id).await {
                                    Ok(()) => toast.success("Exchange reconciliation complete."),
                                    Err(error) => {
                                        toast.error(format!("Could not reconcile this exchange: {error}"))
                                    }
                                }
                                reconciling.set(false);
                                detail.restart();
                                transactions.restart();
                            });
                        },
                        if *reconciling.read() {
                            "Reconciling..."
                        } else {
                            "Run reconciliation"
                        }
                    }
                }
            }

            if let Some(warning) = conversion_warning {
                div { class: "md3-exchange-warning-banner", "{warning}" }
            }

            div { class: "md3-kpi-grid md3-kpi-grid-five",
                Kpi {
                    title: "BTC bought / fiat spent".to_string(),
                    value: Some(
                        format!(
                            "{} / {}",
                            format_btc(kpi.bought_sats),
                            format_fiat(kpi.fiat_spent, &currency),
                        ),
                    ),
                }
                Kpi {
                    title: "BTC sold / fiat received".to_string(),
                    value: Some(
                        format!(
                            "{} / {}",
                            format_btc(kpi.sold_sats),
                            format_fiat(kpi.fiat_received, &currency),
                        ),
                    ),
                }
                Kpi {
                    title: "Withdrawn / deposited".to_string(),
                    value: Some(
                        format!(
                            "{} / {}",
                            format_btc(kpi.withdrawn_sats),
                            format_btc(kpi.deposited_sats),
                        ),
                    ),
                }
                Kpi {
                    title: "BTC fees".to_string(),
                    value: Some(format_btc(kpi.btc_fees_sats)),
                }
                Kpi {
                    title: "Unmatched".to_string(),
                    value: Some(kpi.unmatched_count.to_string()),
                }
            }

            div { class: if add_open || editor_open() { "md3-exchange-workbench md3-exchange-workbench-inspector" } else { "md3-exchange-workbench" },
                section { class: "md3-card md3-card-elevated md3-exchange-table-card", {table_content} }
                if editor_open() {
                    ExchangeInspector {
                        key: "{inspector_key}",
                        exchange_id,
                        transaction: editor_transaction(),
                        on_close: move |_| close_editor.call(()),
                        on_saved: move |saved: ExchangeTransaction| {
                            toast.success("Exchange transaction saved.");
                            close_editor.call(());
                            detail.restart();
                            transactions.restart();
                            let _ = saved;
                        },
                    }
                }
            }
        }

        if show_import_data() {
            ImportDataDialog {
                exchange_id: Some(exchange_id),
                exchange_name: Some(account.name.clone()),
                on_close: move |_| show_import_data.set(false),
                on_imported: move |summary: ImportDataSummary| {
                    toast.success(import_data_summary_message(&summary));
                    show_import_data.set(false);
                    detail.restart();
                    transactions.restart();
                },
            }
        }

        if let Some(transaction) = link_target() {
            {
                let linked_id = transaction.id;
                rsx! {
            ExchangeLinkDialog {
                key: "{transaction.id}",
                transaction,
                on_close: move |_| link_target.set(None),
                on_changed: move |_| {
                    if editor_transaction()
                        .as_ref()
                        .is_some_and(|row| row.id == linked_id)
                    {
                        close_editor.call(());
                    }
                    link_target.set(None);
                    toast.success("On-chain link updated.");
                    detail.restart();
                    transactions.restart();
                },
            }
                }
            }
        }

        if let Some(transaction) = delete_target() {
            ConfirmDialog {
                title: "Remove transaction".to_string(),
                message: format!(
                    "Remove this {} transaction from {}?",
                    transaction.tx_type.as_str(),
                    account.name,
                ),
                confirm_text: "Remove".to_string(),
                cancel_text: "Cancel".to_string(),
                on_cancel: move |_| delete_target.set(None),
                on_confirm: move |_| {
                    let transaction = transaction.clone();
                    if editor_transaction()
                        .as_ref()
                        .is_some_and(|row| row.id == transaction.id)
                    {
                        close_editor.call(());
                    }
                    delete_target.set(None);
                    remove_transaction(toast, detail, transactions, exchange_id, transaction.id);
                },
            }
        }
    }
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

fn navigate_to_detail(
    navigator: dioxus::router::Navigator,
    exchange_id: i64,
    transaction_id: Option<i64>,
) {
    navigator.replace(Route::ExchangeDetail {
        exchange_id,
        transaction_id: transaction_id.map(|id| id.to_string()),
    });
}

fn remove_transaction(
    mut toast: Toast,
    mut detail: Resource<Result<ExchangeDetailData, String>>,
    mut transactions: Resource<
        Result<crate::domain::table::TablePage<ExchangeTransaction>, String>,
    >,
    exchange_id: i64,
    transaction_id: i64,
) {
    spawn(async move {
        match delete_exchange_transaction_api(exchange_id, transaction_id).await {
            Ok(true) => toast.success("Exchange transaction removed."),
            Ok(false) => toast.error("Exchange transaction was not found."),
            Err(error) => toast.error(format!("Could not remove the transaction: {error}")),
        }
        detail.restart();
        transactions.restart();
    });
}

fn format_fee(fee: Option<f64>, currency: Option<&str>) -> String {
    match (fee, currency) {
        (Some(fee), Some(currency)) if is_bitcoin_currency(currency) => {
            format_btc(fee.round() as i64)
        }
        (Some(fee), Some(currency)) => format!("{fee} {currency}"),
        (Some(fee), None) => format!("{fee}"),
        _ => "-".to_string(),
    }
}

fn format_tx_hash(hash: &str) -> String {
    if hash.len() <= 18 {
        hash.to_string()
    } else {
        format!("{}...{}", &hash[..10], &hash[hash.len() - 8..])
    }
}

fn import_data_summary_message(summary: &ImportDataSummary) -> String {
    let mut message = format!(
        "Imported {} transaction(s){}{}.",
        summary.inserted,
        if summary.auto_linked > 0 {
            format!(", auto-linked {}", summary.auto_linked)
        } else {
            String::new()
        },
        if summary.duplicates > 0 {
            format!(", skipped {} duplicate(s)", summary.duplicates)
        } else {
            String::new()
        }
    );
    if summary.skipped_non_btc > 0 {
        message.push_str(&format!(
            " Skipped {} non-BTC row(s).",
            summary.skipped_non_btc
        ));
    }
    message
}

const ICON_EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04a1 1 0 0 0 0-1.41l-2.34-2.34a1 1 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";
const ICON_DELETE: &str =
    "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";

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
