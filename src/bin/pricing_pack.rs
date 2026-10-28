//! Pack the Kaggle source CSVs placed in `assets/pricing/raw_data/` into the
//! compressed pricing file consumed by the `historical_prices` seed
//! migration (`assets/pricing/btc_usd.bin`).
//!
//! Every `.csv` in the raw-data directory is read; the close-time and close
//! columns are located by header name so both the Binance-style kline exports
//! and the older SRK daily export work. Close prices are trimmed to two
//! decimals (USD cents) before they are written.
//!
//! The application ships as a single Dioxus binary without a library target,
//! so this tool loads the codec modules with `#[path]` instead of importing
//! them: writer and reader then share exactly the same format implementation.
//!
//! Usage: `cargo run --bin pricing_pack` from the repository root. The
//! git-ignored source CSVs must be downloaded from Kaggle first (see the
//! pricing README).

// `#[path]` loads the shared modules from the application source tree; the
// `domain` alias makes `crate::domain::civil_time` resolve exactly like in
// the application crate, so `pricing_format.rs` compiles unchanged in both.
#[path = "../domain/civil_time.rs"]
pub mod civil_time;

// The application uses the decoding half of the shared codec and this tool
// the encoding half, so unused-function warnings are expected here.
#[allow(dead_code)]
#[path = "../data/pricing_format.rs"]
mod pricing_format;

mod domain {
    pub use crate::civil_time;
}

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::domain::civil_time::parse_candle_timestamp;
use crate::pricing_format::{format_record, PriceRecord};

/// Directory holding the git-ignored Kaggle exports. Every `.csv` in it is
/// read.
const RAW_DATA_DIR: &str = "assets/pricing/raw_data";
/// Packed output committed to the repository and bundled with the app.
const OUTPUT: &str = "assets/pricing/btc_usd.bin";

/// Header names that carry the candle close time. Binance-style exports use
/// `Close time`; the older SRK daily export uses `Date`.
const CLOSE_TIME_HEADERS: [&str; 2] = ["Close time", "Date"];
/// Header name that carries the candle close price in both export styles.
const CLOSE_HEADER: &str = "Close";

fn main() -> Result<(), String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source_paths = source_csvs(&root.join(RAW_DATA_DIR))?;

    let mut records = Vec::new();
    for path in &source_paths {
        let contents = read(path)?;
        let (mut parsed, skipped) =
            parse_price_csv(&contents).map_err(|error| format!("{}: {error}", path.display()))?;
        println!(
            "{}: {} records ({} rows skipped)",
            path.strip_prefix(&root).unwrap_or(path).display(),
            parsed.len(),
            skipped
        );
        records.append(&mut parsed);
    }

    let duplicates = deduplicate(&mut records);
    println!("{duplicates} duplicate close times dropped");

    if records.is_empty() {
        return Err("no importable records in the raw pricing CSVs".to_string());
    }

    write_packed(&root.join(OUTPUT), &records)
}

/// List every CSV in `directory`, finest candle interval first. The order
/// matters: records that share a close time keep the order in which their
/// exports were read, so the most precise export supplies the duplicate.
fn source_csvs(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;

    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
            .path();
        if path.extension().and_then(OsStr::to_str) == Some("csv") {
            paths.push(path);
        }
    }

    if paths.is_empty() {
        return Err(format!(
            "no .csv files in {}; download the Kaggle exports first",
            directory.display()
        ));
    }

    paths.sort_by(|left, right| {
        source_interval_seconds(left)
            .unwrap_or(u64::MAX)
            .cmp(&source_interval_seconds(right).unwrap_or(u64::MAX))
            .then_with(|| left.cmp(right))
    });
    Ok(paths)
}

/// Candle width encoded in an export name (`btc_15m_data_…csv` is 900
/// seconds). Names without a recognized interval sort last but are still
/// read.
fn source_interval_seconds(path: &Path) -> Option<u64> {
    let stem = path.file_stem()?.to_str()?;
    let interval = stem.strip_prefix("btc_")?.split('_').next()?;
    let (amount, unit) = interval.split_at(interval.len().checked_sub(1)?);
    let amount: u64 = amount.parse().ok()?;
    let unit_seconds = match unit {
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return None,
    };
    amount.checked_mul(unit_seconds)
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

/// Parse one Kaggle export. The close-time and close columns are located by
/// header name so every export style works without hard-coded column indexes.
/// Rows that fail to parse (including the truncated tail rows of the exports)
/// are skipped; the second return value counts them.
fn parse_price_csv(contents: &str) -> Result<(Vec<PriceRecord>, usize), String> {
    let mut lines = contents.lines();
    let header = lines.next().ok_or_else(|| "file is empty".to_string())?;
    let columns: Vec<&str> = header.split(',').map(str::trim).collect();

    let close_time_column = columns
        .iter()
        .position(|column| CLOSE_TIME_HEADERS.contains(column))
        .ok_or_else(|| format!("header has none of {CLOSE_TIME_HEADERS:?}"))?;
    let close_column = columns
        .iter()
        .position(|column| *column == CLOSE_HEADER)
        .ok_or_else(|| format!("header has no {CLOSE_HEADER} column"))?;
    let required_columns = close_time_column.max(close_column) + 1;

    let mut records = Vec::new();
    let mut skipped = 0usize;
    for line in lines {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() < required_columns {
            skipped += 1;
            continue;
        }
        let record = parse_candle_timestamp(fields[close_time_column])
            .zip(to_cents(fields[close_column]))
            .map(|(close_time, close_cents)| PriceRecord {
                close_time,
                close_cents,
            });
        match record {
            Some(record) => records.push(record),
            None => skipped += 1,
        }
    }
    Ok((records, skipped))
}

/// Sort by close time and keep the first occurrence of each close time.
/// `sort_by_key` is stable, so equal close times keep the order in which their
/// exports were read and the finest export wins. Returns how many duplicates
/// were dropped.
fn deduplicate(records: &mut Vec<PriceRecord>) -> usize {
    records.sort_by_key(|record| record.close_time);

    let mut seen = HashSet::new();
    let before = records.len();
    records.retain(|record| seen.insert(record.close_time));
    before - records.len()
}

fn write_packed(path: &Path, records: &[PriceRecord]) -> Result<(), String> {
    let file = fs::File::create(path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    let mut writer = BufWriter::new(file);
    for record in records {
        writer
            .write_all(format_record(record).as_bytes())
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    // A trailing newline keeps the file friendly to POSIX tools; the parser
    // tolerates it.
    writeln!(writer).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    writer
        .flush()
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;

    println!("wrote {} records to {}", records.len(), path.display());
    Ok(())
}

/// Convert a decimal close price to integer cents, trimming any fraction
/// beyond the second decimal place instead of rounding it. Rejects values
/// that are not plain non-negative decimals or that overflow `u64`.
fn to_cents(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    let (whole, fraction) = raw.split_once('.').unwrap_or((raw, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    let dollars: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let mut fraction_digits = fraction.bytes();
    let cents = fraction_digits
        .next()
        .map(|digit| u64::from(digit - b'0') * 10)
        .unwrap_or(0)
        + fraction_digits
            .next()
            .map(|digit| u64::from(digit - b'0'))
            .unwrap_or(0);
    dollars.checked_mul(100)?.checked_add(cents)
}

#[cfg(test)]
mod tests {
    use super::{deduplicate, parse_price_csv, source_interval_seconds, to_cents, PriceRecord};
    use crate::domain::civil_time::epoch_seconds;
    use std::path::Path;

    #[test]
    fn trims_prices_to_two_decimal_places() {
        assert_eq!(to_cents("144.5399932861328"), Some(14_453));
        assert_eq!(to_cents("1.999"), Some(199));
        assert_eq!(to_cents("0.009"), Some(0));
        assert_eq!(to_cents("116.99"), Some(11_699));
        assert_eq!(to_cents("0.5"), Some(50));
        assert_eq!(to_cents("139.0"), Some(13_900));
    }

    #[test]
    fn rejects_invalid_prices() {
        for raw in ["", ".", "-1.00", "NaN", "inf", "1e3", "12.34.56", "  "] {
            assert_eq!(to_cents(raw), None, "accepted {raw:?}");
        }
    }

    #[test]
    fn rejects_overflowing_prices() {
        assert_eq!(to_cents("184467440737095516.15"), Some(u64::MAX));
        assert_eq!(to_cents("184467440737095516.16"), None);
    }

    #[test]
    fn parses_both_export_headers() {
        let binance = "Open time,Open,High,Low,Close,Volume,Close time\n\
            2018-01-01 00:00:00.000000 ,1,1,1,13556.157,0,2018-01-01 00:14:59.999000 \n";
        let (records, skipped) = parse_price_csv(binance).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(
            records,
            [PriceRecord {
                close_time: epoch_seconds(2018, 1, 1, 0, 14, 59),
                close_cents: 1_355_615,
            }]
        );

        let srk = "SNo,Name,Symbol,Date,High,Low,Open,Close,Volume,Marketcap\n\
            1,Bitcoin,BTC,2013-04-29 23:59:59,1,1,1,144.5399932861328,0,0\n";
        let (records, skipped) = parse_price_csv(srk).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(
            records,
            [PriceRecord {
                close_time: epoch_seconds(2013, 4, 29, 23, 59, 59),
                close_cents: 14_453,
            }]
        );
    }

    #[test]
    fn skips_rows_with_a_malformed_timestamp_or_close() {
        let contents = "Open time,Open,High,Low,Close,Volume,Close time\n\
            \n\
            ,1,1,1,1,0,\n\
            2018-01-01 00:00:00,1,1,1,not-a-price,0,2018-01-01 00:14:59\n\
            2018-01-01 00:00:00,1,1,1,2.5,0,2018-01-01 00:14:59\n";
        let (records, skipped) = parse_price_csv(contents).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].close_cents, 250);
        // The blank line, the empty timestamp, and the malformed close.
        assert_eq!(skipped, 3);
    }

    #[test]
    fn rejects_files_without_the_required_columns() {
        assert!(parse_price_csv("foo,bar\n1,2\n").is_err());
        assert!(parse_price_csv("").is_err());
    }

    #[test]
    fn deduplicate_sorts_by_time_and_keeps_the_first_value() {
        let mut records = vec![
            PriceRecord {
                close_time: 20,
                close_cents: 2_001,
            },
            PriceRecord {
                close_time: 10,
                close_cents: 1_001,
            },
            PriceRecord {
                close_time: 20,
                close_cents: 2_002,
            },
        ];

        assert_eq!(deduplicate(&mut records), 1);
        assert_eq!(
            records,
            [
                PriceRecord {
                    close_time: 10,
                    close_cents: 1_001,
                },
                PriceRecord {
                    close_time: 20,
                    close_cents: 2_001,
                },
            ]
        );
    }

    #[test]
    fn recognizes_the_export_interval_from_its_name() {
        assert_eq!(
            source_interval_seconds(Path::new("btc_15m_data_2018_to_2025.csv")),
            Some(900)
        );
        assert_eq!(
            source_interval_seconds(Path::new("btc_1d_data_2013_to_2018.csv")),
            Some(86_400)
        );
        assert_eq!(source_interval_seconds(Path::new("prices.csv")), None);
    }
}
