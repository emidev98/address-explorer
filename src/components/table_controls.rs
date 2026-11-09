//! Reusable controls of the shared data-table pattern: the pagination bar with its editable page index,
//! sortable column headers, and the opt-in multi-selection cells used by
//! tables with bulk actions.
//!
//! Every data-table screen composes these instead of re-implementing them,
//! so all tables paginate and sort the same way. The controls only report
//! intent through their event handlers; the screen owns the query state and
//! the backend decides ordering.
use crate::components::Checkbox;
use crate::domain::table::{SortDirection, TableQuery};
use dioxus::prelude::*;
use std::collections::HashSet;
use std::hash::Hash;
/// Reactive page and sort state shared by every data table.
#[derive(Clone, Copy, PartialEq)]
pub struct TableState {
    pub page: Signal<u32>,
    pub sort_by: Signal<Option<String>>,
    pub sort_dir: Signal<SortDirection>,
}
impl TableState {
    /// Current 0-based page index.
    pub fn page(self) -> u32 {
        (self.page)()
    }
    /// The query for the current page and sort.
    pub fn query(self, page_size: u32) -> TableQuery {
        TableQuery {
            page: self.page(),
            page_size,
            sort_by: (self.sort_by)(),
            sort_dir: (self.sort_dir)(),
        }
    }
    /// Direction this column currently sorts in, or `None` when inactive.
    pub fn direction_for(self, key: &str) -> Option<SortDirection> {
        ((self.sort_by)().as_deref() == Some(key)).then_some((self.sort_dir)())
    }
    /// Sort by `key`, toggling the direction of the active column, and restart
    /// the table at the first page.
    pub fn sort(mut self, key: &str, default_direction: SortDirection) {
        let (sort_by, sort_dir) = toggled_sort(
            (self.sort_by)().as_deref(),
            (self.sort_dir)(),
            key,
            default_direction,
        );
        self.sort_by.set(sort_by);
        self.sort_dir.set(sort_dir);
        self.page.set(0);
    }
    /// Return to the first page after a filter change.
    pub fn reset_page(mut self) {
        self.page.set(0);
    }
}
/// Create the page and sort state of one table.
pub fn use_table_state(default_direction: SortDirection) -> TableState {
    TableState {
        page: use_signal(|| 0_u32),
        sort_by: use_signal(|| Option::<String>::None),
        sort_dir: use_signal(|| default_direction),
    }
}
/// Next sort key and direction for a header click.
pub fn toggled_sort(
    current: Option<&str>,
    current_direction: SortDirection,
    key: &str,
    default_direction: SortDirection,
) -> (Option<String>, SortDirection) {
    if current == Some(key) {
        (Some(key.to_string()), current_direction.toggled())
    } else {
        (Some(key.to_string()), default_direction)
    }
}
/// Pagination bar of a data table, rendered directly below it: Previous and
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
                onclick: move | _ | on_page.call(page
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .saturating_sub(1)),
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
                    oninput: move |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        event | editing.set(Some(event.value())),
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
                onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        on_page.call(page + 1),
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
/// Multi-selection state of one table.
///
/// The selection is keyed by the table's row identifier (`i64` ids or the
/// address string). It is opt-in: a table only gains selection when it
/// renders [`TableSelectionHeader`] and [`TableSelectionCell`], so tables
/// without bulk actions keep their existing layout.
pub struct TableSelection<T: 'static> {
    selected: Signal<HashSet<T>>,
}
impl<T: 'static> Clone for TableSelection<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: 'static> Copy for TableSelection<T> {}
/// Create the multi-selection state of one table. Call it once with the
/// other hooks of the table component; the selected ids survive re-renders
/// until the screen clears them explicitly.
pub fn use_table_selection<T>() -> TableSelection<T>
where
    T: Clone + Eq + Hash + 'static,
{
    TableSelection {
        selected: use_signal(HashSet::<T>::new),
    }
}
impl<T> TableSelection<T>
where
    T: Clone + Eq + Hash + 'static,
{
    /// The checked ids as an owned set for one render pass.
    pub fn snapshot(&self) -> HashSet<T> {
        self.selected.read().clone()
    }
    /// Number of checked rows.
    pub fn len(&self) -> usize {
        self.selected.read().len()
    }
    /// Whether no row is checked.
    pub fn is_empty(&self) -> bool {
        self.selected.read().is_empty()
    }
    /// The checked ids. HashSet iteration order is unspecified; services
    /// normalize and deduplicate the list before using it.
    pub fn selected_ids(&self) -> Vec<T> {
        self.selected.read().iter().cloned().collect()
    }
    /// Check or uncheck one row.
    pub fn set(&mut self, id: T, selected: bool) {
        let mut guard = self.selected.write();
        if selected {
            guard.insert(id);
        } else {
            guard.remove(&id);
        }
    }
    /// Check or uncheck every id in the list, leaving other selections
    /// untouched.
    pub fn set_many(&mut self, ids: &[T], selected: bool) {
        let mut guard = self.selected.write();
        if selected {
            guard.extend(ids.iter().cloned());
        } else {
            for id in ids {
                guard.remove(id);
            }
        }
    }
    /// Whether every id in a non-empty list is checked.
    pub fn all_selected(&self, ids: &[T]) -> bool {
        if ids.is_empty() {
            return false;
        }
        let guard = self.selected.read();
        ids.iter().all(|id| guard.contains(id))
    }
    /// Whether any id in the list is checked.
    pub fn some_selected(&self, ids: &[T]) -> bool {
        let guard = self.selected.read();
        ids.iter().any(|id| guard.contains(id))
    }
    /// Drop the whole selection.
    pub fn clear(&mut self) {
        self.selected.write().clear();
    }
}
/// Select-all checkbox plus the bulk-action header cell of a table.
///
/// The action is rendered in the `th` directly next to the selector, so it
/// reads as applying to the selection; the row-level actions stay in the
/// second body cell. Screen-specific actions are passed as children, which
/// keeps the control reusable for future bulk operations beyond deletion.
#[component]
pub fn TableSelectionHeader(
    all_selected: bool,
    #[props(default)] indeterminate: bool,
    select_all_label: String,
    on_toggle_all: EventHandler<bool>,
    #[props(default)] disabled: bool,
    children: Element,
) -> Element {
    rsx! {
        th { class: "md3-table-select-cell",
            Checkbox {
                checked: all_selected,
                indeterminate,
                aria_label: select_all_label,
                disabled,
                on_change: move |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        checked | on_toggle_all.call(checked),
            }
        }
        th { class: "md3-table-select-actions", {children} }
    }
}
/// Checkbox cell of one selectable table row.
///
/// Clicks are stopped at the cell so a selectable row that also opens a
/// detail view or navigates from its row click does not trigger that action
/// when the checkbox is toggled.
#[component]
pub fn TableSelectionCell(checked: bool, label: String, on_change: EventHandler<bool>) -> Element {
    rsx! {
        td {
            class: "md3-table-select-cell",
            onclick: move | event | event
                                                                                                                                                                                                                                                                                                                                                                                                                                                        .stop_propagation(),
            Checkbox {
                checked,
                aria_label: label,
                on_change: move |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        checked | on_change.call(checked),
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::toggled_sort;
    use crate::domain::table::SortDirection;
    #[test]
    fn clicking_a_new_column_uses_its_default_direction() {
        assert_eq!(
            toggled_sort(None, SortDirection::Desc, "date", SortDirection::Asc),
            (Some("date".to_string()), SortDirection::Asc),
        );
    }
    #[test]
    fn clicking_the_active_column_toggles_the_direction() {
        assert_eq!(
            toggled_sort(
                Some("date"),
                SortDirection::Asc,
                "date",
                SortDirection::Desc
            ),
            (Some("date".to_string()), SortDirection::Desc),
        );
        assert_eq!(
            toggled_sort(
                Some("date"),
                SortDirection::Desc,
                "date",
                SortDirection::Asc
            ),
            (Some("date".to_string()), SortDirection::Asc),
        );
    }
}
