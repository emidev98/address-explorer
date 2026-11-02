//! Provider pickers used by the address and exchange dialogs. Options are
//! grouped by category and the selected provider is previewed with its brand
//! badge, which a native `<option>` cannot render.
//!
//! [`CatalogSelect`] loads the options itself. [`CatalogOptionsSelect`] is the
//! presentational half for callers that already own a provider list, such as
//! the CSV import dialog that auto-fills the account name from the selection.

use std::collections::BTreeMap;

use dioxus::prelude::*;

use crate::api::catalogs::catalog_options_api;
use crate::components::provider_badge::{BadgeSize, ProviderBadge};
use crate::domain::catalog::{CatalogEntry, CatalogKind};

#[component]
pub fn CatalogSelect(
    kind: CatalogKind,
    value: Option<i64>,
    on_change: EventHandler<Option<i64>>,
    #[props(default)] disabled: bool,
    #[props(default)] id: String,
    #[props(default)] placeholder: String,
) -> Element {
    let options = use_resource(move || async move {
        catalog_options_api(kind.as_str().to_string())
            .await
            .map_err(|error| error.to_string())
    });

    let state = options();
    let loading = state.is_none();
    let error = match &state {
        Some(Err(error)) => Some(error.clone()),
        _ => None,
    };
    let entries = match &state {
        Some(Ok(entries)) => entries.clone(),
        _ => Vec::new(),
    };

    rsx! {
        CatalogOptionsSelect {
            kind,
            value,
            on_change,
            disabled,
            id,
            placeholder,
            entries,
            loading,
        }
        if let Some(error) = error {
            p { class: "text-body-small md3-settings-error",
                "Could not load {kind.plural_label()}: {error}"
            }
        }
    }
}

/// Presentational provider select: grouped options plus a badge for the
/// current selection.
#[component]
pub fn CatalogOptionsSelect(
    kind: CatalogKind,
    value: Option<i64>,
    on_change: EventHandler<Option<i64>>,
    #[props(default)] disabled: bool,
    #[props(default)] id: String,
    #[props(default)] placeholder: String,
    entries: Vec<CatalogEntry>,
    #[props(default)] loading: bool,
) -> Element {
    let grouped = group_options(&entries);
    let selected = entries
        .iter()
        .find(|entry| Some(entry.id) == value)
        .cloned();
    let placeholder = if placeholder.is_empty() {
        format!("Select a {} type", kind.as_str())
    } else {
        placeholder
    };

    rsx! {
        div { class: "md3-text-field",
            select {
                id: "{id}",
                class: "md3-input",
                value: value.map(|id| id.to_string()).unwrap_or_default(),
                disabled: disabled || loading,
                onchange: move |event| on_change.call(event.value().parse::<i64>().ok()),
                option { value: "", "{placeholder}" }
                for (group, entries) in grouped {
                    optgroup { label: "{group}",
                        for entry in entries {
                            option { value: "{entry.id}", "{entry.name}" }
                        }
                    }
                }
            }
        }
        if let Some(entry) = selected {
            div { class: "md3-catalog-selected",
                ProviderBadge {
                    name: entry.name.clone(),
                    color: entry.brand_color.clone(),
                    size: BadgeSize::Compact,
                }
                span { class: "text-body-small text-on-surface-variant",
                    "{entry.category_name.clone().unwrap_or_else(|| \"Uncategorized\".to_string())}"
                }
            }
        }
    }
}

/// Group provider options by category, alphabetically, with Uncategorized
/// last.
fn group_options(entries: &[CatalogEntry]) -> Vec<(String, Vec<CatalogEntry>)> {
    let mut groups: BTreeMap<String, Vec<CatalogEntry>> = BTreeMap::new();
    for entry in entries {
        let group = entry
            .category_name
            .clone()
            .unwrap_or_else(|| "Uncategorized".to_string());
        groups.entry(group).or_default().push(entry.clone());
    }
    let mut groups = groups.into_iter().collect::<Vec<_>>();
    groups.sort_by(|(left, _), (right, _)| {
        let left_uncategorized = left == "Uncategorized";
        let right_uncategorized = right == "Uncategorized";
        left_uncategorized
            .cmp(&right_uncategorized)
            .then_with(|| left.cmp(right))
    });
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: i64, name: &str, category: Option<&str>) -> CatalogEntry {
        CatalogEntry {
            id,
            name: name.to_string(),
            slug: name.to_lowercase(),
            category_name: category.map(str::to_string),
            brand_color: None,
        }
    }

    #[test]
    fn options_are_grouped_by_category_with_uncategorized_last() {
        let groups = group_options(&[
            entry(1, "Ledger", Some("Hardware wallet")),
            entry(2, "Trezor", Some("Hardware wallet")),
            entry(3, "Acme", None),
            entry(4, "MetaMask", Some("Browser extension")),
        ]);

        let labels: Vec<&str> = groups.iter().map(|(label, _)| label.as_str()).collect();
        assert_eq!(
            labels,
            ["Browser extension", "Hardware wallet", "Uncategorized"]
        );
        assert_eq!(groups[1].1.len(), 2);
    }
}
