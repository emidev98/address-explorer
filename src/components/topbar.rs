use dioxus::prelude::*;

use crate::api::settings::save_side_nav_collapsed_api;
use crate::app::Route;
use crate::components::{use_interface_settings, use_toast, InterfaceSettingsContext, Toast};

const ICON_ADDRESSES: &str = "M21 18v1c0 1.1-.9 2-2 2H5c-1.11 0-2-.9-2-2V5c0-1.1.89-2 2-2h14c1.1 0 2 .9 2 2v1h-9c-1.11 0-2 .9-2 2v8c0 1.1.89 2 2 2h9zm-9-2h10V8H12v8zm4-2.5c-.83 0-1.5-.67-1.5-1.5s.67-1.5 1.5-1.5 1.5.67 1.5 1.5-.67 1.5-1.5 1.5z";
const ICON_FLOW: &str = "M6.99 11 3 15l3.99 4v-3H14v-2H6.99v-3zM21 9l-3.99-4v3H10v2h7.01v3L21 9z";
const ICON_EXCHANGES: &str =
    "M4 10h3v7H4zM10.5 10h3v7h-3zM2 19h20v3H2zM17 10h3v7h-3zM12 1 2 6v2h20V6z";
const ICON_PRICES: &str = "M3.5 18.49l6-6.01 4 4L22 6.92l-1.41-1.41-7.09 7.97-4-4L2 16.99z";
const ICON_SETTINGS: &str = "M19.43 12.98c.04-.32.07-.64.07-.98s-.03-.66-.07-.98l2.11-1.65c.19-.15.24-.42.12-.64l-2-3.46c-.12-.22-.39-.3-.61-.22l-2.49 1c-.52-.4-1.08-.73-1.69-.98l-.38-2.65C14.46 2.18 14.25 2 14 2h-4c-.25 0-.46.18-.49.42l-.38 2.65c-.61.25-1.17.59-1.69.98l-2.49-1c-.23-.09-.49 0-.61.22l-2 3.46c-.13.22-.07.49.12.64l2.11 1.65c-.04.32-.07.65-.07.98s.03.66.07.98l-2.11 1.65c-.19.15-.24.42-.12.64l2 3.46c.12.22.39.3.61.22l2.49-1c.52.4 1.08.73 1.69.98l.38 2.65c.03.24.24.42.49.42h4c.25 0 .46-.18.49-.42l.38-2.65c.61-.25 1.17-.59 1.69-.98l2.49 1c.23.09.49 0 .61-.22l2-3.46c.12-.22.07-.49-.12-.64l-2.11-1.65zM12 15.5c-1.93 0-3.5-1.57-3.5-3.5s1.57-3.5 3.5-3.5 3.5 1.57 3.5 3.5-1.57 3.5-3.5 3.5z";
const ICON_CHEVRON_LEFT: &str = "M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z";
const ICON_CHEVRON_RIGHT: &str = "M8.59 16.59 10 18l6-6-6-6-1.41 1.41L13.17 12z";

/// One icon shown by a navigation destination.
fn nav_icon(path: &'static str) -> Element {
    rsx! {
        svg {
            class: "md3-nav-icon",
            view_box: "0 0 24 24",
            width: "24",
            height: "24",
            fill: "currentColor",
            path { d: path }
        }
    }
}

/// The destinations shown in every navigation presentation.
fn screens() -> [(Route, &'static str, &'static str); 4] {
    [
        (Route::Addresses {}, "Addresses", ICON_ADDRESSES),
        (Route::Flow {}, "Flow", ICON_FLOW),
        (Route::Exchanges {}, "Exchanges", ICON_EXCHANGES),
        (Route::Prices {}, "Historical Prices", ICON_PRICES),
    ]
}

/// Settings subsections shown in the side panel, compact flyout, and drawer.
fn settings_sections() -> [(Route, &'static str); 4] {
    [
        (Route::SettingsGeneral {}, "General"),
        (Route::SettingsExchanges {}, "Exchanges"),
        (Route::SettingsWalletTypes {}, "Wallet Types"),
        (Route::SettingsExchangeTypes {}, "Exchange Types"),
    ]
}

fn destination_is_active(current: &Route, route: &Route) -> bool {
    current == route
        || matches!(
            (current, route),
            (Route::ExchangeDetail { .. }, Route::Exchanges {})
        )
}

fn settings_is_active(current: &Route) -> bool {
    matches!(
        current,
        Route::SettingsGeneral {}
            | Route::SettingsExchanges {}
            | Route::SettingsWalletTypes {}
            | Route::SettingsExchangeTypes {}
    )
}

/// Title shown in the compact top bar on phones.
fn current_title(current: &Route) -> &'static str {
    match current {
        Route::Addresses {} => "Addresses",
        Route::Flow {} => "Flow",
        Route::Exchanges {} | Route::ExchangeDetail { .. } => "Exchanges",
        Route::Prices {} => "Historical Prices",
        Route::SettingsGeneral {}
        | Route::SettingsExchanges {}
        | Route::SettingsWalletTypes {}
        | Route::SettingsExchangeTypes {} => "Settings",
    }
}

/// Close the mobile drawer and request focus restoration to its toggle.
fn close_mobile_navigation(mut open: Signal<bool>, mut restore_focus: Signal<bool>) {
    open.set(false);
    restore_focus.set(true);
}

/// Close the compact Settings flyout and request focus restoration to its gear.
fn close_compact_settings(mut open: Signal<bool>, mut restore_focus: Signal<bool>) {
    open.set(false);
    restore_focus.set(true);
}

/// Toggle the side panel, applying the preference immediately and restoring
/// it if the persistence request fails.
fn persist_side_nav_collapsed(
    interface_settings: InterfaceSettingsContext,
    mut saving: Signal<bool>,
    mut toast: Toast,
    collapsed: bool,
) {
    let previous = interface_settings.get();
    if previous.side_nav_collapsed == collapsed {
        return;
    }

    let mut next = previous;
    next.side_nav_collapsed = collapsed;
    interface_settings.set(next);
    saving.set(true);

    spawn(async move {
        match save_side_nav_collapsed_api(collapsed).await {
            Ok(()) => toast.success(if collapsed {
                "Side panel compressed."
            } else {
                "Side panel expanded."
            }),
            Err(error) => {
                interface_settings.set(previous);
                toast.error(format!(
                    "Could not save the side panel state: {error}. The previous value was restored."
                ));
            }
        }
        saving.set(false);
    });
}

/// One destination link. Expanded presentations show an icon and label;
/// compact presentations show the icon only, with an accessible label and a
/// native tooltip.
#[component]
fn NavLink(
    to: Route,
    label: String,
    icon: Option<&'static str>,
    active: bool,
    compact: bool,
    nested: bool,
    on_navigate: EventHandler<()>,
) -> Element {
    let mut class = String::from("md3-nav-panel-link");
    if nested {
        class.push_str(" md3-nav-panel-link-nested");
    }
    if active {
        class.push_str(" active");
    }
    let title = compact.then(|| label.clone());

    rsx! {
        Link {
            to,
            class,
            aria_label: label.clone(),
            title,
            aria_current: if active { "page" } else { "false" },
            onclick: move |_| on_navigate.call(()),
            if let Some(icon) = icon {
                {nav_icon(icon)}
            }
            if !compact {
                span { class: "md3-nav-panel-link-text", "{label}" }
            }
        }
    }
}

/// Full navigation: destinations at the top and Settings beneath them.
/// `settings_bottom` pins the Settings group to the bottom of the side panel;
/// the mobile drawer keeps the settings directly after the destinations.
#[component]
fn FullNavPanel(current: Route, settings_bottom: bool, on_navigate: EventHandler<()>) -> Element {
    let settings_active = settings_is_active(&current);
    let class = if settings_bottom {
        "md3-nav-panel md3-nav-panel-settings-bottom"
    } else {
        "md3-nav-panel"
    };

    rsx! {
        nav { class, aria_label: "Primary",
            div { class: "md3-nav-panel-primary",
                for (route, label, icon) in screens() {
                    {
                        let active = destination_is_active(&current, &route);
                        rsx! {
                            NavLink {
                                to: route,
                                label: label.to_string(),
                                icon: Some(icon),
                                active,
                                compact: false,
                                nested: false,
                                on_navigate,
                            }
                        }
                    }
                }
            }
            div { class: "md3-nav-panel-settings",
                p { class: if settings_active { "md3-nav-panel-label active" } else { "md3-nav-panel-label" },
                    {nav_icon(ICON_SETTINGS)}
                    span { "Settings" }
                }
                for (route, label) in settings_sections() {
                    {
                        let active = current == route;
                        rsx! {
                            NavLink {
                                to: route,
                                label: label.to_string(),
                                icon: None,
                                active,
                                compact: false,
                                nested: true,
                                on_navigate,
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Compact icon-rail navigation. Settings is a single gear at the bottom; its
/// flyout is rendered by the shell so the scrolling panel cannot clip it.
#[component]
fn CompactNavPanel(
    current: Route,
    settings_menu_open: bool,
    on_toggle_settings: EventHandler<()>,
    on_close_settings: EventHandler<()>,
    on_navigate: EventHandler<()>,
) -> Element {
    let settings_active = settings_is_active(&current);

    rsx! {
        nav {
            class: "md3-nav-panel md3-nav-panel-compact",
            aria_label: "Primary",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close_settings.call(());
                }
            },
            div { class: "md3-nav-panel-primary",
                for (route, label, icon) in screens() {
                    {
                        let active = destination_is_active(&current, &route);
                        rsx! {
                            NavLink {
                                to: route,
                                label: label.to_string(),
                                icon: Some(icon),
                                active,
                                compact: true,
                                nested: false,
                                on_navigate,
                            }
                        }
                    }
                }
            }
            div { class: "md3-nav-panel-compact-settings",
                button {
                    id: "md3-compact-settings-toggle",
                    r#type: "button",
                    class: if settings_active { "md3-nav-panel-link md3-nav-compact-settings-toggle active" } else { "md3-nav-panel-link md3-nav-compact-settings-toggle" },
                    aria_label: "Settings sections",
                    aria_haspopup: "menu",
                    aria_expanded: settings_menu_open.to_string(),
                    aria_controls: "md3-compact-settings-menu",
                    title: "Settings",
                    onclick: move |_| on_toggle_settings.call(()),
                    {nav_icon(ICON_SETTINGS)}
                }
            }
        }
    }
}

/// Top-level navigation shell. Renders one entry per application screen,
/// highlights the currently active screen, and exposes the Settings sections
/// in a hover/tap dropdown attached to the Settings item. On tablet and wider
/// windows the same destinations can move into a persistent side panel; on
/// phones they collapse into a menu button and slide-in drawer. The side
/// panel can be compressed into an icon rail, with its Settings group pinned
/// above the compress/expand button in the footer.
#[component]
pub fn Topbar() -> Element {
    let current = use_route::<Route>();
    let interface_settings = use_interface_settings();
    let navigation_position = interface_settings.get().navigation_position;
    let side_nav_collapsed = interface_settings.get().side_nav_collapsed;
    let toast = use_toast();
    let mut settings_menu_open = use_signal(|| false);
    let mut mobile_menu_open = use_signal(|| false);
    let mut restore_mobile_focus = use_signal(|| false);
    let mut compact_settings_menu_open = use_signal(|| false);
    let mut restore_compact_settings_focus = use_signal(|| false);
    let side_nav_saving = use_signal(|| false);
    let settings_active = settings_is_active(&current);
    let general_active = current == Route::SettingsGeneral {};
    let exchanges_active = current == Route::SettingsExchanges {};
    let wallet_types_active = current == Route::SettingsWalletTypes {};
    let exchange_types_active = current == Route::SettingsExchangeTypes {};
    let title = current_title(&current);

    // Return keyboard focus to the menu button after the drawer closes.
    use_effect(move || {
        if restore_mobile_focus() && !mobile_menu_open() {
            document::eval("document.getElementById('md3-nav-menu-button')?.focus();");
            restore_mobile_focus.set(false);
        }
    });

    // Return keyboard focus to the compact Settings gear after its flyout
    // closes.
    use_effect(move || {
        if restore_compact_settings_focus() && !compact_settings_menu_open() {
            document::eval("document.getElementById('md3-compact-settings-toggle')?.focus();");
            restore_compact_settings_focus.set(false);
        }
    });

    // A navigation change (including browser back/forward) closes both
    // overlays. Reading the router inside the effect subscribes it to updates.
    let router_context = router();
    use_effect(move || {
        let _ = router_context.full_route_string();
        mobile_menu_open.set(false);
        compact_settings_menu_open.set(false);
    });

    rsx! {
        div {
            class: "md3-app-shell",
            "data-nav-position": navigation_position.as_str(),
            "data-side-nav-collapsed": side_nav_collapsed.to_string(),
            header { class: "md3-top-app-bar",
                button {
                    id: "md3-nav-menu-button",
                    r#type: "button",
                    class: "md3-nav-menu-button",
                    aria_label: if mobile_menu_open() { "Close navigation menu" } else { "Open navigation menu" },
                    aria_controls: "md3-nav-drawer",
                    aria_expanded: mobile_menu_open().to_string(),
                    aria_haspopup: "dialog",
                    onclick: move |_| mobile_menu_open.set(!mobile_menu_open()),
                    svg {
                        view_box: "0 0 24 24",
                        width: "24",
                        height: "24",
                        fill: "currentColor",
                        path { d: "M3 6h18v2H3V6zm0 5h18v2H3v-2zm0 5h18v2H3v-2z" }
                    }
                }
                span { class: "md3-top-app-bar-title", "{title}" }

                nav { class: "md3-top-app-bar-nav", aria_label: "Primary",
                    for (route, label, _) in screens() {
                        {
                            let active = destination_is_active(&current, &route);
                            rsx! {
                                Link {
                                    to: route,
                                    class: if active { "active" } else { "" },
                                    aria_current: if active { "page" } else { "false" },
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
                        div { class: if settings_active { "md3-nav-dropdown-trigger active" } else { "md3-nav-dropdown-trigger" },
                            Link {
                                to: Route::SettingsGeneral {},
                                aria_current: if general_active { "page" } else { "false" },
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
                                aria_current: if general_active { "page" } else { "false" },
                                onclick: move |_| settings_menu_open.set(false),
                                "General"
                            }
                            Link {
                                to: Route::SettingsExchanges {},
                                class: if exchanges_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                                role: "menuitem",
                                aria_current: if exchanges_active { "page" } else { "false" },
                                onclick: move |_| settings_menu_open.set(false),
                                "Exchanges"
                            }
                            Link {
                                to: Route::SettingsWalletTypes {},
                                class: if wallet_types_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                                role: "menuitem",
                                aria_current: if wallet_types_active { "page" } else { "false" },
                                onclick: move |_| settings_menu_open.set(false),
                                "Wallet Types"
                            }
                            Link {
                                to: Route::SettingsExchangeTypes {},
                                class: if exchange_types_active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                                role: "menuitem",
                                aria_current: if exchange_types_active { "page" } else { "false" },
                                onclick: move |_| settings_menu_open.set(false),
                                "Exchange Types"
                            }
                        }
                    }
                }
            }

            aside { id: "md3-side-nav", class: "md3-side-nav",
                div { class: if side_nav_collapsed { "md3-side-nav-brand md3-side-nav-brand-compact" } else { "md3-side-nav-brand" },
                    if side_nav_collapsed {
                        span { class: "md3-side-nav-monogram", "AE" }
                    } else {
                        "Address Explorer"
                    }
                }

                if side_nav_collapsed {
                    CompactNavPanel {
                        current: current.clone(),
                        settings_menu_open: compact_settings_menu_open(),
                        on_toggle_settings: move |_| {
                            compact_settings_menu_open.set(!compact_settings_menu_open());
                        },
                        on_close_settings: move |_| {
                            close_compact_settings(
                                compact_settings_menu_open,
                                restore_compact_settings_focus,
                            );
                        },
                        on_navigate: move |_| {},
                    }
                } else {
                    FullNavPanel {
                        current: current.clone(),
                        settings_bottom: true,
                        on_navigate: move |_| {},
                    }
                }

                if side_nav_collapsed && compact_settings_menu_open() {
                    div {
                        class: "md3-nav-compact-settings-backdrop",
                        role: "presentation",
                        aria_hidden: "true",
                        onclick: move |_| {
                            close_compact_settings(
                                compact_settings_menu_open,
                                restore_compact_settings_focus,
                            );
                        },
                    }
                    div {
                        id: "md3-compact-settings-menu",
                        class: "md3-nav-compact-settings-menu",
                        role: "menu",
                        aria_label: "Settings sections",
                        onkeydown: move |event| {
                            if event.key() == Key::Escape {
                                event.prevent_default();
                                close_compact_settings(
                                    compact_settings_menu_open,
                                    restore_compact_settings_focus,
                                );
                            }
                        },
                        for (route, label) in settings_sections() {
                            {
                                let active = current == route;
                                rsx! {
                                    Link {
                                        to: route,
                                        class: if active { "md3-nav-compact-settings-item active" } else { "md3-nav-compact-settings-item" },
                                        role: "menuitem",
                                        aria_current: if active { "page" } else { "false" },
                                        onclick: move |_| {
                                            close_compact_settings(
                                                compact_settings_menu_open,
                                                restore_compact_settings_focus,
                                            );
                                        },
                                        "{label}"
                                    }
                                }
                            }
                        }
                    }
                }

                div { class: "md3-side-nav-footer",
                    button {
                        id: "md3-side-nav-compress",
                        r#type: "button",
                        class: if side_nav_collapsed { "md3-side-nav-compress md3-side-nav-compress-rail" } else { "md3-side-nav-compress" },
                        aria_label: if side_nav_collapsed { "Expand side panel" } else { "Compress side panel" },
                        aria_controls: "md3-side-nav",
                        aria_expanded: (!side_nav_collapsed).to_string(),
                        title: if side_nav_collapsed { "Expand side panel" } else { "Compress side panel" },
                        disabled: *side_nav_saving.read(),
                        onclick: move |_| {
                            compact_settings_menu_open.set(false);
                            persist_side_nav_collapsed(
                                interface_settings,
                                side_nav_saving,
                                toast,
                                !side_nav_collapsed,
                            );
                        },
                        {
                            nav_icon(if side_nav_collapsed { ICON_CHEVRON_RIGHT } else { ICON_CHEVRON_LEFT })
                        }
                        if !side_nav_collapsed {
                            span { class: "md3-side-nav-compress-label", "Compress side panel" }
                        }
                    }
                }
            }

            main { id: "screens", Outlet::<Route> {} }
        }

        if mobile_menu_open() {
            div {
                class: "md3-nav-drawer-scrim",
                role: "presentation",
                aria_hidden: "true",
                onclick: move |_| {
                    close_mobile_navigation(mobile_menu_open, restore_mobile_focus);
                },
            }
            div {
                id: "md3-nav-drawer",
                class: "md3-nav-drawer",
                role: "dialog",
                aria_modal: "true",
                aria_label: "Navigation",
                onkeydown: move |event| {
                    if event.key() == Key::Escape {
                        event.prevent_default();
                        close_mobile_navigation(mobile_menu_open, restore_mobile_focus);
                    }
                },
                div { class: "md3-nav-drawer-header",
                    span { class: "md3-side-nav-brand", "Address Explorer" }
                    button {
                        r#type: "button",
                        class: "md3-icon-button md3-nav-drawer-close",
                        aria_label: "Close navigation menu",
                        autofocus: true,
                        onclick: move |_| {
                            close_mobile_navigation(mobile_menu_open, restore_mobile_focus);
                        },
                        svg {
                            view_box: "0 0 24 24",
                            width: "24",
                            height: "24",
                            fill: "currentColor",
                            path { d: "M6.4 5 5 6.4 10.6 12 5 17.6 6.4 19 12 13.4 17.6 19 19 17.6 13.4 12 19 6.4 17.6 5 12 10.6z" }
                        }
                    }
                }
                FullNavPanel {
                    current: current.clone(),
                    settings_bottom: false,
                    on_navigate: move |_| {
                        close_mobile_navigation(mobile_menu_open, restore_mobile_focus);
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exchange_detail() -> Route {
        Route::ExchangeDetail {
            exchange_id: 1,
            transaction_id: None,
        }
    }

    #[test]
    fn exchange_detail_keeps_exchanges_active() {
        assert!(destination_is_active(
            &exchange_detail(),
            &Route::Exchanges {}
        ));
        assert!(!destination_is_active(
            &exchange_detail(),
            &Route::Addresses {}
        ));
    }

    #[test]
    fn exchange_detail_uses_the_exchanges_title() {
        assert_eq!(current_title(&exchange_detail()), "Exchanges");
    }

    #[test]
    fn every_settings_route_is_grouped() {
        for (route, label) in settings_sections() {
            assert!(settings_is_active(&route));
            assert_eq!(current_title(&route), "Settings");
            assert!(!label.is_empty());
        }
    }

    #[test]
    fn every_destination_has_a_label_and_icon() {
        for (_, label, icon) in screens() {
            assert!(!label.is_empty());
            assert!(!icon.is_empty());
        }
    }
}
