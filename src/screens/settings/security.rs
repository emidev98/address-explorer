//! Security settings: database encryption and the configurable security policy.
use crate::components::{EncryptionSettings, PageHeader, SecuritySettingsSection};
use dioxus::prelude::*;
/// Security settings screen.
#[component]
pub fn SettingsSecurity() -> Element {
    rsx! {
        PageHeader { title: "Security".to_string() }
        div { class: "md3-settings-grid",
            section { class: "md3-settings-section md3-settings-span", EncryptionSettings {} }
            SecuritySettingsSection {}
        }
    }
}
