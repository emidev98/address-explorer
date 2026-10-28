//! Compact reusable date field with a calendar popup.
//!
//! The input is read-only by design: the date can only change by picking a
//! day in the popup calendar, never by typing. Clicking the field — or
//! pressing Enter/Space while it is focused — opens the popup; Escape, a
//! click outside, or picking a day closes it. The whole control stays 40 px
//! tall and is self-contained, so any screen can drop it in.

use dioxus::prelude::*;

use crate::domain::civil_time::{
    civil_from_days, days_from_civil, epoch_seconds, format_date, parse_date,
};

/// Weekday column headers of the calendar grid, Monday first.
const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

/// Month names of the popup header, January first.
const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A compact date field (`YYYY-MM-DD` value, empty when unset). `on_change`
/// reports the day picked in the calendar popup, in the same format.
#[component]
pub fn DatePicker(
    /// DOM id of the input; labels point at it.
    id: String,
    /// Current date (`YYYY-MM-DD`) or empty.
    value: String,
    aria_label: String,
    on_change: EventHandler<String>,
    #[props(default)] disabled: bool,
    /// Open the calendar popup above the field instead of below it. Modals
    /// with a scrollable body use this so the popup stays inside the panel
    /// and does not add scrollable height.
    #[props(default)]
    popup_above: bool,
) -> Element {
    let mut open = use_signal(|| false);
    let view = use_signal(|| (0_i64, 0_i64));

    let click_value = value.clone();
    let key_value = value.clone();

    rsx! {
        div {
            class: "md3-date-picker",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    open.set(false);
                }
            },
            div { class: "md3-text-field",
                input {
                    id: "{id}",
                    class: "md3-input",
                    r#type: "text",
                    value: "{value}",
                    placeholder: "YYYY-MM-DD",
                    readonly: true,
                    aria_label: aria_label.clone(),
                    disabled,
                    onclick: move |_| toggle_popup(open, view, &click_value, disabled),
                    onkeydown: move |event| {
                        let activation = event.key() == Key::Enter
                            || event.key() == Key::Character(" ".to_string());
                        if activation {
                            toggle_popup(open, view, &key_value, disabled);
                        }
                    },
                }
                {icon(ICON_CALENDAR)}
            }
            if open() {
                // Transparent layer catching outside clicks; the popup sits
                // above it, so clicks inside the popup never reach it.
                div {
                    class: "md3-date-picker-scrim",
                    onclick: move |_| open.set(false),
                }
                {calendar_popup(view, &value, today_day(), on_change, open, popup_above)}
            }
        }
    }
}

/// Open the popup showing the month of the current value (or of today when
/// the value is unset), or close it when it is already open.
fn toggle_popup(mut open: Signal<bool>, mut view: Signal<(i64, i64)>, value: &str, disabled: bool) {
    if disabled {
        return;
    }
    if open() {
        open.set(false);
    } else {
        view.set(initial_view_month(value, today_day()));
        open.set(true);
    }
}

/// The month grid for `view`: navigation, weekday headers, and the day
/// cells. Picking a day reports it through `on_change` and closes the popup.
fn calendar_popup(
    mut view: Signal<(i64, i64)>,
    value: &str,
    today: (i64, i64, i64),
    on_change: EventHandler<String>,
    mut open: Signal<bool>,
    popup_above: bool,
) -> Element {
    let (view_year, view_month) = view();
    let length = month_length(view_year, view_month);
    let offset = monday_first_weekday(view_year, view_month);
    let today_days = days_from_civil(today.0, today.1, today.2);
    let selected_days = parse_date(value).map(|epoch| epoch.div_euclid(86_400));
    let month_name = MONTH_NAMES[(view_month - 1) as usize];
    let popup_class = if popup_above {
        "md3-date-picker-popup md3-date-picker-popup-above"
    } else {
        "md3-date-picker-popup"
    };

    rsx! {
        div {
            class: "{popup_class}",
            role: "dialog",
            aria_label: "Choose a date",
            div { class: "md3-date-picker-nav",
                button {
                    r#type: "button",
                    class: "md3-icon-button",
                    aria_label: "Previous month",
                    onclick: move |_| {
                        let (year, month) = view();
                        view.set(shift_month(year, month, -1));
                    },
                    {icon(ICON_CHEVRON_LEFT)}
                }
                span { class: "md3-date-picker-month text-title-small", "{month_name} {view_year}" }
                button {
                    r#type: "button",
                    class: "md3-icon-button",
                    aria_label: "Next month",
                    onclick: move |_| {
                        let (year, month) = view();
                        view.set(shift_month(year, month, 1));
                    },
                    {icon(ICON_CHEVRON_RIGHT)}
                }
            }
            div { class: "md3-date-picker-grid",
                for weekday in WEEKDAYS {
                    span { class: "md3-date-picker-weekday", "{weekday}" }
                }
                for _ in 0..offset {
                    span {}
                }
                for day in 1..=length {
                    {
                        let day_days = days_from_civil(view_year, view_month, day);
                        let mut classes = String::from("md3-date-picker-day");
                        if Some(day_days) == selected_days {
                            classes.push_str(" md3-date-picker-selected");
                        }
                        if day_days == today_days {
                            classes.push_str(" md3-date-picker-today");
                        }
                        let picked = format_date(epoch_seconds(view_year, view_month, day, 0, 0, 0));
                        rsx! {
                            button {
                                r#type: "button",
                                class: "{classes}",
                                aria_label: "{month_name} {day}, {view_year}",
                                onclick: move |_| {
                                    on_change.call(picked.clone());
                                    open.set(false);
                                },
                                "{day}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Days in a civil month.
fn month_length(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = shift_month(year, month, 1);
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

/// Leading blank cells before day 1 in a Monday-first grid. Day 0 of the
/// epoch (1970-01-01) was a Thursday, so the Monday-first index of `days` is
/// `(days + 3) mod 7`.
fn monday_first_weekday(year: i64, month: i64) -> i64 {
    (days_from_civil(year, month, 1) + 3).rem_euclid(7)
}

/// The month `delta` months away from `(year, month)` (negative moves back).
fn shift_month(year: i64, month: i64, delta: i64) -> (i64, i64) {
    let total = (year * 12 + (month - 1)) + delta;
    (total.div_euclid(12), total.rem_euclid(12) + 1)
}

/// The month the popup opens on: the value's month, or today's when the
/// field is empty or invalid.
fn initial_view_month(value: &str, today: (i64, i64, i64)) -> (i64, i64) {
    parse_date(value)
        .map(|epoch| civil_from_days(epoch.div_euclid(86_400)))
        .map(|(year, month, _)| (year, month))
        .unwrap_or((today.0, today.1))
}

/// Today's civil date in UTC: the browser clock on wasm32 (`SystemTime`
/// panics there), the system clock during server-side rendering.
fn today_day() -> (i64, i64, i64) {
    #[cfg(target_arch = "wasm32")]
    let epoch = (js_sys::Date::now() / 1_000.0) as i64;
    #[cfg(not(target_arch = "wasm32"))]
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(epoch.div_euclid(86_400))
}

// Material icon paths (24px viewBox): calendar_month, chevron_left,
// chevron_right.
const ICON_CALENDAR: &str = "M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 16H5V10h14v10zm0-12H5V6h14v2z";
const ICON_CHEVRON_LEFT: &str = "M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z";
const ICON_CHEVRON_RIGHT: &str = "M8.59 16.59 10 18l6-6-6-6-1.41 1.41L13.17 12z";

/// Render one Material icon path as an inline SVG.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_length_covers_leap_february_and_year_end() {
        assert_eq!(month_length(2024, 2), 29);
        assert_eq!(month_length(2023, 2), 28);
        assert_eq!(month_length(2026, 12), 31);
        assert_eq!(month_length(2026, 4), 30);
    }

    #[test]
    fn monday_first_weekday_matches_known_thursday_month_starts() {
        // 1970-01-01 was a Thursday: three blank cells before day 1.
        assert_eq!(monday_first_weekday(1970, 1), 3);
        // 2026-10-27 is a Tuesday, so 2026-10-01 is a Thursday too.
        assert_eq!(monday_first_weekday(2026, 10), 3);
        // 2026-11-01 is a Sunday: six blank cells in a Monday-first grid.
        assert_eq!(monday_first_weekday(2026, 11), 6);
    }

    #[test]
    fn shift_month_crosses_year_boundaries_in_both_directions() {
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2025, 12, 1), (2026, 1));
        assert_eq!(shift_month(2026, 6, 12), (2027, 6));
    }

    #[test]
    fn initial_view_month_prefers_the_value_and_falls_back_to_today() {
        assert_eq!(initial_view_month("2024-02-10", (2026, 10, 27)), (2024, 2));
        assert_eq!(initial_view_month("", (2026, 10, 27)), (2026, 10));
        assert_eq!(initial_view_month("not-a-date", (2026, 10, 27)), (2026, 10));
    }
}
