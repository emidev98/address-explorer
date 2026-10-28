//! UTC civil-date conversions shared by the pricing data migrations and the
//! price service. Pure integer algorithms (Howard Hinnant's days-from-civil)
//! keep these conversions dependency-free.
#![allow(dead_code)]

/// Days since 1970-01-01 for a proleptic Gregorian date.
pub const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

/// UTC epoch seconds for a civil date-time.
pub const fn epoch_seconds(
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
) -> i64 {
    days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second
}

/// Inverse of [`days_from_civil`].
pub const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Format the UTC calendar date of an epoch timestamp as `YYYY-MM-DD`.
pub fn format_date(epoch_second: i64) -> String {
    let (year, month, day) = civil_from_days(epoch_second.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Format an epoch timestamp as `YYYY-MM-DD HH:mm:ss` in UTC.
///
/// This is the cell format of the "First tx received" and "Last tx sent"
/// columns of the addresses overview, matching the UTXO Utils layout.
pub fn format_datetime(epoch_second: i64) -> String {
    let (year, month, day) = civil_from_days(epoch_second.div_euclid(86_400));
    let seconds_of_day = epoch_second.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        seconds_of_day / 3_600,
        seconds_of_day % 3_600 / 60,
        seconds_of_day % 60,
    )
}

/// Parse a UTC ISO 8601 date-time such as `2026-10-27T13:00:00.000000000Z`
/// into epoch seconds, truncating sub-second precision.
///
/// Coin Metrics returns its metric rows in this layout. Offsets other than
/// UTC are rejected because the rest of the application works in UTC.
pub fn parse_iso_timestamp(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    let date = raw.get(..10)?;
    if raw.get(10..11)? != "T" {
        return None;
    }
    let time = raw.get(11..19)?;
    let suffix = raw.get(19..)?;
    if !(suffix.is_empty() || suffix == "Z" || suffix.starts_with('.')) {
        return None;
    }
    if suffix.starts_with('.') && !suffix.ends_with('Z') {
        return None;
    }

    let day_start = parse_date(date)?;
    if time.get(2..3) != Some(":") || time.get(5..6) != Some(":") {
        return None;
    }
    let hour: i64 = time.get(0..2)?.parse().ok()?;
    let minute: i64 = time.get(3..5)?.parse().ok()?;
    let second: i64 = time.get(6..8)?.parse().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    Some(day_start + hour * 3_600 + minute * 60 + second)
}

/// Parse a `YYYY-MM-DD` calendar date (the value of a native
/// `<input type="date">`) into UTC epoch seconds at the start of that day.
pub fn parse_date(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if raw.len() != 10 || raw.get(4..5) != Some("-") || raw.get(7..8) != Some("-") {
        return None;
    }
    let year: i64 = raw.get(0..4)?.parse().ok()?;
    let month: i64 = raw.get(5..7)?.parse().ok()?;
    let day: i64 = raw.get(8..10)?.parse().ok()?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    Some(epoch_seconds(year, month, day, 0, 0, 0))
}

/// Parse the `Close time` cell of a Binance-style kline CSV row.
///
/// Accepts `YYYY-MM-DD HH:MM:SS` with an optional sub-second fraction and an
/// optional trailing `UTC` marker; returns UTC epoch seconds with the
/// sub-second part truncated. Returns `None` for cells that do not match the
/// layout, which includes the truncated tail row of the export where the
/// timestamp is empty.
pub fn parse_candle_timestamp(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    let raw = raw.strip_suffix("UTC").map(str::trim).unwrap_or(raw);
    let date = raw.get(..10)?;
    let rest = raw.get(10..)?.strip_prefix(' ')?;
    let time = rest.get(..8)?;

    if date.get(4..5) != Some("-") || date.get(7..8) != Some("-") {
        return None;
    }
    let year: i64 = date.get(0..4)?.parse().ok()?;
    let month: i64 = date.get(5..7)?.parse().ok()?;
    let day: i64 = date.get(8..10)?.parse().ok()?;

    if time.get(2..3) != Some(":") || time.get(5..6) != Some(":") {
        return None;
    }
    let hour: i64 = time.get(0..2)?.parse().ok()?;
    let minute: i64 = time.get(3..5)?.parse().ok()?;
    let second: i64 = time.get(6..8)?.parse().ok()?;

    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    Some(epoch_seconds(year, month, day, hour, minute, second))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_unix_epoch() {
        assert_eq!(epoch_seconds(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn epoch_format_date_round_trip() {
        let ts = epoch_seconds(2026, 6, 15, 23, 29, 59);
        assert_eq!(format_date(ts), "2026-06-15");
        assert_eq!(format_date(0), "1970-01-01");
    }

    #[test]
    fn format_datetime_renders_utc_civil_time() {
        assert_eq!(format_datetime(0), "1970-01-01 00:00:00");
        assert_eq!(
            format_datetime(epoch_seconds(2026, 6, 15, 23, 29, 59)),
            "2026-06-15 23:29:59"
        );
        // Negative timestamps stay on the proleptic calendar.
        assert_eq!(format_datetime(-1), "1969-12-31 23:59:59");
    }

    #[test]
    fn leap_year_rules_match_gregorian_calendar() {
        // 2000 is a leap year, 1900 is not.
        assert_eq!(
            format_date(epoch_seconds(2000, 2, 29, 0, 0, 0) + 86_400),
            "2000-03-01"
        );
        assert_eq!(
            format_date(epoch_seconds(1900, 3, 1, 0, 0, 0) - 86_400),
            "1900-02-28"
        );
    }

    #[test]
    fn parses_csv_close_time_cells() {
        assert_eq!(
            parse_candle_timestamp("2018-01-01 00:14:59.999000 "),
            Some(epoch_seconds(2018, 1, 1, 0, 14, 59))
        );
        assert_eq!(
            parse_candle_timestamp("2026-06-16 23:59:59.999000 UTC"),
            Some(epoch_seconds(2026, 6, 16, 23, 59, 59))
        );
        assert_eq!(
            parse_candle_timestamp("2024-02-29 12:00:00"),
            Some(epoch_seconds(2024, 2, 29, 12, 0, 0))
        );
    }

    #[test]
    fn parses_iso_utc_timestamps() {
        let epoch = epoch_seconds(2026, 10, 27, 13, 45, 12);
        assert_eq!(
            parse_iso_timestamp("2026-10-27T13:45:12.123456789Z"),
            Some(epoch)
        );
        assert_eq!(parse_iso_timestamp("2026-10-27T13:45:12Z"), Some(epoch));
        assert_eq!(parse_iso_timestamp("2026-10-27T13:45:12"), Some(epoch));
    }

    #[test]
    fn rejects_non_utc_or_malformed_iso_timestamps() {
        for raw in [
            "",
            "2026-10-27",
            "2026-10-27 13:45:12Z",
            "2026-10-27T13:45:12+02:00",
            "2026-10-27T24:00:00Z",
            "2026-13-27T13:45:12Z",
            "not a timestamp",
        ] {
            assert_eq!(parse_iso_timestamp(raw), None, "accepted {raw:?}");
        }
    }

    #[test]
    fn parses_date_input_values_at_day_start() {
        assert_eq!(
            parse_date("2026-06-15"),
            Some(epoch_seconds(2026, 6, 15, 0, 0, 0))
        );
        assert_eq!(parse_date("1970-01-01"), Some(0));
        assert_eq!(
            parse_date(" 2024-02-29 "),
            Some(epoch_seconds(2024, 2, 29, 0, 0, 0))
        );
    }

    #[test]
    fn rejects_malformed_date_input_values() {
        for raw in [
            "",
            "2026-6-15",
            "2026/06/15",
            "2026-13-01",
            "2026-06-00",
            "not a date",
        ] {
            assert_eq!(parse_date(raw), None, "accepted {raw:?}");
        }
    }

    #[test]
    fn rejects_malformed_cells() {
        for raw in [
            "",
            "not a date",
            "2018/01/01 00:00:00",
            "2018-01-01T00:00:00",
            "2018-13-01 00:00:00",
            "2018-01-01 24:00:00",
        ] {
            assert_eq!(parse_candle_timestamp(raw), None, "accepted {raw:?}");
        }
    }
}
