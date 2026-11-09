use crate::api::prices::ensure_currency_history_api;
use crate::api::settings::{
    save_default_fiat_currency_api, save_navigation_position_api, save_show_date_range_support_api,
    save_theme_api,
};
use crate::components::{
    use_fiat_currency, use_interface_settings, use_toast, BackendSettingsSection, Checkbox,
    DataSettingsSection, FiatCurrency, InterfaceSettingsContext, PageHeader, Toast, ToastKind,
};
use crate::domain::interface_settings::NavigationPosition;
use crate::domain::price::{CurrencyHistorySummary, FIAT_CURRENCIES, USD_SYMBOL};
use crate::domain::theme::{ThemeMode, ThemePalette, ThemePreference};
use dioxus::prelude::*;
/// General settings: theme, currency, interface hints, backend, and data.
#[component]
pub fn SettingsGeneral() -> Element {
    let toast = use_toast();
    let theme = use_context::<Signal<ThemePreference>>();
    let saving = use_signal(|| false);
    let currency = use_fiat_currency();
    let currency_saving = use_signal(|| false);
    let currency_status = use_signal(|| Option::<String>::None);
    let interface_settings = use_interface_settings();
    let support_saving = use_signal(|| false);
    let navigation_saving = use_signal(|| false);
    let currency_code = currency.code();
    rsx! {
        PageHeader { title: "General".to_string() }
        div { class: "md3-settings-grid",
            section { class: "md3-settings-section md3-settings-span",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "Theme"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Choose a Material 3 color palette and appearance. Your selection is saved automatically."
                    }
                }
                ThemePaletteSelect {
                    value: theme().palette,
                    disabled: *saving.read(),
                    on_select: move |palette| {
                        let next = ThemePreference {
                            palette,
                            ..theme()
                        };
                        persist_theme(theme, saving, toast, next);
                    },
                }
                hr { class: "md3-divider" }
                div { class: "md3-theme-appearance",
                    h4 { class: "text-title-small leading-title-small text-on-surface",
                        "Appearance"
                    }
                    div {
                        class: "md3-chip-group",
                        role: "group",
                        aria_label: "Appearance mode",
                        for mode in ThemeMode::ALL {
                            button {
                                r#type: "button",
                                class: if theme().mode == mode { "md3-chip md3-chip-selected" } else { "md3-chip" },
                                aria_pressed: (theme().mode == mode).to_string(),
                                disabled: *saving.read(),
                                onclick: move |_| {
                                    let next = ThemePreference { mode, ..theme() };
                                    persist_theme(theme, saving, toast, next);
                                },
                                "{mode.label()}"
                            }
                        }
                    }
                }
                hr { class: "md3-divider" }
                div {
                    h4 { class: "text-title-small leading-title-small text-on-surface",
                        "Navigation"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Choose where primary navigation appears on tablet and wider windows. Auto keeps the top app bar until wide windows, then moves to the side panel. Top always keeps the top app bar; Side uses the side panel from tablet widths. On phones, every option uses the menu button and slide-in drawer."
                    }
                }
                div {
                    class: "md3-chip-group",
                    role: "group",
                    aria_label: "Navigation position",
                    for position in NavigationPosition::ALL {
                        button {
                            r#type: "button",
                            class: if interface_settings.get().navigation_position == position { "md3-chip md3-chip-selected" } else { "md3-chip" },
                            aria_pressed: (interface_settings.get().navigation_position == position).to_string(),
                            disabled: *navigation_saving.read(),
                            onclick: move |_| {
                                persist_navigation_position(
                                    interface_settings,
                                    navigation_saving,
                                    toast,
                                    position,
                                );
                            },
                            "{position.label()}"
                        }
                    }
                }
                hr { class: "md3-divider" }
                div {
                    h4 { class: "text-title-small leading-title-small text-on-surface",
                        "Default fiat currency"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Fiat prices across the app are displayed in this currency. Selecting a currency also converts the stored BTC price history into it and saves the result for later use. Your selection is saved automatically."
                    }
                }
                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "default-fiat-currency",
                    "Currency"
                }
                div { class: "md3-text-field md3-settings-control",
                    select {
                        id: "default-fiat-currency",
                        class: "md3-input",
                        value: "{currency_code}",
                        disabled: *currency_saving.read(),
                        onchange: move |event| {
                            persist_currency(
                                currency,
                                currency_saving,
                                currency_status,
                                toast,
                                event.value(),
                            );
                        },
                        for code in FIAT_CURRENCIES {
                            option { value: "{code}", "{code}" }
                        }
                    }
                }
                if let Some(status) = currency_status() {
                    p {
                        class: "text-body-small text-on-surface-variant",
                        role: "status",
                        aria_live: "polite",
                        "{status}"
                    }
                }
                hr { class: "md3-divider" }
                div {
                    h4 { class: "text-title-small leading-title-small text-on-surface",
                        "Date range"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Show the YYYY-MM-DD – YYYY-MM-DD hint and validation message below date range fields across the application. Supporting text is hidden by default."
                    }
                }
                Checkbox {
                    checked: interface_settings.get().show_date_range_support,
                    disabled: *support_saving.read(),
                    label: "Show date range support text".to_string(),
                    on_change: move |enabled| {
                        persist_date_range_support(interface_settings, support_saving, toast, enabled);
                    },
                }
            }
            BackendSettingsSection {}
            DataSettingsSection {}
        }
    }
}
/// Compact palette picker. A native `<option>` cannot render the palette
/// swatches, so the trigger and its listbox show each palette's swatch and
/// name instead.
#[component]
fn ThemePaletteSelect(
    value: ThemePalette,
    #[props(default)] disabled: bool,
    on_select: EventHandler<ThemePalette>,
) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div {
            class: "md3-select md3-settings-control",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    open.set(false);
                }
            },
            button {
                r#type: "button",
                class: "md3-select-trigger",
                aria_haspopup: "listbox",
                aria_expanded: open().to_string(),
                aria_controls: "theme-palette-options",
                aria_label: "Theme palette",
                disabled,
                onclick: move |_| open.set(!open()),
                span { class: "md3-theme-swatch md3-theme-swatch-{value}" }
                span { class: "md3-select-value", "{value.label()}" }
                span { class: "md3-select-chevron", aria_hidden: "true", "⌄" }
            }
            if open() {
                div {
                    class: "md3-select-scrim",
                    role: "presentation",
                    onclick: move |_| open.set(false),
                }
                div {
                    id: "theme-palette-options",
                    class: "md3-select-menu",
                    role: "listbox",
                    aria_label: "Theme palette",
                    onclick: move |event| event.stop_propagation(),
                    for palette in ThemePalette::ALL {
                        button {
                            r#type: "button",
                            class: "md3-select-option",
                            role: "option",
                            aria_selected: (palette == value).to_string(),
                            onclick: move |_| {
                                open.set(false);
                                on_select.call(palette);
                            },
                            span { class: "md3-theme-swatch md3-theme-swatch-{palette}" }
                            span { class: "md3-select-value", "{palette.label()}" }
                            if palette == value {
                                span {
                                    class: "md3-select-check",
                                    aria_hidden: "true",
                                    "✓"
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
/// Apply the new navigation placement immediately, then persist it. If the
/// write fails, restore the previous value so the shell never presents an
/// uncommitted position as saved.
fn persist_navigation_position(
    interface_settings: InterfaceSettingsContext,
    mut saving: Signal<bool>,
    mut toast: Toast,
    position: NavigationPosition,
) {
    let previous = interface_settings.get();
    if previous.navigation_position == position {
        return;
    }
    let mut next = previous;
    next.navigation_position = position;
    interface_settings.set(next);
    saving.set(true);
    spawn(async move {
        match save_navigation_position_api(position).await {
            Ok(()) => toast.success(format!("Navigation position set to {}.", position.label())),
            Err(error) => {
                interface_settings.set(previous);
                toast
                    .error(
                        format!(
                            "Could not save the navigation position: {error}. The previous value was restored.",
                        ),
                    );
            }
        }
        saving.set(false);
    });
}
/// Apply the new date-range support preference immediately, then persist it.
/// If the write fails, restore the previous value so the checkbox never
/// presents an uncommitted setting as saved.
fn persist_date_range_support(
    interface_settings: InterfaceSettingsContext,
    mut saving: Signal<bool>,
    mut toast: Toast,
    enabled: bool,
) {
    let previous = interface_settings.get();
    let mut next = previous;
    next.show_date_range_support = enabled;
    interface_settings.set(next);
    saving.set(true);
    spawn(async move {
        match save_show_date_range_support_api(enabled).await {
            Ok(()) => toast.success(if enabled {
                "Date range support text shown."
            } else {
                "Date range support text hidden."
            }),
            Err(error) => {
                interface_settings.set(previous);
                toast
                    .error(
                        format!(
                            "Could not save the date range support setting: {error}. The previous value was restored.",
                        ),
                    );
            }
        }
        saving.set(false);
    });
}
/// Apply the new preference immediately, then persist it. If the write fails,
/// restore the previous preference so the UI never presents an uncommitted
/// theme as saved. Controls are disabled while the save is in flight so rapid
/// clicks cannot leave the database with an older value than the UI.
fn persist_theme(
    mut theme: Signal<ThemePreference>,
    mut saving: Signal<bool>,
    mut toast: Toast,
    next: ThemePreference,
) {
    let previous = theme();
    theme.set(next);
    saving.set(true);
    spawn(async move {
        match save_theme_api(next).await {
            Ok(()) => toast.success("Theme saved."),
            Err(error) => {
                theme.set(previous);
                toast.error(format!(
                    "Could not save theme: {error}. The previous theme was restored.",
                ));
            }
        }
        saving.set(false);
    });
}
/// Apply the new default fiat currency immediately, then persist it. If the
/// write fails, restore the previous value so the UI never presents an
/// uncommitted currency as saved. A successful save is followed by a
/// best-effort conversion of the stored USD history into the new currency:
/// that conversion is independent from the preference, so its failure keeps
/// the saved value and is reported as a warning instead of a rollback.
///
/// The control is disabled while the workflow is in flight so rapid changes
/// cannot leave the database with an older value than the UI.
fn persist_currency(
    currency: FiatCurrency,
    mut saving: Signal<bool>,
    mut status: Signal<Option<String>>,
    mut toast: Toast,
    next: String,
) {
    if next == currency.code() {
        return;
    }
    let previous = currency.code();
    currency.set(next.clone());
    saving.set(true);
    status.set(Some("Saving the default fiat currency…".to_string()));
    spawn(async move {
        let saved = match save_default_fiat_currency_api(next).await {
            Ok(saved) => saved,
            Err(error) => {
                currency.set(previous);
                status.set(None);
                saving.set(false);
                toast
                    .error(
                        format!(
                            "Could not save the default fiat currency: {error}. The previous value was restored.",
                        ),
                    );
                return;
            }
        };
        currency.set(saved.clone());
        if saved == USD_SYMBOL {
            status.set(None);
            saving.set(false);
            toast.success("Default fiat currency saved.");
            return;
        }
        status.set(Some(format!("Preparing {saved} historical prices…")));
        match ensure_currency_history_api(saved.clone()).await {
            Ok(summary) => {
                let (kind, message) = currency_change_message(&saved, &summary);
                status.set(summary.warning);
                toast.show(kind, message);
            }
            Err(error) => {
                let message = format!(
                    "Default fiat currency saved to {saved}, but historical prices could not be converted: {error}. Historical Prices will retry.",
                );
                status.set(Some(message.clone()));
                toast.show(ToastKind::Info, message);
            }
        }
        saving.set(false);
    });
}
/// Toast copy for a successful preference save followed by history
/// preparation.
fn currency_change_message(symbol: &str, summary: &CurrencyHistorySummary) -> (ToastKind, String) {
    let mut message = if summary.stored > 0 {
        format!(
            "Default fiat currency saved. Converted {} historical prices to {symbol}.",
            summary.stored,
        )
    } else {
        "Default fiat currency saved.".to_string()
    };
    if let Some(warning) = &summary.warning {
        message.push(' ');
        message.push_str(warning);
        (ToastKind::Info, message)
    } else {
        (ToastKind::Success, message)
    }
}
