//! Settings shell. General, On-chain Linking, Wallet Types, and Exchange Types
//! are separate routes selected from the Settings group in the application
//! navigation (the top app bar dropdown, side panel, or mobile drawer).

mod catalog;
mod general;
mod on_chain_linking;

pub use catalog::{SettingsExchangeTypes, SettingsWalletTypes};
pub use general::SettingsGeneral;
pub use on_chain_linking::SettingsOnChainLinking;

use dioxus::prelude::*;

use crate::app::Route;

#[component]
pub fn SettingsLayout() -> Element {
    rsx! {
        section { id: "settings", class: "md3-screen", Outlet::<Route> {} }
    }
}
