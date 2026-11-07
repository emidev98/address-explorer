//! Settings shell. General, Exchanges, Wallet Types, and Exchange Types are
//! separate routes selected from the Settings group in the application
//! navigation (the top app bar dropdown, side panel, or mobile drawer).

mod catalog;
mod exchanges;
mod general;

pub use catalog::{SettingsExchangeTypes, SettingsWalletTypes};
pub use exchanges::SettingsExchanges;
pub use general::SettingsGeneral;

use dioxus::prelude::*;

use crate::app::Route;

#[component]
pub fn SettingsLayout() -> Element {
    rsx! {
        section { id: "settings", class: "md3-screen", Outlet::<Route> {} }
    }
}
