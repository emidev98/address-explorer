//! Validated date values and the date-range selection state machine shared by
//! the single [`DatePicker`](crate::components::DatePicker) component.
//!
//! All values are UTC civil dates in the ISO `YYYY-MM-DD` layout, matching the
//! rest of the application. The module is deliberately free of Dioxus and RSX
//! so range ordering, bounds, typing, and click transitions can be tested as
//! ordinary Rust.

use std::fmt;

use super::civil_time::{civil_from_days, format_date, parse_date, SECONDS_PER_DAY};

/// A validated `YYYY-MM-DD` UTC civil date, stored as days since 1970-01-01.
///
/// The value is a day ordinal rather than a raw string, so impossible dates
/// cannot be constructed through the public API and comparisons are exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IsoDate(i64);

impl IsoDate {
    /// Parse and validate a `YYYY-MM-DD` date.
    pub fn parse(raw: &str) -> Result<Self, DateSelectionError> {
        let raw = raw.trim();
        if raw.len() != 10 {
            return Err(DateSelectionError::InvalidFormat);
        }

        let epoch = parse_date(raw).ok_or(DateSelectionError::InvalidFormat)?;
        let day = epoch.div_euclid(SECONDS_PER_DAY);
        let canonical = format_date(day * SECONDS_PER_DAY);
        if canonical != raw {
            return Err(DateSelectionError::InvalidDate(raw.to_string()));
        }

        Ok(Self(day))
    }

    /// The UTC civil date containing `epoch_second`.
    pub fn from_epoch(epoch_second: i64) -> Self {
        Self(epoch_second.div_euclid(SECONDS_PER_DAY))
    }

    /// The UTC civil date `days` days after 1970-01-01.
    pub fn from_epoch_day(days: i64) -> Self {
        Self(days)
    }

    /// UTC epoch seconds at the start of this day.
    pub fn epoch_start(&self) -> i64 {
        self.0 * SECONDS_PER_DAY
    }

    /// Inclusive UTC epoch seconds at the end of this day.
    pub fn epoch_end(&self) -> i64 {
        self.epoch_start() + SECONDS_PER_DAY - 1
    }

    /// Days since 1970-01-01.
    pub fn epoch_day(&self) -> i64 {
        self.0
    }

    /// The `(year, month, day)` civil parts.
    pub fn year_month_day(&self) -> (i64, i64, i64) {
        civil_from_days(self.0)
    }
}

impl fmt::Display for IsoDate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&format_date(self.epoch_start()))
    }
}

impl std::str::FromStr for IsoDate {
    type Err = DateSelectionError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

/// Whether the field chooses a range or one date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DateSelectionMode {
    /// A `from` date, a `to` date, both, or neither. Either side may be open.
    #[default]
    Range,
    /// One date, stored in `from`; `to` is never used.
    Single,
}

/// Inclusive bounds on the days a picker may choose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateBounds {
    pub min: Option<IsoDate>,
    pub max: Option<IsoDate>,
}

impl DateBounds {
    pub fn new(min: Option<IsoDate>, max: Option<IsoDate>) -> Self {
        Self { min, max }
    }

    /// Whether the configured bounds are coherent (`min <= max`).
    pub fn is_valid(&self) -> bool {
        match (&self.min, &self.max) {
            (Some(min), Some(max)) => min <= max,
            _ => true,
        }
    }

    /// Whether `date` is inside the inclusive bounds.
    pub fn contains(&self, date: &IsoDate) -> bool {
        self.is_valid()
            && self.min.as_ref().is_none_or(|min| min <= date)
            && self.max.as_ref().is_none_or(|max| max >= date)
    }
}

/// One value produced by the picker.
///
/// Both fields use `Option` because open-ended ranges are supported. A
/// `Single`-mode value only stores `from`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateSelection {
    pub from: Option<IsoDate>,
    pub to: Option<IsoDate>,
}

impl DateSelection {
    pub fn new(from: Option<IsoDate>, to: Option<IsoDate>) -> Self {
        Self { from, to }
    }

    pub fn single(date: IsoDate) -> Self {
        Self {
            from: Some(date),
            to: None,
        }
    }

    pub fn from_epoch_bounds(from: Option<i64>, to: Option<i64>) -> Self {
        Self {
            from: from.map(IsoDate::from_epoch),
            to: to.map(IsoDate::from_epoch),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.from.is_none() && self.to.is_none()
    }

    /// The selection a picker should fall back to when its value is cleared.
    ///
    /// With both bounds, this is the full `min..max` range. With one bound, it
    /// is the corresponding open-ended range. `Single` mode uses the min (or
    /// max when no min exists).
    pub fn default_in_bounds(mode: DateSelectionMode, bounds: &DateBounds) -> Self {
        if !bounds.is_valid() {
            return Self::default();
        }

        match mode {
            DateSelectionMode::Single => Self::new(bounds.min.or(bounds.max), None),
            DateSelectionMode::Range => match (bounds.min, bounds.max) {
                (Some(min), Some(max)) => Self::new(Some(min), Some(max)),
                (Some(min), None) => Self::new(Some(min), None),
                (None, Some(max)) => Self::new(None, Some(max)),
                (None, None) => Self::default(),
            },
        }
    }

    /// Whether `date` is one of the endpoints or inside the selected range.
    pub fn contains(&self, date: &IsoDate) -> bool {
        match (&self.from, &self.to) {
            (Some(from), Some(to)) => from <= date && date <= to,
            (Some(from), None) => from == date,
            (None, Some(to)) => to == date,
            (None, None) => false,
        }
    }

    /// Inclusive epoch bounds, using the whole `to` day.
    pub fn epoch_bounds(&self) -> (Option<i64>, Option<i64>) {
        (
            self.from.as_ref().map(IsoDate::epoch_start),
            self.to.as_ref().map(IsoDate::epoch_end),
        )
    }

    /// The text shown in the single input.
    pub fn display(&self, mode: DateSelectionMode) -> String {
        if mode == DateSelectionMode::Single {
            return self
                .from
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
        }

        match (&self.from, &self.to) {
            (Some(from), Some(to)) => format!("{from} – {to}"),
            (Some(from), None) => format!("{from} – …"),
            (None, Some(to)) => format!("… – {to}"),
            (None, None) => String::new(),
        }
    }

    /// Placeholder matching [`DateSelection::display`].
    pub fn placeholder(mode: DateSelectionMode) -> &'static str {
        match mode {
            DateSelectionMode::Range => "YYYY-MM-DD – YYYY-MM-DD",
            DateSelectionMode::Single => "YYYY-MM-DD",
        }
    }

    /// Parse the single field's text.
    ///
    /// Range mode accepts both dates, one date (a `from`-only range), a
    /// leading dash followed by a date (a `to`-only range), or an empty value.
    /// Single mode only accepts one date.
    pub fn parse_input(raw: &str, mode: DateSelectionMode) -> Result<Self, DateSelectionError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(Self::default());
        }

        if mode == DateSelectionMode::Single {
            if split_range(raw).is_some() {
                return Err(DateSelectionError::SingleDateOnly);
            }
            return Ok(Self::single(IsoDate::parse(raw)?));
        }

        let Some((left, right)) = split_range(raw) else {
            // A lone date is the start of an open-ended range.
            return Ok(Self::new(Some(IsoDate::parse(raw)?), None));
        };

        let from = if open_marker(left) {
            None
        } else {
            Some(IsoDate::parse(left)?)
        };
        let to = if open_marker(right) {
            None
        } else {
            Some(IsoDate::parse(right)?)
        };

        Ok(Self::new(from, to))
    }

    /// Validate the value against a mode and inclusive bounds.
    pub fn validate(
        &self,
        mode: DateSelectionMode,
        bounds: &DateBounds,
    ) -> Result<(), DateSelectionError> {
        if !bounds.is_valid() {
            return Err(DateSelectionError::InvalidBounds);
        }
        if mode == DateSelectionMode::Single && self.to.is_some() {
            return Err(DateSelectionError::SingleDateOnly);
        }

        for date in [&self.from, &self.to].into_iter().flatten() {
            if !bounds.contains(date) {
                return Err(DateSelectionError::OutOfBounds {
                    min: bounds.min,
                    max: bounds.max,
                });
            }
        }

        if let (Some(from), Some(to)) = (&self.from, &self.to) {
            if from > to {
                return Err(DateSelectionError::Order);
            }
        }

        Ok(())
    }
}

/// Whether one side of a typed range represents an open end.
fn open_marker(raw: &str) -> bool {
    let raw = raw.trim();
    raw.is_empty() || raw == "…" || raw == "..."
}

/// Split a typed range at an en dash, em dash, spaced hyphen, or " to ".
fn split_range(raw: &str) -> Option<(&str, &str)> {
    for separator in ["–", "—", " - ", " to "] {
        if let Some(parts) = raw.split_once(separator) {
            return Some(parts);
        }
    }
    None
}

/// User-facing validation failure of a date selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateSelectionError {
    InvalidFormat,
    InvalidDate(String),
    Order,
    OutOfBounds {
        min: Option<IsoDate>,
        max: Option<IsoDate>,
    },
    SingleDateOnly,
    InvalidBounds,
}

impl fmt::Display for DateSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat => formatter.write_str("Use a date in the YYYY-MM-DD format."),
            Self::InvalidDate(raw) => {
                write!(formatter, "{raw} is not a real calendar date.")
            }
            Self::Order => formatter.write_str("The From date must be on or before the To date."),
            Self::OutOfBounds { min, max } => match (min, max) {
                (Some(min), Some(max)) => {
                    write!(formatter, "Choose a date between {min} and {max}.")
                }
                (Some(min), None) => {
                    write!(formatter, "Choose a date on or after {min}.")
                }
                (None, Some(max)) => {
                    write!(formatter, "Choose a date on or before {max}.")
                }
                (None, None) => formatter.write_str("Choose a valid date."),
            },
            Self::SingleDateOnly => formatter.write_str("Choose a single date, not a range."),
            Self::InvalidBounds => {
                formatter.write_str("The minimum date must be on or before the maximum date.")
            }
        }
    }
}

impl std::error::Error for DateSelectionError {}

/// Mutable selection state of an open picker.
///
/// A first click starts a range; a second click completes it. Clicking before
/// the pending start swaps the two dates, so the draft can never contain a
/// reversed range. Clearing either endpoint leaves the other side open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateSelectionDraft {
    selection: DateSelection,
    pending_start: Option<IsoDate>,
}

impl DateSelectionDraft {
    pub fn new(selection: DateSelection) -> Self {
        let pending_start = selection.from.filter(|_| selection.to.is_none());
        Self {
            selection,
            pending_start,
        }
    }

    pub fn selection(&self) -> &DateSelection {
        &self.selection
    }

    pub fn pending_start(&self) -> Option<&IsoDate> {
        self.pending_start.as_ref()
    }

    /// Handle a day click (or keyboard activation).
    pub fn select_day(&mut self, day: IsoDate, mode: DateSelectionMode) {
        if mode == DateSelectionMode::Single {
            self.selection = DateSelection::single(day);
            self.pending_start = None;
            return;
        }

        if let Some(start) = self.pending_start.take() {
            let (from, to) = if day < start {
                (day, start)
            } else {
                (start, day)
            };
            self.selection = DateSelection::new(Some(from), Some(to));
            return;
        }

        // Clicking one endpoint of a complete range re-opens the end so it can
        // be adjusted; clicking anywhere else starts a new range.
        if let (Some(from), Some(to)) = (&self.selection.from, &self.selection.to) {
            if day == *from || day == *to {
                let start = *from;
                self.selection = DateSelection::new(Some(start), None);
                self.pending_start = Some(start);
                return;
            }
        }

        // A to-only value completed by clicking on or before its end.
        if self.selection.from.is_none() {
            if let Some(to) = &self.selection.to {
                if day <= *to {
                    self.selection = DateSelection::new(Some(day), Some(*to));
                    self.pending_start = None;
                    return;
                }
            }
        }

        self.selection = DateSelection::new(Some(day), None);
        self.pending_start = Some(day);
    }

    /// Clear the start, leaving the end open.
    pub fn clear_from(&mut self) {
        self.selection.from = None;
        self.pending_start = None;
    }

    /// Clear the end, leaving the start open and ready for a new end.
    pub fn clear_to(&mut self) {
        self.selection.to = None;
        self.pending_start = self.selection.from;
    }

    /// The range to highlight while the user is choosing an end date.
    pub fn preview(&self, hover: Option<&IsoDate>) -> DateSelection {
        let Some(start) = &self.pending_start else {
            return self.selection;
        };
        let Some(end) = hover else {
            return self.selection;
        };

        if end < start {
            DateSelection::new(Some(*end), Some(*start))
        } else {
            DateSelection::new(Some(*start), Some(*end))
        }
    }
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
    fn iso_dates_reject_impossible_calendar_days() {
        assert!(IsoDate::parse("2024-02-29").is_ok());
        assert!(IsoDate::parse("2023-02-29").is_err());
        assert!(IsoDate::parse("2026-02-30").is_err());
        assert!(IsoDate::parse("2026-04-31").is_err());
        assert!(IsoDate::parse("2026-13-01").is_err());
        assert!(IsoDate::parse("2026-01-00").is_err());
        assert!(IsoDate::parse("2026-1-01").is_err());
        assert!(IsoDate::parse("not a date").is_err());
    }

    #[test]
    fn iso_dates_round_trip_epoch_values() {
        let date = IsoDate::from_epoch(0);
        assert_eq!(date.to_string(), "1970-01-01");
        assert_eq!(date.epoch_start(), 0);
        assert_eq!(date.epoch_end(), 86_399);
        assert_eq!(IsoDate::from_epoch_day(0), date);
    }

    #[test]
    fn range_display_distinguishes_open_sides() {
        assert_eq!(
            range("2026-01-01", "2026-01-31").display(DateSelectionMode::Range),
            "2026-01-01 – 2026-01-31"
        );
        assert_eq!(
            DateSelection::new(Some(date("2026-01-01")), None).display(DateSelectionMode::Range),
            "2026-01-01 – …"
        );
        assert_eq!(
            DateSelection::new(None, Some(date("2026-01-31"))).display(DateSelectionMode::Range),
            "… – 2026-01-31"
        );
        assert_eq!(
            DateSelection::default().display(DateSelectionMode::Range),
            ""
        );
        assert_eq!(
            DateSelection::single(date("2026-01-01")).display(DateSelectionMode::Single),
            "2026-01-01"
        );
    }

    #[test]
    fn typed_ranges_accept_open_sides_and_reject_reversed_dates() {
        assert_eq!(
            DateSelection::parse_input("2026-01-01 – 2026-01-31", DateSelectionMode::Range)
                .unwrap(),
            range("2026-01-01", "2026-01-31")
        );
        assert_eq!(
            DateSelection::parse_input("2026-01-01 –", DateSelectionMode::Range).unwrap(),
            DateSelection::new(Some(date("2026-01-01")), None)
        );
        assert_eq!(
            DateSelection::parse_input("– 2026-01-31", DateSelectionMode::Range).unwrap(),
            DateSelection::new(None, Some(date("2026-01-31")))
        );
        assert_eq!(
            DateSelection::parse_input("2026-01-31", DateSelectionMode::Range).unwrap(),
            DateSelection::new(Some(date("2026-01-31")), None)
        );
        assert_eq!(
            DateSelection::parse_input("2026-01-01 – …", DateSelectionMode::Range).unwrap(),
            DateSelection::new(Some(date("2026-01-01")), None)
        );
        assert_eq!(
            DateSelection::parse_input("… – 2026-01-31", DateSelectionMode::Range).unwrap(),
            DateSelection::new(None, Some(date("2026-01-31")))
        );

        let reversed =
            DateSelection::parse_input("2026-01-31 – 2026-01-01", DateSelectionMode::Range)
                .unwrap();
        assert_eq!(
            reversed.validate(DateSelectionMode::Range, &DateBounds::default()),
            Err(DateSelectionError::Order)
        );
    }

    #[test]
    fn display_round_trips_through_typed_input() {
        for selection in [
            DateSelection::default(),
            DateSelection::new(Some(date("2026-01-01")), None),
            DateSelection::new(None, Some(date("2026-01-31"))),
            range("2026-01-01", "2026-01-31"),
        ] {
            assert_eq!(
                DateSelection::parse_input(
                    &selection.display(DateSelectionMode::Range),
                    DateSelectionMode::Range
                )
                .unwrap(),
                selection
            );
        }
    }

    #[test]
    fn single_mode_rejects_range_syntax() {
        assert_eq!(
            DateSelection::parse_input("2026-01-01 – 2026-01-31", DateSelectionMode::Single),
            Err(DateSelectionError::SingleDateOnly)
        );
        assert_eq!(
            DateSelection::parse_input("2026-01-01", DateSelectionMode::Single).unwrap(),
            DateSelection::single(date("2026-01-01"))
        );
    }

    #[test]
    fn validation_enforces_inclusive_bounds() {
        let bounds = DateBounds::new(Some(date("2026-01-10")), Some(date("2026-01-20")));
        assert_eq!(
            range("2026-01-10", "2026-01-20").validate(DateSelectionMode::Range, &bounds),
            Ok(())
        );
        assert!(range("2026-01-09", "2026-01-20")
            .validate(DateSelectionMode::Range, &bounds)
            .is_err());
        assert!(range("2026-01-10", "2026-01-21")
            .validate(DateSelectionMode::Range, &bounds)
            .is_err());
        assert!(!DateBounds::new(Some(date("2026-01-20")), Some(date("2026-01-10"))).is_valid());
    }

    #[test]
    fn clearing_falls_back_to_the_configured_bounds() {
        let bounds = DateBounds::new(Some(date("2026-01-10")), Some(date("2026-01-20")));
        assert_eq!(
            DateSelection::default_in_bounds(DateSelectionMode::Range, &bounds),
            range("2026-01-10", "2026-01-20")
        );
        assert_eq!(
            DateSelection::default_in_bounds(DateSelectionMode::Single, &bounds),
            DateSelection::single(date("2026-01-10"))
        );
        assert_eq!(
            DateSelection::default_in_bounds(
                DateSelectionMode::Range,
                &DateBounds::new(Some(date("2026-01-10")), None)
            ),
            DateSelection::new(Some(date("2026-01-10")), None)
        );
        assert_eq!(
            DateSelection::default_in_bounds(
                DateSelectionMode::Range,
                &DateBounds::new(None, Some(date("2026-01-20")))
            ),
            DateSelection::new(None, Some(date("2026-01-20")))
        );
        assert!(
            DateSelection::default_in_bounds(DateSelectionMode::Range, &DateBounds::default())
                .is_empty()
        );
    }

    #[test]
    fn calendar_clicks_build_and_swap_ranges() {
        let mut draft = DateSelectionDraft::default();
        draft.select_day(date("2026-01-10"), DateSelectionMode::Range);
        assert_eq!(
            draft.selection(),
            &DateSelection::new(Some(date("2026-01-10")), None)
        );
        assert_eq!(draft.pending_start(), Some(&date("2026-01-10")));

        draft.select_day(date("2026-01-20"), DateSelectionMode::Range);
        assert_eq!(draft.selection(), &range("2026-01-10", "2026-01-20"));
        assert_eq!(draft.pending_start(), None);

        let mut reversed = DateSelectionDraft::default();
        reversed.select_day(date("2026-01-20"), DateSelectionMode::Range);
        reversed.select_day(date("2026-01-10"), DateSelectionMode::Range);
        assert_eq!(reversed.selection(), &range("2026-01-10", "2026-01-20"));
    }

    #[test]
    fn a_third_click_starts_a_new_range_and_to_only_ranges_complete() {
        let mut draft = DateSelectionDraft::new(range("2026-01-10", "2026-01-20"));
        draft.select_day(date("2026-02-01"), DateSelectionMode::Range);
        assert_eq!(
            draft.selection(),
            &DateSelection::new(Some(date("2026-02-01")), None)
        );

        let mut to_only =
            DateSelectionDraft::new(DateSelection::new(None, Some(date("2026-01-31"))));
        to_only.select_day(date("2026-01-10"), DateSelectionMode::Range);
        assert_eq!(to_only.selection(), &range("2026-01-10", "2026-01-31"));
    }

    #[test]
    fn clearing_endpoints_creates_open_ranges() {
        let mut draft = DateSelectionDraft::new(range("2026-01-10", "2026-01-20"));
        draft.clear_from();
        assert_eq!(
            draft.selection(),
            &DateSelection::new(None, Some(date("2026-01-20")))
        );

        let mut draft = DateSelectionDraft::new(range("2026-01-10", "2026-01-20"));
        draft.clear_to();
        assert_eq!(
            draft.selection(),
            &DateSelection::new(Some(date("2026-01-10")), None)
        );
        assert_eq!(draft.pending_start(), Some(&date("2026-01-10")));
    }

    #[test]
    fn preview_highlights_the_pending_range_in_both_directions() {
        let mut draft = DateSelectionDraft::default();
        draft.select_day(date("2026-01-20"), DateSelectionMode::Range);
        assert_eq!(
            draft.preview(Some(&date("2026-01-10"))),
            range("2026-01-10", "2026-01-20")
        );
        assert_eq!(
            draft.preview(Some(&date("2026-01-25"))),
            range("2026-01-20", "2026-01-25")
        );
    }

    #[test]
    fn single_mode_only_selects_one_date() {
        let mut draft = DateSelectionDraft::new(range("2026-01-10", "2026-01-20"));
        draft.select_day(date("2026-02-01"), DateSelectionMode::Single);
        assert_eq!(
            draft.selection(),
            &DateSelection::single(date("2026-02-01"))
        );
        assert_eq!(draft.pending_start(), None);
    }
}
