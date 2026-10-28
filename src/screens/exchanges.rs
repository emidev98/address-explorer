//! Exchanges list screen: provider KPIs, ledger accounts, and CSV import.

use dioxus::prelude::*;

use crate::api::exchanges::{delete_exchange_api, list_exchanges_api};
use crate::api::settings::get_exchange_settings_api;
use crate::app::Route;
use crate::components::{
    use_toast, ConfirmDialog, ExchangeAccountDialog, ImportDataDialog, Kpi, SortableHeader,
    TablePagination, Toast,
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
    let mut delete_target = use_signal(|| Option::<ExchangeAccountRow>::None);
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
        Some(Ok(data)) => rsx! {
            TablePagination {
                page: page(),
                page_size: PAGE_SIZE,
                total: data.page.total,
                on_page: move |next| page.set(next),
            }
            div { class: "md3-table-container",
                table { class: "md3-table md3-exchanges-table",
                    thead {
                        tr {
                            th {}
                            SortableHeader {
                                label: "Name".to_string(),
                                active_direction: (sort_by().as_deref() == Some("name")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "name", SortDirection::Asc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Txs".to_string(),
                                active_direction: (sort_by().as_deref() == Some("txs")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "txs", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "OnChain Operations".to_string(),
                                active_direction: (sort_by().as_deref() == Some("onchain")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "onchain", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "First Transaction".to_string(),
                                active_direction: (sort_by().as_deref() == Some("first_tx")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "first_tx", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Last Transaction".to_string(),
                                active_direction: (sort_by().as_deref() == Some("last_tx")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "last_tx", SortDirection::Desc); page.set(0); },
                            }
                            SortableHeader {
                                label: "Last Modification".to_string(),
                                active_direction: (sort_by().as_deref() == Some("last_modified")).then_some(sort_dir()),
                                on_sort: move |_| { toggle_sort(sort_by, sort_dir, "last_modified", SortDirection::Desc); page.set(0); },
                            }
                        }
                    }
                    tbody {
                        for row in &data.page.items {
                            {
                                let view_row = row.clone();
                                let edit_row = row.clone();
                                let delete_row = row.clone();
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
                                        class: "md3-exchange-row",
                                        onclick: move |_| {
                                            navigator
                                                .push(Route::ExchangeDetail {
                                                    exchange_id: view_row.id,
                                                    transaction_id: None,
                                                });
                                        },
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
                                                            delete_target.set(Some(delete_row.clone()));
                                                        } else {
                                                            remove_exchange(
                                                                toast,
                                                                list,
                                                                delete_row.id,
                                                                delete_row.name.clone(),
                                                                delete_row.tx_count,
                                                            );
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
                                                    navigator.push(Route::ExchangeDetail {
                                                        exchange_id: view_row.id,
                                                        transaction_id: None,
                                                    });
                                                },
                                                span { class: "md3-exchange-name-cell",
                                                    strong { "{row.name}" }
                                                    if let Some(provider) = &row.provider_name {
                                                        span { class: "md3-exchange-provider", "{provider}" }
                                                    }
                                                }
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
        },
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

        if let Some(account) = delete_target() {
            ConfirmDialog {
                title: "Remove Exchange".to_string(),
                message: format!(
                    "Remove \"{}\" and all its {} transactions? This cannot be undone.",
                    account.name,
                    account.tx_count,
                ),
                confirm_text: "Remove".to_string(),
                cancel_text: "Cancel".to_string(),
                on_cancel: move |_| delete_target.set(None),
                on_confirm: move |_| {
                    let account = account.clone();
                    delete_target.set(None);
                    remove_exchange(toast, list, account.id, account.name.clone(), account.tx_count);
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

fn remove_exchange(
    mut toast: Toast,
    mut list: Resource<Result<ExchangeAccountList, String>>,
    id: i64,
    name: String,
    tx_count: i64,
) {
    spawn(async move {
        match delete_exchange_api(id).await {
            Ok(true) => toast.success(format!(
                "Exchange \"{name}\" and its {tx_count} transactions removed."
            )),
            Ok(false) => toast.error(format!("Exchange \"{name}\" was not found.")),
            Err(error) => toast.error(format!("Could not remove the exchange: {error}")),
        }
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
