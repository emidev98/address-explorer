//! Material 3 theme tokens for chart options.
//!
//! ECharts draws on a canvas, so chart options cannot consume the app's
//! component classes or Tailwind utilities. Instead, chart components
//! reference theme colors with these tokens — `var(--md-sys-color-*)`
//! strings, with an optional `~alpha` suffix — and theme fonts with the
//! `var(--font-*)` tokens. The ECharts host resolves them against the
//! computed page style when it applies (and re-applies) the option, keeping
//! charts in sync with the selected palette, appearance mode, and typography.
//! Chart elements that should follow the app theme MUST use these tokens
//! instead of hardcoded values.

use serde_json::{json, Value};

// Strong chromatic role, always paired with its `on-*` content color so fills
// and labels keep the contrast the palette guarantees.
pub const PRIMARY: &str = "var(--md-sys-color-primary)";
pub const ON_PRIMARY: &str = "var(--md-sys-color-on-primary)";

// Container roles for lower-emphasis fills, likewise paired.
pub const PRIMARY_CONTAINER: &str = "var(--md-sys-color-primary-container)";
pub const ON_PRIMARY_CONTAINER: &str = "var(--md-sys-color-on-primary-container)";
pub const SECONDARY_CONTAINER: &str = "var(--md-sys-color-secondary-container)";
pub const ON_SECONDARY_CONTAINER: &str = "var(--md-sys-color-on-secondary-container)";
pub const TERTIARY_CONTAINER: &str = "var(--md-sys-color-tertiary-container)";
pub const ON_TERTIARY_CONTAINER: &str = "var(--md-sys-color-on-tertiary-container)";
pub const ERROR_CONTAINER: &str = "var(--md-sys-color-error-container)";
pub const ON_ERROR_CONTAINER: &str = "var(--md-sys-color-on-error-container)";

// Neutral roles for text, axes, lines, and surfaces.
pub const SURFACE_CONTAINER: &str = "var(--md-sys-color-surface-container)";
pub const SURFACE_CONTAINER_LOW: &str = "var(--md-sys-color-surface-container-low)";
pub const ON_SURFACE: &str = "var(--md-sys-color-on-surface)";
pub const ON_SURFACE_VARIANT: &str = "var(--md-sys-color-on-surface-variant)";
pub const OUTLINE_VARIANT: &str = "var(--md-sys-color-outline-variant)";
pub const INVERSE_SURFACE: &str = "var(--md-sys-color-inverse-surface)";
pub const INVERSE_ON_SURFACE: &str = "var(--md-sys-color-inverse-on-surface)";

/// Chart text follows the active theme's UI font. The ECharts host resolves
/// these tokens through the computed document style because canvas text does
/// not inherit the document font.
pub const FONT_FAMILY: &str = "var(--font-sans)";
/// Heading face for chart titles (serif in Aurum and Sumi).
pub const FONT_HEADING: &str = "var(--font-heading)";
/// Numeric face for axis labels (monospace in Sumi).
pub const FONT_NUMERIC: &str = "var(--font-numeric)";

/// Token for one `--md-sys-color-*` role at a fixed opacity, for area fills
/// and soft backgrounds the canvas cannot express through CSS. `role` is the
/// role name without the prefix, e.g. `alpha("primary", 0.15)`.
pub fn alpha(role: &str, opacity: f64) -> String {
    format!("var(--md-sys-color-{role}~{opacity})")
}

/// Default text styling of one option: app font and readable default color.
pub fn text_style() -> Value {
    json!({
        "color": ON_SURFACE,
        "fontFamily": FONT_FAMILY,
    })
}

/// Axis labels: theme numeric face, readable secondary color, 11 px.
pub fn axis_label() -> Value {
    json!({
        "color": ON_SURFACE_VARIANT,
        "fontFamily": FONT_NUMERIC,
        "fontSize": 11,
    })
}

/// Declarative marker understood by the ECharts host: numeric tooltip values
/// are rendered with exactly two decimal places. The host turns this into a
/// `valueFormatter` function, so chart options stay serializable JSON.
pub const TOOLTIP_TWO_DECIMALS: &str = "@two-decimals";

/// Tooltip fragment styled like a Material 3 tooltip (inverse surface); the
/// chart adds its `trigger` and any formatter.
pub fn tooltip(trigger: &str) -> Value {
    json!({
        "trigger": trigger,
        "confine": true,
        "backgroundColor": INVERSE_SURFACE,
        "borderWidth": 0,
        "textStyle": {
            "color": INVERSE_ON_SURFACE,
            "fontFamily": FONT_FAMILY,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_builds_the_suffixed_token_the_host_resolves() {
        assert_eq!(alpha("primary", 0.15), "var(--md-sys-color-primary~0.15)");
    }

    #[test]
    fn tooltip_follows_the_inverse_surface_tooltip_style() {
        let tooltip = tooltip("axis");

        assert_eq!(tooltip["trigger"], "axis");
        assert_eq!(tooltip["backgroundColor"], INVERSE_SURFACE);
        assert_eq!(tooltip["textStyle"]["color"], INVERSE_ON_SURFACE);
    }

    #[test]
    fn charts_use_the_theme_font_tokens() {
        assert_eq!(FONT_FAMILY, "var(--font-sans)");
        assert_eq!(FONT_HEADING, "var(--font-heading)");
        assert_eq!(FONT_NUMERIC, "var(--font-numeric)");
        assert_eq!(text_style()["fontFamily"], FONT_FAMILY);
        assert_eq!(tooltip("axis")["textStyle"]["fontFamily"], FONT_FAMILY);
    }

    #[test]
    fn axis_labels_use_the_numeric_face_and_secondary_color() {
        let label = axis_label();

        assert_eq!(label["fontFamily"], FONT_NUMERIC);
        assert_eq!(label["color"], ON_SURFACE_VARIANT);
        assert_eq!(label["fontSize"], 11);
    }
}
