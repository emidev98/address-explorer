use super::*;
use dioxus::prelude::*;

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
        Modal {
            title: "Manage categories".to_string(),
            title_id: "catalog-categories-title".to_string(),
            class: "md3-catalog-dialog".to_string(),
            on_close: move |_| {
                if delete_target().is_some() {
                    if !*deleting.read() {
                        delete_target.set(None);
                    }
                } else {
                    close_categories(creating, renaming_busy, deleting, on_close);
                }
            },
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
                                                            Icon { name: IconName::Edit, size: 20 }
                                                        }
                                                        button {
                                                            r#type: "button",
                                                            class: "md3-icon-button md3-icon-button-danger",
                                                            title: "Delete category",
                                                            aria_label: "Delete {category_name}",
                                                            onclick: move |_| {
                                                                start_category_delete(kind, category.clone(), error, delete_target);
                                                            },
                                                            Icon { name: IconName::Delete, size: 20 }
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
