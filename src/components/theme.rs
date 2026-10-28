//! Presentation-side helpers that apply a [`ThemePreference`] to the
//! document. The types themselves live in `domain::theme`.

use dioxus::prelude::*;

use crate::domain::theme::ThemePreference;

/// Build the inline script that selects the palette and appearance mode.
///
/// The script is safe to render in the document head because both values are
/// fixed identifiers from the enums above. Running it before the body is
/// parsed prevents a flash of the default theme when a stored preference is
/// different.
pub fn attribute_script(preference: ThemePreference) -> String {
    format!(
        "document.documentElement.dataset.theme = '{}'; \
         document.documentElement.dataset.mode = '{}';",
        preference.palette.as_str(),
        preference.mode.as_str()
    )
}

/// Apply a preference to the document root after the app has loaded.
pub fn apply(preference: ThemePreference) {
    document::eval(&attribute_script(preference));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::theme::{ThemeMode, ThemePalette};

    #[test]
    fn attribute_script_sets_palette_and_mode() {
        let script = attribute_script(ThemePreference {
            palette: ThemePalette::GraphiteLime,
            mode: ThemeMode::Dark,
        });

        assert!(script.contains("dataset.theme = 'graphite-lime'"));
        assert!(script.contains("dataset.mode = 'dark'"));
    }
}
