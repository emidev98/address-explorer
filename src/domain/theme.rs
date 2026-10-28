//! Theme preferences: the color palette and appearance mode selected by the
//! user.
//!
//! The document root receives `data-theme` and `data-mode` attributes, which
//! the Material 3 palette stylesheet uses to resolve its color roles.

use serde::{Deserialize, Serialize};

/// Color palettes provided by `assets/material3-tailwind-palettes.css`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ThemePalette {
    Ocean,
    Forest,
    Sunset,
    #[default]
    Orchid,
    GraphiteLime,
}

impl ThemePalette {
    pub const ALL: [Self; 5] = [
        Self::Ocean,
        Self::Forest,
        Self::Sunset,
        Self::Orchid,
        Self::GraphiteLime,
    ];

    /// Stable value used in SQLite and in `data-theme`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Sunset => "sunset",
            Self::Orchid => "orchid",
            Self::GraphiteLime => "graphite-lime",
        }
    }

    /// User-facing palette name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ocean => "Ocean",
            Self::Forest => "Forest",
            Self::Sunset => "Sunset",
            Self::Orchid => "Orchid",
            Self::GraphiteLime => "Graphite Lime",
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
    Dark,
    #[default]
    System,
}

impl ThemeMode {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    /// Stable value used in SQLite and in `data-mode`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    /// User-facing mode name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::System => "System",
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
    fn default_preference_is_orchid_system() {
        let preference = ThemePreference::default();
        assert_eq!(preference.palette, ThemePalette::Orchid);
        assert_eq!(preference.mode, ThemeMode::System);
    }

    #[test]
    fn palette_values_round_trip() {
        for palette in ThemePalette::ALL {
            assert_eq!(ThemePalette::from_value(palette.as_str()), Some(palette));
        }
        assert_eq!(ThemePalette::from_value("unknown"), None);
    }

    #[test]
    fn mode_values_round_trip() {
        for mode in ThemeMode::ALL {
            assert_eq!(ThemeMode::from_value(mode.as_str()), Some(mode));
        }
        assert_eq!(ThemeMode::from_value("unknown"), None);
    }
}
