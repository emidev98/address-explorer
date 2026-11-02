//! Settings shell. General, Wallet Types, and Exchange Types are separate
//! routes selected from the Settings dropdown in the top app bar.

mod catalog;
mod general;

pub use catalog::{SettingsExchangeTypes, SettingsWalletTypes};
pub use general::SettingsGeneral;

use dioxus::prelude::*;

use crate::app::Route;

#[component]
pub fn SettingsLayout() -> Element {
    rsx! {
        section { id: "settings", class: "md3-screen",
            Outlet::<Route> {}
        }
    }
}
