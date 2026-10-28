//! Root component and top-level navigation.

use dioxus::prelude::*;

use crate::api::settings::get_theme_api;
use crate::components::theme::{apply as apply_theme, attribute_script};
use crate::components::{provide_toast, ToastHost, Topbar};
use crate::domain::theme::ThemePreference;
use crate::screens::{Addresses, ExchangeDetail, Exchanges, Flow, Prices, Settings};

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
    #[route("/settings")]
    Settings {},
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

    let theme = use_signal(|| initial_theme);
    use_effect(move || apply_theme(theme()));
    use_context_provider(|| theme);
    provide_toast();

    rsx! {
        // This runs in the document head before the app markup is parsed.
        document::Script { "{attribute_script(initial_theme)}" }
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
}
