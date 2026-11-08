//! Settings shell. General, On-chain Linking, Wallet Types, and Exchange Types
//! are separate routes selected from the Settings group in the application
//! navigation (the top app bar dropdown, side panel, or mobile drawer).
mod catalog;
mod general;
mod on_chain_linking;
use crate::app::Route;
pub use catalog::{SettingsExchangeTypes, SettingsWalletTypes};
use dioxus::prelude::*;
pub use general::SettingsGeneral;
pub use on_chain_linking::SettingsOnChainLinking;
#[component]
pub fn SettingsLayout() -> Element {
    rsx! {
        section { id: "settings", class: "md3-screen", Outlet::<Route> {} }
    }
}
