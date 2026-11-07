//! Exchanges list screen: provider KPIs, ledger accounts, and CSV import.

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::exchanges::{
    delete_exchange_accounts_api, list_exchange_accounts_api, reconcile_exchange_accounts_api,
};
use crate::app::Route;
use crate::components::{
    import_data_summary_message, use_fiat_currency, use_interface_settings, use_table_selection,
    use_table_state, use_toast, BadgeSize, ConfirmDialog, ExchangeAccountDialog, ExchangeCountKpi,
    ExchangeKpiGrid, Icon, IconName, ImportDataDialog, PageHeader, ProviderBadge, SortableHeader,
    TablePagination, TableSelection, TableSelectionCell, TableSelectionHeader, Toast, ToastKind,
};
use crate::domain::civil_time;
use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountList, ExchangeAccountRow, ExchangeAccountTableQuery,
    ExchangeReconcileBatchSummary, ExchangeReconcileFailure, ImportDataSummary,
};
use crate::domain::table::{SortDirection, DEFAULT_PAGE_SIZE};

#[component]
pub fn Exchanges() -> Element {
    let mut toast = use_toast();
    let fiat = use_fiat_currency();
    let navigator = use_navigator();
    let mut table = use_table_state(SortDirection::Desc);
    let mut show_import_data = use_signal(|| false);
    let mut edit_target = use_signal(|| Option::<ExchangeAccount>::None);
    let mut delete_target = use_signal(|| Option::<Vec<ExchangeAccountRow>>::None);
    let mut selection = use_table_selection::<i64>();
    let confirm_destructive = use_interface_settings().get().confirm_destructive_actions;
    let mut reconciling = use_signal(|| false);
    let reconciling_ids = use_signal(BTreeMap::<i64, bool>::new);

    let mut list = use_resource(move || {
        // The server values the KPIs in the saved default currency; reading
        // the shared signal refetches the cards when the setting changes.
        let _currency = fiat.code();
        async move {
            list_exchange_accounts_api(ExchangeAccountTableQuery {
                table: table.query(DEFAULT_PAGE_SIZE),
            })
            .await
            .map_err(|error| error.to_string())
        }
    });

    // A deletion can empty the current page while earlier pages still have
    // rows; step back until the page has content again.
    use_effect(move || {
        if let Some(Ok(data)) = list() {
            if data.page.items.is_empty() && table.page() > 0 {
                table.page.set(table.page() - 1);
                selection.clear();
            }
        }
    });

    let state = list();
    let loading = state.is_none();
    let kpis = match &state {
        Some(Ok(data)) => Some(data.kpis.clone()),
        _ => None,
    };
    let exchange_count = match &state {
        None => ExchangeCountKpi::Loading,
        Some(Ok(data)) => ExchangeCountKpi::Count(data.page.total),
        Some(Err(_)) => ExchangeCountKpi::Unavailable,
    };
    let conversion_warning = state
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|data| data.conversion_warning.clone());

    let content = match &state {
        None => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading exchanges..." }
        },
        Some(Err(error)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Error loading exchanges: {error}" }
        },
        Some(Ok(data)) if data.page.items.is_empty() => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "No exchange accounts stored yet. Use Import Data to import a CSV export."
            }
        },
        Some(Ok(data)) => {
            let total = data.page.total;
            let items = data.page.items.clone();
            let page_ids = items.iter().map(|row| row.id).collect::<Vec<_>>();
            let selected_snapshot = selection.snapshot();
            let selected_count = selection.len();
            let all_selected = selection.all_selected(&page_ids);
            let selectable_ids = page_ids.clone();
            let rows_for_bulk_delete = items.clone();
            let reconciling_snapshot = reconciling_ids.read().clone();
            let any_row_reconciling = !reconciling_snapshot.is_empty();
            let delete_action_label = if selected_count == 0 {
                "Select exchanges to delete".to_string()
            } else {
                format!(
                    "Delete {selected_count} {}",
                    if selected_count == 1 {
                        "exchange"
                    } else {
                        "exchanges"
                    }
                )
            };
            let reconcile_action_label = if selected_count == 0 {
                "Select exchanges to reconcile".to_string()
            } else {
                format!(
                    "Reconcile {selected_count} {}",
                    if selected_count == 1 {
                        "exchange"
                    } else {
                        "exchanges"
                    }
                )
            };
            rsx! {
                div { class: "md3-table-container",
                    table { class: "md3-table md3-exchanges-table",
                        thead {
                            tr {
                                TableSelectionHeader {
                                    all_selected,
                                    indeterminate: selection.some_selected(&page_ids)
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    && !all_selected,
                                    select_all_label: "Select all exchanges on this page".to_string(),
                                    disabled: page_ids.is_empty(),
                                    on_toggle_all: move |checked| {
                                        selection.set_many(&selectable_ids, checked);
                                    },
                                    div { class: "md3-table-actions",
                                        button {
                                            r#type: "button",
                                            class: "md3-icon-button",
                                            disabled: selection.is_empty()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            || *reconciling.read()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            || any_row_reconciling,
                                            title: "{reconcile_action_label}",
                                            aria_label: "{reconcile_action_label}",
                                            onclick: move |_| {
                                                let ids = selection.selected_ids();
                                                if ids.is_empty() {
                                                    return;
                                                }
                                                reconciling.set(true);
                                                spawn(async move {
                                                    match reconcile_exchange_accounts_api(ids).await {
                                                        Ok(summary) => {
                                                            let (kind, message) =
                                                                reconcile_batch_summary_toast(&summary);
                                                            toast.show(kind, message);
                                                            if summary.failed.is_empty() {
                                                                selection.clear();
                                                            }
                                                        }
                                                        Err(error) => {
                                                            toast.error(format!("Could not reconcile the exchanges: {error}"));
                                                        }
                                                    }
                                                    reconciling.set(false);
                                                    list.restart();
                                                });
                                            },
                                            if *reconciling.read() {
                                                "…"
                                            } else {
                                                Icon { name: IconName::Sync, size: 20 }
                                            }
                                        }
                                        button {
                                            r#type: "button",
                                            class: "md3-icon-button md3-icon-button-danger",
                                            disabled: selection.is_empty()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            || *reconciling.read()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            || any_row_reconciling,
                                            title: "{delete_action_label}",
                                            aria_label: "{delete_action_label}",
                                            onclick: move |_| {
                                                let snapshot = selection.snapshot();
                                                let accounts = rows_for_bulk_delete
                                                    .iter()
                                                    .filter(|row| snapshot.contains(&row.id))
                                                    .cloned()
                                                    .collect::<Vec<_>>();
                                                if accounts.is_empty() {
                                                    return;
                                                }
                                                if confirm_destructive {
                                                    delete_target.set(Some(accounts));
                                                } else {
                                                    remove_exchanges(toast, list, selection, accounts);
                                                }
                                            },
                                            Icon { name: IconName::Delete, size: 20 }
                                        }
                                    }
                                }
                                SortableHeader {
                                    label: "Account".to_string(),
                                    active_direction: table.direction_for("name"),
                                    on_sort: move |_| {
                                        table.sort("name", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                th { "Provider" }
                                SortableHeader {
                                    label: "Txs".to_string(),
                                    active_direction: table.direction_for("txs"),
                                    on_sort: move |_| {
                                        table.sort("txs", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "On-chain Txs".to_string(),
                                    active_direction: table.direction_for("onchain"),
                                    on_sort: move |_| {
                                        table.sort("onchain", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "First Transaction".to_string(),
                                    active_direction: table.direction_for("first_tx"),
                                    on_sort: move |_| {
                                        table.sort("first_tx", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Last Transaction".to_string(),
                                    active_direction: table.direction_for("last_tx"),
                                    on_sort: move |_| {
                                        table.sort("last_tx", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Last Modification".to_string(),
                                    active_direction: table.direction_for("last_modified"),
                                    on_sort: move |_| {
                                        table.sort("last_modified", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                            }
                        }
                        tbody {
                            for row in &items {
                                {
                                    let view_row = row.clone();
                                    let edit_row = row.clone();
                                    let delete_row = row.clone();
                                    let reconcile_id = row.id;
                                    let row_reconciling = reconciling_snapshot
                                        .get(&row.id)
                                        .copied()
                                        .unwrap_or(false);
                                    let row_selected = selected_snapshot.contains(&row.id);
                                    let first = row
                                        .first_tx_time
                                        .map(civil_time::format_short_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let last = row
                                        .last_tx_time
                                        .map(civil_time::format_short_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let modified = civil_time::format_date(row.last_modified_at);
                                    rsx! {
                                        tr {
                                            class: if row_selected { "md3-exchange-row md3-table-row-selected" } else { "md3-exchange-row" },
                                            onclick: move |_| {
                                                navigator
                                                    .push(Route::ExchangeAccountDetail {
                                                        account_id: view_row.id,
                                                        transaction_id: None,
                                                    });
                                            },
                                            TableSelectionCell {
                                                checked: row_selected,
                                                label: format!("Select {}", row.name),
                                                on_change: {
                                                    let id = row.id;
                                                    move |checked| selection.set(id, checked)
                                                },
                                            }
                                            td {
                                                div { class: "md3-table-actions",
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: if row_reconciling { "Reconciling..." } else { "Run reconciliation" },
                                                        aria_label: if row_reconciling { "Reconciling exchange" } else { "Run reconciliation for this exchange" },
                                                        disabled: row_reconciling || *reconciling.read(),
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            start_row_reconcile(reconciling_ids, toast, list, reconcile_id);
                                                        },
                                                        if row_reconciling {
                                                            "…"
                                                        } else {
                                                            Icon { name: IconName::Sync, size: 20 }
                                                        }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Remove exchange",
                                                        aria_label: "Remove exchange",
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            if confirm_destructive {
                                                                delete_target.set(Some(vec![delete_row.clone()]));
                                                            } else {
                                                                remove_exchanges(toast, list, selection, vec![delete_row.clone()]);
                                                            }
                                                        },
                                                        Icon { name: IconName::Delete, size: 20 }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "Edit exchange",
                                                        aria_label: "Edit exchange",
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            edit_target
                                                                .set(
                                                                    Some(ExchangeAccount {
                                                                        id: edit_row.id,
                                                                        name: edit_row.name.clone(),
                                                                        provider_id: edit_row.provider_id,
                                                                        provider_name: edit_row.provider_name.clone(),
                                                                        created_at: 0,
                                                                        last_imported_at: None,
                                                                        last_modified_at: edit_row.last_modified_at,
                                                                    }),
                                                                );
                                                        },
                                                        Icon { name: IconName::Edit, size: 20 }
                                                    }
                                                }
                                            }
                                            td {
                                                button {
                                                    r#type: "button",
                                                    class: "md3-exchange-name-link",
                                                    aria_label: "Open {row.name}",
                                                    onclick: move |event| {
                                                        event.stop_propagation();
                                                        navigator
                                                            .push(Route::ExchangeAccountDetail {
                                                                account_id: view_row.id,
                                                                transaction_id: None,
                                                            });
                                                    },
                                                    strong { "{row.name}" }
                                                }
                                            }
                                            td {
                                                if let Some(provider) = &row.provider_name {
                                                    span { class: "md3-exchange-provider md3-exchange-provider-badge",
                                                        ProviderBadge {
                                                            name: provider.clone(),
                                                            color: row.provider_color.clone(),
                                                            size: BadgeSize::Compact,
                                                        }
                                                        "{provider}"
                                                    }
                                                } else {
                                                    "-"
                                                }
                                            }
                                            td { "{row.tx_count}" }
                                            td {
                                                title: "{row.onchain_matched_count} of {row.onchain_total_count} deposit and withdrawal operations matched",
                                                aria_label: "{row.onchain_matched_count} of {row.onchain_total_count} deposit and withdrawal operations matched",
                                                "{row.onchain_matched_count}/{row.onchain_total_count}"
                                            }
                                            td { "{first}" }
                                            td { "{last}" }
                                            td { "{modified}" }
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

    rsx! {
        section { id: "exchanges", class: "md3-screen",
            PageHeader {
                title: "Exchanges".to_string(),
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    onclick: move |_| show_import_data.set(true),
                    "Import Data"
                }
            }

            if let Some(warning) = conversion_warning {
                div { class: "md3-exchange-warning-banner", "{warning}" }
            }

            ExchangeKpiGrid { kpis, exchange_count, loading }

            {content}
        }

        if show_import_data() {
            ImportDataDialog {
                on_close: move |_| show_import_data.set(false),
                on_imported: move |summary: ImportDataSummary| {
                    toast.success(import_data_summary_message(&summary));
                    show_import_data.set(false);
                    list.restart();
                },
            }
        }

        if let Some(account) = edit_target() {
            ExchangeAccountDialog {
                account,
                on_close: move |_| edit_target.set(None),
                on_saved: move |updated: ExchangeAccount| {
                    toast.success(format!("Exchange \"{}\" updated.", updated.name));
                    edit_target.set(None);
                    list.restart();
                },
            }
        }

        if let Some(accounts) = delete_target() {
            {
                let count = accounts.len();
                let tx_count: i64 = accounts.iter().map(|account| account.tx_count).sum();
                let (title, message) = if count == 1 {
                    (
                        "Remove Exchange".to_string(),
                        format!(
                            "Remove \"{}\" and all its {} transactions? This cannot be undone.",
                            accounts[0].name,
                            accounts[0].tx_count,
                        ),
                    )
                } else {
                    (
                        "Remove Exchanges".to_string(),
                        format!(
                            "Remove {count} selected exchanges and all their {tx_count} transactions? This cannot be undone.",
                        ),
                    )
                };
                rsx! {
                    ConfirmDialog {
                        title,
                        message,
                        confirm_text: "Remove".to_string(),
                        cancel_text: "Cancel".to_string(),
                        on_cancel: move |_| delete_target.set(None),
                        on_confirm: move |_| {
                            let accounts = accounts.clone();
                            delete_target.set(None);
                            remove_exchanges(toast, list, selection, accounts);
                        },
                    }
                }
            }
        }
    }
}

/// Reconcile one exchange row. The row stays disabled until its own request
/// finishes; other rows and the bulk action remain independent.
fn start_row_reconcile(
    mut reconciling_ids: Signal<BTreeMap<i64, bool>>,
    mut toast: Toast,
    mut list: Resource<Result<ExchangeAccountList, String>>,
    exchange_id: i64,
) {
    if reconciling_ids
        .read()
        .get(&exchange_id)
        .copied()
        .unwrap_or(false)
    {
        return;
    }
    reconciling_ids.write().insert(exchange_id, true);
    spawn(async move {
        match reconcile_exchange_accounts_api(vec![exchange_id]).await {
            Ok(summary) => {
                let (kind, message) = reconcile_batch_summary_toast(&summary);
                toast.show(kind, message);
            }
            Err(error) => toast.error(format!("Could not reconcile the exchange: {error}")),
        }
        reconciling_ids.write().remove(&exchange_id);
        list.restart();
    });
}

fn remove_exchanges(
    mut toast: Toast,
    mut list: Resource<Result<ExchangeAccountList, String>>,
    mut selection: TableSelection<i64>,
    accounts: Vec<ExchangeAccountRow>,
) {
    if accounts.is_empty() {
        return;
    }
    let ids = accounts
        .iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    let names = accounts
        .iter()
        .map(|account| account.name.clone())
        .collect::<Vec<_>>();
    let tx_count: i64 = accounts.iter().map(|account| account.tx_count).sum();
    spawn(async move {
        match delete_exchange_accounts_api(ids).await {
            Ok(0) => toast.error("No exchanges were found to remove."),
            Ok(_) if names.len() == 1 => toast.success(format!(
                "Exchange \"{}\" and its {tx_count} transactions removed.",
                names[0]
            )),
            Ok(deleted) => {
                let (noun, pronoun) = if deleted == 1 {
                    ("exchange", "its")
                } else {
                    ("exchanges", "their")
                };
                toast.success(format!(
                    "Removed {deleted} {noun} and {pronoun} {tx_count} transactions."
                ));
            }
            Err(error) => toast.error(format!("Could not remove the exchanges: {error}")),
        }
        selection.clear();
        list.restart();
    });
}

/// Toast for a finished bulk reconciliation: the successful count followed by
/// the first failure when one or more accounts could not be reconciled.
fn reconcile_batch_summary_toast(summary: &ExchangeReconcileBatchSummary) -> (ToastKind, String) {
    let reconciled = summary.reconciled;
    let failed = summary.failed.len();
    if failed == 0 {
        return (
            ToastKind::Success,
            format!("Reconciled {reconciled} {}.", exchange_noun(reconciled)),
        );
    }

    let first = &summary.failed[0];
    let first_name = failure_name(first);
    if reconciled == 0 {
        let message = if failed == 1 {
            format!("Could not reconcile \"{first_name}\": {}.", first.error)
        } else {
            format!(
                "Could not reconcile {failed} selected exchanges. First error for \"{first_name}\": {}.",
                first.error
            )
        };
        return (ToastKind::Error, message);
    }

    (
        ToastKind::Error,
        format!(
            "Reconciled {reconciled} {}. {failed} {} failed (\"{first_name}\": {}).",
            exchange_noun(reconciled),
            exchange_noun(failed),
            first.error
        ),
    )
}

fn failure_name(failure: &ExchangeReconcileFailure) -> String {
    failure
        .name
        .clone()
        .unwrap_or_else(|| format!("#{}", failure.exchange_id))
}

fn exchange_noun(count: usize) -> &'static str {
    if count == 1 {
        "exchange"
    } else {
        "exchanges"
    }
}

#[cfg(test)]
mod tests {
    use super::reconcile_batch_summary_toast;
    use crate::components::ToastKind;
    use crate::domain::exchange::{ExchangeReconcileBatchSummary, ExchangeReconcileFailure};

    #[test]
    fn reconcile_toast_reports_success_and_partial_failures() {
        assert_eq!(
            reconcile_batch_summary_toast(&ExchangeReconcileBatchSummary {
                reconciled: 2,
                failed: Vec::new(),
            }),
            (ToastKind::Success, "Reconciled 2 exchanges.".to_string())
        );

        let failure = ExchangeReconcileFailure {
            exchange_id: 7,
            name: Some("Kraken".to_string()),
            error: "on-chain cache unavailable".to_string(),
        };
        assert_eq!(
            reconcile_batch_summary_toast(&ExchangeReconcileBatchSummary {
                reconciled: 1,
                failed: vec![failure.clone()],
            }),
            (
                ToastKind::Error,
                "Reconciled 1 exchange. 1 exchange failed (\"Kraken\": on-chain cache unavailable)."
                    .to_string()
            )
        );
        assert_eq!(
            reconcile_batch_summary_toast(&ExchangeReconcileBatchSummary {
                reconciled: 0,
                failed: vec![
                    failure,
                    ExchangeReconcileFailure {
                        exchange_id: 8,
                        name: None,
                        error: "database unavailable".to_string(),
                    },
                ],
            }),
            (
                ToastKind::Error,
                "Could not reconcile 2 selected exchanges. First error for \"Kraken\": on-chain cache unavailable."
                    .to_string()
            )
        );
    }
}
