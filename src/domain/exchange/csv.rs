#![cfg_attr(not(feature = "server"), allow(dead_code))]

//! Exchange CSV parsing.
//!
//! The parser is deliberately independent from HTTP, SQLite, and Dioxus so
//! the same rules can be unit-tested and reused by the import service. All
//! naive timestamps are interpreted as UTC.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

use super::{ExchangeTxType, ImportDataMapping};

const SATOSHIS_PER_BTC: f64 = 100_000_000.0;

/// One parsed ledger row before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedExchangeTx {
    pub fingerprint: String,
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    /// BTC/XBT amounts are normalised to satoshis; non-BTC amounts keep the
    /// source unit and are discarded by the import workflow.
    pub amount: f64,
    pub currency: String,
    pub fiat_amount: Option<f64>,
    pub fiat_currency: Option<String>,
    pub fee: Option<f64>,
    pub fee_currency: Option<String>,
    pub tx_hash: Option<String>,
    pub description: Option<String>,
    pub raw_data: BTreeMap<String, String>,
}

/// Parsed CSV document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvDocument {
    pub headers: Vec<String>,
    pub rows: Vec<BTreeMap<String, String>>,
}

/// Stable parser identity shown by the import dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParserInfo {
    pub id: &'static str,
    pub name: &'static str,
}

/// Built-in CSV formats in detection priority order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParserKind {
    CryptoCom,
    Revolut,
    Binance,
    Kraken,
    Coinbase,
    Okx,
    Bitget,
    Bybit,
    GenericTrade,
}

impl ParserKind {
    pub const ALL: [Self; 9] = [
        Self::CryptoCom,
        Self::Revolut,
        Self::Binance,
        Self::Kraken,
        Self::Coinbase,
        Self::Okx,
        Self::Bitget,
        Self::Bybit,
        Self::GenericTrade,
    ];

    pub fn info(self) -> ParserInfo {
        match self {
            Self::CryptoCom => ParserInfo {
                id: "crypto.com",
                name: "Crypto.com",
            },
            Self::Revolut => ParserInfo {
                id: "revolut",
                name: "Revolut",
            },
            Self::Binance => ParserInfo {
                id: "binance",
                name: "Binance",
            },
            Self::Kraken => ParserInfo {
                id: "kraken",
                name: "Kraken",
            },
            Self::Coinbase => ParserInfo {
                id: "coinbase",
                name: "Coinbase",
            },
            Self::Okx => ParserInfo {
                id: "okx",
                name: "OKX",
            },
            Self::Bitget => ParserInfo {
                id: "bitget",
                name: "Bitget",
            },
            Self::Bybit => ParserInfo {
                id: "bybit",
                name: "Bybit",
            },
            Self::GenericTrade => ParserInfo {
                id: "generic-trades",
                name: "Generic trades",
            },
        }
    }
}

/// Whether an asset code is Bitcoin in either common spelling.
pub fn is_bitcoin_currency(currency: &str) -> bool {
    matches!(currency.trim().to_ascii_uppercase().as_str(), "BTC" | "XBT")
}

/// Normalize a source amount: Bitcoin rows become integer satoshis.
pub fn normalize_amount(amount: f64, currency: &str) -> f64 {
    if is_bitcoin_currency(currency) {
        (amount * SATOSHIS_PER_BTC).round()
    } else {
        amount
    }
}

/// Deterministic duplicate identity of one ledger row.
pub fn build_fingerprint(
    timestamp: i64,
    amount: f64,
    currency: &str,
    tx_type: ExchangeTxType,
    tx_hash: Option<&str>,
) -> String {
    if let Some(hash) = tx_hash.map(str::trim).filter(|hash| !hash.is_empty()) {
        return format!("txhash:{hash}");
    }
    let amount = if amount.fract() == 0.0 {
        format!("{}", amount as i64)
    } else {
        format!("{amount}")
    };
    format!(
        "{timestamp}_{amount}_{}_{}",
        currency.trim().to_ascii_uppercase(),
        tx_type.as_str()
    )
}

/// Parse RFC 4180-style CSV text with quoted fields, escaped quotes, CRLF,
/// and newlines inside quoted fields.
pub fn parse_csv_text(text: &str) -> Result<CsvDocument, String> {
    let text = text.trim_start_matches('\u{feff}').trim_end();
    if text.is_empty() {
        return Err("The selected file is empty.".to_string());
    }
    let lines = split_csv_lines(text);
    let Some(header_line) = lines.first() else {
        return Err("The selected file has no header row.".to_string());
    };
    let headers = parse_csv_line(header_line);
    if headers.iter().all(|header| header.trim().is_empty()) {
        return Err("The selected file has no header row.".to_string());
    }
    let mut rows = Vec::new();
    for line in lines.iter().skip(1) {
        let values = parse_csv_line(line);
        if values.iter().all(|value| value.is_empty()) {
            continue;
        }
        let mut row = BTreeMap::new();
        for (index, header) in headers.iter().enumerate() {
            row.insert(
                header.clone(),
                values.get(index).cloned().unwrap_or_default(),
            );
        }
        rows.push(row);
    }
    Ok(CsvDocument { headers, rows })
}

fn split_csv_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                current.push('"');
                chars.next();
            }
            '"' => {
                current.push(ch);
                in_quotes = !in_quotes;
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
            }
            '\n' if !in_quotes => lines.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => fields.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }
    fields.push(current);
    fields
}

fn has_headers(headers: &[String], required: &[&str]) -> bool {
    required
        .iter()
        .all(|required| headers.iter().any(|header| header == required))
}

/// Detect a built-in parser from its required headers.
pub fn detect_parser(headers: &[String]) -> Option<ParserKind> {
    ParserKind::ALL.into_iter().find(|parser| {
        let required: &[&str] = match parser {
            ParserKind::CryptoCom => &["Timestamp (UTC)", "Transaction Kind", "Currency"],
            ParserKind::Revolut => &["Type", "Started Date", "Completed Date", "Currency"],
            ParserKind::Binance => &["UTC_Time", "Account", "Operation", "Coin", "Change"],
            ParserKind::Kraken => &[
                "txid", "refid", "time", "type", "aclass", "asset", "amount", "fee",
            ],
            ParserKind::Coinbase => &[
                "Timestamp",
                "Transaction Type",
                "Asset",
                "Quantity Transacted",
                "Notes",
            ],
            ParserKind::Okx => &["Bill ID", "Time", "Type", "Currency", "Amount"],
            ParserKind::Bitget => &["Time", "Currency", "Type", "Amount", "TransactionId"],
            ParserKind::Bybit => &["Date(UTC)", "Type", "Asset", "Amount", "Transaction ID"],
            ParserKind::GenericTrade => &[
                "Symbol", "Type", "Quantity", "Price", "Value", "Fees", "Date",
            ],
        };
        has_headers(headers, required)
    })
}

/// Parse rows with one of the built-in formats.
pub fn parse_rows(kind: ParserKind, rows: &[BTreeMap<String, String>]) -> Vec<ParsedExchangeTx> {
    rows.iter()
        .filter_map(|row| match kind {
            ParserKind::CryptoCom => parse_crypto_com(row),
            ParserKind::Revolut => parse_revolut(row),
            ParserKind::Binance => parse_binance(row),
            ParserKind::Kraken => parse_kraken(row),
            ParserKind::Coinbase => parse_coinbase(row),
            ParserKind::Okx => parse_okx(row),
            ParserKind::Bitget => parse_bitget(row),
            ParserKind::Bybit => parse_bybit(row),
            ParserKind::GenericTrade => parse_generic_trade(row),
        })
        .collect()
}

/// Parse rows with a user-defined mapping.
pub fn parse_manual_rows(
    rows: &[BTreeMap<String, String>],
    mapping: &ImportDataMapping,
) -> Vec<ParsedExchangeTx> {
    if !mapping.is_complete() {
        return Vec::new();
    }
    rows.iter()
        .filter_map(|row| {
            let raw_timestamp = value(row, &mapping.timestamp)?;
            let timestamp = parse_timestamp_with_optional_format(
                &raw_timestamp,
                if mapping.timestamp_format.trim().is_empty() {
                    None
                } else {
                    Some(mapping.timestamp_format.trim())
                },
            )?;
            let currency = value(row, &mapping.currency)
                .unwrap_or_default()
                .to_ascii_uppercase();
            if currency.is_empty() {
                return None;
            }
            let raw_amount = parse_number(&value(row, &mapping.amount).unwrap_or_default())?;
            let amount = normalize_amount(raw_amount, &currency);
            let tx_type = normalise_type(&value(row, &mapping.tx_type).unwrap_or_default());
            let fiat_amount = optional_field_number(row, &mapping.fiat_amount);
            let fiat_currency = optional_field(row, &mapping.fiat_currency);
            let fee_currency = optional_field(row, &mapping.fee_currency);
            let fee = optional_field_number(row, &mapping.fee).map(|value| {
                if fee_currency.as_deref().is_some_and(is_bitcoin_currency) {
                    (value * SATOSHIS_PER_BTC).round()
                } else {
                    value
                }
            });
            let tx_hash = optional_field(row, &mapping.tx_hash);
            let description = optional_field(row, &mapping.description);
            Some(ParsedExchangeTx {
                fingerprint: build_fingerprint(
                    timestamp,
                    amount,
                    &currency,
                    tx_type,
                    tx_hash.as_deref(),
                ),
                timestamp,
                tx_type,
                amount,
                currency,
                fiat_amount,
                fiat_currency,
                fee,
                fee_currency,
                tx_hash,
                description,
                raw_data: row.clone(),
            })
        })
        .collect()
}

fn value(row: &BTreeMap<String, String>, key: &str) -> Option<String> {
    if key.trim().is_empty() {
        return None;
    }
    row.get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn optional_field(row: &BTreeMap<String, String>, key: &str) -> Option<String> {
    value(row, key)
}

fn optional_field_number(row: &BTreeMap<String, String>, key: &str) -> Option<f64> {
    value(row, key).and_then(|value| parse_number(&value))
}

fn parse_number(raw: &str) -> Option<f64> {
    let cleaned = raw
        .trim()
        .trim_start_matches('+')
        .replace([',', ' '], "")
        .replace('(', "-")
        .replace(')', "");
    cleaned
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn row_timestamp(row: &BTreeMap<String, String>, key: &str) -> Option<i64> {
    parse_timestamp_with_optional_format(value(row, key)?.as_str(), None)
}

/// Accept both chrono strftime formats and the dayjs-style tokens used by
/// UTXO Utils (`YYYY-MM-DD HH:mm:ss`). A format containing `%` is treated as
/// chrono unchanged.
fn translate_dayjs_format(format: &str) -> String {
    if format.contains('%') {
        return format.to_string();
    }
    const TOKENS: &[(&str, &str)] = &[
        ("YYYY", "%Y"),
        ("MMMM", "%B"),
        ("MMM", "%b"),
        ("YY", "%y"),
        ("MM", "%m"),
        ("DD", "%d"),
        ("HH", "%H"),
        ("hh", "%I"),
        ("mm", "%M"),
        ("ss", "%S"),
        ("M", "%m"),
        ("D", "%d"),
        ("H", "%H"),
        ("h", "%I"),
        ("m", "%M"),
        ("s", "%S"),
        ("A", "%p"),
        ("a", "%p"),
    ];
    let mut translated = String::new();
    let mut remaining = format;
    'outer: while !remaining.is_empty() {
        for (token, replacement) in TOKENS {
            if remaining.starts_with(token) {
                translated.push_str(replacement);
                remaining = &remaining[token.len()..];
                continue 'outer;
            }
        }
        let ch = remaining.chars().next().unwrap_or_default();
        translated.push(ch);
        remaining = &remaining[ch.len_utf8()..];
    }
    translated
}

fn parse_timestamp_with_optional_format(raw: &str, format: Option<&str>) -> Option<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(format) = format.map(str::trim).filter(|format| !format.is_empty()) {
        let translated = translate_dayjs_format(format);
        let format = translated.as_str();
        if let Ok(value) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(value.and_utc().timestamp());
        }
        if let Ok(value) = NaiveDate::parse_from_str(raw, format) {
            return Some(value.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
        }
        return None;
    }
    if let Ok(value) = DateTime::parse_from_rfc3339(raw) {
        return Some(value.with_timezone(&Utc).timestamp());
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%d",
    ] {
        if let Ok(value) = NaiveDateTime::parse_from_str(raw, format) {
            return Some(value.and_utc().timestamp());
        }
        if let Ok(value) = NaiveDate::parse_from_str(raw, format) {
            return Some(value.and_hms_opt(0, 0, 0)?.and_utc().timestamp());
        }
    }
    None
}

fn generic_trade_timestamp(raw: &str) -> Option<i64> {
    let normalized = pad_single_digit_day(raw);
    for format in [
        "%b %d, %Y, %I:%M:%S %p",
        "%b %d, %Y, %H:%M:%S",
        "%b %d, %Y, %I:%M %p",
    ] {
        if let Ok(value) = NaiveDateTime::parse_from_str(&normalized, format) {
            return Some(value.and_utc().timestamp());
        }
    }
    parse_timestamp_with_optional_format(raw, None)
}

fn pad_single_digit_day(raw: &str) -> String {
    // "Jul 5, 2023, 08:00:00 AM" / "Jul 05, 2023..." are both accepted by
    // chrono's `%d`, so only collapse repeated whitespace here.
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(" ,", ",")
}

fn normalise_type(raw: &str) -> ExchangeTxType {
    let lower = raw.trim().to_ascii_lowercase();
    if lower.contains("purchase") || lower.contains("buy") {
        ExchangeTxType::Buy
    } else if lower.contains("sell") {
        ExchangeTxType::Sell
    } else if lower.contains("reward")
        || lower.contains("earn")
        || lower.contains("interest")
        || lower.contains("cashback")
    {
        ExchangeTxType::Reward
    } else if lower.contains("withdraw") {
        ExchangeTxType::Withdrawal
    } else if lower.contains("deposit") || lower.contains("topup") {
        ExchangeTxType::Deposit
    } else {
        ExchangeTxType::Unknown
    }
}

fn parse_crypto_com(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "Timestamp (UTC)")?;
    let currency = value(row, "Currency")
        .unwrap_or_default()
        .to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Amount")?.as_str())?;
    let to_currency = value(row, "To Currency").map(|value| value.to_ascii_uppercase());
    let to_amount = value(row, "To Amount").and_then(|value| parse_number(&value));
    let buy_with_fiat =
        to_currency.as_deref() == Some("BTC") && to_amount.is_some_and(|amount| amount != 0.0);
    let effective_currency = if buy_with_fiat {
        to_currency.unwrap_or(currency)
    } else {
        currency
    };
    let effective_amount = if buy_with_fiat {
        to_amount.unwrap_or(raw_amount)
    } else {
        raw_amount
    };
    let amount = normalize_amount(effective_amount, &effective_currency);
    let tx_type = match value(row, "Transaction Kind").unwrap_or_default().as_str() {
        "crypto_withdrawal" => ExchangeTxType::Withdrawal,
        "crypto_deposit" => ExchangeTxType::Deposit,
        "viban_purchase" | "crypto_purchase" | "crypto_exchange" => ExchangeTxType::Buy,
        "crypto_earn_interest_paid"
        | "crypto_earn_program_created"
        | "card_cashback"
        | "referral_card_cashback"
        | "reimbursement"
        | "supercharger_reward_to_app_credited" => ExchangeTxType::Reward,
        "crypto_earn_program_withdrawn"
        | "exchange_to_crypto_transfer"
        | "supercharger_withdrawal" => ExchangeTxType::Withdrawal,
        "crypto_to_exchange_transfer" | "supercharger_deposit" => ExchangeTxType::Deposit,
        "dust_conversion_credited" | "crypto_wallet_swap_credited" => ExchangeTxType::Buy,
        "dust_conversion_debited" | "crypto_wallet_swap_debited" => ExchangeTxType::Sell,
        _ => ExchangeTxType::Unknown,
    };
    let fiat_amount = value(row, "Native Amount (in USD)").and_then(|value| parse_number(&value));
    let fiat_currency = value(row, "Native Currency").map(|value| value.to_ascii_uppercase());
    let tx_hash = value(row, "Transaction Hash");
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(
            timestamp,
            amount,
            &effective_currency,
            tx_type,
            tx_hash.as_deref(),
        ),
        timestamp,
        tx_type,
        amount,
        currency: effective_currency,
        fiat_amount,
        fiat_currency,
        fee: None,
        fee_currency: None,
        tx_hash,
        description: value(row, "Transaction Description"),
        raw_data: row.clone(),
    })
}

fn parse_revolut(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    if let Some(state) = value(row, "State") {
        if !state.eq_ignore_ascii_case("COMPLETED") {
            return None;
        }
    }
    let timestamp = row_timestamp(row, "Completed Date")?;
    let currency = value(row, "Currency")
        .unwrap_or_default()
        .to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Amount")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let type_raw = value(row, "Type").unwrap_or_default().to_ascii_uppercase();
    let tx_type = match type_raw.as_str() {
        "EXCHANGE" => {
            if raw_amount >= 0.0 {
                ExchangeTxType::Buy
            } else {
                ExchangeTxType::Sell
            }
        }
        "CRYPTO_WITHDRAWAL" => ExchangeTxType::Withdrawal,
        "CRYPTO_DEPOSIT" => ExchangeTxType::Deposit,
        "TOPUP" => ExchangeTxType::Deposit,
        "CARD_PAYMENT" => ExchangeTxType::Sell,
        "REWARD" | "CASHBACK" => ExchangeTxType::Reward,
        _ => ExchangeTxType::Unknown,
    };
    let fiat_amount = value(row, "Fiat amount (inc. fees)")
        .or_else(|| value(row, "Fiat amount"))
        .and_then(|value| parse_number(&value));
    let fiat_currency = value(row, "Base currency").map(|value| value.to_ascii_uppercase());
    let fee = value(row, "Fee").and_then(|value| parse_number(&value));
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, None),
        timestamp,
        tx_type,
        amount,
        currency,
        fiat_amount,
        fiat_currency: fiat_currency.clone(),
        fee,
        fee_currency: fiat_currency,
        tx_hash: None,
        description: value(row, "Description"),
        raw_data: row.clone(),
    })
}

fn parse_binance(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "UTC_Time")?;
    let currency = value(row, "Coin").unwrap_or_default().to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Change")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let operation = value(row, "Operation").unwrap_or_default();
    let tx_type = match operation.as_str() {
        "Buy" | "Transaction_Buy" | "Binance Payment" => ExchangeTxType::Buy,
        "Sell" | "Transaction_Sell" => ExchangeTxType::Sell,
        "Fiat_Deposit"
        | "Deposit"
        | "Crypto_Deposit"
        | "Simple Earn Flexible Subscription"
        | "Simple Earn Locked Subscription"
        | "POS savings purchase"
        | "Savings purchase"
        | "ETH 2.0 Staking" => ExchangeTxType::Deposit,
        "Fiat_Withdrawal"
        | "Withdraw"
        | "Crypto_Withdrawal"
        | "Simple Earn Flexible Redemption"
        | "Simple Earn Locked Redemption"
        | "POS savings redemption"
        | "Savings Principal redemption" => ExchangeTxType::Withdrawal,
        "Commission History"
        | "Referral Kickback"
        | "Distribution"
        | "Airdrop Assets"
        | "Launchpool Earnings / Airdrop"
        | "Simple Earn Flexible Interest"
        | "Simple Earn Locked Rewards"
        | "ETH 2.0 Staking Rewards"
        | "Savings Interest"
        | "POS savings interest"
        | "Liquid Swap rewards"
        | "Interest"
        | "Cashback" => ExchangeTxType::Reward,
        _ => ExchangeTxType::Unknown,
    };
    let description = value(row, "Remark").or_else(|| {
        if operation.is_empty() {
            None
        } else {
            Some(operation.clone())
        }
    });
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, None),
        timestamp,
        tx_type,
        amount,
        currency,
        fiat_amount: None,
        fiat_currency: None,
        fee: None,
        fee_currency: None,
        tx_hash: None,
        description,
        raw_data: row.clone(),
    })
}

fn parse_kraken(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "time")?;
    let raw_asset = value(row, "asset").unwrap_or_default();
    let currency = normalise_kraken_asset(&raw_asset);
    let raw_amount = parse_number(value(row, "amount")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let type_raw = value(row, "type").unwrap_or_default();
    let subtype = value(row, "subtype").unwrap_or_default();
    let tx_type = match type_raw.to_ascii_lowercase().as_str() {
        "trade" | "margin trade" => {
            if raw_amount >= 0.0 {
                ExchangeTxType::Buy
            } else {
                ExchangeTxType::Sell
            }
        }
        "deposit" | "receive" => ExchangeTxType::Deposit,
        "withdrawal" | "spend" => ExchangeTxType::Withdrawal,
        "earn" | "staking" => match subtype.to_ascii_lowercase().as_str() {
            "reward" => ExchangeTxType::Reward,
            "allocation" | "autoallocate" | "spottostaking" => ExchangeTxType::Deposit,
            "deallocation" | "stakingfromspot" => ExchangeTxType::Withdrawal,
            _ => ExchangeTxType::Reward,
        },
        _ => ExchangeTxType::Unknown,
    };
    let fee = value(row, "fee").and_then(|value| parse_number(&value));
    let description = [type_raw, subtype]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" – ");
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, None),
        timestamp,
        tx_type,
        amount,
        currency: currency.clone(),
        fiat_amount: None,
        fiat_currency: None,
        fee,
        fee_currency: Some(currency),
        tx_hash: None,
        description: Some(description).filter(|value| !value.is_empty()),
        raw_data: row.clone(),
    })
}

fn normalise_kraken_asset(asset: &str) -> String {
    match asset.trim() {
        "XXBT" | "XBT" => "BTC".to_string(),
        "XETH" => "ETH".to_string(),
        "XLTC" => "LTC".to_string(),
        "XXRP" => "XRP".to_string(),
        "XXLM" => "XLM".to_string(),
        "XXDG" => "DOGE".to_string(),
        "ZUSD" => "USD".to_string(),
        "ZEUR" => "EUR".to_string(),
        "ZGBP" => "GBP".to_string(),
        "ZCAD" => "CAD".to_string(),
        "ZJPY" => "JPY".to_string(),
        other => other.to_ascii_uppercase(),
    }
}

fn parse_coinbase(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "Timestamp")?;
    let tx_type_raw = value(row, "Transaction Type").unwrap_or_default();
    let tx_type = match tx_type_raw.as_str() {
        "Buy" | "Advanced Trade Buy" => ExchangeTxType::Buy,
        "Sell" | "Advanced Trade Sell" => ExchangeTxType::Sell,
        "Send" => ExchangeTxType::Withdrawal,
        "Receive" => ExchangeTxType::Deposit,
        "Convert" => ExchangeTxType::Buy,
        "Rewards Income" | "Coinbase Earn" | "Learning Reward" | "Staking Income"
        | "Inflation Reward" => ExchangeTxType::Reward,
        _ => ExchangeTxType::Unknown,
    };
    let quantity = parse_number(value(row, "Quantity Transacted")?.as_str())?;
    let raw_amount = if matches!(
        tx_type_raw.as_str(),
        "Send" | "Sell" | "Advanced Trade Sell" | "Convert"
    ) {
        -quantity
    } else {
        quantity
    };
    let currency = value(row, "Asset").unwrap_or_default().to_ascii_uppercase();
    let amount = normalize_amount(raw_amount, &currency);
    let spot_currency = value(row, "Spot Price Currency").map(|value| value.to_ascii_uppercase());
    let total = value(row, "Total (inclusive of fees and/or spread)")
        .and_then(|value| parse_number(&value))
        .map(f64::abs);
    let fee = value(row, "Fees and/or Spread").and_then(|value| parse_number(&value));
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, None),
        timestamp,
        tx_type,
        amount,
        currency,
        fiat_amount: total,
        fiat_currency: spot_currency.clone(),
        fee,
        fee_currency: spot_currency,
        tx_hash: None,
        description: value(row, "Notes").or_else(|| Some(tx_type_raw).filter(|v| !v.is_empty())),
        raw_data: row.clone(),
    })
}

fn parse_okx(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "Time")?;
    let currency = value(row, "Currency")
        .unwrap_or_default()
        .to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Amount")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let type_raw = value(row, "Type").unwrap_or_default();
    let sub_type = value(row, "Sub-type").unwrap_or_default();
    let lower_type = type_raw.to_ascii_lowercase();
    let lower_sub = sub_type.to_ascii_lowercase();
    let tx_type = if lower_type.contains("deposit") || lower_sub.contains("deposit") {
        ExchangeTxType::Deposit
    } else if lower_type.contains("withdraw") || lower_sub.contains("withdrawal") {
        ExchangeTxType::Withdrawal
    } else if lower_type == "trade" || lower_type == "trading" {
        if raw_amount >= 0.0 {
            ExchangeTxType::Buy
        } else {
            ExchangeTxType::Sell
        }
    } else if lower_type.contains("earn")
        || lower_type.contains("staking")
        || lower_type.contains("interest")
        || lower_type.contains("reward")
        || lower_type.contains("savings")
        || lower_sub.contains("reward")
        || lower_sub.contains("interest")
    {
        ExchangeTxType::Reward
    } else if lower_type.contains("purchase") || lower_type.contains("buy") {
        ExchangeTxType::Buy
    } else if lower_type.contains("sell") || lower_type.contains("redemption") {
        ExchangeTxType::Sell
    } else {
        ExchangeTxType::Unknown
    };
    let bill_id = value(row, "Bill ID");
    let fee = value(row, "Fee").and_then(|value| parse_number(&value));
    let description = [type_raw, sub_type]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" – ");
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, bill_id.as_deref()),
        timestamp,
        tx_type,
        amount,
        currency: currency.clone(),
        fiat_amount: None,
        fiat_currency: None,
        fee,
        fee_currency: Some(currency),
        tx_hash: None,
        description: Some(description).filter(|value| !value.is_empty()),
        raw_data: row.clone(),
    })
}

fn parse_bitget(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    if let Some(status) = value(row, "Status") {
        let status = status.to_ascii_lowercase();
        if status != "successful" && status != "success" {
            return None;
        }
    }
    let timestamp = row_timestamp(row, "Time")?;
    let currency = value(row, "Currency")
        .unwrap_or_default()
        .to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Amount")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let type_raw = value(row, "Type").unwrap_or_default();
    let tx_type = match type_raw.as_str() {
        "Deposit" => ExchangeTxType::Deposit,
        "Withdraw" | "Withdrawal" => ExchangeTxType::Withdrawal,
        "Buy" | "P2P purchase" => ExchangeTxType::Buy,
        "Sell" | "P2P sell" => ExchangeTxType::Sell,
        "Earn" | "Interest" | "Cashback" | "Reward" | "Staking reward" => ExchangeTxType::Reward,
        "Savings" => ExchangeTxType::Deposit,
        "Savings redemption" => ExchangeTxType::Withdrawal,
        _ => ExchangeTxType::Unknown,
    };
    let fee = value(row, "Fee").and_then(|value| parse_number(&value));
    let tx_id = value(row, "TransactionId");
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, tx_id.as_deref()),
        timestamp,
        tx_type,
        amount,
        currency: currency.clone(),
        fiat_amount: None,
        fiat_currency: None,
        fee,
        fee_currency: Some(currency),
        tx_hash: tx_id,
        description: value(row, "Note").or_else(|| Some(type_raw).filter(|v| !v.is_empty())),
        raw_data: row.clone(),
    })
}

fn parse_bybit(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = row_timestamp(row, "Date(UTC)")?;
    let currency = value(row, "Asset").unwrap_or_default().to_ascii_uppercase();
    let raw_amount = parse_number(value(row, "Amount")?.as_str())?;
    let amount = normalize_amount(raw_amount, &currency);
    let type_raw = value(row, "Type").unwrap_or_default();
    let tx_type = match type_raw.as_str() {
        "Deposit" => ExchangeTxType::Deposit,
        "Withdraw" | "Withdrawal" => ExchangeTxType::Withdrawal,
        "Buy" => ExchangeTxType::Buy,
        "Sell" => ExchangeTxType::Sell,
        "Trade" => {
            if raw_amount >= 0.0 {
                ExchangeTxType::Buy
            } else {
                ExchangeTxType::Sell
            }
        }
        "Earn" | "Reward" | "Interest" | "Staking reward" | "Cashback" | "Airdrop" => {
            ExchangeTxType::Reward
        }
        _ => ExchangeTxType::Unknown,
    };
    let is_on_chain = matches!(
        tx_type,
        ExchangeTxType::Deposit | ExchangeTxType::Withdrawal
    );
    let tx_id = value(row, "Transaction ID");
    let fee = value(row, "Fee").and_then(|value| parse_number(&value));
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(
            timestamp,
            amount,
            &currency,
            tx_type,
            if is_on_chain { tx_id.as_deref() } else { None },
        ),
        timestamp,
        tx_type,
        amount,
        currency: currency.clone(),
        fiat_amount: None,
        fiat_currency: None,
        fee,
        fee_currency: Some(currency),
        tx_hash: if is_on_chain { tx_id } else { None },
        description: value(row, "Remark").or_else(|| Some(type_raw).filter(|v| !v.is_empty())),
        raw_data: row.clone(),
    })
}

fn parse_generic_trade(row: &BTreeMap<String, String>) -> Option<ParsedExchangeTx> {
    let timestamp = generic_trade_timestamp(value(row, "Date")?.as_str())?;
    let currency = value(row, "Symbol")
        .unwrap_or_default()
        .to_ascii_uppercase();
    let raw_type = value(row, "Type").unwrap_or_default();
    let tx_type = if raw_type.to_ascii_lowercase().contains("buy") {
        ExchangeTxType::Buy
    } else if raw_type.to_ascii_lowercase().contains("sell") {
        ExchangeTxType::Sell
    } else {
        ExchangeTxType::Unknown
    };
    let quantity = parse_number(value(row, "Quantity")?.as_str())?;
    let signed_quantity = if tx_type == ExchangeTxType::Sell {
        -quantity
    } else {
        quantity
    };
    let amount = normalize_amount(signed_quantity, &currency);
    let value_money = parse_money(value(row, "Value").unwrap_or_default().as_str());
    let fee_money = parse_money(value(row, "Fees").unwrap_or_default().as_str());
    let fiat_currency = value_money.1.clone().or_else(|| fee_money.1.clone());
    let description = value(row, "Price").map(|price| format!("Price: {price}"));
    Some(ParsedExchangeTx {
        fingerprint: build_fingerprint(timestamp, amount, &currency, tx_type, None),
        timestamp,
        tx_type,
        amount,
        currency,
        fiat_amount: value_money.0,
        fiat_currency: fiat_currency.clone(),
        fee: fee_money.0,
        fee_currency: fee_money.1.or(fiat_currency),
        tx_hash: None,
        description,
        raw_data: row.clone(),
    })
}

fn parse_money(raw: &str) -> (Option<f64>, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    // Ordered like the UTXO Utils symbol map so multi-character symbols win
    // over the bare dollar sign.
    const SYMBOLS: &[(&str, &str)] = &[
        ("€", "EUR"),
        ("A$", "AUD"),
        ("CA$", "CAD"),
        ("CHF", "CHF"),
        ("CN¥", "CNY"),
        ("Kč", "CZK"),
        ("kr", "DKK"),
        ("£", "GBP"),
        ("HK$", "HKD"),
        ("Ft", "HUF"),
        ("Rp", "IDR"),
        ("₹", "INR"),
        ("¥", "JPY"),
        ("₩", "KRW"),
        ("MX$", "MXN"),
        ("RM", "MYR"),
        ("NZ$", "NZD"),
        ("zł", "PLN"),
        ("lei", "RON"),
        ("SEK", "SEK"),
        ("S$", "SGD"),
        ("₺", "TRY"),
        ("$", "USD"),
    ];
    let symbol = SYMBOLS
        .iter()
        .find(|(symbol, _)| trimmed.contains(symbol))
        .map(|(_, code)| (*code).to_string());
    let amount: String = trimmed
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == '-' || *ch == '+')
        .collect();
    (amount.parse::<f64>().ok(), symbol)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn csv_tokenizer_handles_quotes_crlf_and_embedded_newlines() {
        let text = "A,B\r\n\"x,1\",\"hello\r\nworld\"\r\n\"a\"\"b\",plain\r\n";
        let document = parse_csv_text(text).unwrap();
        assert_eq!(document.headers, ["A", "B"]);
        assert_eq!(document.rows.len(), 2);
        assert_eq!(document.rows[0]["A"], "x,1");
        assert_eq!(document.rows[0]["B"], "hello\r\nworld");
        assert_eq!(document.rows[1]["A"], "a\"b");
    }

    #[test]
    fn detects_all_builtin_parsers() {
        let cases = [
            (
                ParserKind::CryptoCom,
                headers(&["Timestamp (UTC)", "Transaction Kind", "Currency"]),
            ),
            (
                ParserKind::Revolut,
                headers(&["Type", "Started Date", "Completed Date", "Currency"]),
            ),
            (
                ParserKind::Binance,
                headers(&["UTC_Time", "Account", "Operation", "Coin", "Change"]),
            ),
            (
                ParserKind::Kraken,
                headers(&[
                    "txid", "refid", "time", "type", "aclass", "asset", "amount", "fee",
                ]),
            ),
            (
                ParserKind::Coinbase,
                headers(&[
                    "Timestamp",
                    "Transaction Type",
                    "Asset",
                    "Quantity Transacted",
                    "Notes",
                ]),
            ),
            (
                ParserKind::Okx,
                headers(&["Bill ID", "Time", "Type", "Currency", "Amount"]),
            ),
            (
                ParserKind::Bitget,
                headers(&["Time", "Currency", "Type", "Amount", "TransactionId"]),
            ),
            (
                ParserKind::Bybit,
                headers(&["Date(UTC)", "Type", "Asset", "Amount", "Transaction ID"]),
            ),
            (
                ParserKind::GenericTrade,
                headers(&[
                    "Symbol", "Type", "Quantity", "Price", "Value", "Fees", "Date",
                ]),
            ),
        ];
        for (kind, header) in cases {
            assert_eq!(detect_parser(&header), Some(kind));
        }
        assert_eq!(detect_parser(&headers(&["Unknown"])), None);
    }

    #[test]
    fn binance_signed_change_and_operation_are_mapped() {
        let row = BTreeMap::from([
            ("UTC_Time".to_string(), "2024-01-02 03:04:05".to_string()),
            ("Account".to_string(), "Spot".to_string()),
            ("Operation".to_string(), "Buy".to_string()),
            ("Coin".to_string(), "BTC".to_string()),
            ("Change".to_string(), "0.5".to_string()),
        ]);
        let parsed = parse_rows(ParserKind::Binance, &[row]).remove(0);
        assert_eq!(parsed.timestamp, 1_704_164_645);
        assert_eq!(parsed.tx_type, ExchangeTxType::Buy);
        assert_eq!(parsed.amount, 50_000_000.0);
        assert_eq!(parsed.currency, "BTC");
    }

    #[test]
    fn coinbase_outgoing_rows_are_negative() {
        let row = BTreeMap::from([
            ("Timestamp".to_string(), "2024-01-02T03:04:05Z".to_string()),
            ("Transaction Type".to_string(), "Send".to_string()),
            ("Asset".to_string(), "BTC".to_string()),
            ("Quantity Transacted".to_string(), "0.25".to_string()),
            ("Notes".to_string(), "to wallet".to_string()),
        ]);
        let parsed = parse_rows(ParserKind::Coinbase, &[row]).remove(0);
        assert_eq!(parsed.tx_type, ExchangeTxType::Withdrawal);
        assert_eq!(parsed.amount, -25_000_000.0);
    }

    #[test]
    fn manual_mapping_parses_optional_fields_and_timestamp_format() {
        let row = BTreeMap::from([
            ("When".to_string(), "01/02/2024 03:04".to_string()),
            ("Qty".to_string(), "0.1".to_string()),
            ("Asset".to_string(), "BTC".to_string()),
            ("Kind".to_string(), "deposit".to_string()),
            ("Fiat".to_string(), "100".to_string()),
            ("FiatCur".to_string(), "EUR".to_string()),
            ("NetworkFee".to_string(), "0.0001".to_string()),
            ("FeeCur".to_string(), "BTC".to_string()),
        ]);
        let mapping = ImportDataMapping {
            timestamp: "When".into(),
            timestamp_format: "%d/%m/%Y %H:%M".into(),
            amount: "Qty".into(),
            currency: "Asset".into(),
            tx_type: "Kind".into(),
            fiat_amount: "Fiat".into(),
            fiat_currency: "FiatCur".into(),
            fee: "NetworkFee".into(),
            fee_currency: "FeeCur".into(),
            ..ImportDataMapping::default()
        };
        let parsed = parse_manual_rows(&[row], &mapping).remove(0);
        assert_eq!(parsed.amount, 10_000_000.0);
        assert_eq!(parsed.fee, Some(10_000.0));
        assert_eq!(parsed.fiat_currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn generic_trade_dates_and_money_symbols_parse() {
        let row = BTreeMap::from([
            ("Date".to_string(), "Jul 5, 2023, 08:00:00 AM".to_string()),
            ("Symbol".to_string(), "BTC".to_string()),
            ("Type".to_string(), "Sell".to_string()),
            ("Quantity".to_string(), "0.25".to_string()),
            ("Price".to_string(), "30000".to_string()),
            ("Value".to_string(), "€7,500.00".to_string()),
            ("Fees".to_string(), "€10.00".to_string()),
        ]);
        let parsed = parse_rows(ParserKind::GenericTrade, &[row]).remove(0);
        assert_eq!(parsed.tx_type, ExchangeTxType::Sell);
        assert_eq!(parsed.amount, -25_000_000.0);
        assert_eq!(parsed.fiat_amount, Some(7500.0));
        assert_eq!(parsed.fiat_currency.as_deref(), Some("EUR"));
        assert_eq!(parsed.fee, Some(10.0));
    }

    #[test]
    fn manual_mapping_accepts_dayjs_style_timestamp_formats() {
        let row = BTreeMap::from([
            ("When".to_string(), "2024-01-02 03:04:05".to_string()),
            ("Qty".to_string(), "0.1".to_string()),
            ("Asset".to_string(), "BTC".to_string()),
            ("Kind".to_string(), "buy".to_string()),
        ]);
        let mapping = ImportDataMapping {
            timestamp: "When".into(),
            timestamp_format: "YYYY-MM-DD HH:mm:ss".into(),
            amount: "Qty".into(),
            currency: "Asset".into(),
            tx_type: "Kind".into(),
            ..ImportDataMapping::default()
        };
        let parsed = parse_manual_rows(&[row], &mapping).remove(0);
        assert_eq!(parsed.timestamp, 1_704_164_645);
    }
    #[test]
    fn non_btc_amounts_are_not_normalized() {
        assert_eq!(normalize_amount(2.5, "ETH"), 2.5);
        assert_eq!(normalize_amount(0.00000001, "BTC"), 1.0);
        assert!(is_bitcoin_currency("xbt"));
    }

    #[test]
    fn bundled_exchange_samples_detect_and_parse_at_least_100_btc_operations() {
        // The datasets under `assets/exchange_samples/` back the Import Data demo and must keep matching the built-in parsers.
        let samples = [
            (
                "revolut_btc.csv",
                ParserKind::Revolut,
                include_str!("../../../assets/exchange_samples/revolut_btc.csv"),
            ),
            (
                "binance_btc.csv",
                ParserKind::Binance,
                include_str!("../../../assets/exchange_samples/binance_btc.csv"),
            ),
            (
                "crypto_com_btc.csv",
                ParserKind::CryptoCom,
                include_str!("../../../assets/exchange_samples/crypto_com_btc.csv"),
            ),
        ];

        for (name, expected, text) in samples {
            let document = parse_csv_text(text).unwrap_or_else(|error| panic!("{name}: {error}"));
            assert_eq!(detect_parser(&document.headers), Some(expected), "{name}");

            let parsed = parse_rows(expected, &document.rows);
            let btc_rows = parsed
                .iter()
                .filter(|row| is_bitcoin_currency(&row.currency))
                .collect::<Vec<_>>();
            assert!(
                btc_rows.len() >= 100,
                "{name} must contain at least 100 BTC operations, found {}",
                btc_rows.len()
            );
            assert!(
                parsed.iter().any(|row| !is_bitcoin_currency(&row.currency)),
                "{name} must exercise the importer's non-BTC skipping"
            );

            let fingerprints = btc_rows
                .iter()
                .map(|row| row.fingerprint.clone())
                .collect::<std::collections::HashSet<_>>();
            assert_eq!(
                fingerprints.len(),
                btc_rows.len(),
                "{name} contains duplicate operation fingerprints"
            );

            for kind in [
                ExchangeTxType::Buy,
                ExchangeTxType::Sell,
                ExchangeTxType::Reward,
                ExchangeTxType::Withdrawal,
                ExchangeTxType::Deposit,
            ] {
                assert!(
                    btc_rows.iter().any(|row| row.tx_type == kind),
                    "{name} is missing a {} operation",
                    kind.as_str()
                );
            }
            assert!(
                btc_rows
                    .iter()
                    .all(|row| row.amount != 0.0 && row.timestamp > 0 && row.currency == "BTC"),
                "{name} contains an invalid BTC operation"
            );
        }
    }

    #[test]
    fn hashes_are_the_preferred_fingerprint() {
        assert_eq!(
            build_fingerprint(1, 2.0, "BTC", ExchangeTxType::Deposit, Some(" abc ")),
            "txhash:abc"
        );
        assert_eq!(
            build_fingerprint(1, 2.0, "btc", ExchangeTxType::Deposit, None),
            "1_2_BTC_deposit"
        );
    }
}
