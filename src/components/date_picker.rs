//! Single Material 3 date field that manages one date or a from–to range.
//!
//! The field is a controlled component: callers own the committed
//! [`DateSelection`] and receive a validated replacement through `on_change`.
//! Calendar edits are a draft until OK (or Enter for typed input), so invalid
//! ranges never reach the screen. The popup follows the M3 docked date-picker
//! anatomy, with quick preset chips above the calendar, while the compact field
//! shows both endpoints in one input.
use super::icon::{Icon, IconName};
use super::interface_settings::use_interface_settings;
use crate::domain::calendar::{
    clamp_day, first_selectable_in_month, full_date_label, month_grid, month_has_selectable,
    month_label, month_length, shift_month, shift_year, year_options, CalendarDay, MONTH_NAMES,
    WEEKDAYS,
};
use crate::domain::civil_time::{days_from_civil, now_epoch_seconds};
use crate::domain::date_presets::{RangePreset, SingleDatePreset};
use crate::domain::date_selection::{
    DateBounds, DateSelection, DateSelectionDraft, DateSelectionError, DateSelectionMode, IsoDate,
};
use dioxus::prelude::*;
/// Which list menu is open above the calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarMenu {
    Month,
    Year,
}
/// Signals owned by one picker instance.
#[derive(Clone, Copy)]
struct PickerSignals {
    open: Signal<bool>,
    typed: Signal<String>,
    typed_valid: Signal<bool>,
    draft: Signal<DateSelectionDraft>,
    error: Signal<Option<DateSelectionError>>,
    dirty: Signal<bool>,
    focused: Signal<Option<IsoDate>>,
    view: Signal<(i64, i64)>,
    hover: Signal<Option<IsoDate>>,
    menu: Signal<Option<CalendarMenu>>,
    today: Signal<IsoDate>,
    focus_pending: Signal<bool>,
    menu_focus_pending: Signal<bool>,
}
impl PickerSignals {
    fn is_open(&self) -> bool {
        (self.open)()
    }
    fn typed_text(&self) -> String {
        (self.typed)()
    }
    fn typed_is_valid(&self) -> bool {
        (self.typed_valid)()
    }
    fn draft_value(&self) -> DateSelectionDraft {
        (self.draft)()
    }
    fn current_error(&self) -> Option<DateSelectionError> {
        (self.error)()
    }
    fn is_dirty(&self) -> bool {
        (self.dirty)()
    }
    fn focused_day(&self) -> Option<IsoDate> {
        (self.focused)()
    }
    fn view_month(&self) -> (i64, i64) {
        (self.view)()
    }
    fn hovered_day(&self) -> Option<IsoDate> {
        (self.hover)()
    }
    fn menu_value(&self) -> Option<CalendarMenu> {
        (self.menu)()
    }
    fn focus_pending_now(&self) -> bool {
        (self.focus_pending)()
    }
    fn menu_focus_pending_now(&self) -> bool {
        (self.menu_focus_pending)()
    }
    fn set_typed(self, raw: String, mode: DateSelectionMode, bounds: DateBounds) {
        let mut this = self;
        this.menu.set(None);
        this.menu_focus_pending.set(false);
        this.typed.set(raw.clone());
        this.dirty.set(true);
        match parse_and_validate(&raw, mode, &bounds) {
            Ok(selection) => {
                this.typed_valid.set(true);
                this.error.set(None);
                this.draft.set(DateSelectionDraft::new(selection));
                if this.is_open() {
                    show_selection(&mut this, &selection);
                }
            }
            Err(error) => {
                this.typed_valid.set(false);
                if should_show_typed_error(&raw) {
                    this.error.set(Some(error));
                } else {
                    this.error.set(None);
                }
            }
        }
    }
    fn sync_typed(self, mode: DateSelectionMode, bounds: DateBounds) {
        let mut this = self;
        match parse_and_validate(&this.typed_text(), mode, &bounds) {
            Ok(selection) => {
                this.typed_valid.set(true);
                this.error.set(None);
                this.draft.set(DateSelectionDraft::new(selection));
                if this.is_open() {
                    show_selection(&mut this, &selection);
                }
            }
            Err(error) => {
                this.typed_valid.set(false);
                this.error.set(Some(error));
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn commit_typed(
        self,
        value: &DateSelection,
        mode: DateSelectionMode,
        bounds: DateBounds,
        on_change: EventHandler<DateSelection>,
        id: &str,
        close: bool,
        focus_after: bool,
    ) -> bool {
        let mut this = self;
        let raw = if !this.is_dirty() && !this.is_open() {
            value.display(mode)
        } else {
            this.typed_text()
        };
        match parse_and_validate(&raw, mode, &bounds) {
            Ok(selection) => {
                apply_selection(this, selection, mode, on_change, close, focus_after, id);
                true
            }
            Err(error) => {
                this.error.set(Some(error));
                this.typed_valid.set(false);
                false
            }
        }
    }
    fn open_popup(
        self,
        value: &DateSelection,
        mode: DateSelectionMode,
        bounds: DateBounds,
        id: &str,
    ) {
        let mut this = self;
        if this.is_open() {
            this.cancel(value, mode, id);
            return;
        }
        if this.is_dirty() {
            match parse_and_validate(&this.typed_text(), mode, &bounds) {
                Ok(selection) => {
                    this.typed.set(selection.display(mode));
                    this.typed_valid.set(true);
                    this.error.set(None);
                    this.draft.set(DateSelectionDraft::new(selection));
                    this.dirty.set(false);
                }
                Err(_) => {
                    this.typed.set(value.display(mode));
                    this.typed_valid.set(true);
                    this.draft.set(DateSelectionDraft::new(*value));
                    this.error.set(None);
                    this.dirty.set(false);
                }
            }
        } else {
            this.typed.set(value.display(mode));
            this.typed_valid.set(value.validate(mode, &bounds).is_ok());
            this.error.set(None);
            this.draft.set(DateSelectionDraft::new(*value));
        }
        let selection = *this.draft_value().selection();
        let today = current_utc_date();
        this.today.set(today);
        let (view, focus) = initial_view_focus(&selection, &bounds, &today);
        this.view.set(view);
        this.focused.set(Some(focus));
        this.error.set(None);
        this.hover.set(None);
        this.menu.set(None);
        this.focus_pending.set(true);
        this.open.set(true);
    }
    fn cancel(self, value: &DateSelection, mode: DateSelectionMode, id: &str) {
        let mut this = self;
        this.typed.set(value.display(mode));
        this.typed_valid.set(true);
        this.draft.set(DateSelectionDraft::new(*value));
        this.error.set(None);
        this.dirty.set(false);
        this.open.set(false);
        this.menu.set(None);
        this.hover.set(None);
        focus_input(id);
    }
    fn select_day(
        self,
        value: DateSelection,
        day: &IsoDate,
        mode: DateSelectionMode,
        bounds: DateBounds,
        on_change: EventHandler<DateSelection>,
        id: &str,
    ) {
        if !bounds.contains(day) || !bounds.is_valid() {
            return;
        }
        let mut this = self;
        let mut draft = if this.is_dirty() {
            this.draft_value()
        } else {
            DateSelectionDraft::new(value)
        };
        draft.select_day(*day, mode);
        this.draft.set(draft);
        this.typed.set(draft.selection().display(mode));
        this.typed_valid.set(true);
        this.error.set(None);
        this.dirty.set(true);
        this.hover.set(None);
        this.focused.set(Some(*day));
        this.focus_pending.set(true);
        this.menu.set(None);
        this.menu_focus_pending.set(false);
        let (year, month, _) = day.year_month_day();
        if (year, month) != this.view_month() {
            this.view.set((year, month));
        }
        if selection_is_complete(draft.selection(), mode) {
            this.commit_draft(value, mode, bounds, on_change, id);
        }
    }
    fn clear_from(self, value: DateSelection, mode: DateSelectionMode) {
        let mut this = self;
        let mut draft = if this.is_dirty() {
            this.draft_value()
        } else {
            DateSelectionDraft::new(value)
        };
        draft.clear_from();
        this.draft.set(draft);
        this.typed.set(draft.selection().display(mode));
        this.typed_valid.set(true);
        this.error.set(None);
        this.dirty.set(true);
        this.hover.set(None);
        if let Some(to) = draft.selection().to {
            show_selection(&mut this, &DateSelection::new(None, Some(to)));
            this.focus_pending.set(true);
        }
    }
    fn clear_to(self, value: DateSelection, mode: DateSelectionMode) {
        let mut this = self;
        let mut draft = if this.is_dirty() {
            this.draft_value()
        } else {
            DateSelectionDraft::new(value)
        };
        draft.clear_to();
        this.draft.set(draft);
        this.typed.set(draft.selection().display(mode));
        this.typed_valid.set(true);
        this.error.set(None);
        this.dirty.set(true);
        this.hover.set(None);
        if let Some(from) = draft.selection().from {
            show_selection(&mut this, &DateSelection::new(Some(from), None));
            this.focus_pending.set(true);
        }
    }
    fn clear_all(
        self,
        mode: DateSelectionMode,
        bounds: DateBounds,
        on_change: EventHandler<DateSelection>,
        id: &str,
    ) {
        apply_selection(
            self,
            DateSelection::default_in_bounds(mode, &bounds),
            mode,
            on_change,
            true,
            true,
            id,
        );
    }
    fn commit_draft(
        self,
        value: DateSelection,
        mode: DateSelectionMode,
        bounds: DateBounds,
        on_change: EventHandler<DateSelection>,
        id: &str,
    ) -> bool {
        let mut this = self;
        let selection = if this.is_dirty() {
            *this.draft_value().selection()
        } else {
            value
        };
        match selection.validate(mode, &bounds) {
            Ok(()) => {
                apply_selection(this, selection, mode, on_change, true, true, id);
                true
            }
            Err(error) => {
                this.error.set(Some(error));
                false
            }
        }
    }
    fn move_focus(self, target: IsoDate, bounds: DateBounds) {
        let mut this = self;
        let target = clamp_to_bounds(&target, &bounds);
        this.focused.set(Some(target));
        this.focus_pending.set(true);
        this.hover.set(None);
        let (year, month, _) = target.year_month_day();
        if (year, month) != this.view_month() {
            this.view.set((year, month));
        }
    }
    fn step_month(self, delta: i64, bounds: DateBounds) {
        let mut this = self;
        let (year, month) = this.view_month();
        let (next_year, next_month) = shift_month(year, month, delta);
        if month_has_selectable(next_year, next_month, &bounds) {
            this.view.set((next_year, next_month));
            if let Some(day) = first_selectable_in_month(next_year, next_month, &bounds) {
                this.focused.set(Some(day));
            }
        }
        this.menu.set(None);
        this.menu_focus_pending.set(false);
    }
    fn set_view(self, year: i64, month: i64, bounds: DateBounds) {
        let mut this = self;
        if let Some((year, month)) = nearest_selectable_month(year, month, &bounds) {
            this.view.set((year, month));
            if let Some(day) = first_selectable_in_month(year, month, &bounds) {
                this.focused.set(Some(day));
                this.focus_pending.set(true);
            }
        }
        this.menu.set(None);
        this.menu_focus_pending.set(false);
    }
    fn set_year(self, year: i64, preferred_month: i64, bounds: DateBounds) {
        let mut this = self;
        if let Some((year, month)) = nearest_selectable_month(year, preferred_month, &bounds) {
            this.view.set((year, month));
            if let Some(day) = first_selectable_in_month(year, month, &bounds) {
                this.focused.set(Some(day));
                this.focus_pending.set(true);
            }
        }
        this.menu.set(None);
        this.menu_focus_pending.set(false);
    }
    fn request_calendar_focus(self) {
        let mut this = self;
        this.focus_pending.set(true);
    }
    fn close_menu(self) {
        let mut this = self;
        this.menu.set(None);
        this.menu_focus_pending.set(false);
        this.focus_pending.set(true);
    }
    fn toggle_menu(self, menu: CalendarMenu) {
        let mut this = self;
        if this.menu_value() == Some(menu) {
            this.menu.set(None);
            this.menu_focus_pending.set(false);
        } else {
            this.menu.set(Some(menu));
            this.menu_focus_pending.set(true);
        }
    }
}
/// Whether picking the current draft should apply it immediately: a single
/// date in Single mode, or both endpoints in Range mode.
fn selection_is_complete(selection: &DateSelection, mode: DateSelectionMode) -> bool {
    match mode {
        DateSelectionMode::Single => selection.from.is_some(),
        DateSelectionMode::Range => selection.from.is_some() && selection.to.is_some(),
    }
}
/// The text the single input shows. While the field is idle, the controlled
/// `value` is authoritative; otherwise the user's typed/drafted text wins.
fn displayed_selection_text(
    value: &DateSelection,
    mode: DateSelectionMode,
    typed: &str,
    dirty: bool,
) -> String {
    if dirty {
        typed.to_string()
    } else {
        value.display(mode)
    }
}
fn parse_and_validate(
    raw: &str,
    mode: DateSelectionMode,
    bounds: &DateBounds,
) -> Result<DateSelection, DateSelectionError> {
    let selection = DateSelection::parse_input(raw, mode)?;
    selection.validate(mode, bounds)?;
    Ok(selection)
}
fn should_show_typed_error(raw: &str) -> bool {
    let raw = raw.trim();
    raw.is_empty()
        || raw.len() >= 10
        || raw.contains('–')
        || raw.contains('—')
        || raw.contains(" - ")
        || raw.contains(" to ")
}
fn apply_selection(
    state: PickerSignals,
    next: DateSelection,
    mode: DateSelectionMode,
    on_change: EventHandler<DateSelection>,
    close: bool,
    focus_after: bool,
    id: &str,
) {
    let mut state = state;
    state.typed.set(next.display(mode));
    state.typed_valid.set(true);
    state.draft.set(DateSelectionDraft::new(next));
    state.error.set(None);
    state.dirty.set(false);
    if close {
        state.open.set(false);
        state.menu.set(None);
        state.hover.set(None);
    }
    on_change.call(next);
    if focus_after {
        focus_input(id);
    }
}
fn focus_element(id: &str) {
    let id = serde_json::to_string(id).unwrap_or_else(|_| "\"\"".to_string());
    document::eval(&format!(
        "const el = document.getElementById({id}); if (el) el.focus();"
    ));
}
fn focus_input(id: &str) {
    focus_element(id);
}
fn show_selection(state: &mut PickerSignals, selection: &DateSelection) {
    if let Some(date) = selection.from.as_ref().or(selection.to.as_ref()) {
        let (year, month, _) = date.year_month_day();
        if (year, month) != state.view_month() {
            state.view.set((year, month));
        }
        state.focused.set(Some(*date));
        state.focus_pending.set(false);
    }
}
fn clamp_to_bounds(date: &IsoDate, bounds: &DateBounds) -> IsoDate {
    if !bounds.is_valid() {
        return *date;
    }
    let mut date = *date;
    if let Some(min) = &bounds.min {
        if &date < min {
            date = *min;
        }
    }
    if let Some(max) = &bounds.max {
        if &date > max {
            date = *max;
        }
    }
    date
}
fn nearest_selectable_month(year: i64, month: i64, bounds: &DateBounds) -> Option<(i64, i64)> {
    if month_has_selectable(year, month, bounds) {
        return Some((year, month));
    }
    for distance in 1..=12 {
        for candidate in [month - distance, month + distance] {
            if (1..=12).contains(&candidate) && month_has_selectable(year, candidate, bounds) {
                return Some((year, candidate));
            }
        }
    }
    None
}
fn year_has_selectable(year: i64, bounds: &DateBounds) -> bool {
    (1..=12).any(|month| month_has_selectable(year, month, bounds))
}
fn initial_view_focus(
    selection: &DateSelection,
    bounds: &DateBounds,
    today: &IsoDate,
) -> ((i64, i64), IsoDate) {
    let preferred = selection.from.as_ref().or(selection.to.as_ref());
    let preferred_in_bounds = preferred.filter(|date| bounds.contains(date));
    let view = preferred_in_bounds
        .map(IsoDate::year_month_day)
        .map(|(year, month, _)| (year, month))
        .or_else(|| {
            bounds
                .contains(today)
                .then(|| today.year_month_day())
                .map(|(year, month, _)| (year, month))
        })
        .or_else(|| {
            bounds
                .min
                .as_ref()
                .map(IsoDate::year_month_day)
                .map(|(year, month, _)| (year, month))
        })
        .or_else(|| {
            bounds
                .max
                .as_ref()
                .map(IsoDate::year_month_day)
                .map(|(year, month, _)| (year, month))
        })
        .or_else(|| {
            preferred
                .map(IsoDate::year_month_day)
                .map(|(year, month, _)| (year, month))
        })
        .unwrap_or_else(|| {
            let (year, month, _) = today.year_month_day();
            (year, month)
        });
    let focus = preferred
        .filter(|date| {
            bounds.contains(date)
                && date.year_month_day().0 == view.0
                && date.year_month_day().1 == view.1
        })
        .cloned()
        .or_else(|| {
            (bounds.contains(today)
                && today.year_month_day().0 == view.0
                && today.year_month_day().1 == view.1)
                .then_some(*today)
        })
        .or_else(|| first_selectable_in_month(view.0, view.1, bounds))
        .unwrap_or_else(|| IsoDate::from_epoch_day(days_from_civil(view.0, view.1, 1)));
    (view, focus)
}
/// Today's UTC civil date from the runtime clock.
pub(crate) fn current_utc_date() -> IsoDate {
    IsoDate::from_epoch(now_epoch_seconds())
}
/// Reusable Material 3 date field.
///
/// `value` is the committed/default selection. While the user has not edited
/// the field, it is rendered directly, so data-driven defaults appear as soon
/// as the parent provides them. `min_date` and `max_date` are inclusive bounds
/// for both endpoints. In `Range` mode an empty side is an open end; in
/// `Single` mode only `from` is used. `on_change` is called with
/// a fully validated value as soon as a complete range or single date is
/// picked, after OK for open-ended ranges, after Enter or a valid blur for
/// typed input, and when the field is cleared. Clearing resets to the
/// configured bounds when they exist.
///
/// Supporting text follows the app-wide
/// `InterfaceSettings::show_date_range_support` preference. It is hidden by
/// default but remains available to assistive technology.
#[component]
pub fn DatePicker(
    /// DOM id of the single text input.
    id: String,
    /// Visible label and accessible name of the field.
    label: String,
    /// Committed selection.
    value: DateSelection,
    /// Emits a validated selection.
    on_change: EventHandler<DateSelection>,
    /// Range or one-date mode.
    #[props(default)]
    mode: DateSelectionMode,
    /// Inclusive earliest selectable day.
    #[props(default)]
    min_date: Option<IsoDate>,
    /// Inclusive latest selectable day.
    #[props(default)]
    max_date: Option<IsoDate>,
    #[props(default)] disabled: bool,
    /// Open the popup above the field, for fields near a dialog's bottom.
    #[props(default)]
    popup_above: bool,
    /// Align the popup with the field's right edge.
    #[props(default)]
    popup_align_end: bool,
    /// Force one calendar month even on wide viewports.
    #[props(default)]
    compact: bool,
) -> Element {
    let show_support = use_interface_settings().show_date_range_support();
    let bounds = DateBounds::new(min_date, max_date);
    let today_value = current_utc_date();
    let (initial_view, initial_focus) = initial_view_focus(&value, &bounds, &today_value);
    let open = use_signal(|| false);
    let typed = use_signal(|| value.display(mode));
    let typed_valid = use_signal(|| value.validate(mode, &bounds).is_ok());
    let draft = use_signal(|| DateSelectionDraft::new(value));
    let error = use_signal(|| Option::<DateSelectionError>::None);
    let dirty = use_signal(|| false);
    let focused = use_signal(|| Some(initial_focus));
    let view = use_signal(|| initial_view);
    let hover = use_signal(|| Option::<IsoDate>::None);
    let menu = use_signal(|| Option::<CalendarMenu>::None);
    let today = use_signal(|| today_value);
    let focus_pending = use_signal(|| false);
    let menu_focus_pending = use_signal(|| false);
    let state = PickerSignals {
        open,
        typed,
        typed_valid,
        draft,
        error,
        dirty,
        focused,
        view,
        hover,
        menu,
        today,
        focus_pending,
        menu_focus_pending,
    };
    let focus_effect_id = id.clone();
    let mut focus_effect_state = state;
    use_effect(move || {
        if !focus_effect_state.is_open() || !focus_effect_state.focus_pending_now() {
            return;
        }
        let Some(day) = focus_effect_state.focused_day() else {
            return;
        };
        let day_id = serde_json::to_string(&format!("{focus_effect_id}-day-primary-{day}"))
            .unwrap_or_else(|_| "\"\"".to_string());
        document::eval(&format!(
            "const el = document.getElementById({day_id}); if (el) el.focus();"
        ));
        focus_effect_state.focus_pending.set(false);
    });
    let menu_effect_id = id.clone();
    let mut menu_effect_state = state;
    use_effect(move || {
        if !menu_effect_state.is_open() || !menu_effect_state.menu_focus_pending_now() {
            return;
        }
        let Some(menu) = menu_effect_state.menu_value() else {
            menu_effect_state.menu_focus_pending.set(false);
            return;
        };
        let (year, month) = menu_effect_state.view_month();
        let item = match menu {
            CalendarMenu::Month => format!("{menu_effect_id}-month-{month}"),
            CalendarMenu::Year => format!("{menu_effect_id}-year-{year}"),
        };
        let item = serde_json::to_string(&item).unwrap_or_else(|_| "\"\"".to_string());
        document::eval(&format!(
            "const el = document.getElementById({item}); if (el) el.focus();"
        ));
        menu_effect_state.menu_focus_pending.set(false);
    });
    let working_draft = if state.is_dirty() {
        state.draft_value()
    } else {
        DateSelectionDraft::new(value)
    };
    let selection = *working_draft.selection();
    let displayed_text =
        displayed_selection_text(&value, mode, &state.typed_text(), state.is_dirty());
    let displayed_valid = if state.is_dirty() {
        state.typed_is_valid()
    } else {
        value.validate(mode, &bounds).is_ok()
    };
    let preview_end = state.hovered_day().or_else(|| state.focused_day());
    let preview = working_draft.preview(preview_end.as_ref());
    let pending_start = working_draft.pending_start().cloned();
    let display_error = if state.is_dirty() {
        state
            .current_error()
            .or_else(|| selection.validate(mode, &bounds).err())
    } else {
        selection.validate(mode, &bounds).err()
    }
    .or_else(|| (!bounds.is_valid()).then_some(DateSelectionError::InvalidBounds));
    let error_text = display_error.as_ref().map(ToString::to_string);
    let helper = match mode {
        DateSelectionMode::Range => "YYYY-MM-DD – YYYY-MM-DD · either side can be open",
        DateSelectionMode::Single => "YYYY-MM-DD",
    };
    let input_aria_label = format!("{label}, {}", DateSelection::placeholder(mode));
    let clear_aria_label = if bounds.min.is_some() || bounds.max.is_some() {
        match mode {
            DateSelectionMode::Range => "Reset date range",
            DateSelectionMode::Single => "Reset date",
        }
    } else {
        match mode {
            DateSelectionMode::Range => "Clear dates",
            DateSelectionMode::Single => "Clear date",
        }
    };
    let (view_year, view_month) = state.view_month();
    let (second_year, second_month) = shift_month(view_year, view_month, 1);
    let show_second_month = !compact && month_has_selectable(second_year, second_month, &bounds);
    let previous_month = shift_month(view_year, view_month, -1);
    let next_month = shift_month(view_year, view_month, 1);
    let previous_enabled = month_has_selectable(previous_month.0, previous_month.1, &bounds);
    let next_enabled = month_has_selectable(next_month.0, next_month.1, &bounds);
    let can_commit = bounds.is_valid()
        && (!state.is_dirty() || (state.typed_is_valid() && state.current_error().is_none()))
        && selection.validate(mode, &bounds).is_ok();
    let announcement = selection_announcement(&working_draft, mode);
    let popup_class = [
        "md3-date-picker-popup",
        if popup_above {
            "md3-date-picker-popup-above"
        } else {
            ""
        },
        if popup_align_end {
            "md3-date-picker-popup-end"
        } else {
            ""
        },
        if compact {
            "md3-date-picker-popup-compact"
        } else {
            ""
        },
        if !show_second_month {
            "md3-date-picker-popup-single"
        } else {
            ""
        },
    ]
    .into_iter()
    .filter(|class| !class.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    let field_class = if state.is_open() {
        "md3-date-picker md3-date-picker-open"
    } else {
        "md3-date-picker"
    };
    let described_by = if display_error.is_some() {
        format!("{id}-error")
    } else {
        format!("{id}-helper")
    };
    let clear_visible = !displayed_text.is_empty() || !selection.is_empty();
    let input_field_class = if clear_visible && !disabled {
        "md3-text-field md3-date-picker-field md3-date-picker-field-with-clear"
    } else {
        "md3-text-field md3-date-picker-field"
    };
    let input_state = state;
    let blur_state = state;
    let enter_state = state;
    let toggle_state = state;
    let clear_state = state;
    let cancel_state = state;
    let menu_escape_state = state;
    let ok_state = state;
    let prev_state = state;
    let next_state = state;
    let month_menu_state = state;
    let year_menu_state = state;
    let value_for_input_key = value;
    let value_for_blur = value;
    let value_for_scrim = value;
    let value_for_popup_key = value;
    let value_for_cancel_button = value;
    let value_for_toggle = value;
    let id_for_input_key = id.clone();
    let id_for_blur = id.clone();
    let id_for_scrim = id.clone();
    let id_for_popup_key = id.clone();
    let id_for_cancel_button = id.clone();
    let id_for_toggle = id.clone();
    let id_for_clear = id.clone();
    let id_for_ok = id.clone();
    let calendar_key_id = id.clone();
    let calendar_key_value = value;
    let field_id = id.clone();
    let field_label = label.clone();
    let range_from = selection.from;
    let range_to = selection.to;
    let from_text = range_from
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "Open".to_string());
    let to_text = range_to
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "Open".to_string());
    let month_label_text = MONTH_NAMES[(view_month - 1) as usize];
    rsx! {
        div { class: "{field_class}",
            div { class: "md3-date-picker-label-row",
                label {
                    class: "text-body-small text-on-surface-variant",
                    r#for: "{field_id}",
                    "{field_label}"
                }
            }
            div { class: "{input_field_class}",
                input {
                    id: "{field_id}",
                    class: "md3-input md3-date-picker-input",
                    r#type: "text",
                    value: "{displayed_text}",
                    placeholder: DateSelection::placeholder(mode),
                    aria_label: "{input_aria_label}",
                    autocomplete: "off",
                    spellcheck: false,
                    disabled,
                    aria_invalid: (display_error.is_some() || !displayed_valid).to_string(),
                    aria_describedby: "{described_by}",
                    oninput: move |event| {
                        input_state.set_typed(event.value(), mode, bounds);
                    },
                    onblur: move |_| {
                        if blur_state.is_open() {
                            blur_state.sync_typed(mode, bounds);
                        } else if blur_state.is_dirty() {
                            blur_state
                                .commit_typed(
                                    &value_for_blur,
                                    mode,
                                    bounds,
                                    on_change,
                                    &id_for_blur,
                                    false,
                                    false,
                                );
                        }
                    },
                    onkeydown: move |event| match event.key() {
                        Key::Enter => {
                            event.prevent_default();
                            enter_state
                                .commit_typed(
                                    &value_for_input_key,
                                    mode,
                                    bounds,
                                    on_change,
                                    &id_for_input_key,
                                    true,
                                    true,
                                );
                        }
                        Key::Escape => {
                            event.prevent_default();
                            if enter_state.menu_value().is_some() {
                                enter_state.close_menu();
                            } else {
                                enter_state.cancel(&value_for_input_key, mode, &id_for_input_key);
                            }
                        }
                        Key::ArrowDown => {
                            event.prevent_default();
                            if enter_state.is_open() {
                                enter_state.request_calendar_focus();
                            } else {
                                enter_state
                                    .open_popup(&value_for_input_key, mode, bounds, &id_for_input_key);
                            }
                        }
                        _ => {}
                    },
                }
                if clear_visible && !disabled {
                    button {
                        r#type: "button",
                        class: "md3-icon-button md3-date-picker-icon md3-date-picker-clear",
                        aria_label: clear_aria_label,
                        onclick: move |_| {
                            clear_state.clear_all(mode, bounds, on_change, &id_for_clear);
                        },
                        Icon { name: IconName::Close, size: 20 }
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-icon-button md3-date-picker-icon md3-date-picker-calendar",
                    aria_label: if state.is_open() { "Close calendar" } else { "Open calendar" },
                    aria_haspopup: "dialog",
                    aria_expanded: state.is_open().to_string(),
                    aria_controls: "{id}-popup",
                    disabled,
                    onclick: move |_| {
                        toggle_state.open_popup(&value_for_toggle, mode, bounds, &id_for_toggle);
                    },
                    Icon { name: IconName::Calendar, size: 20 }
                }
            }
            if let Some(error) = error_text {
                if show_support {
                    p {
                        id: "{id}-error",
                        class: "md3-date-picker-support md3-date-picker-error text-body-small",
                        role: "alert",
                        "{error}"
                    }
                } else {
                    span {
                        id: "{id}-error",
                        class: "md3-date-picker-error md3-visually-hidden",
                        role: "alert",
                        "{error}"
                    }
                }
            } else if show_support {
                p {
                    id: "{id}-helper",
                    class: "md3-date-picker-support md3-date-picker-helper text-body-small",
                    "{helper}"
                }
            } else {
                span {
                    id: "{id}-helper",
                    class: "md3-date-picker-helper md3-visually-hidden",
                    "{helper}"
                }
            }
            if state.is_open() {
                div {
                    class: "md3-date-picker-scrim",
                    role: "presentation",
                    onclick: move |_| {
                        cancel_state.cancel(&value_for_scrim, mode, &id_for_scrim);
                    },
                }
                div {
                    id: "{id}-popup",
                    class: "{popup_class}",
                    role: "dialog",
                    aria_modal: "false",
                    aria_label: if mode == DateSelectionMode::Single { "Choose a date" } else { "Choose a date range" },
                    onkeydown: move |event| {
                        if event.key() == Key::Escape {
                            event.prevent_default();
                            if menu_escape_state.menu_value().is_some() {
                                menu_escape_state.close_menu();
                            } else {
                                cancel_state.cancel(&value_for_popup_key, mode, &id_for_popup_key);
                            }
                        }
                    },
                    div { class: "md3-date-picker-header",
                        button {
                            r#type: "button",
                            class: "md3-icon-button",
                            aria_label: "Previous month",
                            disabled: !previous_enabled,
                            onclick: move | _
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        | prev_state.step_month(- 1, bounds),
                            Icon { name: IconName::ChevronLeft, size: 20 }
                        }
                        div { class: "md3-date-picker-heading",
                            button {
                                r#type: "button",
                                class: "md3-date-picker-heading-button",
                                aria_haspopup: "listbox",
                                aria_expanded: (state.menu_value() == Some(CalendarMenu::Month)).to_string(),
                                onclick: move |_| {
                                    month_menu_state.toggle_menu(CalendarMenu::Month);
                                },
                                "{month_label_text}"
                                Icon { name: IconName::Dropdown, size: 20 }
                            }
                            button {
                                r#type: "button",
                                class: "md3-date-picker-heading-button",
                                aria_haspopup: "listbox",
                                aria_expanded: (state.menu_value() == Some(CalendarMenu::Year)).to_string(),
                                onclick: move |_| {
                                    year_menu_state.toggle_menu(CalendarMenu::Year);
                                },
                                "{view_year}"
                                Icon { name: IconName::Dropdown, size: 20 }
                            }
                        }
                        button {
                            r#type: "button",
                            class: "md3-icon-button",
                            aria_label: "Next month",
                            disabled: !next_enabled,
                            onclick: move |_| next_state.step_month(1, bounds),
                            Icon { name: IconName::ChevronRight, size: 20 }
                        }
                    }
                    match state.menu_value() {
                        Some(CalendarMenu::Month) => {
                            month_menu(state, view_year, view_month, bounds, &id)
                        }
                        Some(CalendarMenu::Year) => year_menu(state, view_year, view_month, bounds, &id),
                        None => rsx! {},
                    }
                    {quick_presets_view(state, mode, bounds, today(), selection, on_change, &id)}
                    if mode == DateSelectionMode::Range {
                        div { class: "md3-date-picker-range-summary",
                            div { class: "md3-date-picker-range-item",
                                span { class: "md3-date-picker-range-label", "From" }
                                span { class: "md3-date-picker-range-value", "{from_text}" }
                                if range_from.is_some() {
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-date-picker-range-clear",
                                        aria_label: "Clear start date",
                                        onclick: move |_| state.clear_from(value, mode),
                                        Icon { name: IconName::Close, size: 20 }
                                    }
                                }
                            }
                            div { class: "md3-date-picker-range-item",
                                span { class: "md3-date-picker-range-label", "To" }
                                span { class: "md3-date-picker-range-value", "{to_text}" }
                                if range_to.is_some() {
                                    button {
                                        r#type: "button",
                                        class: "md3-icon-button md3-date-picker-range-clear",
                                        aria_label: "Clear end date",
                                        onclick: move |_| state.clear_to(value, mode),
                                        Icon { name: IconName::Close, size: 20 }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "md3-date-picker-months",
                        {
                            month_grid_view(
                                &id,
                                view_year,
                                view_month,
                                true,
                                show_second_month,
                                state,
                                on_change,
                                mode,
                                bounds,
                                &selection,
                                &preview,
                                pending_start.as_ref(),
                                state.focused_day().as_ref(),
                                &today(),
                                &calendar_key_value,
                                &calendar_key_id,
                            )
                        }
                        if show_second_month {
                            {
                                month_grid_view(
                                    &id,
                                    second_year,
                                    second_month,
                                    false,
                                    false,
                                    state,
                                    on_change,
                                    mode,
                                    bounds,
                                    &selection,
                                    &preview,
                                    pending_start.as_ref(),
                                    state.focused_day().as_ref(),
                                    &today(),
                                    &calendar_key_value,
                                    &calendar_key_id,
                                )
                            }
                        }
                    }
                    div {
                        class: "md3-visually-hidden",
                        role: "status",
                        aria_live: "polite",
                        "{announcement}"
                    }
                    div { class: "md3-date-picker-divider" }
                    div { class: "md3-date-picker-actions",
                        button {
                            r#type: "button",
                            class: "md3-button-text",
                            onclick: move |_| {
                                cancel_state.cancel(&value_for_cancel_button, mode, &id_for_cancel_button);
                            },
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "md3-button-text",
                            disabled: !can_commit,
                            onclick: move |_| {
                                ok_state.commit_draft(value, mode, bounds, on_change, &id_for_ok);
                            },
                            "OK"
                        }
                    }
                }
            }
        }
    }
}
/// Quick preset chips shown above the range summary and calendar.
fn quick_presets_view(
    state: PickerSignals,
    mode: DateSelectionMode,
    bounds: DateBounds,
    today: IsoDate,
    selection: DateSelection,
    on_change: EventHandler<DateSelection>,
    field_id: &str,
) -> Element {
    let entries: Vec<(&'static str, Option<DateSelection>, bool)> = match mode {
        DateSelectionMode::Range => RangePreset::ALL
            .into_iter()
            .map(|preset| {
                let preset_selection = preset.selection(today, &bounds);
                (
                    preset.label(),
                    preset_selection,
                    preset_selection == Some(selection),
                )
            })
            .collect(),
        DateSelectionMode::Single => SingleDatePreset::ALL
            .into_iter()
            .map(|preset| {
                let preset_selection = preset.selection(today, &bounds).map(DateSelection::single);
                (
                    preset.label(),
                    preset_selection,
                    preset_selection == Some(selection),
                )
            })
            .collect(),
    };
    let group_label = match mode {
        DateSelectionMode::Range => "Quick date ranges",
        DateSelectionMode::Single => "Quick dates",
    };
    rsx! {
        div {
            class: "md3-chip-group md3-date-picker-quick-ranges",
            role: "group",
            aria_label: group_label,
            for (label, preset_selection, selected) in entries {
                {
                    let click_state = state;
                    let click_on_change = on_change;
                    let click_id = field_id.to_string();
                    let aria_label = match preset_selection {
                        Some(value) => format!("{label}: {}", value.display(mode)),
                        None => format!("{label}: unavailable within the selected date bounds"),
                    };
                    let chip_class = if selected {
                        "md3-chip md3-date-picker-quick-range md3-chip-selected"
                    } else {
                        "md3-chip md3-date-picker-quick-range"
                    };
                    rsx! {
                        button {
                            r#type: "button",
                            class: chip_class,
                            disabled: preset_selection.is_none(),
                            aria_label: "{aria_label}",
                            onclick: move |_| {
                                if let Some(value) = preset_selection {
                                    apply_selection(
                                        click_state,
                                        value,
                                        mode,
                                        click_on_change,
                                        true,
                                        true,
                                        &click_id,
                                    );
                                }
                            },
                            "{label}"
                        }
                    }
                }
            }
        }
    }
}
fn month_menu(
    state: PickerSignals,
    year: i64,
    selected_month: i64,
    bounds: DateBounds,
    field_id: &str,
) -> Element {
    let enabled_months: Vec<i64> = (1..=12)
        .filter(|month| month_has_selectable(year, *month, &bounds))
        .collect();
    let first_month = enabled_months.first().copied();
    let last_month = enabled_months.last().copied();
    rsx! {
        div {
            class: "md3-date-picker-menu",
            role: "listbox",
            aria_label: "Month",
            for month in 1..=12 {
                {
                    let enabled = month_has_selectable(year, month, &bounds);
                    let selected = month == selected_month;
                    let menu_state = state;
                    let next_month = enabled_months
                        .iter()
                        .copied()
                        .find(|candidate| *candidate > month)
                        .or_else(|| enabled_months.first().copied());
                    let previous_month = enabled_months
                        .iter()
                        .copied()
                        .rfind(|candidate| *candidate < month)
                        .or_else(|| enabled_months.last().copied());
                    let menu_id = field_id.to_string();
                    rsx! {
                        button {
                            id: "{field_id}-month-{month}",
                            r#type: "button",
                            class: if selected { "md3-date-picker-menu-item md3-date-picker-menu-item-selected" } else { "md3-date-picker-menu-item" },
                            role: "option",
                            aria_selected: selected
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .to_string(),
                            aria_disabled: (!enabled).to_string(),
                            disabled: !enabled,
                            onclick: move |_| menu_state.set_view(year, month, bounds),
                            onkeydown: move |event| {
                                let target = match event.key() {
                                    Key::ArrowDown => next_month,
                                    Key::ArrowUp => previous_month,
                                    Key::Home => first_month,
                                    Key::End => last_month,
                                    _ => None,
                                };
                                if let Some(target) = target {
                                    event.prevent_default();
                                    focus_element(&format!("{menu_id}-month-{target}"));
                                }
                            },
                            "{MONTH_NAMES[(month - 1) as usize]}"
                        }
                    }
                }
            }
        }
    }
}
fn year_menu(
    state: PickerSignals,
    view_year: i64,
    selected_month: i64,
    bounds: DateBounds,
    field_id: &str,
) -> Element {
    let years = year_options(view_year, &bounds);
    let enabled_years: Vec<i64> = years
        .iter()
        .copied()
        .filter(|year| year_has_selectable(*year, &bounds))
        .collect();
    let first_year = enabled_years.first().copied();
    let last_year = enabled_years.last().copied();
    rsx! {
        div {
            class: "md3-date-picker-menu md3-date-picker-menu-years",
            role: "listbox",
            aria_label: "Year",
            for year in years {
                {
                    let enabled = year_has_selectable(year, &bounds);
                    let selected = year == view_year;
                    let menu_state = state;
                    let position = enabled_years.iter().position(|candidate| *candidate == year);
                    let next_year = position
                        .and_then(|index| enabled_years.get(index + 1).copied())
                        .or_else(|| enabled_years.first().copied());
                    let previous_year = position
                        .and_then(|index| {
                            index.checked_sub(1).and_then(|index| enabled_years.get(index).copied())
                        })
                        .or_else(|| enabled_years.last().copied());
                    let menu_id = field_id.to_string();
                    rsx! {
                        button {
                            id: "{field_id}-year-{year}",
                            r#type: "button",
                            class: if selected { "md3-date-picker-menu-item md3-date-picker-menu-item-selected" } else { "md3-date-picker-menu-item" },
                            role: "option",
                            aria_selected: selected
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .to_string(),
                            aria_disabled: (!enabled).to_string(),
                            disabled: !enabled,
                            onclick: move |_| {
                                menu_state.set_year(year, selected_month, bounds);
                            },
                            onkeydown: move |event| {
                                let target = match event.key() {
                                    Key::ArrowDown => next_year,
                                    Key::ArrowUp => previous_year,
                                    Key::Home => first_year,
                                    Key::End => last_year,
                                    _ => None,
                                };
                                if let Some(target) = target {
                                    event.prevent_default();
                                    focus_element(&format!("{menu_id}-year-{target}"));
                                }
                            },
                            "{year}"
                        }
                    }
                }
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn month_grid_view(
    field_id: &str,
    year: i64,
    month: i64,
    primary: bool,
    show_secondary: bool,
    state: PickerSignals,
    on_change: EventHandler<DateSelection>,
    mode: DateSelectionMode,
    bounds: DateBounds,
    selection: &DateSelection,
    preview: &DateSelection,
    pending_start: Option<&IsoDate>,
    focused: Option<&IsoDate>,
    today: &IsoDate,
    key_value: &DateSelection,
    key_id: &str,
) -> Element {
    let grid = month_grid(year, month);
    let panel = if primary { "primary" } else { "secondary" };
    let panel_class = if primary {
        "md3-date-picker-month-panel"
    } else {
        "md3-date-picker-month-panel md3-date-picker-month-panel-secondary"
    };
    let grid_label = month_label(year, month);
    let key_value = *key_value;
    let key_id = key_id.to_string();
    rsx! {
        div { class: "{panel_class}",
            if !primary {
                div { class: "md3-date-picker-month-label", "{month_label(year, month)}" }
            } else if show_secondary {
                div {
                    class: "md3-date-picker-month-label md3-date-picker-month-label-spacer",
                    aria_hidden: "true",
                    "{month_label(year, month)}"
                }
            }
            div {
                class: "md3-date-picker-grid",
                role: "grid",
                aria_label: "{grid_label}",
                onmouseleave: move |_| {
                    let mut state = state;
                    state.hover.set(None);
                },
                onkeydown: move |event: KeyboardEvent| {
                    if !primary {
                        return;
                    }
                    let key_state = state;
                    handle_calendar_key(event, key_state, mode, bounds, &key_value, &key_id);
                },
                div { class: "md3-date-picker-week", role: "row",
                    for index in 0..7 {
                        span {
                            class: "md3-date-picker-weekday",
                            role: "columnheader",
                            aria_label: crate
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        ::domain::calendar::WEEKDAY_NAMES[index],
                            "{WEEKDAYS[index]}"
                        }
                    }
                }
                for week in grid.days.chunks(7) {
                    div { class: "md3-date-picker-week", role: "row",
                        for day in week {
                            {
                                let day = day.clone();
                                day_cell(
                                    field_id,
                                    panel,
                                    primary,
                                    day,
                                    key_value,
                                    state,
                                    on_change,
                                    mode,
                                    bounds,
                                    selection,
                                    preview,
                                    pending_start,
                                    focused,
                                    today,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn day_cell(
    field_id: &str,
    panel: &str,
    primary: bool,
    day: CalendarDay,
    value: DateSelection,
    state: PickerSignals,
    on_change: EventHandler<DateSelection>,
    mode: DateSelectionMode,
    bounds: DateBounds,
    selection: &DateSelection,
    preview: &DateSelection,
    pending_start: Option<&IsoDate>,
    focused: Option<&IsoDate>,
    today: &IsoDate,
) -> Element {
    let date = day.date;
    let enabled = bounds.contains(&date) && bounds.is_valid();
    let is_today = date == *today;
    let selected_start = selection.from.as_ref() == Some(&date);
    let selected_end = selection.to.as_ref() == Some(&date);
    let preview_start = preview.from.as_ref() == Some(&date);
    let preview_end = preview.to.as_ref() == Some(&date);
    let in_preview = preview.contains(&date);
    let pending = pending_start.is_some();
    let is_focused = primary && focused == Some(&date);
    let mut classes = String::from("md3-date-picker-day");
    if !day.in_month {
        classes.push_str(" md3-date-picker-day-outside");
    }
    if !enabled {
        classes.push_str(" md3-date-picker-day-disabled");
    }
    if is_today {
        classes.push_str(" md3-date-picker-day-today");
    }
    if selected_start {
        classes.push_str(" md3-date-picker-day-selected md3-date-picker-day-start");
    }
    if selected_end {
        classes.push_str(" md3-date-picker-day-selected md3-date-picker-day-end");
    }
    if in_preview && !selected_start && !selected_end {
        classes.push_str(" md3-date-picker-day-in-range");
    }
    if pending && !selected_start && !selected_end && (preview_start || preview_end) {
        classes.push_str(" md3-date-picker-day-preview-edge");
    }
    let mut label = full_date_label(&date);
    if is_today {
        label = format!("Today, {label}");
    }
    if selected_start {
        label = format!("Start date, {label}");
    } else if selected_end {
        label = format!("End date, {label}");
    } else if in_preview {
        label = format!("In selected range, {label}");
    }
    if !enabled {
        label = format!("Unavailable, {label}");
    }
    let tabindex = if is_focused { "0" } else { "-1" };
    let click_state = state;
    let click_on_change = on_change;
    let click_id = field_id.to_string();
    let mut hover_state = state;
    let mut focus_state = state;
    let click_date = date;
    let hover_date = date;
    let focus_date = date;
    rsx! {
        button {
            id: "{field_id}-day-{panel}-{date}",
            r#type: "button",
            class: "{classes}",
            role: "gridcell",
            tabindex: "{tabindex}",
            aria_label: "{label}",
            aria_selected: (selected_start || selected_end || in_preview).to_string(),
            aria_current: if is_today { "date" } else { "false" },
            aria_disabled: (!
                                                                                                                                                                                                                                                                                                                                                                                                                                                        enabled).to_string(),
            disabled: !enabled,
            onclick: move |_| {
                click_state
                    .select_day(value, &click_date, mode, bounds, click_on_change, &click_id);
            },
            onmouseenter: move |_| hover_state.hover.set(Some(hover_date)),
            onfocus: move |_| {
                if primary {
                    focus_state.focused.set(Some(focus_date));
                }
            },
            "{date.year_month_day().2}"
        }
    }
}
fn handle_calendar_key(
    event: KeyboardEvent,
    state: PickerSignals,
    mode: DateSelectionMode,
    bounds: DateBounds,
    value: &DateSelection,
    id: &str,
) {
    let Some(current) = state.focused_day() else {
        return;
    };
    let (view_year, view_month) = state.view_month();
    let (current_year, current_month, current_day) = current.year_month_day();
    let shift = event.modifiers().contains(Modifiers::SHIFT);
    let target = match event.key() {
        Key::ArrowLeft => Some(IsoDate::from_epoch_day(current.epoch_day() - 1)),
        Key::ArrowRight => Some(IsoDate::from_epoch_day(current.epoch_day() + 1)),
        Key::ArrowUp => Some(IsoDate::from_epoch_day(current.epoch_day() - 7)),
        Key::ArrowDown => Some(IsoDate::from_epoch_day(current.epoch_day() + 7)),
        Key::Home => Some(IsoDate::from_epoch_day(days_from_civil(
            view_year, view_month, 1,
        ))),
        Key::End => Some(IsoDate::from_epoch_day(days_from_civil(
            view_year,
            view_month,
            month_length(view_year, view_month),
        ))),
        Key::PageUp if shift => {
            let (year, month) = shift_year(current_year, current_month, -1);
            Some(IsoDate::from_epoch_day(days_from_civil(
                year,
                month,
                clamp_day(year, month, current_day),
            )))
        }
        Key::PageDown if shift => {
            let (year, month) = shift_year(current_year, current_month, 1);
            Some(IsoDate::from_epoch_day(days_from_civil(
                year,
                month,
                clamp_day(year, month, current_day),
            )))
        }
        Key::PageUp => {
            let (year, month) = shift_month(current_year, current_month, -1);
            Some(IsoDate::from_epoch_day(days_from_civil(
                year,
                month,
                clamp_day(year, month, current_day),
            )))
        }
        Key::PageDown => {
            let (year, month) = shift_month(current_year, current_month, 1);
            Some(IsoDate::from_epoch_day(days_from_civil(
                year,
                month,
                clamp_day(year, month, current_day),
            )))
        }
        Key::Character(ref character) if shift && character.eq_ignore_ascii_case("m") => {
            event.prevent_default();
            event.stop_propagation();
            state.toggle_menu(CalendarMenu::Month);
            return;
        }
        Key::Character(ref character) if shift && character.eq_ignore_ascii_case("y") => {
            event.prevent_default();
            event.stop_propagation();
            state.toggle_menu(CalendarMenu::Year);
            return;
        }
        Key::Escape => {
            event.prevent_default();
            event.stop_propagation();
            if state.menu_value().is_some() {
                state.close_menu();
            } else {
                state.cancel(value, mode, id);
            }
            return;
        }
        _ => None,
    };
    if let Some(target) = target {
        event.prevent_default();
        event.stop_propagation();
        state.move_focus(target, bounds);
    }
}
/// Text announced for screen readers after range changes.
fn selection_announcement(draft: &DateSelectionDraft, mode: DateSelectionMode) -> String {
    let selection = draft.selection();
    if mode == DateSelectionMode::Single {
        return selection
            .from
            .as_ref()
            .map(|date| format!("Date selected: {}.", full_date_label(date)))
            .unwrap_or_else(|| "No date selected.".to_string());
    }
    if let Some(start) = draft.pending_start() {
        return format!(
            "Start date selected: {}. Choose an end date.",
            full_date_label(start),
        );
    }
    match (&selection.from, &selection.to) {
        (Some(from), Some(to)) => {
            format!(
                "Date range selected: {} through {}.",
                full_date_label(from),
                full_date_label(to),
            )
        }
        (Some(from), None) => {
            format!("Open-ended range starting {}.", full_date_label(from))
        }
        (None, Some(to)) => format!("Open-ended range ending {}.", full_date_label(to)),
        (None, None) => "No dates selected.".to_string(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_error_waits_for_a_complete_attempt() {
        assert!(!should_show_typed_error("2026-0"));
        assert!(!should_show_typed_error("2026-01"));
        assert!(should_show_typed_error("not a date"));
        assert!(should_show_typed_error("2026-01-01 –"));
    }
    #[test]
    fn initial_view_uses_today_when_no_value_or_bound_is_available() {
        let today = IsoDate::parse("2026-11-06").unwrap();
        let (view, focus) =
            initial_view_focus(&DateSelection::default(), &DateBounds::default(), &today);
        assert_eq!(view, (2026, 11));
        assert_eq!(focus, today);
    }
    #[test]
    fn initial_view_prefers_the_selection_month() {
        let today = IsoDate::parse("2026-11-06").unwrap();
        let selection = DateSelection::new(
            Some(IsoDate::parse("2024-02-10").unwrap()),
            Some(IsoDate::parse("2024-03-01").unwrap()),
        );
        let (view, focus) = initial_view_focus(&selection, &DateBounds::default(), &today);
        assert_eq!(view, (2024, 2));
        assert_eq!(focus, IsoDate::parse("2024-02-10").unwrap());
    }
    #[test]
    fn a_complete_range_applies_immediately() {
        assert!(!selection_is_complete(
            &DateSelection::new(Some(IsoDate::parse("2026-01-01").unwrap()), None),
            DateSelectionMode::Range,
        ),);
        assert!(selection_is_complete(
            &DateSelection::new(
                Some(IsoDate::parse("2026-01-01").unwrap()),
                Some(IsoDate::parse("2026-01-31").unwrap()),
            ),
            DateSelectionMode::Range,
        ),);
        assert!(selection_is_complete(
            &DateSelection::single(IsoDate::parse("2026-01-01").unwrap()),
            DateSelectionMode::Single,
        ),);
    }
    #[test]
    fn controlled_default_value_wins_when_the_field_is_idle() {
        let selection = DateSelection::new(
            Some(IsoDate::parse("2026-01-01").unwrap()),
            Some(IsoDate::parse("2026-01-31").unwrap()),
        );
        assert_eq!(
            displayed_selection_text(&selection, DateSelectionMode::Range, "", false),
            "2026-01-01 – 2026-01-31",
        );
        assert_eq!(
            displayed_selection_text(&selection, DateSelectionMode::Range, "2026-02-01", true,),
            "2026-02-01",
        );
    }
}
