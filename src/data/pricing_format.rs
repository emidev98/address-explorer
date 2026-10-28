//! Codec for the packed BTC/USD pricing file specified by
//! `assets/pricing/README.md`.
//!
//! Records are written back to back as `<timestamp><cents>,`:
//!
//! - `timestamp` — 14 ASCII digits, the UTC close time as `YYYYMMDDHHmmss`;
//! - `cents` — the close price in USD cents as a decimal integer, so
//!   `1328427` means $13,284.27;
//! - a comma terminates every record, including the last one.
//!
//! Decoding skips malformed records so a damaged or hand-edited export can
//! never abort the pricing seed migration halfway.

use crate::domain::civil_time::{civil_from_days, epoch_seconds};

/// Number of ASCII digits in the `YYYYMMDDHHmmss` record prefix.
const TIMESTAMP_DIGITS: usize = 14;

/// One decoded pricing record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriceRecord {
    /// UTC close time as epoch seconds.
    pub close_time: i64,
    /// Close price in USD cents ($13,284.27 is `1328427`).
    pub close_cents: u64,
}

/// Encode `record` in its packed form, including the terminating comma.
// Only the packing tool (`src/bin/pricing_pack.rs`) and the tests encode
// records; the application itself only decodes.
#[allow(dead_code)]
pub fn format_record(record: &PriceRecord) -> String {
    let seconds_of_day = record.close_time.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(record.close_time.div_euclid(86_400));
    format!(
        "{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}{cents},",
        hour = seconds_of_day / 3_600,
        minute = seconds_of_day % 3_600 / 60,
        second = seconds_of_day % 60,
        cents = record.close_cents,
    )
}

/// Decode the packed pricing file, skipping malformed records. Surrounding
/// whitespace (including a trailing newline) around a record is tolerated.
pub fn parse_records(contents: &str) -> Vec<PriceRecord> {
    contents.split(',').filter_map(parse_record).collect()
}

/// Decode one record without its terminating comma.
fn parse_record(raw: &str) -> Option<PriceRecord> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let digits = raw.get(..TIMESTAMP_DIGITS)?;
    let cents = raw.get(TIMESTAMP_DIGITS..)?;
    if cents.is_empty() || !cents.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some(PriceRecord {
        close_time: parse_timestamp(digits)?,
        close_cents: cents.parse().ok()?,
    })
}

/// Parse the 14-digit `YYYYMMDDHHmmss` record prefix into UTC epoch seconds.
fn parse_timestamp(digits: &str) -> Option<i64> {
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let field = |range: std::ops::Range<usize>| digits.get(range)?.parse::<i64>().ok();
    let year = field(0..4)?;
    let month = field(4..6)?;
    let day = field(6..8)?;
    let hour = field(8..10)?;
    let minute = field(10..12)?;
    let second = field(12..14)?;

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
    fn format_matches_the_readme_example() {
        let record = PriceRecord {
            close_time: epoch_seconds(2018, 1, 1, 1, 1, 1),
            close_cents: 1_328_427,
        };
        assert_eq!(format_record(&record), "201801010101011328427,");
    }

    #[test]
    fn format_then_parse_round_trips() {
        let records = [
            PriceRecord {
                close_time: epoch_seconds(2013, 4, 29, 23, 59, 59),
                close_cents: 14_454,
            },
            PriceRecord {
                close_time: epoch_seconds(2025, 12, 31, 0, 14, 59),
                close_cents: 10_811_555,
            },
        ];
        let packed: String = records.iter().map(format_record).collect();
        assert_eq!(parse_records(&packed), records);
    }

    #[test]
    fn parse_tolerates_a_trailing_newline() {
        let records = parse_records("201801010101011328427,\n");
        assert_eq!(
            records,
            [PriceRecord {
                close_time: epoch_seconds(2018, 1, 1, 1, 1, 1),
                close_cents: 1_328_427,
            }]
        );
    }

    #[test]
    fn parse_skips_malformed_records() {
        let packed = [
            "201801010101011328427", // valid
            "",                      // empty (trailing comma)
            "20180101010101",        // timestamp without cents
            "201801010101",          // short timestamp
            "201813010101011328427", // month 13
            "2018010101010113a8427", // non-digit cents
            "not-a-record",          // garbage
        ]
        .join(",");
        let records = parse_records(&packed);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].close_cents, 1_328_427);
    }

    #[test]
    fn parse_rejects_out_of_range_timestamp_fields() {
        for digits in [
            "201800010101011328427", // month 00
            "201801000101011328427", // day 00
            "201801012401011328427", // hour 24
            "201801010160011328427", // minute 60
        ] {
            assert!(parse_records(digits).is_empty(), "accepted {digits}");
        }
    }
}
