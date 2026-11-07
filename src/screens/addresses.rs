use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::addresses::{addresses_overview_api, delete_addresses_api, validate_address_api};
use crate::api::prices::latest_price_api;
use crate::api::settings::get_exchange_settings_api;
use crate::api::transactions::{sync_address_transactions_api, sync_addresses_transactions_api};
use crate::components::format::{format_address, format_btc, format_fiat};
use crate::components::{
    read_clipboard_text, use_fiat_currency, use_table_selection, use_toast, write_clipboard_text,
    AddressEditData, BadgeSize, BalanceKpi, ConfirmDialog, HoldingSlice, HoldingsDistributionChart,
    ImportAddressDialog, ImportAddressDialogMode, Kpi, ProviderBadge, SortableHeader,
    TablePagination, TableSelection, TableSelectionCell, TableSelectionHeader, Toast, ToastKind,
};
use crate::domain::address::{
    AddressEntity, AddressImportResult, AddressOverview, AddressOverviewItem, AddressOverviewQuery,
    AddressScope, AddressScopeFilter,
};
use crate::domain::civil_time::format_datetime;
use crate::domain::price::sats_to_fiat;
use crate::domain::table::{SortDirection, TableQuery, DEFAULT_PAGE_SIZE};
use crate::domain::transaction::{TransactionSyncBatchSummary, TransactionSyncSummary};

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
    let mut addresses_to_remove = use_signal(|| Option::<Vec<String>>::None);
    let sync_states = use_signal(BTreeMap::<String, SyncState>::new);
    let bulk_sync_loading = use_signal(|| false);
    let mut selection = use_table_selection::<String>();
    // Backend-driven table state (Section 5.11 of CONTRIBUTING.md): the page,
    // sort, and scope filter are part of the overview query, so changing them
    // re-runs the request. The response still carries every matching row for
    // the KPIs and the holdings chart, plus the requested table page.
    let mut page = use_signal(|| 0_u32);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(SortDirection::default);
    let mut scope_filter = use_signal(AddressScopeFilter::default);
    let mut confirm_destructive = use_signal(|| true);
    let fiat = use_fiat_currency();

    let mut overview = use_resource(move || async move {
        addresses_overview_api(AddressOverviewQuery {
            table: TableQuery {
                page: page(),
                page_size: DEFAULT_PAGE_SIZE,
                sort_by: sort_by(),
                sort_dir: sort_dir(),
            },
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
    let latest_price = use_resource(move || {
        let symbol = fiat.code();
        async move { latest_price_api(symbol).await.ok().map(|price| price.close) }
    });

    // A deletion can empty the current page while earlier pages still have
    // rows; step back until the page has content again.
    use_effect(move || {
        if let Some(Ok(data)) = overview() {
            if data.page.items.is_empty() && page() > 0 {
                page.set(page() - 1);
                selection.clear();
            }
        }
    });

    let overview_state = overview();
    let loading = overview_state.is_none();
    let overview_ready = matches!(&overview_state, Some(Ok(_)));
    let symbol = fiat.code();
    let latest_value = latest_price().flatten();
    let empty_message = format!("No {} balances", scope_filter().empty_noun());
    // Exchange balances come from the imported ledger, so the second card
    // counts ledger deposits and withdrawals instead of on-chain confirmed
    // transactions while the exchange scope is selected.
    let tx_kpi_title = if scope_filter() == AddressScopeFilter::Exchanges {
        "Deposits & Withdrawals"
    } else {
        "Transactions"
    };

    // The KPI values cover the same filtered population as the table and the
    // treemap: the backend excludes exchange addresses without a ledger
    // balance from the address count, the transaction count, and the balance.
    // In the exchange scope the transaction count is the number of ledger
    // deposits and withdrawals, not on-chain confirmed transactions.
    let kpi_values: [Option<String>; 2] = match &overview_state {
        Some(Ok(data)) => [
            Some(data.address_count.to_string()),
            Some(data.tx_count.to_string()),
        ],
        _ => [None, None],
    };
    // The fiat equivalent lives in the balance card's details overlay instead
    // of a second card, matching the exchange amount cards.
    let balance_sats = match &overview_state {
        Some(Ok(data)) => Some(data.spendable_balance_sats),
        _ => None,
    };

    // The table page and the treemap show the same listing, so the Scope chips
    // select the population of both views: the backend already filters by
    // scope and drops the balance-less exchange addresses from both `items`
    // and `page`.
    let listed_items: Vec<AddressOverviewItem> = match &overview_state {
        Some(Ok(data)) => data.items.clone(),
        _ => Vec::new(),
    };
    let table_items: Vec<AddressOverviewItem> = match &overview_state {
        Some(Ok(data)) => data.page.items.clone(),
        _ => Vec::new(),
    };
    let table_total = match &overview_state {
        Some(Ok(data)) => data.page.total,
        _ => 0,
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

    let sort_direction_for = |key: &str| (sort_by().as_deref() == Some(key)).then_some(sort_dir());

    let page_ids = table_items
        .iter()
        .map(|item| item.address.clone())
        .collect::<Vec<_>>();
    let selected_snapshot = selection.snapshot();
    let selected_count = selection.len();
    let all_selected = selection.all_selected(&page_ids);
    let selectable_ids = page_ids.clone();
    // A bulk sync and a row sync must not overlap on the same address: the
    // batch endpoint would duplicate the network walk of a selected address
    // that is already syncing.
    let selected_syncing = {
        let states = sync_states.read();
        selected_snapshot
            .iter()
            .any(|address| states.get(address).is_some_and(|state| state.loading))
    };
    let sync_busy = bulk_sync_loading() || selected_syncing;
    let bulk_sync_action_label = if selected_count == 0 {
        "Select addresses to sync transactions".to_string()
    } else {
        format!(
            "Sync transactions for {selected_count} {}",
            if selected_count == 1 {
                "address"
            } else {
                "addresses"
            }
        )
    };
    let delete_action_label = if selected_count == 0 {
        "Select addresses to delete".to_string()
    } else {
        format!(
            "Delete {selected_count} {}",
            if selected_count == 1 {
                "address"
            } else {
                "addresses"
            }
        )
    };

    let table_content = match &overview_state {
        Some(Ok(_)) => rsx! {
            div { class: "md3-table-container",
                table { class: "md3-table",
                    thead {
                        tr {
                            TableSelectionHeader {
                                all_selected,
                                indeterminate: selection.some_selected(&page_ids)
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    && !all_selected,
                                select_all_label: "Select all addresses on this page".to_string(),
                                disabled: page_ids.is_empty(),
                                on_toggle_all: move |checked| {
                                    selection.set_many(&selectable_ids, checked);
                                },
                                div { class: "md3-table-actions",
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button",
                                        disabled: selection.is_empty() || sync_busy,
                                        title: "{bulk_sync_action_label}",
                                        aria_label: "{bulk_sync_action_label}",
                                        onclick: move |_| {
                                            let addresses = selection.selected_ids();
                                            start_bulk_transaction_sync(
                                                sync_states,
                                                bulk_sync_loading,
                                                toast,
                                                overview,
                                                selection,
                                                addresses,
                                            );
                                        },
                                        if bulk_sync_loading() {
                                            "…"
                                        } else {
                                            {icon(ICON_SYNC)}
                                        }
                                    }
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-icon-button-danger",
                                        disabled: selection.is_empty() || sync_busy,
                                        title: "{delete_action_label}",
                                        aria_label: "{delete_action_label}",
                                        onclick: move |_| {
                                            let addresses = selection.selected_ids();
                                            start_address_delete(
                                                toast,
                                                addresses_to_remove,
                                                overview,
                                                selection,
                                                confirm_destructive(),
                                                addresses,
                                            );
                                        },
                                        {icon(ICON_DELETE)}
                                    }
                                }
                            }
                            SortableHeader {
                                label: "Address".to_string(),
                                active_direction: sort_direction_for("address"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "address", SortDirection::Asc);
                                    selection.clear();
                                },
                            }
                            SortableHeader {
                                label: "Label".to_string(),
                                active_direction: sort_direction_for("label"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "label", SortDirection::Asc);
                                    selection.clear();
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
                                                onclick: move |_| {
                                                    scope_filter.set(filter);
                                                    page.set(0);
                                                    selection.clear();
                                                },
                                                "{filter_label}"
                                            }
                                        }
                                    }
                                }
                            }
                            SortableHeader {
                                label: "Txs".to_string(),
                                active_direction: sort_direction_for("txs"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "txs", SortDirection::Desc);
                                    selection.clear();
                                },
                            }
                            SortableHeader {
                                label: "Spendable Balance".to_string(),
                                active_direction: sort_direction_for("balance"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "balance", SortDirection::Desc);
                                    selection.clear();
                                },
                            }
                            SortableHeader {
                                label: "Current price".to_string(),
                                active_direction: sort_direction_for("price"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "price", SortDirection::Desc);
                                    selection.clear();
                                },
                            }
                            SortableHeader {
                                label: "First received tx".to_string(),
                                active_direction: sort_direction_for("first_tx"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "first_tx", SortDirection::Desc);
                                    selection.clear();
                                },
                            }
                            SortableHeader {
                                label: "Last sent tx".to_string(),
                                active_direction: sort_direction_for("last_tx"),
                                on_sort: move |_| {
                                    toggle_sort(sort_by, sort_dir, page, "last_tx", SortDirection::Desc);
                                    selection.clear();
                                },
                            }
                        }
                    }
                    tbody {
                        if table_items.is_empty() {
                            tr {
                                td {
                                    colspan: "10",
                                    class: "md3-table-empty text-body-medium",
                                    "{empty_message}"
                                }
                            }
                        } else {
                            for item in &table_items {
                                {
                                    let row = item.clone();
                                    let sync_address = row.address.clone();
                                    let delete_address = row.address.clone();
                                    let selection_address = row.address.clone();
                                    let row_selected = selected_snapshot.contains(&selection_address);
                                    let copy_target = row.address.clone();
                                    let short_address = format_address(&row.address);
                                    let label = row
                                        .description
                                        .clone()
                                        .unwrap_or_else(|| "-".to_string());
                                    let current_value = latest_value
                                        .map(|price| {
                                            format_fiat(sats_to_fiat(row.spendable_balance_sats, price), &symbol)
                                        })
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
                                    rsx! {
                                        tr { class: if row_selected { "md3-table-row-selected" } else { "" },
                                            TableSelectionCell {
                                                checked: row_selected,
                                                label: format!("Select {}", format_address(&selection_address)),
                                                on_change: move |checked| {
                                                    selection.set(selection_address.clone(), checked);
                                                },
                                            }
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
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Remove address",
                                                        aria_label: "Remove address",
                                                        onclick: move |_| {
                                                            start_address_delete(
                                                                toast,
                                                                addresses_to_remove,
                                                                overview,
                                                                selection,
                                                                confirm_destructive(),
                                                                vec![delete_address.clone()],
                                                            );
                                                        },
                                                        {icon(ICON_DELETE)}
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
                                            td { {scope_cell(&row.scope)} }
                                            td { "{row.confirmed_tx_count}" }
                                            td { "{format_btc(row.spendable_balance_sats)}" }
                                            td { "{current_value}" }
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
            TablePagination {
                page: page(),
                page_size: DEFAULT_PAGE_SIZE,
                total: table_total,
                on_page: move |next| {
                    page.set(next);
                    selection.clear();
                },
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

            div { class: "md3-kpi-grid-three",
                Kpi {
                    title: "Addresses".to_string(),
                    value: kpi_values[0].clone(),
                    loading,
                }
                Kpi {
                    title: tx_kpi_title.to_string(),
                    value: kpi_values[1].clone(),
                    loading,
                }
                BalanceKpi {
                    balance_sats,
                    latest_price: latest_value,
                    currency: symbol.clone(),
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

            if let Some(addresses) = addresses_to_remove() {
                {
                    let title = if addresses.len() == 1 {
                        "Remove address".to_string()
                    } else {
                        "Remove addresses".to_string()
                    };
                    let message = if addresses.len() == 1 {
                        format!("Are you sure you want to remove {}?", format_address(&addresses[0]))
                    } else {
                        format!(
                            "Remove {} selected addresses? Their stored transactions will also be removed. This cannot be undone.",
                            addresses.len(),
                        )
                    };
                    rsx! {
                        ConfirmDialog {
                            title,
                            message,
                            confirm_text: "Yes, remove".to_string(),
                            cancel_text: "Cancel".to_string(),
                            on_cancel: move |_| addresses_to_remove.set(None),
                            on_confirm: move |_| {
                                if let Some(addresses) = addresses_to_remove() {
                                    remove_addresses(toast, selection, addresses_to_remove, overview, addresses);
                                }
                            },
                        }
                    }
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
    mut page: Signal<u32>,
    key: &str,
    default_direction: SortDirection,
) {
    if sort_by().as_deref() == Some(key) {
        sort_dir.set(sort_dir().toggled());
    } else {
        sort_by.set(Some(key.to_string()));
        sort_dir.set(default_direction);
    }
    page.set(0);
}

/// Scope cell: the provider's brand badge, its kind, and its catalog name,
/// or `-` for unclassified addresses.
fn scope_cell(scope: &AddressScope) -> Element {
    let (kind, name, color) = match scope {
        AddressScope::Exchange {
            name, brand_color, ..
        } => ("Exchange", name.clone(), brand_color.clone()),
        AddressScope::Wallet {
            name, brand_color, ..
        } => ("Wallet", name.clone(), brand_color.clone()),
        AddressScope::Unassigned => return rsx! { "-" },
    };
    match name {
        Some(label) => rsx! {
            span { class: "md3-scope-cell",
                ProviderBadge {
                    name: label.clone(),
                    color,
                    size: BadgeSize::Compact,
                }
                span { "{kind} · {label}" }
            }
        },
        None => rsx! {
            span { class: "md3-scope-cell",
                ProviderBadge {
                    name: kind.to_string(),
                    color,
                    size: BadgeSize::Compact,
                }
                span { "{kind}" }
            }
        },
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

/// Open the removal confirmation for the given addresses, or delete them
/// immediately when the "Confirm destructive actions" setting is off.
fn start_address_delete(
    toast: Toast,
    mut addresses_to_remove: Signal<Option<Vec<String>>>,
    overview: Resource<Result<AddressOverview, String>>,
    selection: TableSelection<String>,
    confirm_destructive: bool,
    addresses: Vec<String>,
) {
    if addresses.is_empty() {
        return;
    }
    if confirm_destructive {
        addresses_to_remove.set(Some(addresses));
    } else {
        remove_addresses(toast, selection, addresses_to_remove, overview, addresses);
    }
}

/// Delete the selected addresses after confirmation and refresh the page.
///
/// The stored transactions of the addresses cascade away with them, so the
/// KPIs and the transaction columns update on the refresh.
fn remove_addresses(
    mut toast: Toast,
    mut selection: TableSelection<String>,
    mut addresses_to_remove: Signal<Option<Vec<String>>>,
    mut overview: Resource<Result<AddressOverview, String>>,
    addresses: Vec<String>,
) {
    let count = addresses.len();
    spawn(async move {
        match delete_addresses_api(addresses.clone()).await {
            Ok(0) => toast.error("No addresses were found to remove."),
            Ok(_) if addresses.len() == 1 => {
                toast.success(format!(
                    "Address {} removed successfully!",
                    format_address(&addresses[0])
                ));
            }
            Ok(deleted) => {
                toast.success(if deleted == 1 {
                    "1 address removed successfully!".to_string()
                } else {
                    format!("{deleted} addresses removed successfully!")
                });
            }
            Err(error) => {
                if count == 1 {
                    toast.error(format!(
                        "Could not remove address {}: {error}",
                        format_address(&addresses[0])
                    ));
                } else {
                    toast.error(format!("Could not remove the addresses: {error}"));
                }
            }
        }

        selection.clear();
        addresses_to_remove.set(None);
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

/// Sync the transaction histories of every checked address through one batch
/// request and report the aggregated outcome in a toast.
///
/// The checked rows are marked as loading for the duration so their row
/// actions reflect the running batch. The selection is kept when at least
/// one address failed — the endpoint is idempotent, so retrying the same
/// selection is safe — and cleared once the whole batch succeeds.
fn start_bulk_transaction_sync(
    mut sync_states: Signal<BTreeMap<String, SyncState>>,
    mut bulk_sync_loading: Signal<bool>,
    mut toast: Toast,
    mut overview: Resource<Result<AddressOverview, String>>,
    mut selection: TableSelection<String>,
    addresses: Vec<String>,
) {
    if bulk_sync_loading() {
        return;
    }
    if addresses.is_empty() {
        toast.error("Select at least one address.");
        return;
    }
    // The button is disabled while a selected row syncs, but check again at
    // click time so a render queued before that state changed cannot start
    // two overlapping walks of the same address.
    let already_syncing = {
        let states = sync_states.read();
        addresses
            .iter()
            .any(|address| states.get(address).is_some_and(|state| state.loading))
    };
    if already_syncing {
        toast.show(
            ToastKind::Info,
            "Wait for the running syncs to finish before syncing the selection.",
        );
        return;
    }

    bulk_sync_loading.set(true);
    {
        let mut states = sync_states.write();
        for address in &addresses {
            states.insert(address.clone(), SyncState { loading: true });
        }
    }

    spawn(async move {
        let all_synced = match sync_addresses_transactions_api(addresses.clone()).await {
            Ok(summary) => {
                let (kind, message) = bulk_sync_summary_toast(&summary);
                toast.show(kind, message);
                summary.failed.is_empty()
            }
            Err(error) => {
                toast.error(format!("Could not sync the selected addresses: {error}"));
                false
            }
        };

        {
            let mut states = sync_states.write();
            for address in &addresses {
                states.insert(address.clone(), SyncState { loading: false });
            }
        }
        bulk_sync_loading.set(false);
        if all_synced {
            selection.clear();
        }
        overview.restart();
    });
}

/// Toast for a finished bulk sync: the aggregated totals of the addresses
/// that completed, followed by any rate-limited, truncated, or failed ones.
fn bulk_sync_summary_toast(summary: &TransactionSyncBatchSummary) -> (ToastKind, String) {
    let synced = summary.synced.len();
    let failed = summary.failed.len();

    if synced == 0 {
        let message = match summary.failed.first() {
            Some(failure) if failed == 1 => format!(
                "Could not sync {}: {}",
                format_address(&failure.address),
                failure.error
            ),
            Some(failure) => format!(
                "Could not sync {failed} addresses. First error for {}: {}",
                format_address(&failure.address),
                failure.error
            ),
            None => "No addresses were selected.".to_string(),
        };
        return (ToastKind::Error, message);
    }

    let mut message = format!(
        "Synced {synced} {}: {} transactions stored ({} new).",
        address_noun(synced),
        summary.stored(),
        summary.inserted()
    );

    let rate_limited = summary.rate_limited_count();
    if rate_limited > 0 {
        message.push_str(&format!(
            " {rate_limited} {} paused on the mempool rate limit; sync again to resume.",
            address_noun(rate_limited)
        ));
    }

    let truncated = summary.truncated_count();
    if truncated > 0 {
        message.push_str(&format!(
            " {truncated} {} reached the page limit; more history may remain.",
            address_noun(truncated)
        ));
    }

    if let Some(failure) = summary.failed.first() {
        message.push_str(&format!(
            " {failed} {} failed ({}: {}).",
            address_noun(failed),
            format_address(&failure.address),
            failure.error
        ));
    }

    let kind = if failed > 0 {
        ToastKind::Error
    } else if rate_limited > 0 {
        ToastKind::Info
    } else {
        ToastKind::Success
    };

    (kind, message)
}

/// `address` / `addresses` for the sync result messages.
fn address_noun(count: usize) -> &'static str {
    if count == 1 {
        "address"
    } else {
        "addresses"
    }
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
    use crate::domain::transaction::TransactionSyncFailure;

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
        AddressScope::Exchange {
            id: 1,
            name: None,
            brand_color: None,
        }
    }

    #[test]
    fn exchange_addresses_are_listed_only_with_a_balance() {
        assert!(!item(exchange(), 0).is_listed());
        assert!(item(exchange(), 1).is_listed());
        assert!(item(exchange(), -1).is_listed());
    }

    #[test]
    fn wallet_and_unassigned_addresses_are_listed_at_any_balance() {
        let wallet = AddressScope::Wallet {
            id: 1,
            name: None,
            brand_color: None,
        };

        assert!(item(wallet, 0).is_listed());
        assert!(item(AddressScope::Unassigned, 0).is_listed());
    }

    fn sync_summary(address: &str, stored: usize, inserted: usize) -> TransactionSyncSummary {
        TransactionSyncSummary {
            address: address.to_string(),
            fetched: stored,
            inserted,
            stored,
            truncated: false,
            rate_limited: false,
        }
    }

    #[test]
    fn bulk_sync_toast_aggregates_totals_and_names_the_first_failure() {
        let summary = TransactionSyncBatchSummary {
            synced: vec![
                sync_summary("bc1qfirstaddress", 10, 2),
                sync_summary("bc1qsecondaddress", 5, 0),
            ],
            failed: vec![TransactionSyncFailure {
                address: "bc1qthirdaddress".to_string(),
                error: "address not found".to_string(),
            }],
        };

        let (kind, message) = bulk_sync_summary_toast(&summary);

        assert_eq!(kind, ToastKind::Error);
        assert!(message.contains("Synced 2 addresses"));
        assert!(message.contains("15 transactions stored (2 new)"));
        assert!(message.contains("1 address failed"));
        assert!(message.contains("address not found"));
    }

    #[test]
    fn bulk_sync_toast_reports_a_fully_failed_batch_as_an_error() {
        let summary = TransactionSyncBatchSummary {
            synced: Vec::new(),
            failed: vec![TransactionSyncFailure {
                address: "bc1qfirstaddress".to_string(),
                error: "mempool request failed".to_string(),
            }],
        };

        let (kind, message) = bulk_sync_summary_toast(&summary);

        assert_eq!(kind, ToastKind::Error);
        assert!(message.contains("Could not sync"));
        assert!(message.contains("mempool request failed"));
    }

    #[test]
    fn bulk_sync_toast_reports_rate_limited_addresses_as_info() {
        let summary = TransactionSyncBatchSummary {
            synced: vec![TransactionSyncSummary {
                rate_limited: true,
                ..sync_summary("bc1qfirstaddress", 4, 1)
            }],
            failed: Vec::new(),
        };

        let (kind, message) = bulk_sync_summary_toast(&summary);

        assert_eq!(kind, ToastKind::Info);
        assert!(message.contains("1 address paused on the mempool rate limit"));
    }
}
