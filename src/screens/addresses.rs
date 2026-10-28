use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::addresses::{addresses_overview_api, delete_address_api, validate_address_api};
use crate::api::prices::latest_price_api;
use crate::api::settings::get_exchange_settings_api;
use crate::api::transactions::sync_address_transactions_api;
use crate::components::format::{format_address, format_btc, format_usd};
use crate::components::{
    read_clipboard_text, use_toast, write_clipboard_text, AddressEditData, ConfirmDialog,
    HoldingSlice, HoldingsDistributionChart, ImportAddressDialog, ImportAddressDialogMode, Kpi,
    SortableHeader, Toast, ToastKind,
};
use crate::domain::address::{
    AddressEntity, AddressImportResult, AddressOverview, AddressOverviewItem, AddressOverviewQuery,
    AddressScope, AddressScopeFilter,
};
use crate::domain::civil_time::format_datetime;
use crate::domain::table::SortDirection;
use crate::domain::transaction::TransactionSyncSummary;

/// Per-address transaction sync state shown in the addresses table actions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct SyncState {
    loading: bool,
}

/// Addresses screen: KPIs, the holdings distribution treemap, and the
/// addresses table, replicating the UTXO Utils Addresses page on this
/// application's data (see `docs/addresses-page.md`).
#[component]
pub fn Addresses() -> Element {
    let mut toast = use_toast();
    let mut show_import_dialog = use_signal(|| false);
    let mut edit_target = use_signal(|| Option::<AddressOverviewItem>::None);
    let mut clipboard_text = use_signal(|| Option::<String>::None);
    let mut address_to_remove = use_signal(|| Option::<String>::None);
    let sync_states = use_signal(BTreeMap::<String, SyncState>::new);
    // Backend-driven table state (Section 5.11 of CONTRIBUTING.md): the sort
    // and the scope filter are part of the overview query, so changing them
    // re-runs the request. The overview is not paginated because the KPIs
    // and the holdings chart need every matching row, not one page.
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(SortDirection::default);
    let mut scope_filter = use_signal(AddressScopeFilter::default);
    let mut confirm_destructive = use_signal(|| true);

    let mut overview = use_resource(move || async move {
        addresses_overview_api(AddressOverviewQuery {
            sort_by: sort_by(),
            sort_dir: sort_dir(),
            scope: scope_filter(),
        })
        .await
        .map_err(|err| err.to_string())
    });
    let exchange_settings = use_resource(|| async move { get_exchange_settings_api().await.ok() });
    use_effect(move || {
        if let Some(Some(settings)) = exchange_settings() {
            confirm_destructive.set(settings.confirm_destructive_actions);
        }
    });
    let latest_price = use_resource(move || async move {
        latest_price_api("USD".to_string())
            .await
            .ok()
            .map(|price| price.close)
    });

    let overview_state = overview();
    let loading = overview_state.is_none();
    let overview_ready = matches!(&overview_state, Some(Ok(_)));
    let latest_usd = latest_price().flatten();
    let empty_message = format!("No {} balances", scope_filter().empty_noun());

    // `address_count` is the global count of the listed population; the
    // backend excludes exchange addresses without a ledger balance.
    let kpi_values: [Option<String>; 4] = match &overview_state {
        Some(Ok(data)) => [
            Some(data.address_count.to_string()),
            Some(data.confirmed_tx_count.to_string()),
            Some(format_btc(data.spendable_balance_sats)),
            latest_usd.map(|price| format_usd(data.spendable_balance_sats as f64 / 1e8 * price)),
        ],
        _ => [None, None, None, None],
    };

    // The table and the treemap show the same rows, so the Scope chips select
    // the population of both views: the backend already filters `items` by
    // scope, and `is_listed` drops the balance-less exchange addresses.
    let listed_items: Vec<AddressOverviewItem> = match &overview_state {
        Some(Ok(data)) => data
            .items
            .iter()
            .filter(|item| is_listed(item))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };

    // The treemap aggregates every listed address by its spendable balance —
    // wallet, exchange ledger, and unclassified holdings alike.
    let holdings: Vec<HoldingSlice> = listed_items
        .iter()
        .map(|item| HoldingSlice {
            label: item
                .description
                .clone()
                .unwrap_or_else(|| format_address(&item.address)),
            balance_sats: item.spendable_balance_sats,
        })
        .collect();

    let table_content = match &overview_state {
        Some(Ok(_)) => rsx! {
            div { class: "md3-table-container",
                table { class: "md3-table",
                    thead {
                        tr {
                            th {}
                            SortableHeader {
                                label: "Address".to_string(),
                                active_direction: (sort_by().as_deref() == Some("address"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "address", SortDirection::Asc);
                                },
                            }
                            SortableHeader {
                                label: "Label".to_string(),
                                active_direction: (sort_by().as_deref() == Some("label"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "label", SortDirection::Asc);
                                },
                            }
                            th {
                                div { class: "md3-table-scope-filter",
                                    span { "Scope" }
                                    div {
                                        class: "md3-chip-group",
                                        role: "group",
                                        aria_label: "Filter by address scope",
                                        for (filter, filter_label) in [
                                            (AddressScopeFilter::All, "All"),
                                            (AddressScopeFilter::Wallets, "Wallets"),
                                            (AddressScopeFilter::Exchanges, "Exchanges"),
                                        ]
                                        {
                                            button {
                                                r#type: "button",
                                                class: if scope_filter() == filter { "md3-chip md3-chip-selected" } else { "md3-chip" },
                                                aria_pressed: (scope_filter() == filter).to_string(),
                                                onclick: move |_| scope_filter.set(filter),
                                                "{filter_label}"
                                            }
                                        }
                                    }
                                }
                            }
                            SortableHeader {
                                label: "Txs".to_string(),
                                active_direction: (sort_by().as_deref() == Some("txs"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "txs", SortDirection::Desc);
                                },
                            }
                            SortableHeader {
                                label: "Spendable Balance".to_string(),
                                active_direction: (sort_by().as_deref() == Some("balance"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "balance", SortDirection::Desc);
                                },
                            }
                            SortableHeader {
                                label: "Current Price".to_string(),
                                active_direction: (sort_by().as_deref() == Some("price"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "price", SortDirection::Desc);
                                },
                            }
                            SortableHeader {
                                label: "First tx received".to_string(),
                                active_direction: (sort_by().as_deref() == Some("first_tx"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "first_tx", SortDirection::Desc);
                                },
                            }
                            SortableHeader {
                                label: "Last tx sent".to_string(),
                                active_direction: (sort_by().as_deref() == Some("last_tx"))
                                                                                                                                                                            .then_some(sort_dir()),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, "last_tx", SortDirection::Desc);
                                },
                            }
                        }
                    }
                    tbody {
                        if listed_items.is_empty() {
                            tr {
                                td {
                                    colspan: "9",
                                    class: "md3-table-empty text-body-medium",
                                    "{empty_message}"
                                }
                            }
                        } else {
                            for item in &listed_items {
                                {
                                    let row = item.clone();
                                    let sync_address = row.address.clone();
                                    let delete_address = row.address.clone();
                                    let copy_target = row.address.clone();
                                    let short_address = format_address(&row.address);
                                    let label = row
                                        .description
                                        .clone()
                                        .unwrap_or_else(|| "-".to_string());
                                    let current_price = latest_usd
                                        .map(|price| { format_usd(row.spendable_balance_sats as f64 / 1e8 * price) })
                                        .unwrap_or_else(|| "-".to_string());
                                    let first_tx = row
                                        .first_tx_time
                                        .map(format_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let last_tx = row
                                        .last_tx_time
                                        .map(format_datetime)
                                        .unwrap_or_else(|| "-".to_string());
                                    let sync_state = sync_states
                                        .read()
                                        .get(&sync_address)
                                        .cloned()
                                        .unwrap_or_default();
                                    let is_exchange = row.scope.is_exchange();
                                    let sync_title = if sync_state.loading {
                                        "Syncing..."
                                    } else if row.tx_index_rate_limited {
                                        "Resume indexing (paused on the mempool rate limit)"
                                    } else if is_exchange {
                                        "Find related transactions"
                                    } else {
                                        "Sync transactions"
                                    };
                                    let scope_label = scope_label(&row.scope);
                                    rsx! {
                                        tr {
                                            td {
                                                div { class: "md3-table-actions",
                                                    if row.tx_index_rate_limited {
                                                        span {
                                                            class: "md3-icon-warning",
                                                            title: "Transaction indexing stopped on the mempool rate limit. Sync again to resume from where it stopped.",
                                                            role: "img",
                                                            aria_label: "Transaction indexing incomplete",
                                                            {icon(ICON_WARNING)}
                                                        }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "{sync_title}",
                                                        aria_label: "{sync_title}",
                                                        disabled: sync_state.loading,
                                                        onclick: move |_| {
                                                            start_transaction_sync(
                                                                sync_states,
                                                                toast,
                                                                overview,
                                                                sync_address.clone(),
                                                                is_exchange,
                                                            )
                                                        },
                                                        if sync_state.loading {
                                                            "…"
                                                        } else {
                                                            {icon(ICON_SYNC)}
                                                        }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "Edit address",
                                                        aria_label: "Edit address",
                                                        onclick: move |_| {
                                                            edit_target.set(Some(row.clone()));
                                                            show_import_dialog.set(true);
                                                        },
                                                        {icon(ICON_EDIT)}
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Remove address",
                                                        aria_label: "Remove address",
                                                        onclick: move |_| {
                                                            if confirm_destructive() {
                                                                address_to_remove.set(Some(delete_address.clone()));
                                                            } else {
                                                                remove_address(toast, address_to_remove, overview, delete_address.clone());
                                                            }
                                                        },
                                                        {icon(ICON_DELETE)}
                                                    }
                                                }
                                            }
                                            td {
                                                div { class: "md3-table-copy",
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "Copy address",
                                                        aria_label: "Copy address",
                                                        onclick: move |_| {
                                                            copy_address(toast, copy_target.clone());
                                                        },
                                                        {icon(ICON_COPY)}
                                                    }
                                                    span { "{short_address}" }
                                                }
                                            }
                                            td { "{label}" }
                                            td { "{scope_label}" }
                                            td { "{row.confirmed_tx_count}" }
                                            td { "{format_btc(row.spendable_balance_sats)}" }
                                            td { "{current_price}" }
                                            td { "{first_tx}" }
                                            td { "{last_tx}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        Some(Err(err)) => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "Error loading addresses: {err}"
                }
            }
        }
        None => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant", "Loading addresses..." }
            }
        }
    };

    rsx! {
        section { id: "addresses", class: "md3-screen",
            div { class: "md3-page-header",
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Addresses"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    onclick: move |_| {
                        edit_target.set(None);
                        show_import_dialog.set(true);
                        clipboard_text.set(None);
                        spawn(async move {
                            let Ok(text) = read_clipboard_text().await else {
                                return;
                            };
                            let candidate = text.trim().to_string();
                            if candidate.is_empty() {
                                return;
                            }

                            // Discard clipboard values that rust-bitcoin does
                            // not recognize as mainnet addresses so the
                            // dialog input only ever receives valid input.
                            if let Ok(address) = validate_address_api(candidate).await {
                                clipboard_text.set(Some(address));
                            }
                        });
                    },
                    "Import Address"
                }
            }

            div { class: "md3-kpi-grid",
                Kpi {
                    title: "Addresses".to_string(),
                    value: kpi_values[0].clone(),
                    loading,
                }
                Kpi {
                    title: "Confirmed Tx".to_string(),
                    value: kpi_values[1].clone(),
                    loading,
                }
                Kpi {
                    title: "BTC Balance".to_string(),
                    value: kpi_values[2].clone(),
                    loading,
                }
                Kpi {
                    title: "USD Balance".to_string(),
                    value: kpi_values[3].clone(),
                    loading,
                }
            }

            if overview_ready {
                section { class: "md3-chart-card",
                    div { class: "md3-chart-body",
                        if holdings.is_empty() {
                            p { class: "text-body-medium text-on-surface-variant",
                                "{empty_message}"
                            }
                        } else {
                            HoldingsDistributionChart {
                                entries: holdings,
                                aria_label: "Aggregated coins by address".to_string(),
                            }
                        }
                    }
                }
            }

            {table_content}

            if show_import_dialog() {
                ImportAddressDialog {
                    clipboard_text,
                    mode: edit_target()
                        .map(|item| {
                            ImportAddressDialogMode::Modify(AddressEditData {
                                address: item.address,
                                description: item.description,
                                scope: item.scope,
                            })
                        })
                        .unwrap_or_default(),
                    on_close: move |_| {
                        show_import_dialog.set(false);
                        edit_target.set(None);
                    },
                    on_imported: move |result: AddressImportResult| {
                        let (kind, message) = import_result_toast(&result);
                        toast.show(kind, message);
                        show_import_dialog.set(false);
                        edit_target.set(None);
                        overview.restart();
                    },
                    on_updated: move |updated: AddressEntity| {
                        toast.success(format!("Address {} updated.", format_address(&updated.address)));
                        show_import_dialog.set(false);
                        edit_target.set(None);
                        overview.restart();
                    },
                }
            }

            if let Some(address) = address_to_remove() {
                ConfirmDialog {
                    title: "Remove address".to_string(),
                    message: format!("Are you sure you want to remove {}?", format_address(&address)),
                    confirm_text: "Yes, remove".to_string(),
                    cancel_text: "Cancel".to_string(),
                    on_cancel: move |_| address_to_remove.set(None),
                    on_confirm: move |_| {
                        if let Some(address) = address_to_remove() {
                            remove_address(toast, address_to_remove, overview, address);
                        }
                    },
                }
            }
        }
    }
}

/// Apply a header click to the sort state: clicking the active column
/// toggles its direction; clicking another column selects it with its
/// default direction (text reads A→Z, figures read highest/newest first).
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

/// Whether an overview row is listed in the table and aggregated by the
/// holdings chart.
///
/// Exchange-scoped addresses are only listed while their stored ledger
/// balance is non-zero: the reverse search relates an exchange to every
/// stored address it transacted with, and an exchange address without a
/// balance would crowd the table without telling anything. Wallets and
/// unclassified addresses stay listed at any balance. The backend's global
/// address count applies the same rule, so the "Addresses" KPI matches the
/// listed population.
fn is_listed(item: &AddressOverviewItem) -> bool {
    !item.scope.is_exchange() || item.spendable_balance_sats != 0
}

/// Display label of the Scope column: the owner kind with its catalog name,
/// or `-` for unclassified addresses.
fn scope_label(scope: &AddressScope) -> String {
    match scope {
        AddressScope::Exchange { name, .. } => match name {
            Some(name) => format!("Exchange · {name}"),
            None => "Exchange".to_string(),
        },
        AddressScope::Wallet { name, .. } => match name {
            Some(name) => format!("Wallet · {name}"),
            None => "Wallet".to_string(),
        },
        AddressScope::Unassigned => "-".to_string(),
    }
}

/// Toast for a finished import, including the indexing outcome.
fn import_result_toast(result: &AddressImportResult) -> (ToastKind, String) {
    let imported = format_address(&result.address.address);

    if let Some(error) = &result.sync_error {
        (
            ToastKind::Error,
            format!(
                "Imported {imported}, but transaction indexing failed: {error}. \
                 Use the sync action to retry."
            ),
        )
    } else if result.address.tx_index_rate_limited {
        (
            ToastKind::Info,
            format!(
                "Imported {imported}. Transaction indexing paused on the mempool rate limit; \
                 use the sync action to resume from where it stopped."
            ),
        )
    } else {
        (ToastKind::Success, format!("Imported {imported}."))
    }
}

/// Delete the selected address after confirmation and refresh the page.
///
/// The stored transactions of the address cascade away with it, so the KPIs
/// and the transaction columns update on the refresh.
fn remove_address(
    mut toast: Toast,
    mut address_to_remove: Signal<Option<String>>,
    mut overview: Resource<Result<AddressOverview, String>>,
    address: String,
) {
    spawn(async move {
        match delete_address_api(address.clone()).await {
            Ok(()) => {
                toast.success(format!(
                    "Address {} removed successfully!",
                    format_address(&address)
                ));
            }
            Err(error) => {
                toast.error(format!(
                    "Could not remove address {}: {error}",
                    format_address(&address)
                ));
            }
        }

        address_to_remove.set(None);
        overview.restart();
    });
}

/// Copy one address to the clipboard and report the outcome in a toast.
fn copy_address(mut toast: Toast, address: String) {
    spawn(async move {
        match write_clipboard_text(&address).await {
            Ok(()) => {
                toast.success(format!(
                    "Address {} copied to clipboard!",
                    format_address(&address)
                ));
            }
            Err(error) => toast.error(format!("Could not copy the address: {error}")),
        }
    });
}

/// Start a sync and keep the row action disabled until the server operation
/// finishes; the outcome is reported in a toast.
///
/// Exchange addresses use the reverse search (`related_search`); their button
/// labels and result messages describe that mode so the two sync behaviors
/// are not confused in the UI.
fn start_transaction_sync(
    mut sync_states: Signal<BTreeMap<String, SyncState>>,
    mut toast: Toast,
    mut overview: Resource<Result<AddressOverview, String>>,
    address: String,
    related_search: bool,
) {
    if sync_states
        .read()
        .get(&address)
        .is_some_and(|state| state.loading)
    {
        return;
    }

    sync_states
        .write()
        .insert(address.clone(), SyncState { loading: true });

    spawn(async move {
        match sync_address_transactions_api(address.clone()).await {
            Ok(summary) => {
                let (kind, message) = sync_summary_toast(&summary, related_search);
                toast.show(kind, message);
            }
            Err(error) => toast.error(format!("Could not sync transactions: {error}")),
        }

        sync_states
            .write()
            .insert(address, SyncState { loading: false });
        overview.restart();
    });
}

fn sync_summary_toast(
    summary: &TransactionSyncSummary,
    related_search: bool,
) -> (ToastKind, String) {
    if summary.rate_limited {
        return (
            ToastKind::Info,
            format!(
                "Indexing paused on the mempool rate limit after storing {} transactions. \
                 Sync again to resume from where it stopped.",
                summary.stored
            ),
        );
    }

    let (kind, message) = if summary.fetched == 0 {
        let message = if related_search {
            "No related transactions found yet. Sync your other stored addresses first; \
             related transactions appear once their indexing covers shared transfers."
                .to_string()
        } else {
            "No transactions found.".to_string()
        };
        (ToastKind::Info, message)
    } else if summary.inserted == 0 {
        (
            ToastKind::Success,
            format!(
                "Already up to date: {} transactions stored.",
                summary.stored
            ),
        )
    } else {
        (
            ToastKind::Success,
            format!(
                "Stored {} transactions ({} new).",
                summary.stored, summary.inserted
            ),
        )
    };

    if summary.truncated {
        (
            kind,
            format!("{message} The page limit was reached; more history may remain."),
        )
    } else {
        (kind, message)
    }
}

// Material icon paths (24px viewBox) for the table actions and copy cell.
const ICON_SYNC: &str = "M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z";
const ICON_EDIT: &str = "M3 17.25V21h3.75L17.81 9.94l-3.75-3.75L3 17.25zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.9959.9959 0 0 0-1.41 0l-1.83 1.83 3.75 3.75 1.83-1.83z";
const ICON_DELETE: &str =
    "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z";
const ICON_COPY: &str = "M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z";
const ICON_WARNING: &str = "M1 21h22L12 2 1 21zm12-3h-2v-2h2v2zm0-4h-2v-4h2v4z";

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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(scope: AddressScope, spendable_balance_sats: i64) -> AddressOverviewItem {
        AddressOverviewItem {
            address: "bc1qtest".to_string(),
            description: None,
            address_type: None,
            spendable_balance_sats,
            confirmed_tx_count: 0,
            first_tx_time: None,
            last_tx_time: None,
            scope,
            tx_index_rate_limited: false,
        }
    }

    fn exchange() -> AddressScope {
        AddressScope::Exchange { id: 1, name: None }
    }

    #[test]
    fn exchange_addresses_are_listed_only_with_a_balance() {
        assert!(!is_listed(&item(exchange(), 0)));
        assert!(is_listed(&item(exchange(), 1)));
        assert!(is_listed(&item(exchange(), -1)));
    }

    #[test]
    fn wallet_and_unassigned_addresses_are_listed_at_any_balance() {
        let wallet = AddressScope::Wallet { id: 1, name: None };

        assert!(is_listed(&item(wallet, 0)));
        assert!(is_listed(&item(AddressScope::Unassigned, 0)));
    }
}
