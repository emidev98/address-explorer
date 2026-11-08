//! Quick calendar presets shown above the shared date picker's calendar.
//!
//! Presets are computed from a caller-supplied `today`, so the UI stays
//! deterministic in tests. Weeks follow the Monday-first calendar used by the
//! picker. "This" presets run from the period start through today; "Last"
//! presets cover the whole previous period. Range presets clip to the
//! picker's inclusive bounds and are unavailable when no day overlaps them.
use super::calendar::{shift_month, weekday_index};
use super::civil_time::days_from_civil;
use super::date_selection::{DateBounds, DateSelection, IsoDate};
/// One quick range offered above the calendar in Range mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangePreset {
    LastWeek,
    LastMonth,
    LastYear,
    ThisWeek,
    ThisMonth,
    ThisYear,
}
impl RangePreset {
    /// Every range preset in display order.
    pub const ALL: [Self; 6] = [
        Self::LastWeek,
        Self::LastMonth,
        Self::LastYear,
        Self::ThisWeek,
        Self::ThisMonth,
        Self::ThisYear,
    ];
    /// The chip label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LastWeek => "Last week",
            Self::LastMonth => "Last month",
            Self::LastYear => "Last year",
            Self::ThisWeek => "This week",
            Self::ThisMonth => "This month",
            Self::ThisYear => "This year",
        }
    }
    /// The preset as a complete range clipped to `bounds`, or `None` when the
    /// period does not overlap the selectable days.
    pub fn selection(self, today: IsoDate, bounds: &DateBounds) -> Option<DateSelection> {
        if !bounds.is_valid() {
            return None;
        }
        let (start, end) = self.period(today);
        clipped_selection(start, end, bounds)
    }
    fn period(self, today: IsoDate) -> (IsoDate, IsoDate) {
        let (year, month, _) = today.year_month_day();
        let week_start = start_of_week(today);
        match self {
            Self::LastWeek => (
                IsoDate::from_epoch_day(week_start.epoch_day() - 7),
                IsoDate::from_epoch_day(week_start.epoch_day() - 1),
            ),
            Self::LastMonth => {
                let (previous_year, previous_month) = shift_month(year, month, -1);
                (
                    IsoDate::from_epoch_day(days_from_civil(previous_year, previous_month, 1)),
                    IsoDate::from_epoch_day(days_from_civil(year, month, 1) - 1),
                )
            }
            Self::LastYear => (
                IsoDate::from_epoch_day(days_from_civil(year - 1, 1, 1)),
                IsoDate::from_epoch_day(days_from_civil(year, 1, 1) - 1),
            ),
            Self::ThisWeek => (week_start, today),
            Self::ThisMonth => (
                IsoDate::from_epoch_day(days_from_civil(year, month, 1)),
                today,
            ),
            Self::ThisYear => (IsoDate::from_epoch_day(days_from_civil(year, 1, 1)), today),
        }
    }
}
/// One quick date offered above the calendar in Single mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleDatePreset {
    StartOfYear,
    StartOfMonth,
    StartOfWeek,
    Today,
}
impl SingleDatePreset {
    /// Every single-date preset in display order.
    pub const ALL: [Self; 4] = [
        Self::StartOfYear,
        Self::StartOfMonth,
        Self::StartOfWeek,
        Self::Today,
    ];
    /// The chip label.
    pub const fn label(self) -> &'static str {
        match self {
            Self::StartOfYear => "Start of year",
            Self::StartOfMonth => "Start of month",
            Self::StartOfWeek => "Start of week",
            Self::Today => "Today",
        }
    }
    /// The date this preset picks.
    pub fn date(self, today: IsoDate) -> IsoDate {
        let (year, month, _) = today.year_month_day();
        match self {
            Self::StartOfYear => IsoDate::from_epoch_day(days_from_civil(year, 1, 1)),
            Self::StartOfMonth => IsoDate::from_epoch_day(days_from_civil(year, month, 1)),
            Self::StartOfWeek => start_of_week(today),
            Self::Today => today,
        }
    }
    /// The preset date when it is inside `bounds`.
    pub fn selection(self, today: IsoDate, bounds: &DateBounds) -> Option<IsoDate> {
        if !bounds.is_valid() {
            return None;
        }
        let date = self.date(today);
        bounds.contains(&date).then_some(date)
    }
}
/// The Monday starting the week that contains `date`.
fn start_of_week(date: IsoDate) -> IsoDate {
    IsoDate::from_epoch_day(date.epoch_day() - weekday_index(&date) as i64)
}
/// Clip an inclusive period to the picker bounds.
fn clipped_selection(start: IsoDate, end: IsoDate, bounds: &DateBounds) -> Option<DateSelection> {
    let start = bounds.min.map_or(start, |min| start.max(min));
    let end = bounds.max.map_or(end, |max| end.min(max));
    (start <= end).then(|| DateSelection::new(Some(start), Some(end)))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn date(raw: &str) -> IsoDate {
        IsoDate::parse(raw).unwrap()
    }
    fn range(from: &str, to: &str) -> DateSelection {
        DateSelection::new(Some(date(from)), Some(date(to)))
    }
    #[test]
    fn this_week_runs_from_monday_through_today() {
        let today = date("2026-11-06");
        assert_eq!(
            RangePreset::ThisWeek.selection(today, &DateBounds::default()),
            Some(range("2026-11-02", "2026-11-06")),
        );
    }
    #[test]
    fn last_week_is_the_previous_monday_through_sunday() {
        let today = date("2026-11-06");
        assert_eq!(
            RangePreset::LastWeek.selection(today, &DateBounds::default()),
            Some(range("2026-10-26", "2026-11-01")),
        );
    }
    #[test]
    fn month_and_year_presets_cross_the_year_boundary() {
        let today = date("2026-01-15");
        let bounds = DateBounds::default();
        assert_eq!(
            RangePreset::LastMonth.selection(today, &bounds),
            Some(range("2025-12-01", "2025-12-31")),
        );
        assert_eq!(
            RangePreset::LastYear.selection(today, &bounds),
            Some(range("2025-01-01", "2025-12-31")),
        );
        assert_eq!(
            RangePreset::ThisMonth.selection(today, &bounds),
            Some(range("2026-01-01", "2026-01-15")),
        );
        assert_eq!(
            RangePreset::ThisYear.selection(today, &bounds),
            Some(range("2026-01-01", "2026-01-15")),
        );
    }
    #[test]
    fn range_presets_clip_to_bounds_and_skip_periods_outside_them() {
        let today = date("2026-11-06");
        let bounds = DateBounds::new(Some(date("2026-11-04")), Some(date("2026-11-05")));
        assert_eq!(
            RangePreset::ThisWeek.selection(today, &bounds),
            Some(range("2026-11-04", "2026-11-05")),
        );
        assert_eq!(
            RangePreset::ThisMonth.selection(today, &bounds),
            Some(range("2026-11-04", "2026-11-05")),
        );
        assert_eq!(
            RangePreset::ThisYear.selection(today, &bounds),
            Some(range("2026-11-04", "2026-11-05")),
        );
        assert_eq!(RangePreset::LastWeek.selection(today, &bounds), None);
        assert_eq!(RangePreset::LastMonth.selection(today, &bounds), None);
        assert_eq!(RangePreset::LastYear.selection(today, &bounds), None);
    }
    #[test]
    fn single_presets_pick_period_starts() {
        let today = date("2026-11-06");
        let bounds = DateBounds::default();
        assert_eq!(
            SingleDatePreset::StartOfYear.selection(today, &bounds),
            Some(date("2026-01-01")),
        );
        assert_eq!(
            SingleDatePreset::StartOfMonth.selection(today, &bounds),
            Some(date("2026-11-01")),
        );
        assert_eq!(
            SingleDatePreset::StartOfWeek.selection(today, &bounds),
            Some(date("2026-11-02")),
        );
        assert_eq!(
            SingleDatePreset::Today.selection(today, &bounds),
            Some(today)
        );
    }
    #[test]
    fn single_presets_require_the_target_inside_bounds() {
        let today = date("2026-01-15");
        let bounds = DateBounds::new(Some(date("2026-01-10")), Some(date("2026-01-20")));
        assert_eq!(
            SingleDatePreset::StartOfYear.selection(today, &bounds),
            None
        );
        assert_eq!(
            SingleDatePreset::StartOfMonth.selection(today, &bounds),
            None
        );
        assert_eq!(
            SingleDatePreset::StartOfWeek.selection(today, &bounds),
            Some(date("2026-01-12")),
        );
        assert_eq!(
            SingleDatePreset::Today.selection(today, &bounds),
            Some(today)
        );
    }
    #[test]
    fn invalid_bounds_disable_every_preset() {
        let today = date("2026-11-06");
        let bounds = DateBounds::new(Some(date("2026-12-01")), Some(date("2026-01-01")));
        assert!(RangePreset::ALL
            .into_iter()
            .all(|preset| preset.selection(today, &bounds).is_none()),);
        assert!(SingleDatePreset::ALL
            .into_iter()
            .all(|preset| preset.selection(today, &bounds).is_none()),);
    }
}
