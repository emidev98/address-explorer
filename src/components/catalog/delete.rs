use super::*;
use dioxus::prelude::*;
/// Confirmation panel for a provider or category deletion. It is embedded in
/// a dialog by [`CatalogDeleteDialog`] and directly in the categories dialog
/// while a category deletion is being confirmed.
#[component]
pub fn CatalogDeleteImpact(
    kind: CatalogKind,
    preview: CatalogDeletePreview,
    #[props(default)] confirm_phrase: Option<String>,
    #[props(default)] busy: bool,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<()>,
) -> Element {
    let mut typed = use_signal(String::new);
    let requires_phrase = confirm_phrase.is_some();
    let phrase_matches = !requires_phrase
        || confirm_phrase
            .as_deref()
            .map(|phrase| typed().trim().eq_ignore_ascii_case(phrase.trim()))
            .unwrap_or(false);
    let mut types = vec![(
        "Types",
        preview.provider_count,
        if preview.provider_count == 1 {
            kind.singular_label()
        } else {
            kind.plural_label()
        },
    )];
    if preview.category_count > 0 {
        types.push((
            "Categories",
            preview.category_count,
            if preview.category_count == 1 {
                "category"
            } else {
                "categories"
            },
        ));
    }
    rsx! {
        p { class: "text-body-medium md3-delete-warning",
            "This permanently deletes the selected rows and all related data. This cannot be undone."
        }
        div { class: "md3-delete-impact",
            for (label, count, noun) in types {
                div { class: "md3-delete-impact-row",
                    span { "{label}" }
                    strong { "{count} {noun}" }
                }
            }
            if kind == CatalogKind::Exchange {
                div { class: "md3-delete-impact-row",
                    span { "Addresses" }
                    strong { "{preview.address_count}" }
                }
                div { class: "md3-delete-impact-row",
                    span { "Stored transactions" }
                    strong { "{preview.transaction_count}" }
                }
                div { class: "md3-delete-impact-row",
                    span { "Exchange accounts" }
                    strong { "{preview.exchange_account_count}" }
                }
                div { class: "md3-delete-impact-row",
                    span { "Ledger transactions" }
                    strong { "{preview.ledger_transaction_count}" }
                }
            } else {
                div { class: "md3-delete-impact-row",
                    span { "Addresses" }
                    strong { "{preview.address_count}" }
                }
                div { class: "md3-delete-impact-row",
                    span { "Stored transactions" }
                    strong { "{preview.transaction_count}" }
                }
            }
        }
        if !preview.has_related_rows() {
            p { class: "text-body-small text-on-surface-variant",
                "No related addresses or transactions will be removed."
            }
        }
        if let Some(phrase) = confirm_phrase {
            label {
                class: "text-body-medium text-on-surface-variant",
                r#for: "catalog-delete-confirm",
                "Type \"{phrase}\" to confirm"
            }
            div { class: "md3-text-field",
                input {
                    id: "catalog-delete-confirm",
                    class: "md3-input",
                    value: "{typed}",
                    autofocus: true,
                    disabled: busy,
                    oninput: move | event |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            typed.set(event.value()),
                }
            }
        }
        div { class: "md3-dialog-actions",
            button {
                r#type: "button",
                class: "md3-button-text",
                disabled: busy,
                onclick: move | _
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | on_cancel.call(()),
                "Cancel"
            }
            button {
                r#type: "button",
                class: "md3-button-danger",
                disabled: busy || !phrase_matches,
                onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        on_confirm.call(()),
                if busy {
                    "Deleting..."
                } else {
                    "Delete permanently"
                }
            }
        }
    }
}
/// Modal wrapper around [`CatalogDeleteImpact`].
#[component]
pub fn CatalogDeleteDialog(
    title: String,
    kind: CatalogKind,
    preview: CatalogDeletePreview,
    #[props(default)] confirm_phrase: Option<String>,
    #[props(default)] busy: bool,
    on_cancel: EventHandler<()>,
    on_confirm: EventHandler<()>,
) -> Element {
    rsx! {
        Modal {
            title,
            title_id: "catalog-delete-title".to_string(),
            role: "alertdialog".to_string(),
            dismissible: !busy,
            on_close: move | _ | on_cancel
                                                                                                                                                                                                                                                                                                                                                                    .call(()),
            CatalogDeleteImpact {
                kind,
                preview,
                confirm_phrase,
                busy,
                on_cancel,
                on_confirm,
            }
        }
    }
}
/// Human-readable summary of a confirmed deletion.
pub fn catalog_delete_summary_message(kind: CatalogKind, summary: &CatalogDeleteSummary) -> String {
    let mut pieces = Vec::new();
    if summary.category_count > 0 {
        pieces.push(format!(
            "{} {}",
            summary.category_count,
            if summary.category_count == 1 {
                "category"
            } else {
                "categories"
            },
        ));
    }
    if summary.provider_count > 0 {
        pieces.push(format!(
            "{} {}",
            summary.provider_count,
            if summary.provider_count == 1 {
                kind.singular_label()
            } else {
                kind.plural_label()
            },
        ));
    }
    if summary.address_count > 0 {
        pieces.push(format!("{} addresses", summary.address_count));
    }
    if summary.transaction_count > 0 {
        pieces.push(format!("{} stored transactions", summary.transaction_count));
    }
    if summary.exchange_account_count > 0 {
        pieces.push(format!(
            "{} exchange accounts",
            summary.exchange_account_count
        ));
    }
    if summary.ledger_transaction_count > 0 {
        pieces.push(format!(
            "{} ledger transactions",
            summary.ledger_transaction_count
        ));
    }
    if pieces.is_empty() {
        "Nothing was deleted.".to_string()
    } else {
        format!("Deleted {}.", pieces.join(", "))
    }
}
