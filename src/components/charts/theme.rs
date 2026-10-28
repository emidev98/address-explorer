//! Material 3 theme color tokens for chart options.
//!
//! ECharts draws on a canvas, so chart options cannot consume the app's
//! component classes or Tailwind utilities. Instead, chart components
//! reference theme colors with these tokens — `var(--md-sys-color-*)`
//! strings, with an optional `~alpha` suffix — and the ECharts host resolves
//! them against the computed page style when it applies (and re-applies) the
//! option, keeping charts in sync with the selected palette and the light or
//! dark appearance mode. Chart elements that should follow the app theme
//! MUST use these tokens instead of hardcoded hex colors.

use serde_json::{json, Value};

// Strong chromatic roles, always paired with their `on-*` content color so
// fills and labels keep the contrast the palette guarantees.
pub const PRIMARY: &str = "var(--md-sys-color-primary)";
pub const ON_PRIMARY: &str = "var(--md-sys-color-on-primary)";
pub const SECONDARY: &str = "var(--md-sys-color-secondary)";
pub const ON_SECONDARY: &str = "var(--md-sys-color-on-secondary)";
pub const TERTIARY: &str = "var(--md-sys-color-tertiary)";
pub const ON_TERTIARY: &str = "var(--md-sys-color-on-tertiary)";

// Container roles for lower-emphasis fills, likewise paired.
pub const PRIMARY_CONTAINER: &str = "var(--md-sys-color-primary-container)";
pub const ON_PRIMARY_CONTAINER: &str = "var(--md-sys-color-on-primary-container)";
pub const SECONDARY_CONTAINER: &str = "var(--md-sys-color-secondary-container)";
pub const ON_SECONDARY_CONTAINER: &str = "var(--md-sys-color-on-secondary-container)";
pub const TERTIARY_CONTAINER: &str = "var(--md-sys-color-tertiary-container)";
pub const ON_TERTIARY_CONTAINER: &str = "var(--md-sys-color-on-tertiary-container)";

// Neutral roles for text, axes, lines, and surfaces.
pub const SURFACE: &str = "var(--md-sys-color-surface)";
pub const SURFACE_CONTAINER: &str = "var(--md-sys-color-surface-container)";
pub const SURFACE_CONTAINER_LOW: &str = "var(--md-sys-color-surface-container-low)";
pub const ON_SURFACE: &str = "var(--md-sys-color-on-surface)";
pub const ON_SURFACE_VARIANT: &str = "var(--md-sys-color-on-surface-variant)";
pub const OUTLINE: &str = "var(--md-sys-color-outline)";
pub const OUTLINE_VARIANT: &str = "var(--md-sys-color-outline-variant)";
pub const INVERSE_SURFACE: &str = "var(--md-sys-color-inverse-surface)";
pub const INVERSE_ON_SURFACE: &str = "var(--md-sys-color-inverse-on-surface)";

/// Chart text matches the app's `--font-sans` stack (canvas text does not
/// inherit the document font).
pub const FONT_FAMILY: &str = "Roboto, \"Roboto Flex\", \"Segoe UI\", system-ui, sans-serif";

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
}
