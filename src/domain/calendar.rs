//! Pure calendar math used by the shared date picker.
//!
//! The grid is Monday-first, matching the ISO dates the application stores.
//! Keeping this logic outside the component makes month lengths, leap days,
//! adjacent-month cells, and bounds clipping directly testable.
use super::civil_time::{civil_from_days, days_from_civil};
use super::date_selection::{DateBounds, IsoDate};
/// Weekday column labels, Monday first.
pub const WEEKDAYS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
/// Full weekday names, Monday first.
pub const WEEKDAY_NAMES: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];
/// Month names, January first.
pub const MONTH_NAMES: [&str; 12] = [
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
/// One day cell of a month grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDay {
    pub date: IsoDate,
    /// Whether the day belongs to the grid's month rather than an adjacent
    /// month shown to fill the six-week grid.
    pub in_month: bool,
}
/// A six-week calendar grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthGrid {
    pub year: i64,
    pub month: i64,
    pub days: Vec<CalendarDay>,
}
/// Days in a civil month.
pub fn month_length(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = shift_month(year, month, 1);
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}
/// Leading blank cells before day 1 in a Monday-first grid. Day 0 of the
/// epoch (1970-01-01) was a Thursday, so the Monday-first index of `days` is
/// `(days + 3) mod 7`.
pub fn monday_first_weekday(year: i64, month: i64) -> i64 {
    (days_from_civil(year, month, 1) + 3).rem_euclid(7)
}
/// The month `delta` months away from `(year, month)` (negative moves back).
pub fn shift_month(year: i64, month: i64, delta: i64) -> (i64, i64) {
    month_from_index(month_index(year, month) + delta)
}
/// The same month `delta` years away.
pub fn shift_year(year: i64, month: i64, delta: i64) -> (i64, i64) {
    (year + delta, month)
}
/// A stable ordinal for a month, used for comparisons and navigation.
pub fn month_index(year: i64, month: i64) -> i64 {
    year * 12 + (month - 1)
}
/// Inverse of [`month_index`].
pub fn month_from_index(index: i64) -> (i64, i64) {
    (index.div_euclid(12), index.rem_euclid(12) + 1)
}
/// Clamp a day-of-month to the length of `year`/`month`.
pub fn clamp_day(year: i64, month: i64, day: i64) -> i64 {
    day.clamp(1, month_length(year, month))
}
/// `"January 2026"`.
pub fn month_label(year: i64, month: i64) -> String {
    format!("{} {year}", MONTH_NAMES[(month - 1) as usize])
}
/// The weekday index (Monday = 0) of a date.
pub fn weekday_index(date: &IsoDate) -> usize {
    (date.epoch_day() + 3).rem_euclid(7) as usize
}
/// A full, screen-reader-friendly label such as
/// `"Monday, January 5, 2026"`.
pub fn full_date_label(date: &IsoDate) -> String {
    let (year, month, day) = date.year_month_day();
    format!(
        "{}, {} {day}, {year}",
        WEEKDAY_NAMES[weekday_index(date)],
        MONTH_NAMES[(month - 1) as usize],
    )
}
/// A six-week Monday-first grid for a month.
pub fn month_grid(year: i64, month: i64) -> MonthGrid {
    let first_day = days_from_civil(year, month, 1);
    let offset = monday_first_weekday(year, month);
    let days = (0..42)
        .map(|index| {
            let day_offset = first_day - offset + index;
            let date = IsoDate::from_epoch_day(day_offset);
            let (date_year, date_month, _) = civil_from_days(day_offset);
            CalendarDay {
                date,
                in_month: date_year == year && date_month == month,
            }
        })
        .collect();
    MonthGrid { year, month, days }
}
/// The inclusive first and last selectable day of a month, clipped to
/// `bounds`. `None` means the month cannot be chosen at all.
pub fn month_selectable_bounds(
    year: i64,
    month: i64,
    bounds: &DateBounds,
) -> Option<(IsoDate, IsoDate)> {
    if !bounds.is_valid() {
        return None;
    }
    let first = days_from_civil(year, month, 1);
    let last = first + month_length(year, month) - 1;
    let start = bounds
        .min
        .as_ref()
        .map(IsoDate::epoch_day)
        .map_or(first, |min| min.max(first));
    let end = bounds
        .max
        .as_ref()
        .map(IsoDate::epoch_day)
        .map_or(last, |max| max.min(last));
    (start <= end).then(|| (IsoDate::from_epoch_day(start), IsoDate::from_epoch_day(end)))
}
/// Whether a month contains at least one selectable day.
pub fn month_has_selectable(year: i64, month: i64, bounds: &DateBounds) -> bool {
    month_selectable_bounds(year, month, bounds).is_some()
}
/// The first selectable day of a month.
pub fn first_selectable_in_month(year: i64, month: i64, bounds: &DateBounds) -> Option<IsoDate> {
    month_selectable_bounds(year, month, bounds).map(|(first, _)| first)
}
/// Years worth offering in the year menu for the current view and bounds.
pub fn year_options(view_year: i64, bounds: &DateBounds) -> Vec<i64> {
    match (&bounds.min, &bounds.max) {
        (Some(min), Some(max)) => {
            let (min_year, _, _) = min.year_month_day();
            let (max_year, _, _) = max.year_month_day();
            (min_year..=max_year).collect()
        }
        (Some(min), None) => {
            let (min_year, _, _) = min.year_month_day();
            let end = (view_year + 10).max(min_year + 50);
            (min_year..=end).collect()
        }
        (None, Some(max)) => {
            let (max_year, _, _) = max.year_month_day();
            let start = (view_year - 10).min(max_year - 50);
            (start..=max_year).collect()
        }
        (None, None) => ((view_year - 100)..=(view_year + 10)).collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn date(raw: &str) -> IsoDate {
        IsoDate::parse(raw).unwrap()
    }
    #[test]
    fn month_length_covers_leap_february_and_year_end() {
        assert_eq!(month_length(2024, 2), 29);
        assert_eq!(month_length(2023, 2), 28);
        assert_eq!(month_length(2026, 12), 31);
        assert_eq!(month_length(2026, 4), 30);
    }
    #[test]
    fn monday_first_weekday_matches_known_month_starts() {
        assert_eq!(monday_first_weekday(1970, 1), 3);
        assert_eq!(monday_first_weekday(2026, 10), 3);
        assert_eq!(monday_first_weekday(2026, 11), 6);
    }
    #[test]
    fn shift_month_and_year_cross_boundaries() {
        assert_eq!(shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(shift_month(2025, 12, 1), (2026, 1));
        assert_eq!(shift_month(2026, 6, 12), (2027, 6));
        assert_eq!(shift_year(2024, 2, 1), (2025, 2));
        assert_eq!(clamp_day(2024, 2, 31), 29);
        assert_eq!(clamp_day(2025, 2, 31), 28);
    }
    #[test]
    fn month_grid_has_six_weeks_and_marks_adjacent_months() {
        let grid = month_grid(2026, 1);
        assert_eq!(grid.days.len(), 42);
        assert_eq!(grid.days[0].date, date("2025-12-29"));
        assert!(!grid.days[0].in_month);
        assert_eq!(grid.days[3].date, date("2026-01-01"));
        assert!(grid.days[3].in_month);
    }
    #[test]
    fn selectable_month_bounds_clip_to_the_window() {
        let bounds = DateBounds::new(Some(date("2026-01-10")), Some(date("2026-01-20")));
        assert_eq!(
            month_selectable_bounds(2026, 1, &bounds),
            Some((date("2026-01-10"), date("2026-01-20"))),
        );
        assert_eq!(month_selectable_bounds(2025, 12, &bounds), None);
        assert!(month_has_selectable(2026, 1, &bounds));
        assert_eq!(
            first_selectable_in_month(2026, 1, &bounds),
            Some(date("2026-01-10")),
        );
    }
    #[test]
    fn full_date_labels_include_weekday_and_year() {
        assert_eq!(
            full_date_label(&date("2026-01-05")),
            "Monday, January 5, 2026"
        );
    }
}
