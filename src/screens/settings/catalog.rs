//! Wallet Types / Exchange Types sub-settings: the provider table, filters,
//! KPIs, CRUD dialogs, cascade deletion, and CSV import.

use std::collections::HashSet;

use dioxus::prelude::*;

use crate::api::catalogs::{
    catalog_types_delete_preview_api, delete_catalog_types_api, list_catalog_categories_api,
    list_catalog_types_api,
};
use crate::api::settings::get_exchange_settings_api;
use crate::components::{
    catalog_delete_summary_message, use_toast, BadgeSize, CatalogCategoriesDialog,
    CatalogDeleteDialog, CatalogImportDialog, CatalogTypeDialog, Kpi, ProviderBadge,
    SortableHeader, TablePagination, Toast,
};
use crate::domain::catalog::{
    CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview, CatalogKind, CatalogType,
    CatalogTypePage,
};
use crate::domain::table::{SortDirection, TableQuery};

const PAGE_SIZE: u32 = 25;

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
    let mut page = use_signal(|| 0_u32);
    let sort_by = use_signal(|| Option::<String>::None);
    let sort_dir = use_signal(|| SortDirection::Asc);
    let mut selected = use_signal(HashSet::<i64>::new);
    let mut show_type_dialog = use_signal(|| false);
    let mut edit_target = use_signal(|| Option::<CatalogType>::None);
    let mut show_categories = use_signal(|| false);
    let mut show_import = use_signal(|| false);
    let mut delete_request = use_signal(|| Option::<DeleteRequest>::None);
    let deleting = use_signal(|| false);
    let mut confirm_destructive = use_signal(|| true);

    let settings = use_resource(|| async move { get_exchange_settings_api().await.ok() });
    use_effect(move || {
        if let Some(Some(settings)) = settings() {
            confirm_destructive.set(settings.confirm_destructive_actions);
        }
    });

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
        let query = TableQuery {
            page: page(),
            page_size: PAGE_SIZE,
            sort_by: sort_by(),
            sort_dir: sort_dir(),
        };
        async move {
            list_catalog_types_api(kind.as_str().to_string(), query)
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
    let selected_snapshot = selected.read().clone();
    let selected_count = selected_snapshot.len();
    let delete_action_label = if selected_count == 0 {
        format!("Select {} to delete", kind.plural_label())
    } else {
        format!(
            "Delete '{selected_count}' {}",
            if selected_count == 1 {
                kind.singular_label()
            } else {
                kind.plural_label()
            }
        )
    };
    let all_selected =
        !page_ids.is_empty() && page_ids.iter().all(|id| selected_snapshot.contains(id));

    let confirm_delete = move |_| {
        if let Some(request) = delete_request() {
            perform_delete(
                kind,
                request.ids,
                toast,
                list,
                categories,
                selected,
                delete_request,
                deleting,
            );
        }
    };

    let content = match &state {
        None => rsx! {
            p { class: "text-body-medium text-on-surface-variant", "Loading {kind.plural_label()}..." }
        },
        Some(Err(error)) => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "Error loading {kind.plural_label()}: {error}"
            }
        },
        Some(Ok(_)) if items.is_empty() => rsx! {
            p { class: "text-body-medium text-on-surface-variant",
                "No {kind.plural_label()} yet. Create one or import a CSV."
            }
        },
        Some(Ok(data)) => {
            let total = data.page.total;
            let table = rsx! {
                TablePagination {
                    page: page(),
                    page_size: PAGE_SIZE,
                    total,
                    on_page: move |next| {
                        page.set(next);
                        selected.write().clear();
                    },
                }
                div { class: "md3-table-container",
                    table { class: "md3-table md3-catalog-table",
                        thead {
                            tr {
                                th {
                                    input {
                                        r#type: "checkbox",
                                        aria_label: "Select all types on this page",
                                        checked: all_selected,
                                        onchange: move |event| {
                                            let checked = event.checked();
                                            let mut guard = selected.write();
                                            if checked {
                                                guard.extend(page_ids.iter().copied());
                                            } else {
                                                for id in &page_ids {
                                                    guard.remove(id);
                                                }
                                            }
                                        },
                                    }
                                }
                                th {
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-icon-button-danger",
                                        disabled: selected_count == 0,
                                        title: "{delete_action_label}",
                                        aria_label: "{delete_action_label}",
                                        onclick: move |_| {
                                            let ids = selected.read().iter().copied().collect::<Vec<_>>();
                                            if ids.is_empty() {
                                                return;
                                            }
                                            let count = ids.len();
                                            let title = format!(
                                                "Delete {count} selected {}",
                                                if count == 1 {
                                                    kind.singular_label()
                                                } else {
                                                    kind.plural_label()
                                                }
                                            );
                                            start_delete(
                                                kind,
                                                ids,
                                                title,
                                                confirm_destructive(),
                                                toast,
                                                delete_request,
                                                list,
                                                categories,
                                                selected,
                                                deleting,
                                            );
                                        },
                                        {icon(ICON_DELETE)}
                                    }
                                }
                                SortableHeader {
                                    label: "Name".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("name"))
                                                                            .then_some(sort_dir()),
                                    on_sort: move |_| {
                                        apply_sort(sort_by, sort_dir, page, selected, "name", SortDirection::Asc);
                                    },
                                }
                                SortableHeader {
                                    label: "Category".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("category"))
                                                                            .then_some(sort_dir()),
                                    on_sort: move |_| {
                                        apply_sort(sort_by, sort_dir, page, selected, "category", SortDirection::Asc);
                                    },
                                }
                                SortableHeader {
                                    label: "Custody".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("custody"))
                                                                            .then_some(sort_dir()),
                                    on_sort: move |_| {
                                        apply_sort(sort_by, sort_dir, page, selected, "custody", SortDirection::Asc);
                                    },
                                }
                                if kind == CatalogKind::Wallet {
                                    SortableHeader {
                                        label: "Platform".to_string(),
                                        active_direction: (sort_by().as_deref() == Some("platform"))
                                                                                    .then_some(sort_dir()),
                                        on_sort: move |_| {
                                            apply_sort(sort_by, sort_dir, page, selected, "platform", SortDirection::Asc);
                                        },
                                    }
                                }
                                SortableHeader {
                                    label: "Addresses".to_string(),
                                    active_direction: (sort_by().as_deref() == Some("addresses"))
                                                                            .then_some(sort_dir()),
                                    on_sort: move |_| {
                                        apply_sort(sort_by, sort_dir, page, selected, "addresses", SortDirection::Desc);
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
                                        tr {
                                            td {
                                                input {
                                                    r#type: "checkbox",
                                                    aria_label: "Select {row_name}",
                                                    checked: row_checked,
                                                    onchange: move |event| {
                                                        let checked = event.checked();
                                                        let mut guard = selected.write();
                                                        if checked {
                                                            guard.insert(row_id);
                                                        } else {
                                                            guard.remove(&row_id);
                                                        }
                                                    },
                                                }
                                            }
                                            td {
                                                div { class: "md3-table-actions",
                                                    button {
                                                        r#type: "button",
                                                        class: "md3-icon-button",
                                                        title: "Edit {row_name}",
                                                        aria_label: "Edit {row_name}",
                                                        onclick: move |_| {
                                                            edit_target.set(Some(edit_row.clone()));
                                                            show_type_dialog.set(true);
                                                        },
                                                        {icon(ICON_EDIT)}
                                                    }
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
                                                                confirm_destructive(),
                                                                toast,
                                                                delete_request,
                                                                list,
                                                                categories,
                                                                selected,
                                                                deleting,
                                                            );
                                                        },
                                                        {icon(ICON_DELETE)}
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
            };
            rsx! {
                {table}
            }
        }
    };

    rsx! {
        div { class: "md3-page-header",
            h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                "Settings ({kind.label()})"
            }
            div { class: "md3-page-actions",
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    onclick: move |_| show_categories.set(true),
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
                    page.set(0);
                    selected.write().clear();
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
                on_cancel: move |_| delete_request.set(None),
                on_confirm: confirm_delete,
            }
        }
    }
}

fn apply_sort(
    mut sort_by: Signal<Option<String>>,
    mut sort_dir: Signal<SortDirection>,
    mut page: Signal<u32>,
    mut selected: Signal<HashSet<i64>>,
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
    selected.write().clear();
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
    selected: Signal<HashSet<i64>>,
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
                        selected,
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
    mut selected: Signal<HashSet<i64>>,
    mut delete_request: Signal<Option<DeleteRequest>>,
    mut deleting: Signal<bool>,
) {
    deleting.set(true);
    spawn(async move {
        match delete_catalog_types_api(kind.as_str().to_string(), ids.clone(), true).await {
            Ok(CatalogDeleteOutcome::Deleted(summary)) => {
                toast.success(catalog_delete_summary_message(kind, &summary));
                delete_request.set(None);
                selected.write().clear();
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

#[allow(clippy::too_many_arguments)]
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
