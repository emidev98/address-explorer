use dioxus::prelude::*;

use crate::app::Route;

/// Top-level navigation bar. Renders one entry per application screen,
/// highlights the currently active screen, and exposes the Settings sections
/// in a hover/tap dropdown attached to the Settings item.
#[component]
pub fn Topbar() -> Element {
    let current = use_route::<Route>();
    let mut settings_menu_open = use_signal(|| false);
    let settings_active = matches!(
        &current,
        Route::SettingsGeneral {} | Route::SettingsWalletTypes {} | Route::SettingsExchangeTypes {}
    );
    let general_active = current == Route::SettingsGeneral {};
    let wallet_types_active = current == Route::SettingsWalletTypes {};
    let exchange_types_active = current == Route::SettingsExchangeTypes {};

    let screens = [
        (Route::Addresses {}, "Addresses"),
        (Route::Flow {}, "Flow"),
        (Route::Exchanges {}, "Exchanges"),
        (Route::Prices {}, "Historical Prices"),
    ];

    rsx! {
        header { class: "md3-top-app-bar",
            nav {
                for (route, label) in screens {
                    {
                        let active = current == route
                            || matches!(
                                (&current, &route),
                                (Route::ExchangeDetail { .. }, Route::Exchanges {})
                            );
                        rsx! {
                            Link {
                                to: route.clone(),
                                class: if active { "active" } else { "" },
                                onclick: move |_| settings_menu_open.set(false),
                                "{label}"
                            }
                        }
                    }
                }

                div {
                    class: if settings_menu_open() { "md3-nav-dropdown is-open" } else { "md3-nav-dropdown" },
                    onkeydown: move |event| {
                        if event.key() == Key::Escape {
                            settings_menu_open.set(false);
                        }
                    },
                    div {
                        class: if settings_active { "md3-nav-dropdown-trigger active" } else { "md3-nav-dropdown-trigger" },
                        Link {
                            to: Route::SettingsGeneral {},
                            onclick: move |_| settings_menu_open.set(false),
                            "Settings"
                        }
                        button {
                            r#type: "button",
                            class: "md3-nav-dropdown-toggle",
                            aria_label: "Toggle settings sections",
                            aria_haspopup: "menu",
                            aria_expanded: settings_menu_open().to_string(),
                            aria_controls: "settings-menu",
                            onclick: move |_| settings_menu_open.set(!settings_menu_open()),
                            svg {
                                view_box: "0 0 24 24",
                                width: "20",
                                height: "20",
                                fill: "currentColor",
                                path { d: "M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6 1.41-1.41z" }
                            }
                        }
                    }

                    div {
                        id: "settings-menu",
                        class: "md3-nav-dropdown-menu",
                        role: "menu",
                        aria_label: "Settings sections",
                        Link {
                            to: Route::SettingsGeneral {},
                            class: if general_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                            role: "menuitem",
                            onclick: move |_| settings_menu_open.set(false),
                            "General"
                        }
                        Link {
                            to: Route::SettingsWalletTypes {},
                            class: if wallet_types_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                            role: "menuitem",
                            onclick: move |_| settings_menu_open.set(false),
                            "Wallet Types"
                        }
                        Link {
                            to: Route::SettingsExchangeTypes {},
                            class: if exchange_types_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                            role: "menuitem",
                            onclick: move |_| settings_menu_open.set(false),
                            "Exchange Types"
                        }
                    }
                }
            }

            if settings_menu_open() {
                div {
                    class: "md3-nav-dropdown-backdrop",
                    role: "presentation",
                    onclick: move |_| settings_menu_open.set(false),
                }
            }
        }
        main { id: "screens", Outlet::<Route> {} }
    }
}
