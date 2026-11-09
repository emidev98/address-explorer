//! Exchange detail screen: ledger transaction table, inspector, reconciliation,
//! CSV append, and manual on-chain linking.
use crate::api::exchanges::{
    delete_exchange_transactions_api, get_exchange_account_api, get_exchange_transaction_api,
    list_exchange_transactions_api, reconcile_exchange_account_api,
};
use crate::app::Route;
use crate::components::format::{format_btc, format_fiat, format_tx_hash};
use crate::components::{
    import_data_summary_message, use_fiat_currency, use_interface_settings, use_table_selection,
    use_table_state, use_toast, ConfirmDialog, ExchangeInspector, ExchangeKpiGrid,
    ExchangeLinkDialog, Icon, IconName, ImportDataDialog, SortableHeader, TablePagination,
    TableSelection, TableSelectionCell, TableSelectionHeader, Toast,
};
use crate::domain::civil_time;
use crate::domain::exchange::csv::is_bitcoin_currency;
use crate::domain::exchange::{
    ExchangeAccountDetail as ExchangeAccountDetailData, ExchangeTransaction,
    ExchangeTransactionTableQuery, ImportDataSummary,
};
use crate::domain::table::{SortDirection, DEFAULT_PAGE_SIZE};
use dioxus::prelude::*;
#[component]
pub fn ExchangeAccountDetail(
    account_id: i64,
    #[props(default)] transaction_id: Option<String>,
) -> Element {
    let exchange_id = account_id;
    let mut toast = use_toast();
    let fiat = use_fiat_currency();
    let navigator = use_navigator();
    let mut table = use_table_state(SortDirection::Desc);
    let mut editor_open = use_signal(|| false);
    let mut editor_transaction = use_signal(|| Option::<ExchangeTransaction>::None);
    let mut loaded_query = use_signal(|| Option::<String>::None);
    let mut show_import_data = use_signal(|| false);
    let mut link_target = use_signal(|| Option::<ExchangeTransaction>::None);
    let mut delete_target = use_signal(|| Option::<Vec<ExchangeTransaction>>::None);
    let mut selection = use_table_selection::<i64>();
    let mut reconciling = use_signal(|| false);
    let confirm_destructive = use_interface_settings().get().confirm_destructive_actions;
    let mut detail = use_resource(move || {
        let _currency = fiat.code();
        async move {
            get_exchange_account_api(exchange_id)
                .await
                .map_err(|error| error.to_string())
        }
    });
    let mut transactions = use_resource(move || {
        let _currency = fiat.code();
        async move {
            list_exchange_transactions_api(ExchangeTransactionTableQuery {
                exchange_id,
                table: table.query(DEFAULT_PAGE_SIZE),
            })
            .await
            .map_err(|error| error.to_string())
        }
    });
    use_effect(move || {
        if let Some(Ok(data)) = transactions() {
            if data.items.is_empty() && table.page() > 0 {
                table.page.set(table.page() - 1);
                selection.clear();
            }
        }
    });
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
    let table_currency = kpis
        .as_ref()
        .map(|kpi| kpi.fiat_currency.clone())
        .unwrap_or_else(|| fiat.code());
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
        None => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant", "Loading transactions..." }
            }
        }
        Some(Err(error)) => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "Error loading transactions: {error}"
                }
            }
        }
        Some(Ok(page_data)) if page_data.items.is_empty() => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "No transactions imported yet. Import a CSV or add a transaction manually."
                }
            }
        }
        Some(Ok(page_data)) => {
            let total = page_data.total;
            let items = page_data.items.clone();
            let page_ids = items.iter().map(|row| row.id).collect::<Vec<_>>();
            let selected_snapshot = selection.snapshot();
            let selected_count = selection.len();
            let all_selected = selection.all_selected(&page_ids);
            let selectable_ids = page_ids.clone();
            let rows_for_bulk_delete = items.clone();
            let delete_action_label = if selected_count == 0 {
                "Select transactions to delete".to_string()
            } else {
                format!(
                    "Delete {selected_count} {}",
                    if selected_count == 1 {
                        "transaction"
                    } else {
                        "transactions"
                    },
                )
            };
            rsx! {
                div { class: "md3-table-container",
                    table { class: "md3-table md3-exchange-transactions-table",
                        thead {
                            tr {
                                TableSelectionHeader {
                                    all_selected,
                                    indeterminate: selection
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .some_selected(& page_ids) && ! all_selected,
                                    select_all_label: "Select all transactions on this page".to_string(),
                                    disabled: page_ids
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .is_empty(),
                                    on_toggle_all: move |checked| {
                                        selection.set_many(&selectable_ids, checked);
                                    },
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-icon-button-danger",
                                        disabled: selection
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .is_empty(),
                                        title: "{delete_action_label}",
                                        aria_label: "{delete_action_label}",
                                        onclick: move |_| {
                                            let snapshot = selection.snapshot();
                                            let rows = rows_for_bulk_delete
                                                .iter()
                                                .filter(|row| snapshot.contains(&row.id))
                                                .cloned()
                                                .collect::<Vec<_>>();
                                            if rows.is_empty() {
                                                return;
                                            }
                                            if editor_transaction().as_ref().is_some_and(|row| snapshot.contains(&row.id)) {
                                                close_editor.call(());
                                            }
                                            if confirm_destructive {
                                                delete_target.set(Some(rows));
                                            } else {
                                                remove_transactions(
                                                    toast,
                                                    detail,
                                                    transactions,
                                                    exchange_id,
                                                    selection,
                                                    rows,
                                                );
                                            }
                                        },
                                        Icon { name: IconName::Delete, size: 20 }
                                    }
                                }
                                SortableHeader {
                                    label: "Date".to_string(),
                                    active_direction: table
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .direction_for("date"),
                                    on_sort: move |_| {
                                        table.sort("date", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Type".to_string(),
                                    active_direction: table.direction_for("type"),
                                    on_sort: move |_| {
                                        table.sort("type", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Amount".to_string(),
                                    active_direction: table.direction_for("amount"),
                                    on_sort: move |_| {
                                        table.sort("amount", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                th { "Value ({table_currency})" }
                                SortableHeader {
                                    label: "Fee".to_string(),
                                    active_direction: table.direction_for("fee"),
                                    on_sort: move |_| {
                                        table.sort("fee", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "On-chain".to_string(),
                                    active_direction: table
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .direction_for("on_chain"),
                                    on_sort: move |_| {
                                        table.sort("on_chain", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Exchange Tx Hash".to_string(),
                                    active_direction: table
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .direction_for("tx_hash"),
                                    on_sort: move |_| {
                                        table.sort("tx_hash", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Description".to_string(),
                                    active_direction: table
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .direction_for("description"),
                                    on_sort: move |_| {
                                        table.sort("description", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                            }
                        }
                        tbody {
                            for transaction in &items {
                                {
                                    let row = transaction.clone();
                                    let edit_row = row.clone();
                                    let edit_action_row = row.clone();
                                    let delete_row = row.clone();
                                    let link_row = row.clone();
                                    let inspector_selected = editor_transaction().as_ref().map(|row| row.id)
                                        == Some(row.id);
                                    let row_selected = selected_snapshot.contains(&row.id);
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
                                    let fee = format_fee(
                                        row.fee,
                                        row.fee_currency.as_deref(),
                                        row.converted_fee_amount.zip(row.converted_fee_currency.as_deref()),
                                    );
                                    rsx! {
                                        tr {
                                            class: if inspector_selected && row_selected { "md3-exchange-selected-row md3-table-row-selected" } else if inspector_selected { "md3-exchange-selected-row" } else if row_selected { "md3-table-row-selected" } else { "" },
                                            onclick: move |_| {
                                                editor_transaction.set(Some(edit_row.clone()));
                                                editor_open.set(true);
                                                loaded_query.set(Some(edit_row.id.to_string()));
                                                navigate_to_detail(navigator, exchange_id, Some(edit_row.id));
                                            },
                                            TableSelectionCell {
                                                checked: row_selected,
                                                label: format!("Select {} transaction", row.tx_type.as_str()),
                                                on_change: {
                                                    let id = row.id;
                                                    move |checked| selection.set(id, checked)
                                                },
                                            }
                                            td {
                                                div { class: "md3-table-actions",
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Remove transaction",
                                                        aria_label: "Remove transaction",
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            if editor_transaction().as_ref().is_some_and(|row| row.id == delete_row.id) {
                                                                close_editor.call(());
                                                            }
                                                            if confirm_destructive {
                                                                delete_target.set(Some(vec![delete_row.clone()]));
                                                            } else {
                                                                remove_transactions(
                                                                    toast,
                                                                    detail,
                                                                    transactions,
                                                                    exchange_id,
                                                                    selection,
                                                                    vec![delete_row.clone()],
                                                                );
                                                            }
                                                        },
                                                        Icon { name: IconName::Delete, size: 20 }
                                                    }
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
                                                        Icon { name: IconName::Edit, size: 20 }
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
                                            td { "{fee}" }
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
                TablePagination {
                    page: table.page(),
                    page_size: DEFAULT_PAGE_SIZE,
                    total,
                    on_page: move |next| {
                        table.page.set(next);
                        selection.clear();
                    },
                }
            }
        }
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
    let inspector_key = editor_transaction()
        .as_ref()
        .map(|row| row.id.to_string())
        .unwrap_or_else(|| "new".to_string());
    rsx! {
        section { id: "exchange-detail", class: "md3-screen md3-exchange-detail",
            div { class: "md3-page-header",
                div { class: "md3-exchange-title-row",
                    Link {
                        to: Route::Exchanges {},
                        class: "md3-icon-button md3-exchange-back-button",
                        aria_label: "Back to exchanges",
                        title: "Back to exchanges",
                        Icon { name: IconName::Back, size: 20 }
                    }
                    h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                        "{account.name}"
                    }
                    if let Some(provider) = &account.provider_name {
                        span { class: "md3-exchange-provider", "{provider}" }
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
                        onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        show_import_data.set(true),
                        "Import Data"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *reconciling.read(),
                        onclick: move |_| {
                            reconciling.set(true);
                            spawn(async move {
                                match reconcile_exchange_account_api(exchange_id).await {
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
            ExchangeKpiGrid { kpis, loading: false }
            div { class: if editor_open() { "md3-exchange-workbench md3-exchange-workbench-inspector" } else { "md3-exchange-workbench" },
                div { class: "md3-exchange-table-pane", {table_content} }
                if editor_open() {
                    ExchangeInspector {
                        key: "{inspector_key}",
                        exchange_id,
                        transaction: editor_transaction(),
                        on_close: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        close_editor.call(()),
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
                exchange_name: Some(account
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .name.clone()),
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
                            if editor_transaction().as_ref().is_some_and(|row| row.id == linked_id) {
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
        if let Some(selected_transactions) = delete_target() {
            {
                let count = selected_transactions.len();
                let (title, message) = if count == 1 {
                    (
                        "Remove transaction".to_string(),
                        format!(
                            "Remove this {} transaction from {}?",
                            selected_transactions[0].tx_type.as_str(),
                            account.name,
                        ),
                    )
                } else {
                    (
                        "Remove transactions".to_string(),
                        format!(
                            "Remove {count} selected transactions from {}? This cannot be undone.",
                            account.name,
                        ),
                    )
                };
                rsx! {
                    ConfirmDialog {
                        title,
                        message,
                        confirm_text: "Remove".to_string(),
                        cancel_text: "Cancel".to_string(),
                        on_cancel: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        delete_target.set(None),
                        on_confirm: move |_| {
                            let selected_transactions = selected_transactions.clone();
                            if editor_transaction()
                                .as_ref()
                                .is_some_and(|row| {
                                    selected_transactions.iter().any(|selected| selected.id == row.id)
                                })
                            {
                                close_editor.call(());
                            }
                            delete_target.set(None);
                            remove_transactions(
                                toast,
                                detail,
                                transactions,
                                exchange_id,
                                selection,
                                selected_transactions,
                            );
                        },
                    }
                }
            }
        }
    }
}
fn navigate_to_detail(
    navigator: dioxus::router::Navigator,
    exchange_id: i64,
    transaction_id: Option<i64>,
) {
    navigator.replace(Route::ExchangeAccountDetail {
        account_id: exchange_id,
        transaction_id: transaction_id.map(|id| id.to_string()),
    });
}
fn remove_transactions(
    mut toast: Toast,
    mut detail: Resource<Result<ExchangeAccountDetailData, String>>,
    mut transactions: Resource<
        Result<crate::domain::table::TablePage<ExchangeTransaction>, String>,
    >,
    exchange_id: i64,
    mut selection: TableSelection<i64>,
    rows: Vec<ExchangeTransaction>,
) {
    if rows.is_empty() {
        return;
    }
    let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
    spawn(async move {
        match delete_exchange_transactions_api(exchange_id, ids).await {
            Ok(0) => toast.error("No exchange transactions were found to remove."),
            Ok(1) => toast.success("Exchange transaction removed."),
            Ok(deleted) => toast.success(format!("Removed {deleted} exchange transactions.")),
            Err(error) => toast.error(format!("Could not remove the transactions: {error}")),
        }
        selection.clear();
        detail.restart();
        transactions.restart();
    });
}
fn format_fee(fee: Option<f64>, currency: Option<&str>, converted: Option<(f64, &str)>) -> String {
    let Some(fee) = fee else {
        return "-".to_string();
    };
    let Some(currency) = currency else {
        return format!("{fee}");
    };
    if is_bitcoin_currency(currency) {
        return format_btc(fee.round() as i64);
    }
    match converted {
        Some((amount, target)) => format_fiat(amount, target),
        None => "-".to_string(),
    }
}
#[cfg(test)]
mod tests {
    use super::format_fee;
    #[test]
    fn fiat_fees_prefer_the_converted_value_and_btc_fees_stay_btc() {
        assert_eq!(
            format_fee(Some(10.0), Some("EUR"), Some((12.0, "USD"))),
            "$12.00"
        );
        assert_eq!(format_fee(Some(10.0), Some("EUR"), None), "-");
        assert_eq!(
            format_fee(Some(10_000.0), Some("BTC"), Some((1.0, "USD"))),
            "0.0001 ₿",
        );
        assert_eq!(format_fee(None, Some("EUR"), None), "-");
    }
}
