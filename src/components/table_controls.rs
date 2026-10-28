//! Reusable controls of the shared data-table pattern (Section 5.11 of
//! CONTRIBUTING.md): the pagination bar with its editable page index and
//! sortable column headers.
//!
//! Every data-table screen composes these instead of re-implementing them,
//! so all tables paginate and sort the same way. The controls only report
//! intent through their event handlers; the screen owns the query state and
//! the backend decides ordering.

use dioxus::prelude::*;

use crate::domain::table::SortDirection;

/// Pagination bar of a data table, rendered directly above it: Previous and
/// Next around an editable 1-based page index and the totals. `page` is the
/// 0-based current page; `on_page` receives the 0-based page to show.
/// Entering an index outside the valid range clamps to it.
#[component]
pub fn TablePagination(
    page: u32,
    page_size: u32,
    total: u64,
    on_page: EventHandler<u32>,
) -> Element {
    let total_pages = total.div_ceil(u64::from(page_size.max(1))).max(1);
    let current_page = u64::from(page) + 1;
    // Text of the index input while it is being edited; `None` shows the
    // current page, so Previous/Next clicks update the display.
    let mut editing = use_signal(|| Option::<String>::None);

    let mut commit = move |_: ()| {
        let Some(raw) = editing() else { return };
        editing.set(None);
        let Ok(number) = raw.trim().parse::<u64>() else {
            return;
        };
        let clamped = number.clamp(1, total_pages);
        if clamped != current_page {
            on_page.call((clamped - 1) as u32);
        }
    };

    rsx! {
        div { class: "md3-pagination",
            button {
                r#type: "button",
                class: "md3-button-tonal",
                disabled: page == 0,
                onclick: move |_| on_page.call(page.saturating_sub(1)),
                "Previous"
            }
            form {
                class: "md3-pagination-index",
                onsubmit: move |event| {
                    event.prevent_default();
                    commit(());
                },
                input {
                    class: "md3-input",
                    r#type: "text",
                    inputmode: "numeric",
                    aria_label: "Page number",
                    value: editing().unwrap_or_else(|| current_page.to_string()),
                    oninput: move |event| editing.set(Some(event.value())),
                    onblur: move |_| commit(()),
                }
            }
            span { class: "text-body-small text-on-surface-variant",
                " of {total_pages} · {total} entries"
            }
            button {
                r#type: "button",
                class: "md3-button-tonal",
                disabled: current_page >= total_pages,
                onclick: move |_| on_page.call(page + 1),
                "Next"
            }
        }
    }
}

/// Sortable column header of a data table: a button toggling the backend
/// sort of its column. `active_direction` is the direction this column
/// currently sorts in, or `None` when another column or the table's default
/// order is active; `on_sort` decides the next state.
#[component]
pub fn SortableHeader(
    label: String,
    active_direction: Option<SortDirection>,
    on_sort: EventHandler<()>,
) -> Element {
    let (arrow, state) = match active_direction {
        Some(SortDirection::Asc) => ("↑", "ascending"),
        Some(SortDirection::Desc) => ("↓", "descending"),
        None => ("↕", "not sorted"),
    };

    rsx! {
        th {
            button {
                r#type: "button",
                class: "md3-sort-button",
                aria_label: "Sort by {label}, currently {state}",
                onclick: move |_| on_sort.call(()),
                "{label}"
                span { class: "md3-sort-arrow", aria_hidden: "true", "{arrow}" }
            }
        }
    }
}
