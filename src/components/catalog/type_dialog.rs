use super::*;
use dioxus::prelude::*;
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
        Modal {
            title,
            title_id: "catalog-type-title".to_string(),
            class: "md3-catalog-dialog".to_string(),
            on_close: move |_| request_close(saving, dirty, show_discard, on_close),
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
                    value: category_id().map(| id | id
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            .to_string()).unwrap_or_default(),
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
                    disabled: * saving
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            .read(),
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
                        aria_pressed: platform()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .is_none().to_string(),
                        disabled: *saving.read(),
                        onclick: move | _ | platform
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .set(None),
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
                    oninput: move | event | color.set(event
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            .value()),
                }
                span { class: "md3-catalog-color-hex", "{color}" }
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *saving.read(),
                    onclick: move
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            | _ | color.set(derived_color(& name())),
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
