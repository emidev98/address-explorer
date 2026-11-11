use crate::api::encryption::lock_encryption_api;
use crate::api::settings::save_side_nav_collapsed_api;
use crate::app::Route;
use crate::components::{
    use_encryption, use_interface_settings, use_toast, EncryptionContext, Icon, IconName,
    InterfaceSettingsContext, Toast,
};
use crate::navigation::Destination;
use dioxus::prelude::*;
/// One icon shown by a navigation destination.
fn nav_icon(name: IconName) -> Element {
    rsx! {
        Icon { name, size: 24, class: "md3-nav-icon" }
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
                toast
                    .error(
                        format!(
                            "Could not save the side panel state: {error}. The previous value was restored.",
                        ),
                    );
            }
        }
        saving.set(false);
    });
}
/// Lock the encrypted database and publish the locked status to the vault gate.
fn lock_database(encryption: EncryptionContext, mut saving: Signal<bool>, mut toast: Toast) {
    if *saving.read() {
        return;
    }
    saving.set(true);
    let mut status = encryption.status;
    spawn(async move {
        let result = lock_encryption_api().await;
        saving.set(false);
        match result {
            Ok(saved) => status.set(Some(saved)),
            Err(error) => toast.error(format!("Could not lock the database: {error}")),
        }
    });
}
/// One destination link. Expanded presentations show an icon and label;
/// compact presentations show the icon only, with an accessible label and a
/// native tooltip.
#[component]
fn NavLink(
    to: Route,
    label: String,
    icon: Option<IconName>,
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
/// Full navigation: destinations at the top and, in the mobile drawer,
/// Settings directly after them. The side panel keeps Settings in its footer.
#[component]
fn FullNavPanel(current: Route, show_settings: bool, on_navigate: EventHandler<()>) -> Element {
    let current_destination = Destination::from_route(&current);
    let settings_active = current_destination.is_some_and(Destination::is_settings);
    rsx! {
        nav { class: "md3-nav-panel", aria_label: "Primary",
            div { class: "md3-nav-panel-primary",
                for destination in Destination::PRIMARY {
                    NavLink {
                        to: destination.route(),
                        label: destination.label().to_string(),
                        icon: Some(destination.icon()),
                        active: current_destination == Some(destination),
                        compact: false,
                        nested: false,
                        on_navigate,
                    }
                }
            }
            if show_settings {
                div { class: "md3-nav-panel-settings",
                    p { class: if settings_active { "md3-nav-panel-label active" } else { "md3-nav-panel-label" },
                        {nav_icon(IconName::Settings)}
                        span { "Settings" }
                    }
                    for destination in Destination::SETTINGS {
                        NavLink {
                            to: destination.route(),
                            label: destination
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .label().to_string(),
                            icon: None,
                            active: current_destination ==
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        Some(destination),
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
/// Compact icon-rail navigation. The Settings flyout is owned by the shell
/// footer so both side-panel presentations share the same actions.
#[component]
fn CompactNavPanel(
    current: Route,
    on_close_settings: EventHandler<()>,
    on_navigate: EventHandler<()>,
) -> Element {
    let current_destination = Destination::from_route(&current);
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
                for destination in Destination::PRIMARY {
                    NavLink {
                        to: destination.route(),
                        label: destination
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .label().to_string(),
                        icon: Some(destination.icon()),
                        active: current_destination == Some(destination),
                        compact: true,
                        nested: false,
                        on_navigate,
                    }
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
/// panel can be compressed into an icon rail. Its footer exposes Settings,
/// compress/expand, and, when encryption is on, lock actions. The compact rail
/// stacks Settings, lock, and compress; the expanded panel shows compress,
/// lock, and Settings in a row.
#[component]
pub fn AppShell() -> Element {
    let current = use_route::<Route>();
    let interface_settings = use_interface_settings();
    let encryption = use_encryption();
    let encryption_configured = encryption
        .status
        .read()
        .as_ref()
        .is_some_and(|status| status.configured);
    let navigation_position = interface_settings.get().navigation_position;
    let side_nav_collapsed = interface_settings.get().side_nav_collapsed;
    let toast = use_toast();
    let mut settings_menu_open = use_signal(|| false);
    let mut mobile_menu_open = use_signal(|| false);
    let mut restore_mobile_focus = use_signal(|| false);
    let mut compact_settings_menu_open = use_signal(|| false);
    let mut restore_compact_settings_focus = use_signal(|| false);
    let side_nav_saving = use_signal(|| false);
    let lock_saving = use_signal(|| false);
    let current_destination = Destination::from_route(&current);
    let settings_active = current_destination.is_some_and(Destination::is_settings);
    let general_active = current_destination == Some(Destination::SettingsGeneral);
    let title = current_destination
        .map(Destination::label)
        .unwrap_or("Addresses");
    use_effect(move || {
        if restore_mobile_focus() && !mobile_menu_open() {
            document::eval("document.getElementById('md3-nav-menu-button')?.focus();");
            restore_mobile_focus.set(false);
        }
    });
    use_effect(move || {
        if restore_compact_settings_focus() && !compact_settings_menu_open() {
            document::eval("document.getElementById('md3-compact-settings-toggle')?.focus();");
            restore_compact_settings_focus.set(false);
        }
    });
    let router_context = router();
    use_effect(move || {
        let _ = router_context.full_route_string();
        mobile_menu_open.set(false);
        compact_settings_menu_open.set(false);
    });
    let settings_action = rsx! {
        button {
            id: "md3-compact-settings-toggle",
            r#type: "button",
            class: if settings_active { "md3-side-nav-footer-action active" } else { "md3-side-nav-footer-action" },
            aria_label: "Settings sections",
            aria_haspopup: "menu",
            aria_expanded: compact_settings_menu_open().to_string(),
            aria_controls: "md3-compact-settings-menu",
            "data-tooltip": "Settings",
            onclick: move |_| {
                compact_settings_menu_open.set(!compact_settings_menu_open());
            },
            {nav_icon(IconName::Settings)}
        }
    };
    let compress_action = rsx! {
        button {
            id: "md3-side-nav-compress",
            r#type: "button",
            class: "md3-side-nav-footer-action",
            aria_label: if side_nav_collapsed { "Expand side panel" } else { "Compress side panel" },
            aria_controls: "md3-side-nav",
            aria_expanded: (!side_nav_collapsed).to_string(),
            "data-tooltip": if side_nav_collapsed { "Expand side panel" } else { "Compress side panel" },
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
                nav_icon(
                    if side_nav_collapsed {
                        IconName::ChevronRight
                    } else {
                        IconName::ChevronLeft
                    },
                )
            }
        }
    };
    let lock_action = encryption_configured.then(|| {
        rsx! {
            button {
                id: "md3-side-nav-lock",
                r#type: "button",
                class: "md3-side-nav-footer-action",
                aria_label: "Lock database",
                "data-tooltip": "Lock database",
                disabled: *lock_saving.read(),
                onclick: move |_| {
                    compact_settings_menu_open.set(false);
                    lock_database(encryption, lock_saving, toast);
                },
                {nav_icon(IconName::Lock)}
            }
        }
    });
    let footer_actions: Vec<Element> = if side_nav_collapsed {
        std::iter::once(settings_action)
            .chain(lock_action)
            .chain(std::iter::once(compress_action))
            .collect()
    } else {
        std::iter::once(compress_action)
            .chain(lock_action)
            .chain(std::iter::once(settings_action))
            .collect()
    };
    rsx! {
        div {
            class: "md3-app-shell",
            "data-nav-position": navigation_position
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            .as_str(),
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
                    for destination in Destination::PRIMARY {
                        {
                            let active = current_destination == Some(destination);
                            rsx! {
                                Link {
                                    to: destination.route(),
                                    class: if active { "active" } else { "" },
                                    aria_current: if active { "page" } else { "false" },
                                    onclick: move |_| settings_menu_open.set(false),
                                    "{destination.label()}"
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
                                onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        settings_menu_open.set(false),
                                "Settings"
                            }
                            button {
                                r#type: "button",
                                class: "md3-nav-dropdown-toggle",
                                aria_label: "Toggle settings sections",
                                aria_haspopup: "menu",
                                aria_expanded: settings_menu_open().to_string(),
                                aria_controls: "settings-menu",
                                onclick: move | _ | settings_menu_open.set(!
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        settings_menu_open()),
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
                            for destination in Destination::SETTINGS {
                                {
                                    let active = current_destination == Some(destination);
                                    rsx! {
                                        Link {
                                            to: destination.route(),
                                            class: if active { "md3-nav-dropdown-item active" } else { "md3-nav-dropdown-item" },
                                            role: "menuitem",
                                            aria_current: if active { "page" } else { "false" },
                                            onclick: move |_| settings_menu_open.set(false),
                                            "{destination.label()}"
                                        }
                                    }
                                }
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
                        show_settings: false,
                        on_navigate: move |_| {},
                    }
                }
                if compact_settings_menu_open() {
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
                }
                div {
                    class: "md3-side-nav-footer",
                    onkeydown: move |event| {
                        if event.key() == Key::Escape && compact_settings_menu_open() {
                            event.prevent_default();
                            close_compact_settings(
                                compact_settings_menu_open,
                                restore_compact_settings_focus,
                            );
                        }
                    },
                    for action in footer_actions {
                        {action}
                    }
                    if compact_settings_menu_open() {
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
                            for destination in Destination::SETTINGS {
                                {
                                    let active = current_destination == Some(destination);
                                    rsx! {
                                        Link {
                                            to: destination.route(),
                                            class: if active { "md3-nav-compact-settings-item active" } else { "md3-nav-compact-settings-item" },
                                            role: "menuitem",
                                            aria_current: if active { "page" } else { "false" },
                                            onclick: move |_| {
                                                close_compact_settings(
                                                    compact_settings_menu_open,
                                                    restore_compact_settings_focus,
                                                );
                                            },
                                            "{destination.label()}"
                                        }
                                    }
                                }
                            }
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
                    show_settings: true,
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
    fn exchange_account_detail() -> Route {
        Route::ExchangeAccountDetail {
            account_id: 1,
            transaction_id: None,
        }
    }
    #[test]
    fn exchange_account_detail_keeps_exchanges_active() {
        assert_eq!(
            Destination::from_route(&exchange_account_detail()),
            Some(Destination::Exchanges),
        );
        assert_eq!(
            Destination::from_route(&exchange_account_detail()).map(Destination::label),
            Some("Exchanges"),
        );
    }
    #[test]
    fn every_settings_route_is_grouped() {
        for destination in Destination::SETTINGS {
            assert!(destination.is_settings());
            assert_eq!(
                Destination::from_route(&destination.route()),
                Some(destination)
            );
        }
    }
    #[test]
    fn every_destination_has_a_label_and_icon() {
        for destination in Destination::PRIMARY {
            assert!(!destination.label().is_empty());
            assert!(!destination.icon().path().is_empty());
        }
    }
}
