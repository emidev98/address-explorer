//! Reusable searchable multi-select used by the Flow filters.
//!
//! The control is presentational: it receives already-resolved options and
//! reports the new selection through `on_change`. The screen owns the query
//! state and the backend decides how the selection is applied.
use dioxus::prelude::*;
/// One option of a [`MultiSelect`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiSelectOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    /// Group heading; options are shown grouped in this order.
    pub group: String,
    /// Optional catalog/provider accent color.
    pub color: Option<String>,
    /// Optional trailing count, such as "12 addresses".
    pub count: Option<i64>,
    /// Additional text matched by the search box (address, owner, slug…).
    pub search_text: String,
}
impl MultiSelectOption {
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return true;
        }
        self.label.to_ascii_lowercase().contains(&needle)
            || self
                .description
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains(&needle)
            || self.search_text.to_ascii_lowercase().contains(&needle)
    }
}
/// Searchable multi-select with grouped options, removable chips and keyboard
/// dismissal.
#[component]
pub fn MultiSelect(
    /// Visible field label.
    label: String,
    /// Placeholder shown when nothing is selected.
    placeholder: String,
    options: Vec<MultiSelectOption>,
    selected: Vec<String>,
    on_change: EventHandler<Vec<String>>,
    #[props(default)] loading: bool,
    #[props(default)] disabled: bool,
    #[props(default)] id: String,
) -> Element {
    let mut open = use_signal(|| false);
    let mut search = use_signal(String::new);
    let field_id = if id.is_empty() {
        format!(
            "multiselect-{}",
            label.to_ascii_lowercase().replace(' ', "-")
        )
    } else {
        id.clone()
    };
    let list_id = format!("{field_id}-list");
    let filtered = options
        .iter()
        .filter(|option| option.matches(&search()))
        .cloned()
        .collect::<Vec<_>>();
    let selected_count = selected
        .iter()
        .filter(|id| options.iter().any(|option| &option.id == *id))
        .count();
    rsx! {
        div { class: "md3-filter-field",
            label {
                class: "text-body-small text-on-surface-variant",
                r#for: "{field_id}",
                "{label}"
            }
            div {
                class: "md3-multiselect",
                onkeydown: move |event| {
                    if event.key() == Key::Escape {
                        open.set(false);
                    }
                },
                button {
                    id: "{field_id}",
                    r#type: "button",
                    class: "md3-multiselect-trigger",
                    aria_haspopup: "listbox",
                    aria_expanded: open().to_string(),
                    aria_controls: "{list_id}",
                    disabled: disabled || loading,
                    onclick: move |_| {
                        if open() {
                            open.set(false);
                        } else {
                            search.set(String::new());
                            open.set(true);
                        }
                    },
                    span { class: "md3-multiselect-value",
                        if loading {
                            "Loading…"
                        } else if selected_count == 0 {
                            "{placeholder}"
                        } else {
                            "{selected_count} selected"
                        }
                    }
                    span { class: "md3-multiselect-chevron", aria_hidden: "true", "⌄" }
                }
                if open() {
                    div {
                        class: "md3-multiselect-scrim",
                        role: "presentation",
                        onclick: move | _ | open
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .set(false),
                    }
                    div {
                        class: "md3-multiselect-popover",
                        role: "presentation",
                        onclick: move |event| event.stop_propagation(),
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                r#type: "text",
                                value: "{search}",
                                placeholder: "Search {label}",
                                autofocus: true,
                                aria_label: "Search {label}",
                                oninput: move |event| search.set(event.value()),
                            }
                        }
                        div {
                            id: "{list_id}",
                            class: "md3-multiselect-list",
                            role: "listbox",
                            aria_multiselectable: "true",
                            aria_label: "{label}",
                            if filtered.is_empty() {
                                p { class: "md3-multiselect-empty text-body-small",
                                    "No matching options."
                                }
                            } else {
                                for (group, entries) in super::option_groups::group_options(
                                    &filtered,
                                    |option| option.group.clone(),
                                    "Unclassified",
                                )
                                {
                                    div {
                                        class: "md3-multiselect-group",
                                        role: "group",
                                        aria_label: "{group}",
                                        div { class: "md3-multiselect-group-title text-label-large",
                                            "{group}"
                                        }
                                        for option in entries {
                                            {
                                                let option_id = option.id.clone();
                                                let is_selected = selected.iter().any(|value| value == &option_id);
                                                let mut next = selected.clone();
                                                rsx! {
                                                    button {
                                                        r#type: "button",
                                                        class: if is_selected { "md3-multiselect-option md3-multiselect-option-selected" } else { "md3-multiselect-option" },
                                                        role: "option",
                                                        aria_selected: is_selected
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .to_string(),
                                                        onclick: move |_| {
                                                            if is_selected {
                                                                next.retain(|value| value != &option_id);
                                                            } else {
                                                                next.push(option_id.clone());
                                                            }
                                                            on_change.call(next.clone());
                                                        },
                                                        span { class: "md3-multiselect-check", aria_hidden: "true",
                                                            if is_selected {
                                                                "✓"
                                                            } else {
                                                                ""
                                                            }
                                                        }
                                                        if let Some(color) = &option.color {
                                                            span {
                                                                class: "md3-filter-dot",
                                                                style: "background-color: {color};",
                                                                aria_hidden: "true",
                                                            }
                                                        }
                                                        span { class: "md3-multiselect-option-text",
                                                            span { class: "md3-multiselect-option-label", "{option.label}" }
                                                            if let Some(description) = &option.description {
                                                                span { class: "md3-multiselect-option-description", "{description}" }
                                                            }
                                                        }
                                                        if let Some(count) = option.count {
                                                            span { class: "md3-multiselect-option-count", "{count}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        div { class: "md3-multiselect-actions",
                            button {
                                r#type: "button",
                                class: "md3-button-text",
                                onclick: move |_| on_change.call(Vec::new()),
                                "Clear"
                            }
                            button {
                                r#type: "button",
                                class: "md3-button-filled",
                                onclick: move |_| open.set(false),
                                "Done"
                            }
                        }
                    }
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn option(id: &str, label: &str, group: &str, text: &str) -> MultiSelectOption {
        MultiSelectOption {
            id: id.to_string(),
            label: label.to_string(),
            description: None,
            group: group.to_string(),
            color: None,
            count: None,
            search_text: text.to_string(),
        }
    }
    #[test]
    fn search_matches_label_description_and_search_text_case_insensitively() {
        let mut option = option("1", "Binance", "Exchange · Binance", "binance hot wallet");
        option.description = Some("Centralized exchange".to_string());
        assert!(option.matches("binance"));
        assert!(option.matches("HOT WALLET"));
        assert!(option.matches("centralized"));
        assert!(!option.matches("kraken"));
    }
    #[test]
    fn groups_are_sorted_and_unclassified_is_last() {
        let groups = super::super::option_groups::group_options(
            &[
                option("1", "Ledger", "Hardware wallet", ""),
                option("2", "Unclassified", "Unclassified", ""),
                option("3", "Exodus", "Desktop wallet", ""),
            ],
            |option| option.group.clone(),
            "Unclassified",
        );
        assert_eq!(groups[0].0, "Desktop wallet");
        assert_eq!(groups[1].0, "Hardware wallet");
        assert_eq!(groups[2].0, "Unclassified");
    }
}
