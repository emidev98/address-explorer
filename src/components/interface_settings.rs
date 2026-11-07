//! Shared interface preferences loaded once by the application root.
//!
//! General Settings updates the same signal every screen and shared component
//! reads, so a toggle such as the date-range support text takes effect without
//! another fetch.

use dioxus::prelude::*;

use crate::domain::interface_settings::InterfaceSettings;

/// App-wide interface preferences, provided once by the root component.
#[derive(Clone, Copy)]
pub struct InterfaceSettingsContext(pub Signal<InterfaceSettings>);

impl InterfaceSettingsContext {
    pub fn get(self) -> InterfaceSettings {
        self.0()
    }

    pub fn set(self, settings: InterfaceSettings) {
        let mut signal = self.0;
        signal.set(settings);
    }

    pub fn show_date_range_support(self) -> bool {
        self.get().show_date_range_support
    }
}

/// Access the app-wide interface preferences.
pub fn use_interface_settings() -> InterfaceSettingsContext {
    use_context()
}
