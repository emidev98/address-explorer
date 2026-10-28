use dioxus::prelude::*;

use crate::api::settings::{
    get_default_fiat_currency_api, save_default_fiat_currency_api, save_theme_api,
};
use crate::components::{use_toast, Toast};
use crate::domain::price::{FIAT_CURRENCIES, USD_SYMBOL};
use crate::domain::theme::{ThemeMode, ThemePalette, ThemePreference};

/// Settings screen.
#[component]
pub fn Settings() -> Element {
    let mut toast = use_toast();
    let theme = use_context::<Signal<ThemePreference>>();
    let saving = use_signal(|| false);

    let mut currency = use_signal(|| USD_SYMBOL.to_string());
    let currency_saving = use_signal(|| false);

    // Load the persisted default fiat currency once; the USD fallback keeps
    // the select usable when the read fails.
    let _load_currency = use_resource(move || async move {
        match get_default_fiat_currency_api().await {
            Ok(stored) => currency.set(stored),
            Err(error) => {
                toast.error(format!("Could not load the default fiat currency: {error}"));
            }
        }
    });

    rsx! {
        section { id: "settings", class: "md3-screen",
            h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                "Settings"
            }

            section { class: "md3-settings-section",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "Theme"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Choose a Material 3 color palette and appearance. Your selection is saved automatically."
                    }
                }

                div { class: "md3-theme-grid",
                    for palette in ThemePalette::ALL {
                        button {
                            r#type: "button",
                            class: "md3-theme-option",
                            aria_pressed: (theme().palette == palette).to_string(),
                            disabled: *saving.read(),
                            onclick: move |_| {
                                let next = ThemePreference {
                                    palette,
                                    ..theme()
                                };
                                persist_theme(theme, saving, toast, next);
                            },
                            span { class: "md3-theme-swatch md3-theme-swatch-{palette}" }
                            span { "{palette.label()}" }
                        }
                    }
                }

                div {
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

            }

            section { class: "md3-settings-section",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "Default fiat currency"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Fiat prices across the app, including the Historical Prices table, are displayed in this currency by default. Your selection is saved automatically."
                    }
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "default-fiat-currency",
                    "Currency"
                }
                div { class: "md3-text-field",
                    select {
                        id: "default-fiat-currency",
                        class: "md3-input",
                        value: "{currency}",
                        disabled: *currency_saving.read(),
                        onchange: move |event| {
                            persist_currency(currency, currency_saving, toast, event.value());
                        },
                        for code in FIAT_CURRENCIES {
                            option { value: "{code}", "{code}" }
                        }
                    }
                }

            }
        }
    }
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
                    "Could not save theme: {error}. The previous theme was restored."
                ));
            }
        }
        saving.set(false);
    });
}

/// Apply the new default fiat currency immediately, then persist it. If the
/// write fails, restore the previous value so the UI never presents an
/// uncommitted currency as saved. The control is disabled while the save is
/// in flight so rapid changes cannot leave the database with an older value
/// than the UI.
fn persist_currency(
    mut currency: Signal<String>,
    mut saving: Signal<bool>,
    mut toast: Toast,
    next: String,
) {
    if next == currency() {
        return;
    }

    let previous = currency();
    currency.set(next.clone());
    saving.set(true);

    spawn(async move {
        match save_default_fiat_currency_api(next).await {
            Ok(saved) => {
                currency.set(saved);
                toast.success("Default fiat currency saved.");
            }
            Err(error) => {
                currency.set(previous);
                toast.error(format!(
                    "Could not save the default fiat currency: {error}. The previous value was restored."
                ));
            }
        }
        saving.set(false);
    });
}
