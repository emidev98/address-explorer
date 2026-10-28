//! Presentation formatters shared by the addresses page KPIs, chart, and
//! table. They replicate the UTXO Utils `useFormatter` hook:
//! `BTCFormatter`, `USDFormatter`, and `addressFormatter`.

/// Format a satoshi amount as BTC with up to 8 fraction digits and a ` ₿`
/// suffix, e.g. `1,234.567 ₿`. Trailing fraction zeros are trimmed; the
/// integer part keeps en-US thousands grouping.
pub fn format_btc(satoshis: i64) -> String {
    let negative = satoshis < 0;
    let absolute = (satoshis as i128).abs();
    let whole = absolute / 100_000_000;
    let fraction = absolute % 100_000_000;

    let mut formatted = group_thousands(whole as u128);
    if fraction != 0 {
        let mut digits = format!("{fraction:08}");
        while digits.ends_with('0') {
            digits.pop();
        }
        formatted.push('.');
        formatted.push_str(&digits);
    }
    if negative {
        formatted.insert(0, '-');
    }
    formatted.push_str(" ₿");
    formatted
}

/// Format a USD amount in en-US currency style, e.g. `$1,234.56`.
pub fn format_usd(amount: f64) -> String {
    if !amount.is_finite() {
        return "-".to_string();
    }

    let negative = amount < 0.0;
    let cents = (amount.abs() * 100.0).round() as u128;
    let formatted = format!("${}.{:02}", group_thousands(cents / 100), cents % 100);
    if negative && cents != 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

/// Format a fiat amount with the currency code, e.g. `$1,234.56` for USD and
/// `1,234.56 EUR` for any other symbol.
pub fn format_fiat(amount: f64, symbol: &str) -> String {
    if symbol == "USD" {
        return format_usd(amount);
    }
    if !amount.is_finite() {
        return "-".to_string();
    }

    let negative = amount < 0.0;
    let cents = (amount.abs() * 100.0).round() as u128;
    let formatted = format!(
        "{}.{:02} {symbol}",
        group_thousands(cents / 100),
        cents % 100
    );
    if negative && cents != 0 {
        format!("-{formatted}")
    } else {
        formatted
    }
}

/// Shorten an address to `first6…last4`, e.g. `bc1qar…f5mdq`.
pub fn format_address(address: &str) -> String {
    let head: String = address.chars().take(6).collect();
    let tail: String = address
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}...{tail}")
}

/// Group the digits of a non-negative integer by thousands with commas.
fn group_thousands(value: u128) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::{format_address, format_btc, format_usd};

    #[test]
    fn btc_amounts_render_like_the_utxo_utils_formatter() {
        assert_eq!(format_btc(0), "0 ₿");
        assert_eq!(format_btc(1), "0.00000001 ₿");
        assert_eq!(format_btc(100_000_000), "1 ₿");
        assert_eq!(format_btc(123_456_789), "1.23456789 ₿");
        assert_eq!(format_btc(150_000), "0.0015 ₿");
        assert_eq!(format_btc(123_456_789_000_000), "1,234,567.89 ₿");
        // Trailing fraction zeros are trimmed, the integer part is grouped.
        assert_eq!(format_btc(1_000_000_000), "10 ₿");
        assert_eq!(format_btc(-250_000), "-0.0025 ₿");
    }

    #[test]
    fn usd_amounts_render_in_en_us_currency_style() {
        assert_eq!(format_usd(0.0), "$0.00");
        assert_eq!(format_usd(1_234.5), "$1,234.50");
        assert_eq!(format_usd(66_000.0), "$66,000.00");
        assert_eq!(format_usd(1_234_567.891), "$1,234,567.89");
        assert_eq!(format_usd(-12.5), "-$12.50");
        assert_eq!(format_usd(f64::NAN), "-");
        assert_eq!(format_usd(f64::INFINITY), "-");
    }

    #[test]
    fn fiat_amounts_render_with_the_currency_code() {
        use super::format_fiat;

        assert_eq!(format_fiat(66_000.0, "USD"), "$66,000.00");
        assert_eq!(format_fiat(66_000.0, "EUR"), "66,000.00 EUR");
        assert_eq!(format_fiat(1_234.5, "RON"), "1,234.50 RON");
        assert_eq!(format_fiat(-12.5, "AUD"), "-12.50 AUD");
        assert_eq!(format_fiat(0.0, "EUR"), "0.00 EUR");
        assert_eq!(format_fiat(f64::NAN, "EUR"), "-");
    }

    #[test]
    fn addresses_shorten_to_first6_ellipsis_last4() {
        assert_eq!(
            format_address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"),
            "bc1qar...5mdq"
        );
        assert_eq!(format_address(""), "...");
    }
}
