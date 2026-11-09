//! Browser wrappers for the settings endpoints.

use crate::domain::exchange::OnChainLinkScoringSettings;
use crate::domain::interface_settings::{InterfaceSettings, NavigationPosition};
use crate::domain::theme::ThemePreference;
use dioxus_fullstack_core::ServerFnError;

pub async fn get_theme_api() -> Result<ThemePreference, ServerFnError> {
    crate::web_get_query!("settings.get_theme", "/api/settings/theme", {})
}

pub async fn save_theme_api(preference: ThemePreference) -> Result<(), ServerFnError> {
    crate::web_post_body!("settings.save_theme", "/api/settings/theme", { "preference": preference })
}

pub async fn get_default_fiat_currency_api() -> Result<String, ServerFnError> {
    crate::web_get_query!(
        "settings.get_default_fiat_currency",
        "/api/settings/default-fiat-currency",
        {}
    )
}

pub async fn save_default_fiat_currency_api(symbol: String) -> Result<String, ServerFnError> {
    crate::web_post_body!(
        "settings.save_default_fiat_currency",
        "/api/settings/default-fiat-currency",
        { "symbol": symbol }
    )
}

pub async fn get_on_chain_link_scoring_api() -> Result<OnChainLinkScoringSettings, ServerFnError> {
    crate::web_get_query!(
        "settings.get_on_chain_link_scoring",
        "/api/settings/on-chain-link-scoring",
        {}
    )
}

pub async fn save_confirm_destructive_actions_api(enabled: bool) -> Result<(), ServerFnError> {
    crate::web_post_body!(
        "settings.save_confirm_destructive_actions",
        "/api/settings/confirm-destructive-actions",
        { "enabled": enabled }
    )
}

pub async fn save_on_chain_link_scoring_api(
    settings: OnChainLinkScoringSettings,
) -> Result<OnChainLinkScoringSettings, ServerFnError> {
    crate::web_post_body!(
        "settings.save_on_chain_link_scoring",
        "/api/settings/on-chain-link-scoring",
        { "settings": settings }
    )
}

pub async fn get_interface_settings_api() -> Result<InterfaceSettings, ServerFnError> {
    crate::web_get_query!("settings.get_interface", "/api/settings/interface", {})
}

pub async fn save_show_date_range_support_api(enabled: bool) -> Result<(), ServerFnError> {
    crate::web_post_body!(
        "settings.save_show_date_range_support",
        "/api/settings/date-range-support",
        { "enabled": enabled }
    )
}

pub async fn save_navigation_position_api(
    position: NavigationPosition,
) -> Result<(), ServerFnError> {
    crate::web_post_body!(
        "settings.save_navigation_position",
        "/api/settings/navigation-position",
        { "position": position }
    )
}

pub async fn save_side_nav_collapsed_api(collapsed: bool) -> Result<(), ServerFnError> {
    crate::web_post_body!(
        "settings.save_side_nav_collapsed",
        "/api/settings/side-nav-collapsed",
        { "collapsed": collapsed }
    )
}
