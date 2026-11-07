//! Settings endpoints.

use dioxus::prelude::*;

use crate::domain::exchange::{ExchangeSettings, OnChainLinkScoringSettings};
use crate::domain::interface_settings::{InterfaceSettings, NavigationPosition};
use crate::domain::theme::ThemePreference;

#[cfg(feature = "server")]
use super::open_database;
#[cfg(feature = "server")]
use crate::application::settings;

/// Read the persisted theme preference.
#[server]
#[get("/api/settings/theme")]
pub async fn get_theme_api() -> Result<ThemePreference, ServerFnError> {
    settings::load_theme(&open_database()?).map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist the selected theme preference.
#[server]
#[post("/api/settings/theme")]
pub async fn save_theme_api(preference: ThemePreference) -> Result<(), ServerFnError> {
    settings::save_theme(&open_database()?, &preference)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Read the persisted default fiat currency, USD when never configured.
#[server]
#[get("/api/settings/default-fiat-currency")]
pub async fn get_default_fiat_currency_api() -> Result<String, ServerFnError> {
    settings::load_default_fiat_currency(&open_database()?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist the default fiat currency, returning the normalized symbol.
#[server]
#[post("/api/settings/default-fiat-currency")]
pub async fn save_default_fiat_currency_api(symbol: String) -> Result<String, ServerFnError> {
    settings::save_default_fiat_currency(&open_database()?, &symbol)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[get("/api/settings/exchanges")]
pub async fn get_exchange_settings_api() -> Result<ExchangeSettings, ServerFnError> {
    settings::load_exchange_settings(&open_database()?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/confirm-destructive-actions")]
pub async fn save_confirm_destructive_actions_api(enabled: bool) -> Result<(), ServerFnError> {
    settings::save_confirm_destructive_actions(&open_database()?, enabled)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

#[server]
#[post("/api/settings/on-chain-link-scoring")]
pub async fn save_on_chain_link_scoring_api(
    settings: OnChainLinkScoringSettings,
) -> Result<OnChainLinkScoringSettings, ServerFnError> {
    crate::application::settings::save_on_chain_link_scoring(&open_database()?, &settings)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Read interface preferences such as date-range support-text visibility and
/// the primary-navigation placement.
#[server]
#[get("/api/settings/interface")]
pub async fn get_interface_settings_api() -> Result<InterfaceSettings, ServerFnError> {
    settings::load_interface_settings(&open_database()?)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist the date-range support-text visibility.
#[server]
#[post("/api/settings/date-range-support")]
pub async fn save_date_range_support_api(enabled: bool) -> Result<(), ServerFnError> {
    settings::save_show_date_range_support(&open_database()?, enabled)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist the primary-navigation placement.
#[server]
#[post("/api/settings/navigation-position")]
pub async fn save_navigation_position_api(
    position: NavigationPosition,
) -> Result<(), ServerFnError> {
    settings::save_navigation_position(&open_database()?, position)
        .map_err(|error| ServerFnError::new(error.to_string()))
}

/// Persist whether the side panel is compressed into an icon rail.
#[server]
#[post("/api/settings/side-nav-collapsed")]
pub async fn save_side_nav_collapsed_api(collapsed: bool) -> Result<(), ServerFnError> {
    settings::save_side_nav_collapsed(&open_database()?, collapsed)
        .map_err(|error| ServerFnError::new(error.to_string()))
}
