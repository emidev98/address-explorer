//! Presentation-side helpers that apply a [`ThemePreference`] to the
//! document. The types themselves live in `domain::theme`.

use dioxus::prelude::*;

use crate::domain::theme::ThemePreference;

// Fonts are bundled so every platform renders the same typography. The asset
// URLs are content-hashed, so `@font-face` rules cannot point at a stable
// `/assets/fonts/...` path; [`FontFaces`] injects the rules through
// `document::Style` with the URLs the asset system generated.
const FONT_ROBOTO: Asset = asset!("/assets/fonts/roboto-variable.woff2");
const FONT_FRAUNCES: Asset = asset!("/assets/fonts/fraunces-variable.woff2");
const FONT_INTER: Asset = asset!("/assets/fonts/inter-variable.woff2");
const FONT_PLEX_MONO_REGULAR: Asset = asset!("/assets/fonts/ibm-plex-mono-400.woff2");
const FONT_PLEX_MONO_MEDIUM: Asset = asset!("/assets/fonts/ibm-plex-mono-500.woff2");

/// Render the bundled `@font-face` declarations once in the document head.
#[component]
pub fn FontFaces() -> Element {
    rsx! {
        document::Style { {font_face_css()} }
    }
}

/// Build the `@font-face` rules for the bundled families.
///
/// Roboto, Fraunces, and Inter are variable fonts, so one face covers every
/// weight. IBM Plex Mono is shipped as two static weights because that is how
/// the package publishes its Latin subset.
fn font_face_css() -> String {
    format!(
        r#"
@font-face {{
  font-family: "Roboto";
  src: url("{FONT_ROBOTO}") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}}
@font-face {{
  font-family: "Fraunces";
  src: url("{FONT_FRAUNCES}") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}}
@font-face {{
  font-family: "Inter";
  src: url("{FONT_INTER}") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}}
@font-face {{
  font-family: "IBM Plex Mono";
  src: url("{FONT_PLEX_MONO_REGULAR}") format("woff2");
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}}
@font-face {{
  font-family: "IBM Plex Mono";
  src: url("{FONT_PLEX_MONO_MEDIUM}") format("woff2");
  font-weight: 500;
  font-style: normal;
  font-display: swap;
}}
"#
    )
}

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
            palette: ThemePalette::Forest,
            mode: ThemeMode::Dark,
        });

        assert!(script.contains("dataset.theme = 'forest'"));
        assert!(script.contains("dataset.mode = 'dark'"));
    }

    #[test]
    fn font_faces_declare_the_bundled_families() {
        let css = font_face_css();

        assert!(css.contains("font-family: \"Roboto\""));
        assert!(css.contains("font-family: \"Fraunces\""));
        assert!(css.contains("font-family: \"Inter\""));
        assert!(css.contains("font-family: \"IBM Plex Mono\""));
        assert!(css.contains("font-weight: 100 900"));
        assert_eq!(css.matches("font-family: \"IBM Plex Mono\"").count(), 2);
    }
}
