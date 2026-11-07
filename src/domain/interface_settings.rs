//! App-wide interface preferences that are not tied to a single screen.
//!
//! The persisted values live in the `setting` key/value table. A missing or
//! malformed row always falls back to [`InterfaceSettings::default`], so a
//! fresh database does not need a dedicated migration.

use serde::{Deserialize, Serialize};

/// Where the application shell places its primary navigation on wide screens.
///
/// Every variant uses the compact top bar and slide-in drawer on phones. The
/// difference starts at the tablet breakpoint: [`Self::Side`] shows the side
/// panel from 48 rem, while [`Self::Auto`] keeps the top app bar until 64 rem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NavigationPosition {
    /// Responsive: top app bar on tablets, side panel on wide windows.
    #[default]
    Auto,
    /// Always use the top app bar on tablet and wider windows.
    Top,
    /// Use the side panel on tablet and wider windows.
    Side,
}

impl NavigationPosition {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Top, Self::Side];

    /// Stable value used in SQLite and in the shell's `data-nav-position`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Top => "top",
            Self::Side => "side",
        }
    }

    /// User-facing option name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Top => "Top app bar",
            Self::Side => "Side navigation",
        }
    }

    /// Parse a value previously written by [`Self::as_str`].
    // Only the server-side repository parses stored values; unit tests also
    // exercise this method in client-only builds.
    #[cfg_attr(not(feature = "server"), allow(dead_code))]
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|position| position.as_str() == value)
    }
}

/// Presentation preferences shared by the application shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceSettings {
    /// Show the format hint and validation message below date-range fields
    /// across the application. Hidden by default so compact filter layouts
    /// stay uncluttered.
    pub show_date_range_support: bool,
    /// Primary-navigation placement on tablet and wider screens.
    #[serde(default)]
    pub navigation_position: NavigationPosition,
    /// Whether the persistent side panel is compressed into an icon rail.
    #[serde(default)]
    pub side_nav_collapsed: bool,
    /// Ask before destructive actions that remove related data.
    #[serde(default = "default_confirm_destructive_actions")]
    pub confirm_destructive_actions: bool,
}

fn default_confirm_destructive_actions() -> bool {
    true
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            show_date_range_support: false,
            navigation_position: NavigationPosition::Auto,
            side_nav_collapsed: false,
            confirm_destructive_actions: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_hidden_support_auto_navigation_and_expanded_side_panel() {
        let settings = InterfaceSettings::default();
        assert!(!settings.show_date_range_support);
        assert_eq!(settings.navigation_position, NavigationPosition::Auto);
        assert!(!settings.side_nav_collapsed);
        assert!(settings.confirm_destructive_actions);
    }

    #[test]
    fn navigation_position_values_round_trip() {
        for position in NavigationPosition::ALL {
            assert_eq!(
                NavigationPosition::from_value(position.as_str()),
                Some(position)
            );
        }
        assert_eq!(NavigationPosition::from_value("unknown"), None);
    }

    #[test]
    fn navigation_position_labels_are_user_facing() {
        assert_eq!(NavigationPosition::Auto.label(), "Auto");
        assert_eq!(NavigationPosition::Top.label(), "Top app bar");
        assert_eq!(NavigationPosition::Side.label(), "Side navigation");
    }

    #[test]
    fn missing_serialized_navigation_position_defaults_to_auto() {
        let settings: InterfaceSettings =
            serde_json::from_str(r#"{"show_date_range_support":true}"#).unwrap();
        assert!(settings.show_date_range_support);
        assert_eq!(settings.navigation_position, NavigationPosition::Auto);
        assert!(!settings.side_nav_collapsed);
        assert!(settings.confirm_destructive_actions);
    }
}
