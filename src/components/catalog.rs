//! Dialogs of the provider catalog sub-settings: create/edit, category
//! management, the cascade-deletion impact, and the create-only CSV import.

use dioxus::prelude::*;

use crate::api::catalogs::{
    catalog_category_delete_preview_api, catalog_import_api, catalog_import_preview_api,
    create_catalog_category_api, create_catalog_type_api, delete_catalog_category_api,
    list_catalog_categories_api, rename_catalog_category_api, update_catalog_type_api,
};
use crate::components::provider_badge::derived_color;
use crate::components::{use_toast, BadgeSize, ConfirmDialog, CsvDropzone, ProviderBadge, Toast};
use crate::domain::catalog::{
    normalize_brand_color, slugify, CatalogCategory, CatalogDeleteOutcome, CatalogDeletePreview,
    CatalogDeleteSummary, CatalogField, CatalogImportPreview, CatalogImportSummary, CatalogKind,
    CatalogType, CatalogTypeDraft, CatalogValidationError, CustodyModel, WalletPlatform,
};

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
                    oninput: move |event| typed.set(event.value()),
                }
            }
        }

        div { class: "md3-dialog-actions",
            button {
                r#type: "button",
                class: "md3-button-text",
                disabled: busy,
                onclick: move |_| on_cancel.call(()),
                "Cancel"
            }
            button {
                r#type: "button",
                class: "md3-button-danger",
                disabled: busy || !phrase_matches,
                onclick: move |_| on_confirm.call(()),
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
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| {
                if !busy {
                    on_cancel.call(());
                }
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape && !busy {
                    on_cancel.call(());
                }
            },
            div {
                class: "md3-dialog",
                role: "alertdialog",
                aria_modal: "true",
                aria_labelledby: "catalog-delete-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "catalog-delete-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "{title}"
                }
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
}

/// Create or edit one provider.
#[component]
pub fn CatalogTypeDialog(
    kind: CatalogKind,
    #[props(default)] entry: Option<CatalogType>,
    categories: Vec<CatalogCategory>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<CatalogType>,
) -> Element {
    let editing = entry.is_some();
    let initial_name = entry
        .as_ref()
        .map(|entry| entry.name.clone())
        .unwrap_or_default();
    let initial_slug = entry
        .as_ref()
        .map(|entry| entry.slug.clone())
        .unwrap_or_default();
    let initial_category = entry.as_ref().and_then(|entry| entry.category_id);
    let initial_custody = entry.as_ref().and_then(|entry| entry.custody_model);
    let initial_platform = entry.as_ref().and_then(|entry| entry.platform);
    let initial_color = entry
        .as_ref()
        .and_then(|entry| entry.brand_color.clone())
        .unwrap_or_else(|| derived_color(&initial_name));

    let mut name = use_signal(|| initial_name.clone());
    let mut slug = use_signal(|| initial_slug.clone());
    let mut slug_touched = use_signal(|| editing);
    let mut category_id = use_signal(|| initial_category);
    let mut custody = use_signal(|| initial_custody);
    let mut platform = use_signal(|| initial_platform);
    let mut color = use_signal(|| initial_color.clone());
    let mut saving = use_signal(|| false);
    let mut validation = use_signal(|| Option::<CatalogValidationError>::None);
    let mut error = use_signal(|| Option::<String>::None);
    let mut show_discard = use_signal(|| false);

    use_effect(move || {
        if !slug_touched() {
            slug.set(slugify(&name()));
        }
    });

    let normalized_color = normalize_brand_color(Some(&color())).unwrap_or(None);
    let initial_normalized_color = normalize_brand_color(Some(&initial_color)).unwrap_or(None);
    let dirty = name() != initial_name
        || slug() != initial_slug
        || category_id() != initial_category
        || custody() != initial_custody
        || platform() != initial_platform
        || normalized_color != initial_normalized_color;

    let submit = move |_| {
        if *saving.read() {
            return;
        }
        let slug_value = if slug_touched() {
            slug()
        } else {
            slugify(&name())
        };
        let draft = CatalogTypeDraft {
            name: name(),
            slug: Some(slug_value),
            category_id: category_id(),
            custody_model: custody(),
            platform: if kind == CatalogKind::Wallet {
                platform()
            } else {
                None
            },
            brand_color: Some(color()),
        };
        if let Err(failure) = draft.clone().normalize(kind) {
            validation.set(Some(failure));
            return;
        }
        validation.set(None);
        error.set(None);
        saving.set(true);
        let entry_id = entry.as_ref().map(|entry| entry.id);
        spawn(async move {
            let kind_value = kind.as_str().to_string();
            let result = if let Some(id) = entry_id {
                update_catalog_type_api(kind_value, id, draft).await
            } else {
                create_catalog_type_api(kind_value, draft).await
            };
            match result {
                Ok(saved) => on_saved.call(saved),
                Err(failure) => {
                    error.set(Some(failure.to_string()));
                    saving.set(false);
                }
            }
        });
    };

    let validation_state = validation();
    let name_error = validation_state
        .as_ref()
        .and_then(|failure| (failure.field == CatalogField::Name).then(|| failure.message.clone()));
    let slug_error = validation_state
        .as_ref()
        .and_then(|failure| (failure.field == CatalogField::Slug).then(|| failure.message.clone()));
    let color_error = validation_state.as_ref().and_then(|failure| {
        (failure.field == CatalogField::BrandColor).then(|| failure.message.clone())
    });
    let color_valid = normalized_color;
    let title = if editing { "Edit type" } else { "New type" };

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| request_close(saving, dirty, show_discard, on_close),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    request_close(saving, dirty, show_discard, on_close);
                }
            },
            div {
                class: "md3-dialog md3-catalog-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "catalog-type-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "catalog-type-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "{title}"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Name and slug identify the {kind.singular_label()}; the slug is locked after creation."
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "catalog-type-name",
                    "Name *"
                }
                div { class: "md3-text-field",
                    input {
                        id: "catalog-type-name",
                        class: "md3-input",
                        value: "{name}",
                        autofocus: true,
                        disabled: *saving.read(),
                        oninput: move |event| name.set(event.value()),
                    }
                }
                if let Some(message) = name_error {
                    p { class: "text-body-small md3-settings-error", "{message}" }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "catalog-type-slug",
                    "Slug"
                }
                div { class: "md3-text-field",
                    input {
                        id: "catalog-type-slug",
                        class: "md3-input",
                        value: "{slug}",
                        disabled: editing || *saving.read(),
                        oninput: move |event| {
                            slug_touched.set(true);
                            slug.set(event.value());
                        },
                    }
                }
                if editing {
                    p { class: "text-body-small text-on-surface-variant",
                        "The slug is locked after creation."
                    }
                }
                if let Some(message) = slug_error {
                    p { class: "text-body-small md3-settings-error", "{message}" }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "catalog-type-category",
                    "Category"
                }
                div { class: "md3-text-field",
                    select {
                        id: "catalog-type-category",
                        class: "md3-input",
                        value: category_id().map(|id| id.to_string()).unwrap_or_default(),
                        disabled: *saving.read(),
                        onchange: move |event| {
                            category_id.set(event.value().parse::<i64>().ok());
                        },
                        option { value: "", "Uncategorized" }
                        for category in categories {
                            option { value: "{category.id}", "{category.name}" }
                        }
                    }
                }

                label { class: "text-body-medium text-on-surface-variant", "Custody" }
                div {
                    class: "md3-chip-group",
                    role: "group",
                    aria_label: "Custody model",
                    button {
                        r#type: "button",
                        class: if custody().is_none() { "md3-chip md3-chip-selected" } else { "md3-chip" },
                        aria_pressed: custody().is_none().to_string(),
                        disabled: *saving.read(),
                        onclick: move |_| custody.set(None),
                        "Not set"
                    }
                    for model in CustodyModel::ALL {
                        button {
                            r#type: "button",
                            class: if custody() == Some(model) { "md3-chip md3-chip-selected" } else { "md3-chip" },
                            aria_pressed: (custody() == Some(model)).to_string(),
                            disabled: *saving.read(),
                            onclick: move |_| custody.set(Some(model)),
                            "{model.label()}"
                        }
                    }
                }

                if kind == CatalogKind::Wallet {
                    label { class: "text-body-medium text-on-surface-variant", "Platform" }
                    div {
                        class: "md3-chip-group",
                        role: "group",
                        aria_label: "Wallet platform",
                        button {
                            r#type: "button",
                            class: if platform().is_none() { "md3-chip md3-chip-selected" } else { "md3-chip" },
                            aria_pressed: platform().is_none().to_string(),
                            disabled: *saving.read(),
                            onclick: move |_| platform.set(None),
                            "Not set"
                        }
                        for value in WalletPlatform::ALL {
                            button {
                                r#type: "button",
                                class: if platform() == Some(value) { "md3-chip md3-chip-selected" } else { "md3-chip" },
                                aria_pressed: (platform() == Some(value)).to_string(),
                                disabled: *saving.read(),
                                onclick: move |_| platform.set(Some(value)),
                                "{value.label()}"
                            }
                        }
                    }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "catalog-type-color",
                    "Brand color"
                }
                div { class: "md3-catalog-color-row",
                    input {
                        id: "catalog-type-color",
                        r#type: "color",
                        class: "md3-catalog-color-input",
                        value: "{color}",
                        disabled: *saving.read(),
                        oninput: move |event| color.set(event.value()),
                    }
                    span { class: "md3-catalog-color-hex", "{color}" }
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *saving.read(),
                        onclick: move |_| color.set(derived_color(&name())),
                        "Suggested"
                    }
                    ProviderBadge {
                        name: name(),
                        color: color_valid.clone(),
                        size: BadgeSize::Regular,
                    }
                }
                if let Some(message) = color_error {
                    p { class: "text-body-small md3-settings-error", "{message}" }
                }

                if let Some(message) = error() {
                    p { class: "text-body-small md3-settings-error", "{message}" }
                }

                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        disabled: *saving.read(),
                        onclick: move |_| request_close(saving, dirty, show_discard, on_close),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: *saving.read(),
                        onclick: submit,
                        if *saving.read() {
                            "Saving..."
                        } else if editing {
                            "Save changes"
                        } else {
                            "Create"
                        }
                    }
                }
            }
        }

        if show_discard() {
            ConfirmDialog {
                title: "Discard changes".to_string(),
                message: "Close the form and discard your unsaved changes?".to_string(),
                confirm_text: "Discard".to_string(),
                cancel_text: "Keep editing".to_string(),
                on_cancel: move |_| show_discard.set(false),
                on_confirm: move |_| {
                    show_discard.set(false);
                    on_close.call(());
                },
            }
        }
    }
}

/// Category management of one kind. Deleting a category cascades through its
/// providers; that path requires typing the category name.
#[component]
pub fn CatalogCategoriesDialog(
    kind: CatalogKind,
    on_close: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut toast = use_toast();
    let mut categories = use_resource(move || async move {
        list_catalog_categories_api(kind.as_str().to_string())
            .await
            .map_err(|error| error.to_string())
    });
    let mut new_name = use_signal(String::new);
    let mut creating = use_signal(|| false);
    let mut renaming = use_signal(|| Option::<(i64, String)>::None);
    let renaming_busy = use_signal(|| false);
    let mut delete_target = use_signal(|| Option::<(CatalogCategory, CatalogDeletePreview)>::None);
    let mut deleting = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let mut create = move |_| {
        let name = new_name().trim().to_string();
        if name.is_empty() || *creating.read() {
            return;
        }
        creating.set(true);
        error.set(None);
        spawn(async move {
            match create_catalog_category_api(kind.as_str().to_string(), name).await {
                Ok(category) => {
                    toast.success(format!("Category \"{}\" created.", category.name));
                    new_name.set(String::new());
                    categories.restart();
                    on_changed.call(());
                }
                Err(failure) => error.set(Some(failure.to_string())),
            }
            creating.set(false);
        });
    };

    let mut confirm_delete = move |_| {
        let Some((category, _)) = delete_target() else {
            return;
        };
        deleting.set(true);
        error.set(None);
        spawn(async move {
            match delete_catalog_category_api(kind.as_str().to_string(), category.id, true).await {
                Ok(CatalogDeleteOutcome::Deleted(summary)) => {
                    toast.success(catalog_delete_summary_message(kind, &summary));
                    delete_target.set(None);
                    categories.restart();
                    on_changed.call(());
                }
                Ok(CatalogDeleteOutcome::ConfirmationRequired(preview)) => {
                    delete_target.set(Some((category, *preview)));
                }
                Err(failure) => error.set(Some(failure.to_string())),
            }
            deleting.set(false);
        });
    };

    let delete_target_state = delete_target();

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| {
                if delete_target().is_some() {
                    if !*deleting.read() {
                        delete_target.set(None);
                    }
                } else {
                    close_categories(creating, renaming_busy, deleting, on_close);
                }
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    if delete_target().is_some() {
                        if !*deleting.read() {
                            delete_target.set(None);
                        }
                    } else {
                        close_categories(creating, renaming_busy, deleting, on_close);
                    }
                }
            },
            div {
                class: "md3-dialog md3-catalog-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "catalog-categories-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "catalog-categories-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Manage categories"
                }

                if let Some((category, preview)) = delete_target_state {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "Delete \"{category.name}\""
                    }
                    CatalogDeleteImpact {
                        kind,
                        preview,
                        confirm_phrase: (preview.provider_count > 0).then(|| category.name.clone()),
                        busy: deleting(),
                        on_cancel: move |_| delete_target.set(None),
                        on_confirm: move |_| confirm_delete(()),
                    }
                    if let Some(message) = error() {
                        p { class: "text-body-small md3-settings-error", "{message}" }
                    }
                } else {
                    p { class: "text-body-medium text-on-surface-variant",
                        "Categories group providers and power the filters. Deleting a category also deletes its providers and their related data."
                    }

                    form {
                        class: "md3-category-create",
                        onsubmit: move |event| {
                            event.prevent_default();
                            create(());
                        },
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                value: "{new_name}",
                                placeholder: "New category name",
                                aria_label: "New category name",
                                disabled: *creating.read(),
                                oninput: move |event| new_name.set(event.value()),
                            }
                        }
                        button {
                            r#type: "submit",
                            class: "md3-button-filled",
                            disabled: *creating.read() || new_name().trim().is_empty(),
                            if *creating.read() {
                                "Adding..."
                            } else {
                                "Add"
                            }
                        }
                    }

                    if let Some(message) = error() {
                        p { class: "text-body-small md3-settings-error", "{message}" }
                    }

                    if let Some(Ok(category_list)) = categories() {
                        if category_list.is_empty() {
                            p { class: "text-body-medium text-on-surface-variant",
                                "No categories yet. Create the first one above."
                            }
                        } else {
                            div { class: "md3-category-list",
                                for category in category_list {
                                    {
                                        let category_id = category.id;
                                        let category_name = category.name.clone();
                                        let rename_name = category_name.clone();
                                        let count = category.provider_count;
                                        let editing = renaming().is_some_and(|(id, _)| id == category_id);
                                        rsx! {
                                            div { class: "md3-category-row",
                                                if editing {
                                                    form {
                                                        class: "md3-category-rename",
                                                        onsubmit: move |event| {
                                                            event.prevent_default();
                                                            let Some((id, name)) = renaming() else {
                                                                return;
                                                            };
                                                            rename_category(
                                                                kind,
                                                                id,
                                                                name,
                                                                toast,
                                                                categories,
                                                                on_changed,
                                                                error,
                                                                renaming,
                                                                renaming_busy,
                                                            );
                                                        },
                                                        div { class: "md3-text-field",
                                                            input {
                                                                class: "md3-input",
                                                                value: "{renaming().map(|(_, name)| name).unwrap_or_default()}",
                                                                aria_label: "Category name",
                                                                disabled: *renaming_busy.read(),
                                                                oninput: move |event| {
                                                                    renaming.set(Some((category_id, event.value())));
                                                                },
                                                            }
                                                        }
                                                        button {
                                                            r#type: "submit",
                                                            class: "md3-button-tonal",
                                                            disabled: *renaming_busy.read(),
                                                            "Save"
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "md3-button-text",
                                                            disabled: *renaming_busy.read(),
                                                            onclick: move |_| renaming.set(None),
                                                            "Cancel"
                                                        }
                                                    }
                                                } else {
                                                    span { class: "md3-category-name", "{category_name}" }
                                                    span { class: "text-body-small text-on-surface-variant",
                                                        "{count} "
                                                        if count == 1 {
                                                            "type"
                                                        } else {
                                                            "types"
                                                        }
                                                    }
                                                    div { class: "md3-category-actions",
                                                        button {
                                                            r#type: "button",
                                                            class: "md3-icon-button",
                                                            title: "Rename category",
                                                            aria_label: "Rename {category_name}",
                                                            onclick: move |_| {
                                                                renaming.set(Some((category_id, rename_name.clone())));
                                                            },
                                                            {icon(ICON_EDIT)}
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "md3-icon-button md3-icon-button-danger",
                                                            title: "Delete category",
                                                            aria_label: "Delete {category_name}",
                                                            onclick: move |_| {
                                                                start_category_delete(kind, category.clone(), error, delete_target);
                                                            },
                                                            {icon(ICON_DELETE)}
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else if categories().is_none() {
                        p { class: "text-body-medium text-on-surface-variant",
                            "Loading categories..."
                        }
                    } else if let Some(Err(message)) = categories() {
                        p { class: "text-body-small md3-settings-error",
                            "Could not load categories: {message}"
                        }
                    }

                    div { class: "md3-dialog-actions",
                        button {
                            r#type: "button",
                            class: "md3-button-text",
                            disabled: *creating.read() || *renaming_busy.read(),
                            onclick: move |_| close_categories(creating, renaming_busy, deleting, on_close),
                            "Close"
                        }
                    }
                }
            }
        }
    }
}

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
                        }
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
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| {
                if !*importing.read() {
                    on_close.call(());
                }
            },
            onkeydown: move |event| {
                if event.key() == Key::Escape && !*importing.read() {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog md3-import-data-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "catalog-import-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "catalog-import-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Import {kind.plural_label()}"
                }
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
                    disabled: *validating.read() || *importing.read() || summary().is_some(),
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
                                                    td { {import_status_chip(&row.status)} }
                                                }
                                            }
                                        }
                                    }
                                }
                                if preview.rows_truncated {
                                    p { class: "text-body-small text-on-surface-variant",
                                        "Showing the first 200 rows; every valid row is still imported."
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
                            result.uncategorized_rows
                        );
                        rsx! {
                            div { class: "md3-import-result",
                                strong { "Imported {result.created} {noun}." }
                                span { class: "text-body-small text-on-surface-variant",
                                    "{details}"
                                }
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
                        onclick: move |_| on_close.call(()),
                        "Close"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        disabled: !can_import || *importing.read() || *validating.read(),
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
}

fn import_status_chip(status: &crate::domain::catalog::CatalogImportRowStatus) -> Element {
    match status {
        crate::domain::catalog::CatalogImportRowStatus::Create => rsx! {
            span { class: "md3-status-chip md3-status-create", "Create" }
        },
        crate::domain::catalog::CatalogImportRowStatus::Duplicate(message) => rsx! {
            span { class: "md3-status-cell",
                span { class: "md3-status-chip md3-status-duplicate", "Duplicate" }
                span { class: "text-body-small text-on-surface-variant", "{message}" }
            }
        },
        crate::domain::catalog::CatalogImportRowStatus::Invalid(message) => rsx! {
            span { class: "md3-status-cell",
                span { class: "md3-status-chip md3-status-invalid", "Invalid" }
                span { class: "text-body-small text-on-surface-variant", "{message}" }
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn rename_category(
    kind: CatalogKind,
    id: i64,
    name: String,
    mut toast: Toast,
    mut categories: Resource<Result<Vec<CatalogCategory>, String>>,
    on_changed: EventHandler<()>,
    mut error: Signal<Option<String>>,
    mut renaming: Signal<Option<(i64, String)>>,
    mut renaming_busy: Signal<bool>,
) {
    if *renaming_busy.read() {
        return;
    }
    renaming_busy.set(true);
    error.set(None);
    spawn(async move {
        match rename_catalog_category_api(kind.as_str().to_string(), id, name).await {
            Ok(category) => {
                toast.success(format!("Category renamed to \"{}\".", category.name));
                renaming.set(None);
                categories.restart();
                on_changed.call(());
            }
            Err(failure) => error.set(Some(failure.to_string())),
        }
        renaming_busy.set(false);
    });
}

fn start_category_delete(
    kind: CatalogKind,
    category: CatalogCategory,
    mut error: Signal<Option<String>>,
    mut delete_target: Signal<Option<(CatalogCategory, CatalogDeletePreview)>>,
) {
    error.set(None);
    spawn(async move {
        match catalog_category_delete_preview_api(kind.as_str().to_string(), category.id).await {
            Ok(preview) => delete_target.set(Some((category, preview))),
            Err(failure) => error.set(Some(failure.to_string())),
        }
    });
}

fn request_close(
    saving: Signal<bool>,
    dirty: bool,
    mut show_discard: Signal<bool>,
    on_close: EventHandler<()>,
) {
    if *saving.read() {
        return;
    }
    if dirty {
        show_discard.set(true);
    } else {
        on_close.call(());
    }
}

fn close_categories(
    creating: Signal<bool>,
    renaming_busy: Signal<bool>,
    deleting: Signal<bool>,
    on_close: EventHandler<()>,
) {
    if !*creating.read() && !*renaming_busy.read() && !*deleting.read() {
        on_close.call(());
    }
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
            }
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
            }
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
