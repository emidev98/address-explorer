//! Theme preferences: the color palette and appearance mode selected by the
//! user.
//!
//! The document root receives `data-theme` and `data-mode` attributes, which
//! the Material 3 palette stylesheet uses to resolve its color roles.

use serde::{Deserialize, Serialize};

/// Visual themes provided by `assets/material3-tailwind-palettes.css` and the
/// per-theme type/shape overrides in `tailwind.css`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePalette {
    #[default]
    Ocean,
    Forest,
    Aurum,
    Sumi,
    Orange,
}

impl ThemePalette {
    pub const ALL: [Self; 5] = [
        Self::Ocean,
        Self::Forest,
        Self::Aurum,
        Self::Sumi,
        Self::Orange,
    ];

    /// Stable value used in SQLite and in `data-theme`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Aurum => "aurum",
            Self::Sumi => "sumi",
            Self::Orange => "orange",
        }
    }

    /// User-facing palette name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Forest => "Forest",
            Self::Aurum => "Aurum",
            Self::Sumi => "Sumi",
            Self::Orange => "Orange",
        }
    }

    /// Parse a value previously written by [`Self::as_str`].
    // Only the server-side repository parses stored values; unit tests also
    // exercise this method in client-only builds.
    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|palette| palette.as_str() == value)
    }
}

impl std::fmt::Display for ThemePalette {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Appearance mode applied through the `color-scheme` CSS property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    Light,
    #[default]
    Dark,
}

impl ThemeMode {
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    /// Stable value used in SQLite and in `data-mode`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// User-facing mode name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Parse a value previously written by [`Self::as_str`].
    // Only the server-side repository parses stored values; unit tests also
    // exercise this method in client-only builds.
    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }
}

/// Palette and appearance mode selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ThemePreference {
    pub palette: ThemePalette,
    pub mode: ThemeMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preference_is_ocean_dark() {
        let preference = ThemePreference::default();
        assert_eq!(preference.palette, ThemePalette::Ocean);
        assert_eq!(preference.mode, ThemeMode::Dark);
    }

    #[test]
    fn palette_values_round_trip() {
        for palette in ThemePalette::ALL {
            assert_eq!(ThemePalette::from_value(palette.as_str()), Some(palette));
        }
        assert_eq!(ThemePalette::from_value("unknown"), None);
        assert_eq!(ThemePalette::from_value("orchid"), None);
        assert_eq!(ThemePalette::from_value("graphite-lime"), None);
        assert_eq!(ThemePalette::from_value("sunset"), None);
        assert_eq!(ThemePalette::from_value("ledger"), None);
    }

    #[test]
    fn five_palettes_are_available() {
        assert_eq!(ThemePalette::ALL.len(), 5);
        assert_eq!(ThemePalette::Aurum.as_str(), "aurum");
        assert_eq!(ThemePalette::Sumi.as_str(), "sumi");
        assert_eq!(ThemePalette::Orange.as_str(), "orange");
    }

    #[test]
    fn palette_labels_are_unique() {
        let labels: Vec<&str> = ThemePalette::ALL
            .into_iter()
            .map(ThemePalette::label)
            .collect();
        let unique: std::collections::HashSet<&str> = labels.iter().copied().collect();
        assert_eq!(unique.len(), labels.len());
    }

    #[test]
    fn mode_values_round_trip() {
        for mode in ThemeMode::ALL {
            assert_eq!(ThemeMode::from_value(mode.as_str()), Some(mode));
        }
        assert_eq!(ThemeMode::from_value("unknown"), None);
        assert_eq!(ThemeMode::from_value("system"), None);
    }
}
