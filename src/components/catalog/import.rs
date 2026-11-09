use super::*;
use dioxus::prelude::*;
/// CSV import dialog. The preview and the apply call both revalidate against
/// fresh database state, so the counts shown here match the rows created.
#[component]
pub fn CatalogImportDialog(
    kind: CatalogKind,
    on_close: EventHandler<()>,
    on_imported: EventHandler<CatalogImportSummary>,
) -> Element {
    let mut toast = use_toast();
    let mut file_name = use_signal(String::new);
    let mut csv_text = use_signal(String::new);
    let mut preview = use_signal(|| Option::<Result<CatalogImportPreview, String>>::None);
    let mut validating = use_signal(|| false);
    let mut importing = use_signal(|| false);
    let mut summary = use_signal(|| Option::<CatalogImportSummary>::None);
    let mut error = use_signal(|| Option::<String>::None);
    let on_file = move |(name, text): (String, String)| {
        file_name.set(name);
        csv_text.set(text.clone());
        preview.set(None);
        summary.set(None);
        error.set(None);
        if text.trim().is_empty() {
            return;
        }
        validating.set(true);
        spawn(async move {
            let result = catalog_import_preview_api(kind.as_str().to_string(), text)
                .await
                .map_err(|failure| failure.to_string());
            preview.set(Some(result));
            validating.set(false);
        });
    };
    let start_import = move |_| {
        if *importing.read() || summary().is_some() {
            return;
        }
        let text = csv_text();
        if text.trim().is_empty() {
            return;
        }
        importing.set(true);
        error.set(None);
        spawn(async move {
            match catalog_import_api(kind.as_str().to_string(), text).await {
                Ok(result) => {
                    toast.success(format!(
                        "Imported {} {}.",
                        result.created,
                        if result.created == 1 {
                            kind.singular_label()
                        } else {
                            kind.plural_label()
                        },
                    ));
                    summary.set(Some(result));
                    on_imported.call(result);
                }
                Err(failure) => error.set(Some(failure.to_string())),
            }
            importing.set(false);
        });
    };
    let preview_state = preview();
    let can_import = summary().is_none()
        && preview_state
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(|preview| preview.fatal_error.is_none() && preview.valid_rows > 0);
    rsx! {
        Modal {
            title: format!("Import {}", kind.plural_label()),
            title_id: "catalog-import-title".to_string(),
            class: "md3-import-data-dialog".to_string(),
            dismissible: !*importing.read(),
            on_close: move |_| on_close.call(()),
            p { class: "text-body-medium text-on-surface-variant",
                "The import only creates new rows. Duplicate names or slugs are skipped, and invalid rows are reported without blocking the valid ones."
            }
            details { class: "md3-import-format",
                summary { "CSV format" }
                p { class: "text-body-small text-on-surface-variant",
                    "Columns: name, slug, category, custody_model, platform (wallets only), brand_color. Only name is required. An unknown category imports as Uncategorized."
                }
                code { "name,slug,category,custody_model,platform,brand_color" }
            }
            CsvDropzone {
                file_name: file_name(),
                disabled: * validating.read() || * importing.read()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        || summary().is_some(),
                on_file,
            }
            if *validating.read() {
                p { class: "text-body-medium text-on-surface-variant", "Validating..." }
            }
            if let Some(result) = preview_state {
                match result {
                    Err(failure) => rsx! {
                        p { class: "text-body-small md3-settings-error", "{failure}" }
                    },
                    Ok(preview) => rsx! {
                        if let Some(fatal) = preview.fatal_error {
                            p { class: "text-body-small md3-settings-error", "{fatal}" }
                        } else {
                            div { class: "md3-import-summary",
                                span {
                                    strong { "{preview.valid_rows}" }
                                    " valid"
                                }
                                span {
                                    strong { "{preview.duplicate_rows}" }
                                    " duplicates"
                                }
                                span {
                                    strong { "{preview.invalid_rows}" }
                                    " invalid"
                                }
                                span {
                                    strong { "{preview.uncategorized_rows}" }
                                    " uncategorized"
                                }
                            }
                            if !preview.unknown_categories.is_empty() {
                                p { class: "text-body-small md3-import-warning",
                                    "Unknown categories will import as Uncategorized: {preview.unknown_categories.join(\", \")}"
                                }
                            }
                            div { class: "md3-table-container md3-import-preview",
                                table { class: "md3-table",
                                    thead {
                                        tr {
                                            th { "Row" }
                                            th { "Name" }
                                            th { "Slug" }
                                            th { "Category" }
                                            th { "Status" }
                                        }
                                    }
                                    tbody {
                                        for row in preview.rows {
                                            tr {
                                                td { "{row.row}" }
                                                td { "{row.name.clone().unwrap_or_default()}" }
                                                td { "{row.slug.clone().unwrap_or_default()}" }
                                                td { "{row.category.clone().unwrap_or_default()}" }
                                                td { { import_status_chip(& row
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .status) } }
                                            }
                                        }
                                    }
                                }
                            }
                            if preview.rows_truncated {
                                p { class: "text-body-small text-on-surface-variant",
                                    "Showing the first {MAX_PREVIEW_ROWS} rows; every valid row is still imported."
                                }
                            }
                        }
                    },
                }
            }
            if let Some(result) = summary() {
                {
                    let noun = if result.created == 1 {
                        kind.singular_label()
                    } else {
                        kind.plural_label()
                    };
                    let duplicate_noun = if result.skipped_duplicates == 1 {
                        "duplicate"
                    } else {
                        "duplicates"
                    };
                    let details = format!(
                        "{} {} skipped · {} invalid · {} uncategorized",
                        result.skipped_duplicates,
                        duplicate_noun,
                        result.invalid_rows,
                        result.uncategorized_rows,
                    );
                    rsx! {
                        div { class: "md3-import-result",
                            strong { "Imported {result.created} {noun}." }
                            span { class: "text-body-small text-on-surface-variant", "{details}" }
                        }
                    }
                }
            }
            if let Some(failure) = error() {
                p { class: "text-body-small md3-settings-error", "{failure}" }
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *importing.read(),
                    onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        on_close.call(()),
                    "Close"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: ! can_import || * importing.read() || *
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        validating.read(),
                    onclick: start_import,
                    if *importing.read() {
                        "Importing..."
                    } else {
                        "Import valid rows"
                    }
                }
            }
        }
    }
}
fn import_status_chip(status: &crate::domain::catalog::CatalogImportRowStatus) -> Element {
    match status {
        crate::domain::catalog::CatalogImportRowStatus::Create => {
            rsx! {
                span { class: "md3-status-chip md3-status-create", "Create" }
            }
        }
        crate::domain::catalog::CatalogImportRowStatus::Duplicate(message) => {
            rsx! {
                span { class: "md3-status-cell",
                    span { class: "md3-status-chip md3-status-duplicate", "Duplicate" }
                    span { class: "text-body-small text-on-surface-variant", "{message}" }
                }
            }
        }
        crate::domain::catalog::CatalogImportRowStatus::Invalid(message) => {
            rsx! {
                span { class: "md3-status-cell",
                    span { class: "md3-status-chip md3-status-invalid", "Invalid" }
                    span { class: "text-body-small text-on-surface-variant", "{message}" }
                }
            }
        }
    }
}
