//! Root component and top-level navigation.

use dioxus::prelude::*;

use crate::api::settings::get_theme_api;
use crate::components::theme::{apply as apply_theme, attribute_script};
use crate::components::{provide_toast, ToastHost, Topbar};
use crate::domain::theme::ThemePreference;
use crate::screens::{Addresses, Exchanges, Flow, Prices, Settings};

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
