//! Root component and top-level navigation.

use dioxus::prelude::*;

use crate::api::settings::{
    get_default_fiat_currency_api, get_interface_settings_api, get_theme_api,
};
use crate::components::theme::{apply as apply_theme, attribute_script, FontFaces};
use crate::components::{provide_toast, FiatCurrency, InterfaceSettingsContext, ToastHost, Topbar};
use crate::domain::interface_settings::InterfaceSettings;
use crate::domain::price::USD_SYMBOL;
use crate::domain::theme::ThemePreference;
use crate::screens::{
    Addresses, ExchangeDetail, Exchanges, Flow, Prices, SettingsExchangeTypes, SettingsExchanges,
    SettingsGeneral, SettingsLayout, SettingsWalletTypes,
};

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(Topbar)]
    #[route("/")]
    Addresses {},
    #[route("/flow")]
    Flow {},
    #[route("/exchanges")]
    Exchanges {},
    #[route("/exchanges/:exchange_id?:transaction_id")]
    ExchangeDetail {
        exchange_id: i64,
        transaction_id: Option<String>,
    },
    #[route("/prices")]
    Prices {},
    #[layout(SettingsLayout)]
    #[route("/settings")]
    SettingsGeneral {},
    #[route("/settings/exchanges")]
    SettingsExchanges {},
    #[route("/settings/wallet-types")]
    SettingsWalletTypes {},
    #[route("/settings/exchange-types")]
    SettingsExchangeTypes {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[component]
pub fn App() -> Element {
    // Wait for the stored preference while rendering on the server. The
    // result is serialized into the page, so the client hydrates with the
    // same theme instead of fetching it again after the first paint.
    let stored_theme = use_server_future(get_theme_api)?;
    let initial_theme = match stored_theme() {
        Some(Ok(preference)) => preference,
        Some(Err(error)) => {
            eprintln!("failed to load theme preference: {error}");
            ThemePreference::default()
        }
        None => ThemePreference::default(),
    };

    // The default fiat currency is part of the shell state: screens read the
    // same signal, so a change in Settings reaches every open surface without
    // each screen loading the setting on its own.
    let stored_currency = use_server_future(get_default_fiat_currency_api)?;
    let initial_currency = match stored_currency() {
        Some(Ok(symbol)) => symbol,
        Some(Err(error)) => {
            eprintln!("failed to load the default fiat currency: {error}");
            USD_SYMBOL.to_string()
        }
        None => USD_SYMBOL.to_string(),
    };

    // Interface preferences are also part of the shell state; General
    // Settings writes the same signal that screens and shared components read.
    let stored_interface = use_server_future(get_interface_settings_api)?;
    let initial_interface = match stored_interface() {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            eprintln!("failed to load interface settings: {error}");
            InterfaceSettings::default()
        }
        None => InterfaceSettings::default(),
    };

    let theme = use_signal(|| initial_theme);
    use_effect(move || apply_theme(theme()));
    use_context_provider(|| theme);
    let fiat_currency = use_signal(|| initial_currency);
    use_context_provider(|| FiatCurrency(fiat_currency));
    let interface_settings = use_signal(|| initial_interface);
    use_context_provider(|| InterfaceSettingsContext(interface_settings));
    provide_toast();

    rsx! {
        // This runs in the document head before the app markup is parsed.
        document::Script { "{attribute_script(initial_theme)}" }
        FontFaces {}
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        Router::<Route> {}
        // Rendered after the router so toasts layer above page content and
        // modal dialogs.
        ToastHost {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_detail_route_round_trips_the_optional_transaction_query() {
        let route = Route::ExchangeDetail {
            exchange_id: 12,
            transaction_id: Some("abc".to_string()),
        };
        assert_eq!(route.to_string(), "/exchanges/12?transaction_id=abc");
        assert_eq!(
            "/exchanges/12?transaction_id=abc".parse::<Route>().unwrap(),
            route
        );

        let without_query = Route::ExchangeDetail {
            exchange_id: 12,
            transaction_id: None,
        };
        assert_eq!(without_query.to_string(), "/exchanges/12?");
        assert_eq!("/exchanges/12".parse::<Route>().unwrap(), without_query);
    }

    #[test]
    fn settings_routes_round_trip() {
        assert_eq!(Route::SettingsGeneral {}.to_string(), "/settings");
        assert_eq!(
            Route::SettingsExchanges {}.to_string(),
            "/settings/exchanges"
        );
        assert_eq!(
            Route::SettingsWalletTypes {}.to_string(),
            "/settings/wallet-types"
        );
        assert_eq!(
            Route::SettingsExchangeTypes {}.to_string(),
            "/settings/exchange-types"
        );
        assert_eq!(
            "/settings/exchanges".parse::<Route>().unwrap(),
            Route::SettingsExchanges {}
        );
        assert_eq!(
            "/settings/wallet-types".parse::<Route>().unwrap(),
            Route::SettingsWalletTypes {}
        );
    }
}
