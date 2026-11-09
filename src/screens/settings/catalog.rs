//! Wallet Types / Exchange Types sub-settings: the provider table, filters,
//! KPIs, CRUD dialogs, cascade deletion, and CSV import.
use crate::api::catalogs::{
    catalog_types_delete_preview_api, delete_catalog_types_api, list_catalog_categories_api,
    list_catalog_types_api,
};
use crate::components::{
    catalog_delete_summary_message, use_interface_settings, use_table_selection, use_table_state,
    use_toast, BadgeSize, CatalogCategoriesDialog, CatalogDeleteDialog, CatalogImportDialog,
    CatalogTypeDialog, Icon, IconName, Kpi, PageHeader, ProviderBadge, SortableHeader,
    TablePagination, TableSelection, TableSelectionCell, TableSelectionHeader, Toast,
};
use crate::domain::catalog::{
    CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview, CatalogKind, CatalogType,
    CatalogTypePage,
};
use crate::domain::table::{SortDirection, DEFAULT_PAGE_SIZE};
use dioxus::prelude::*;
#[component]
pub fn SettingsWalletTypes() -> Element {
    rsx! {
        CatalogSettingsPage { kind: CatalogKind::Wallet }
    }
}
#[component]
pub fn SettingsExchangeTypes() -> Element {
    rsx! {
        CatalogSettingsPage { kind: CatalogKind::Exchange }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct DeleteRequest {
    title: String,
    ids: Vec<i64>,
    preview: CatalogDeletePreview,
}
#[component]
fn CatalogSettingsPage(kind: CatalogKind) -> Element {
    let mut toast = use_toast();
    let mut table = use_table_state(SortDirection::Asc);
    let mut selection = use_table_selection::<i64>();
    let mut show_type_dialog = use_signal(|| false);
    let mut edit_target = use_signal(|| Option::<CatalogType>::None);
    let mut show_categories = use_signal(|| false);
    let mut show_import = use_signal(|| false);
    let mut delete_request = use_signal(|| Option::<DeleteRequest>::None);
    let deleting = use_signal(|| false);
    let confirm_destructive = use_interface_settings().get().confirm_destructive_actions;
    let mut categories = use_resource(move || async move {
        list_catalog_categories_api(kind.as_str().to_string())
            .await
            .map_err(|error| error.to_string())
    });
    let categories_state = categories();
    let (category_options, categories_error) = match &categories_state {
        Some(Ok(list)) => (list.clone(), None),
        Some(Err(error)) => (Vec::new(), Some(error.clone())),
        None => (Vec::new(), None),
    };
    let mut list = use_resource(move || {
        let query = table.query(DEFAULT_PAGE_SIZE);
        async move {
            list_catalog_types_api(kind.as_str().to_string(), query)
                .await
                .map_err(|error| error.to_string())
        }
    });
    use_effect(move || {
        if let Some(Ok(data)) = list() {
            if data.page.items.is_empty() && table.page() > 0 {
                table.page.set(table.page() - 1);
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
        totals.map(|totals| totals.providers.to_string()),
        totals.map(|totals| totals.categories.to_string()),
        totals.map(|totals| totals.classified_addresses.to_string()),
        totals.map(|totals| totals.uncategorized.to_string()),
    ];
    let items = match &state {
        Some(Ok(data)) => data.page.items.clone(),
        _ => Vec::new(),
    };
    let page_ids = items.iter().map(|row| row.id).collect::<Vec<_>>();
    let selected_snapshot = selection.snapshot();
    let selected_count = selection.len();
    let delete_action_label = if selected_count == 0 {
        format!("Select {} to delete", kind.plural_label())
    } else {
        format!(
            "Delete {selected_count} {}",
            if selected_count == 1 {
                kind.singular_label()
            } else {
                kind.plural_label()
            },
        )
    };
    let all_selected = selection.all_selected(&page_ids);
    let confirm_delete = move |_| {
        if let Some(request) = delete_request() {
            perform_delete(
                kind,
                request.ids,
                toast,
                list,
                categories,
                selection,
                delete_request,
                deleting,
            );
        }
    };
    let content = match &state {
        None => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "Loading {kind.plural_label()}..."
                }
            }
        }
        Some(Err(error)) => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "Error loading {kind.plural_label()}: {error}"
                }
            }
        }
        Some(Ok(_)) if items.is_empty() => {
            rsx! {
                p { class: "text-body-medium text-on-surface-variant",
                    "No {kind.plural_label()} yet. Create one or import a CSV."
                }
            }
        }
        Some(Ok(data)) => {
            let total = data.page.total;
            let selectable_ids = page_ids.clone();
            let table = rsx! {
                div { class: "md3-table-container",
                    table { class: "md3-table md3-catalog-table",
                        thead {
                            tr {
                                TableSelectionHeader {
                                    all_selected,
                                    indeterminate: selection.some_selected(& page_ids) && !
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    all_selected,
                                    select_all_label: "Select all types on this page"
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    .to_string(),
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
                                            let ids = selection.selected_ids();
                                            if ids.is_empty() {
                                                return;
                                            }
                                            let count = ids.len();
                                            let title = format!(
                                                "Delete {count} selected {}",
                                                if count == 1 { kind.singular_label() } else { kind.plural_label() },
                                            );
                                            start_delete(
                                                kind,
                                                ids,
                                                title,
                                                confirm_destructive,
                                                toast,
                                                delete_request,
                                                list,
                                                categories,
                                                selection,
                                                deleting,
                                            );
                                        },
                                        Icon { name: IconName::Delete, size: 20 }
                                    }
                                }
                                SortableHeader {
                                    label: "Name"
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    .to_string(),
                                    active_direction: table.direction_for("name"),
                                    on_sort: move |_| {
                                        table.sort("name", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Category".to_string(),
                                    active_direction: table.direction_for("category"),
                                    on_sort: move |_| {
                                        table.sort("category", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                SortableHeader {
                                    label: "Custody".to_string(),
                                    active_direction: table
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    .direction_for("custody"),
                                    on_sort: move |_| {
                                        table.sort("custody", SortDirection::Asc);
                                        selection.clear();
                                    },
                                }
                                if kind == CatalogKind::Wallet {
                                    SortableHeader {
                                        label: "Platform".to_string(),
                                        active_direction: table.direction_for("platform"),
                                        on_sort: move |_| {
                                            table.sort("platform", SortDirection::Asc);
                                            selection.clear();
                                        },
                                    }
                                }
                                SortableHeader {
                                    label: "Addresses".to_string(),
                                    active_direction: table.direction_for("addresses"),
                                    on_sort: move |_| {
                                        table.sort("addresses", SortDirection::Desc);
                                        selection.clear();
                                    },
                                }
                            }
                        }
                        tbody {
                            for row in &items {
                                {
                                    let row_id = row.id;
                                    let row_name = row.name.clone();
                                    let row_slug = row.slug.clone();
                                    let row_category = row.category_name.clone();
                                    let row_custody = row.custody_model;
                                    let row_platform = row.platform;
                                    let row_color = row.brand_color.clone();
                                    let row_count = row.address_count;
                                    let row_checked = selected_snapshot.contains(&row_id);
                                    let title = format!("Delete \"{row_name}\"");
                                    let edit_row = row.clone();
                                    rsx! {
                                        tr { class: if row_checked { "md3-table-row-selected" } else { "" },
                                            TableSelectionCell {
                                                checked: row_checked,
                                                label: format!("Select {row_name}"),
                                                on_change: move |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                checked | selection.set(row_id, checked),
                                            }
                                            td {
                                                div { class: "md3-table-actions",
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button md3-icon-button-danger",
                                                        title: "Delete {row_name}",
                                                        aria_label: "Delete {row_name}",
                                                        onclick: move |_| {
                                                            start_delete(
                                                                kind,
                                                                vec![row_id],
                                                                title.clone(),
                                                                confirm_destructive,
                                                                toast,
                                                                delete_request,
                                                                list,
                                                                categories,
                                                                selection,
                                                                deleting,
                                                            );
                                                        },
                                                        Icon { name: IconName::Delete, size: 20 }
                                                    }
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "Edit {row_name}",
                                                        aria_label: "Edit {row_name}",
                                                        onclick: move |_| {
                                                            edit_target.set(Some(edit_row.clone()));
                                                            show_type_dialog.set(true);
                                                        },
                                                        Icon { name: IconName::Edit, size: 20 }
                                                    }
                                                }
                                            }
                                            td {
                                                div { class: "md3-catalog-name-cell",
                                                    ProviderBadge {
                                                        name: row_name.clone(),
                                                        color: row_color.clone(),
                                                        size: BadgeSize::Compact,
                                                    }
                                                    div {
                                                        strong { "{row_name}" }
                                                        span { class: "text-body-small text-on-surface-variant", "{row_slug}" }
                                                    }
                                                }
                                            }
                                            td {
                                                if let Some(category) = row_category {
                                                    span { class: "md3-category-chip", "{category}" }
                                                } else {
                                                    span { class: "md3-category-chip md3-category-chip-empty", "Uncategorized" }
                                                }
                                            }
                                            td { "{row_custody.map(|model| model.label()).unwrap_or(\"—\")}" }
                                            if kind == CatalogKind::Wallet {
                                                td { "{row_platform.map(|value| value.label()).unwrap_or(\"—\")}" }
                                            }
                                            td { class: "md3-number-cell", "{row_count}" }
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
            };
            rsx! {
                {table}
            }
        }
    };
    rsx! {
        PageHeader { title: kind.label().to_string(),
            div { class: "md3-page-actions",
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            show_categories.set(true),
                    "Manage categories"
                }
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    onclick: move |_| show_import.set(true),
                    "Import CSV"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    onclick: move |_| {
                        edit_target.set(None);
                        show_type_dialog.set(true);
                    },
                    "New {kind.singular_label()}"
                }
            }
        }
        div { class: "md3-kpi-grid",
            Kpi {
                title: kind.label().to_string(),
                value: kpis[0].clone(),
                loading,
            }
            Kpi {
                title: "Categories".to_string(),
                value: kpis[1].clone(),
                loading,
            }
            Kpi {
                title: "Classified addresses".to_string(),
                value: kpis[2].clone(),
                loading,
            }
            Kpi {
                title: "Uncategorized".to_string(),
                value: kpis[3].clone(),
                loading,
            }
        }
        if let Some(error) = categories_error {
            p { class: "text-body-small md3-settings-error", "Could not load categories: {error}" }
        }
        {content}
        if show_type_dialog() {
            CatalogTypeDialog {
                kind,
                entry: edit_target(),
                categories: category_options.clone(),
                on_close: move |_| {
                    show_type_dialog.set(false);
                    edit_target.set(None);
                },
                on_saved: move |saved: CatalogType| {
                    toast.success(format!("\"{}\" saved.", saved.name));
                    show_type_dialog.set(false);
                    edit_target.set(None);
                    list.restart();
                    categories.restart();
                },
            }
        }
        if show_categories() {
            CatalogCategoriesDialog {
                kind,
                on_close: move |_| {
                    show_categories.set(false);
                    table.reset_page();
                    selection.clear();
                    categories.restart();
                    list.restart();
                },
                on_changed: move |_| {
                    categories.restart();
                    list.restart();
                },
            }
        }
        if show_import() {
            CatalogImportDialog {
                kind,
                on_close: move |_| show_import.set(false),
                on_imported: move |_| {
                    list.restart();
                    categories.restart();
                },
            }
        }
        if let Some(request) = delete_request() {
            CatalogDeleteDialog {
                title: request.title,
                kind,
                preview: request.preview,
                busy: deleting(),
                on_cancel: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        delete_request.set(None),
                on_confirm: confirm_delete,
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn start_delete(
    kind: CatalogKind,
    ids: Vec<i64>,
    title: String,
    confirm_destructive: bool,
    mut toast: Toast,
    mut delete_request: Signal<Option<DeleteRequest>>,
    list: Resource<Result<CatalogTypePage, String>>,
    categories: Resource<Result<Vec<CatalogCategory>, String>>,
    selection: TableSelection<i64>,
    deleting: Signal<bool>,
) {
    spawn(async move {
        match catalog_types_delete_preview_api(kind.as_str().to_string(), ids.clone()).await {
            Ok(preview) => {
                if preview.has_related_rows() || confirm_destructive {
                    delete_request.set(Some(DeleteRequest {
                        title,
                        ids,
                        preview,
                    }));
                } else {
                    perform_delete(
                        kind,
                        ids,
                        toast,
                        list,
                        categories,
                        selection,
                        delete_request,
                        deleting,
                    );
                }
            }
            Err(error) => toast.error(format!("Could not inspect the deletion: {error}")),
        }
    });
}
#[allow(clippy::too_many_arguments)]
fn perform_delete(
    kind: CatalogKind,
    ids: Vec<i64>,
    mut toast: Toast,
    mut list: Resource<Result<CatalogTypePage, String>>,
    mut categories: Resource<Result<Vec<CatalogCategory>, String>>,
    mut selection: TableSelection<i64>,
    mut delete_request: Signal<Option<DeleteRequest>>,
    mut deleting: Signal<bool>,
) {
    deleting.set(true);
    spawn(async move {
        match delete_catalog_types_api(kind.as_str().to_string(), ids.clone(), true).await {
            Ok(CatalogDeleteOutcome::Deleted(summary)) => {
                toast.success(catalog_delete_summary_message(kind, &summary));
                delete_request.set(None);
                selection.clear();
                list.restart();
                categories.restart();
            }
            Ok(CatalogDeleteOutcome::ConfirmationRequired(preview)) => {
                delete_request.set(Some(DeleteRequest {
                    title: "Confirm deletion".to_string(),
                    ids,
                    preview: *preview,
                }));
            }
            Err(error) => toast.error(format!("Could not delete: {error}")),
        }
        deleting.set(false);
    });
}
