//! Brand mark of a provider: a monogram on the stored brand color, or on a
//! deterministic fallback color when the row has no stored color yet.
use dioxus::prelude::*;
/// Badge size. Compact fits table cells and selected-provider hints; Regular
/// fits dialog previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeSize {
    #[default]
    Compact,
    Regular,
}
/// Deterministic palette used when a provider has no stored brand color.
/// The colors are dark enough for white text but varied enough to tell
/// providers apart at a glance.
const DERIVED_COLORS: [&str; 12] = [
    "#1E88E5", "#43A047", "#8E24AA", "#F4511E", "#00897B", "#5E35B1", "#C2185B", "#6D4C41",
    "#3949AB", "#00838F", "#7CB342", "#D81B60",
];
#[component]
pub fn ProviderBadge(
    name: String,
    #[props(default)] color: Option<String>,
    #[props(default)] size: BadgeSize,
) -> Element {
    let background = color
        .as_deref()
        .and_then(normalize_style_color)
        .unwrap_or_else(|| derived_color(&name));
    let foreground = readable_text_color(&background);
    let class = match size {
        BadgeSize::Compact => "md3-provider-badge md3-provider-badge-compact",
        BadgeSize::Regular => "md3-provider-badge",
    };
    rsx! {
        span {
            class: "{class}",
            style: "background-color: {background}; color: {foreground};",
            aria_hidden: "true",
            "{monogram(&name)}"
        }
    }
}
/// First letters of up to two words, uppercased. A single word contributes
/// one letter ("Crypto.com" -> "C"); two words contribute two ("OKX Wallet"
/// -> "OW").
pub fn monogram(name: &str) -> String {
    let mut initials = String::new();
    for word in name.split_whitespace() {
        let Some(character) = word.chars().find(|character| character.is_alphanumeric()) else {
            continue;
        };
        initials.push(character.to_uppercase().next().unwrap_or(character));
        if initials.chars().count() == 2 {
            break;
        }
    }
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}
/// Stable fallback color for a provider name.
pub fn derived_color(name: &str) -> String {
    let index = (stable_hash(name) % DERIVED_COLORS.len() as u64) as usize;
    DERIVED_COLORS[index].to_string()
}
/// Text color with enough contrast against `hex`, using the WCAG relative
/// luminance of the sRGB channels.
pub fn readable_text_color(hex: &str) -> &'static str {
    let (red, green, blue) = parse_hex(hex).unwrap_or((255, 255, 255));
    let luminance = 0.2126 * linear_channel(red)
        + 0.7152 * linear_channel(green)
        + 0.0722 * linear_channel(blue);
    if luminance > 0.45 {
        "#000000"
    } else {
        "#FFFFFF"
    }
}
/// Validate and normalize a color for an inline style. Unrecognized values
/// fall back to the derived color instead of reaching the DOM.
fn normalize_style_color(value: &str) -> Option<String> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    let expanded = match hex.len() {
        3 => hex
            .chars()
            .flat_map(|character| [character, character])
            .collect::<String>(),
        6 => hex.to_string(),
        _ => return None,
    };
    if !expanded
        .chars()
        .all(|character| character.is_ascii_hexdigit())
    {
        return None;
    }
    Some(format!("#{}", expanded.to_ascii_uppercase()))
}
fn parse_hex(value: &str) -> Option<(u8, u8, u8)> {
    let normalized = normalize_style_color(value)?;
    let hex = normalized.trim_start_matches('#');
    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((red, green, blue))
}
fn linear_channel(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.039_28 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monograms_use_one_or_two_words() {
        assert_eq!(monogram("Crypto.com"), "C");
        assert_eq!(monogram("OKX Wallet"), "OW");
        assert_eq!(monogram("Trust Wallet"), "TW");
        assert_eq!(monogram("Ledger"), "L");
        assert_eq!(monogram("  "), "?");
        assert_eq!(monogram("1inch"), "1");
    }
    #[test]
    fn derived_colors_are_stable_and_hex() {
        assert_eq!(derived_color("Ledger"), derived_color("Ledger"));
        assert!(derived_color("Ledger").starts_with('#'));
        assert_eq!(derived_color("Ledger").len(), 7);
    }
    #[test]
    fn text_color_contrasts_with_light_and_dark_backgrounds() {
        assert_eq!(readable_text_color("#FFFFFF"), "#000000");
        assert_eq!(readable_text_color("#F0B90B"), "#000000");
        assert_eq!(readable_text_color("#1D1D1B"), "#FFFFFF");
        assert_eq!(readable_text_color("#1E88E5"), "#FFFFFF");
    }
    #[test]
    fn style_colors_are_validated_and_normalized() {
        assert_eq!(normalize_style_color("#abc"), Some("#AABBCC".to_string()));
        assert_eq!(normalize_style_color("6750a4"), Some("#6750A4".to_string()));
        assert_eq!(normalize_style_color("not-a-color"), None);
    }
}
