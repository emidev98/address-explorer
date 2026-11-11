//! Root component and top-level navigation.
use crate::api::settings::{
    get_default_fiat_currency_api, get_interface_settings_api, get_theme_api,
};
use crate::components::theme::{
    apply as apply_theme, attribute_script, default_attribute_script, FontFaces,
};
use crate::components::{
    use_toast, use_toast_provider, AppShell, BackendSettingsContext, FiatCurrency,
    InterfaceSettingsContext, ToastHost, VaultGate,
};
use crate::domain::interface_settings::InterfaceSettings;
use crate::domain::price::USD_SYMBOL;
use crate::domain::theme::ThemePreference;
use crate::screens::{
    Addresses, ExchangeAccountDetail, Exchanges, FundsFlow, HistoricalPrices,
    SettingsExchangeTypes, SettingsGeneral, SettingsLayout, SettingsOnChainLinking,
    SettingsWalletTypes,
};
use dioxus::prelude::*;
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
pub enum Route {
    #[layout(AppShell)]
    #[route("/")]
    Addresses {},
    #[route("/funds-flow")]
    FundsFlow {},
    #[route("/exchanges")]
    Exchanges {},
    #[route("/exchanges/:account_id?:transaction_id")]
    ExchangeAccountDetail { account_id: i64, transaction_id: Option<String> },
    #[route("/historical-prices")]
    HistoricalPrices {},
    #[layout(SettingsLayout)]
    #[route("/settings")]
    SettingsGeneral {},
    #[route("/settings/on-chain-linking")]
    SettingsOnChainLinking {},
    #[route("/settings/wallet-types")]
    SettingsWalletTypes {},
    #[route("/settings/exchange-types")]
    SettingsExchangeTypes {},
}
const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

/// Application root: provides the backend preference and owns the mobile
/// hard-reload remount. Desktop/web perform a process/page reload instead.
#[cfg(not(all(target_arch = "wasm32", feature = "web-client")))]
#[component]
pub fn Root() -> Element {
    rsx! {
        DocumentHead {}
        RootContent {}
    }
}

/// Web root: initialize the browser worker or verify the remote server before
/// mounting any screen that reads data.
#[cfg(all(target_arch = "wasm32", feature = "web-client"))]
#[component]
pub fn Root() -> Element {
    rsx! {
        DocumentHead {}
        WebBootGate {}
    }
}

/// Web boot gate: initialize the browser worker or verify the remote server
/// before mounting any screen that reads data.
#[cfg(all(target_arch = "wasm32", feature = "web-client"))]
#[component]
fn WebBootGate() -> Element {
    use crate::backend::{self, BackendConfig, BackendMode};

    #[derive(Clone, PartialEq)]
    enum BootState {
        Loading,
        Ready,
        Failed(String),
    }

    let mut state = use_signal(|| BootState::Loading);
    let mut remote_url = use_signal(|| backend::config().base_url.clone().unwrap_or_default());
    use_future(move || async move {
        match crate::web::boot::boot().await {
            Ok(()) => state.set(BootState::Ready),
            Err(error) => state.set(BootState::Failed(error)),
        }
    });

    match state() {
        BootState::Ready => rsx! {
            RootContent {}
        },
        BootState::Loading => rsx! {
            div {
                class: "md3-screen",
                style: "display: flex; min-height: 100vh; align-items: center; justify-content: center;",
                div { role: "status", aria_live: "polite",
                    h1 { class: "text-headline-small", "Preparing Address Explorer" }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Opening the backend and checking the API version…"
                    }
                }
            }
        },
        BootState::Failed(error) => rsx! {
            div {
                class: "md3-screen",
                style: "display: flex; min-height: 100vh; align-items: center; justify-content: center;",
                div { style: "max-width: 42rem;",
                    h1 { class: "text-headline-small", "Address Explorer could not start" }
                    p {
                        class: "text-body-medium",
                        style: "color: var(--md-sys-color-error);",
                        role: "alert",
                        "{error}"
                    }
                    div { class: "md3-chip-group",
                        button {
                            r#type: "button",
                            class: "md3-button-filled",
                            onclick: move |_| {
                                state.set(BootState::Loading);
                                spawn(async move {
                                    match crate::web::boot::boot().await {
                                        Ok(()) => state.set(BootState::Ready),
                                        Err(error) => state.set(BootState::Failed(error)),
                                    }
                                });
                            },
                            "Retry"
                        }
                        if backend::is_remote() {
                            button {
                                r#type: "button",
                                class: "md3-button-outlined",
                                onclick: move |_| {
                                    let config = BackendConfig {
                                        version: 1,
                                        mode: BackendMode::Local,
                                        base_url: backend::config().base_url,
                                    };
                                    if backend::save_config(&config).is_ok() {
                                        backend::set_config(config);
                                        let _ = backend::hard_reload();
                                    }
                                },
                                "Use local database"
                            }
                        }
                    }
                    if !backend::is_remote() {
                        p { class: "text-body-small text-on-surface-variant",
                            "Local mode needs a current 2026+ browser with SharedWorker. You can point this app at a remote Address Explorer server instead."
                        }
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                r#type: "url",
                                placeholder: "https://addresses.example.com",
                                value: "{remote_url}",
                                oninput: move |event| remote_url.set(event.value()),
                            }
                        }
                        button {
                            r#type: "button",
                            class: "md3-button-outlined",
                            onclick: move |_| {
                                let config = BackendConfig {
                                    version: 1,
                                    mode: BackendMode::Remote,
                                    base_url: Some(remote_url()),
                                };
                                match backend::save_config(&config) {
                                    Ok(()) => {
                                        backend::set_config(config);
                                        let _ = backend::hard_reload();
                                    }
                                    Err(error) => state.set(BootState::Failed(error)),
                                }
                            },
                            "Use remote server"
                        }
                    }
                }
            }
        },
    }
}

/// Static document resources mounted outside the web boot gate and vault gate.
/// They must stay in the document while the loading, failure, and locked
/// screens are shown, because those screens render instead of `App`.
#[component]
fn DocumentHead() -> Element {
    let register_service_worker = cfg!(all(target_arch = "wasm32", not(debug_assertions)));
    rsx! {
        document::Script { "{default_attribute_script()}" }
        document::Link { rel: "manifest", href: "/manifest.webmanifest" }
        document::Link { rel: "apple-touch-icon", href: "/icons/apple-touch-icon.png" }
        document::Meta { name: "theme-color", content: "#005a5a" }
        document::Meta { name: "apple-mobile-web-app-capable", content: "yes" }
        document::Meta {
            name: "apple-mobile-web-app-status-bar-style",
            content: "black-translucent",
        }
        FontFaces {}
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        if register_service_worker {
            document::Script { src: "/sw-register.js" }
        }
    }
}

/// Shared root content: backend preference context, reload state, and the app.
#[component]
fn RootContent() -> Element {
    let config = use_signal(crate::backend::config);
    let reload_generation = use_signal(|| 0u32);
    let reloading = use_signal(|| false);
    use_context_provider(|| BackendSettingsContext {
        config,
        reload_generation,
        reloading,
    });

    use_effect(move || {
        if reloading() {
            let mut reloading = reloading;
            spawn(async move {
                let mut eval = document::eval("setTimeout(() => dioxus.send(true), 250)");
                let _ = eval.recv::<bool>().await;
                reloading.set(false);
            });
        }
    });

    rsx! {
        VaultGate { generation: reload_generation() }
        if reloading() {
            div {
                style: "position: fixed; inset: 0; z-index: 50; display: flex; align-items: center; justify-content: center; background: var(--md-sys-color-surface); color: var(--md-sys-color-on-surface);",
                role: "status",
                aria_live: "polite",
                "Reloading…"
            }
        }
    }
}

#[component]
pub fn App() -> Element {
    #[cfg(not(all(target_arch = "wasm32", feature = "web-client")))]
    let stored_theme = use_server_future(get_theme_api)?;
    #[cfg(all(target_arch = "wasm32", feature = "web-client"))]
    let stored_theme = use_resource(get_theme_api);
    let initial_theme = match stored_theme() {
        Some(Ok(preference)) => preference,
        Some(Err(error)) => {
            eprintln!("failed to load theme preference: {error}");
            ThemePreference::default()
        }
        None => ThemePreference::default(),
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-client")))]
    let stored_currency = use_server_future(get_default_fiat_currency_api)?;
    #[cfg(all(target_arch = "wasm32", feature = "web-client"))]
    let stored_currency = use_resource(get_default_fiat_currency_api);
    let initial_currency = match stored_currency() {
        Some(Ok(symbol)) => symbol,
        Some(Err(error)) => {
            eprintln!("failed to load the default fiat currency: {error}");
            USD_SYMBOL.to_string()
        }
        None => USD_SYMBOL.to_string(),
    };
    #[cfg(not(all(target_arch = "wasm32", feature = "web-client")))]
    let stored_interface = use_server_future(get_interface_settings_api)?;
    #[cfg(all(target_arch = "wasm32", feature = "web-client"))]
    let stored_interface = use_resource(get_interface_settings_api);
    let initial_interface = match stored_interface() {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            eprintln!("failed to load interface settings: {error}");
            InterfaceSettings::default()
        }
        None => InterfaceSettings::default(),
    };
    let mut theme = use_signal(|| initial_theme);
    let mut theme_loaded = use_signal(|| false);
    use_effect(move || {
        if !theme_loaded() {
            if let Some(Ok(preference)) = stored_theme() {
                theme.set(preference);
                theme_loaded.set(true);
            }
        }
    });
    use_effect(move || apply_theme(theme()));
    use_context_provider(|| theme);
    let mut fiat_currency = use_signal(|| initial_currency);
    let mut currency_loaded = use_signal(|| false);
    use_effect(move || {
        if !currency_loaded() {
            if let Some(Ok(symbol)) = stored_currency() {
                fiat_currency.set(symbol);
                currency_loaded.set(true);
            }
        }
    });
    use_context_provider(|| FiatCurrency(fiat_currency));
    let mut interface_settings = use_signal(|| initial_interface);
    let mut interface_loaded = use_signal(|| false);
    use_effect(move || {
        if !interface_loaded() {
            if let Some(Ok(settings)) = stored_interface() {
                interface_settings.set(settings);
                interface_loaded.set(true);
            }
        }
    });
    use_context_provider(|| InterfaceSettingsContext(interface_settings));
    use_toast_provider();
    let mut toast = use_toast();
    use_effect(move || {
        if let Some(alert) = crate::backend::take_startup_alert() {
            toast.error(alert.message());
        }
    });
    use_future(move || async move {
        #[cfg(all(feature = "remote-backend", not(target_arch = "wasm32")))]
        {
            loop {
                if crate::backend::is_remote() {
                    if let Ok(health) = crate::api::remote::health_at_current().await {
                        let server_version = health.api_version;
                        let client_version = crate::domain::health::API_VERSION;
                        if server_version != client_version {
                            let url = crate::backend::remote_base_url().unwrap_or_default();
                            let alert = crate::backend::BackendAlert::Blocked {
                                message: format!(
                                    "Remote backend API version {server_version} at {url} no longer matches this app (API {client_version}). Remote calls are blocked; switch to the local backend or a compatible server in Settings → General."
                                ),
                            };
                            crate::backend::block_remote(alert.clone());
                            toast.error(alert.message());
                            break;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(health) = crate::api::health::health_api().await {
                let server_version = health.api_version;
                let client_version = crate::domain::health::API_VERSION;
                if server_version != client_version {
                    let alert = crate::backend::BackendAlert::Blocked {
                        message: format!(
                            "API version mismatch: this client uses API {client_version}, the server uses API {server_version}. Remote calls are blocked; use the local database or a compatible server."
                        ),
                    };
                    crate::backend::block_remote(alert.clone());
                    toast.error(alert.message());
                }
            }
        }
    });
    rsx! {
        document::Script { "{attribute_script(initial_theme)}" }
        Router::<Route> {}
        ToastHost {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exchange_account_detail_route_round_trips_the_optional_transaction_query() {
        let route = Route::ExchangeAccountDetail {
            account_id: 12,
            transaction_id: Some("abc".to_string()),
        };
        assert_eq!(route.to_string(), "/exchanges/12?transaction_id=abc");
        assert_eq!(
            "/exchanges/12?transaction_id=abc".parse::<Route>().unwrap(),
            route
        );
        let without_query = Route::ExchangeAccountDetail {
            account_id: 12,
            transaction_id: None,
        };
        assert_eq!(without_query.to_string(), "/exchanges/12?");
        assert_eq!("/exchanges/12".parse::<Route>().unwrap(), without_query);
    }
    #[test]
    fn settings_routes_round_trip() {
        assert_eq!(Route::SettingsGeneral {}.to_string(), "/settings");
        assert_eq!(
            Route::SettingsOnChainLinking {}.to_string(),
            "/settings/on-chain-linking",
        );
        assert_eq!(
            Route::SettingsWalletTypes {}.to_string(),
            "/settings/wallet-types"
        );
        assert_eq!(
            Route::SettingsExchangeTypes {}.to_string(),
            "/settings/exchange-types",
        );
        assert_eq!(
            "/settings/on-chain-linking".parse::<Route>().unwrap(),
            Route::SettingsOnChainLinking {},
        );
        assert_eq!(
            "/settings/wallet-types".parse::<Route>().unwrap(),
            Route::SettingsWalletTypes {},
        );
        assert_eq!(Route::FundsFlow {}.to_string(), "/funds-flow");
        assert_eq!(Route::HistoricalPrices {}.to_string(), "/historical-prices");
    }
}
