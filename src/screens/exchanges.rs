//! Exchanges list screen: provider KPIs, ledger accounts, and CSV import.

use dioxus::prelude::*;

use crate::api::exchanges::{delete_exchanges_api, list_exchanges_api};
use crate::api::settings::get_exchange_settings_api;
use crate::app::Route;
use crate::components::format::format_btc;
use crate::components::{
    use_table_selection, use_toast, BadgeSize, ConfirmDialog, ExchangeAccountDialog,
    ImportDataDialog, Kpi, ProviderBadge, SortableHeader, TablePagination, TableSelection,
    TableSelectionCell, TableSelectionHeader, Toast,
};
use crate::domain::civil_time;
use crate::domain::exchange::{
    ExchangeAccount, ExchangeAccountList, ExchangeAccountRow, ExchangeAccountTableQuery,
    ImportDataSummary,
};
use crate::domain::table::{SortDirection, TableQuery};

const PAGE_SIZE: u32 = 25;

#[component]
pub fn Exchanges() -> Element {
    let mut toast = use_toast();
    let navigator = use_navigator();
    let mut page = use_signal(|| 0_u32);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Desc);
    let mut show_import_data = use_signal(|| false);
    let mut edit_target = use_signal(|| Option::<ExchangeAccount>::None);
    let mut delete_target = use_signal(|| Option::<Vec<ExchangeAccountRow>>::None);
    let mut selection = use_table_selection::<i64>();
    let mut confirm_destructive = use_signal(|| true);

    let settings = use_resource(|| async move { get_exchange_settings_api().await.ok() });
    use_effect(move || {
        if let Some(Some(settings)) = settings() {
            confirm_destructive.set(settings.confirm_destructive_actions);
        }
    });

    let mut list = use_resource(move || {
        let current_page = page();
        let sort_key = sort_by();
        let direction = sort_dir();
        async move {
            list_exchanges_api(ExchangeAccountTableQuery {
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

    // A deletion can empty the current page while earlier pages still have
    // rows; step back until the page has content again.
    use_effect(move || {
        if let Some(Ok(data)) = list() {
            if data.page.items.is_empty() && page() > 0 {
                page.set(page() - 1);
                selection.clear();
            }
        }
    });

    let state = list();
    let loading = state.is_none();
    let totals = match &state {
        Some(Ok(data)) => Some(data.totals),
        _ => None,
    };
    let kpis = [
        totals.map(|totals| totals.exchange_count.to_string()),
        totals.map(|totals| totals.total_transactions.to_string()),
        totals.map(|totals| {
            format!(
                "{}/{}",
                totals.onchain_matched_count, totals.onchain_total_count
            )
        }),
    ];
    let activity = totals.map(|totals| totals.activity);

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
            rsx! {
                TablePagination {
                    page: page(),
                    page_size: PAGE_SIZE,
                    total,
                    on_page: move |next| {
                        page.set(next);
                        selection.clear();
                    },
                }
                div { class: "md3-table-container",
                    table { class: "md3-table md3-exchanges-table",
                        thead {
                            tr {
                                TableSelectionHeader {
                                    all_selected,
                                    select_all_label: "Select all exchanges on this page".to_string(),
                                    disabled: page_ids.is_empty(),
                                    on_toggle_all: move |checked| {
                                        selection.set_many(&selectable_ids, checked);
                                    },
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-icon-button-danger",
                                        disabled: selection.is_empty(),
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
                                            if confirm_destructive() {
                                                delete_target.set(Some(accounts));
                                            } else {
                                                remove_exchanges(toast, list, selection, accounts);
                                            }
                                        },
                                        {icon(ICON_DELETE)}
                                    }
                                }
                                SortableHeader {
                                    label: "Name".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("name")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "name", SortDirection::Asc);
                                        page.set(0);
                                        selection.clear();
                                    },
                                }
                                th { "Exchange" }
                                SortableHeader {
                                    label: "Txs".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("txs")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "txs", SortDirection::Desc);
                                        page.set(0);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "OnChain Operations".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("onchain")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "onchain", SortDirection::Desc);
                                        page.set(0);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "First Transaction".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("first_tx")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "first_tx", SortDirection::Desc);
                                        page.set(0);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Last Transaction".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("last_tx")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "last_tx", SortDirection::Desc);
                                        page.set(0);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Last Modification".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("last_modified")).then_some(sort_dir()),
                                    on_sort: move |_| {
                                        toggle_sort(sort_by, sort_dir, "last_modified", SortDirection::Desc);
                                        page.set(0);
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
                                    let row_selected = selected_snapshot.contains(&row.id);
                                    let first = row
                                        .first_tx_time
                                        .map(format_short_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let last = row
                                        .last_tx_time
                                        .map(format_short_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let modified = civil_time::format_date(row.last_modified_at);
                                    rsx! {
                                        tr {
                                            class: if row_selected { "md3-exchange-row md3-table-row-selected" } else { "md3-exchange-row" },
                                            onclick: move |_| {
                                                navigator
                                                    .push(Route::ExchangeDetail {
                                                        exchange_id: view_row.id,
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
                                                        {icon(ICON_EDIT)}
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Remove exchange",
                                                        aria_label: "Remove exchange",
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            if confirm_destructive() {
                                                                delete_target.set(Some(vec![delete_row.clone()]));
                                                            } else {
                                                                remove_exchanges(toast, list, selection, vec![delete_row.clone()]);
                                                            }
                                                        },
                                                        {icon(ICON_DELETE)}
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
                                                            .push(Route::ExchangeDetail {
                                                                exchange_id: view_row.id,
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
            }
        }
    };

    rsx! {
        section { id: "exchanges", class: "md3-screen",
            div { class: "md3-page-header",
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Exchanges"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    onclick: move |_| show_import_data.set(true),
                    "Import Data"
                }
            }

            div { class: "md3-kpi-grid md3-kpi-grid-three",
                Kpi {
                    title: "Exchanges".to_string(),
                    value: kpis[0].clone(),
                    loading,
                }
                Kpi {
                    title: "Transactions".to_string(),
                    value: kpis[1].clone(),
                    loading,
                }
                Kpi {
                    title: "OnChain Operations".to_string(),
                    value: kpis[2].clone(),
                    loading,
                }
            }

            div { class: "md3-kpi-grid md3-kpi-grid-six",
                Kpi {
                    title: "Bought".to_string(),
                    value: activity.map(|activity| format_btc(activity.bought_sats)),
                    loading,
                }
                Kpi {
                    title: "Rewarded".to_string(),
                    value: activity.map(|activity| format_btc(activity.rewarded_sats)),
                    loading,
                }
                Kpi {
                    title: "Sold".to_string(),
                    value: activity.map(|activity| format_btc(activity.sold_sats)),
                    loading,
                }
                Kpi {
                    title: "Withdrawn".to_string(),
                    value: activity.map(|activity| format_btc(activity.withdrawn_sats)),
                    loading,
                }
                Kpi {
                    title: "Deposited".to_string(),
                    value: activity.map(|activity| format_btc(activity.deposited_sats)),
                    loading,
                }
                Kpi {
                    title: "Net ledger".to_string(),
                    value: activity.map(|activity| format_btc(activity.net_ledger_sats)),
                    loading,
                }
            }

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
        match delete_exchanges_api(ids).await {
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

fn import_data_summary_message(summary: &ImportDataSummary) -> String {
    if summary.inserted == 0 && summary.created {
        return format!(
            "Exchange account created.{}",
            if summary.skipped_non_btc > 0 {
                format!(" Skipped {} non-BTC rows.", summary.skipped_non_btc)
            } else {
                String::new()
            }
        );
    }
    let mut message = format!(
        "{} {} transaction{}{}{}.",
        if summary.created {
            "Account created and imported"
        } else {
            "Imported"
        },
        summary.inserted,
        if summary.inserted == 1 { "" } else { "s" },
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

fn format_short_datetime(timestamp: i64) -> String {
    let formatted = civil_time::format_datetime(timestamp);
    formatted.get(..16).unwrap_or(&formatted).to_string()
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
