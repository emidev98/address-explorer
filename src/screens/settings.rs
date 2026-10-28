use dioxus::prelude::*;

use crate::api::settings::{
    get_default_fiat_currency_api, get_exchange_settings_api, save_confirm_destructive_actions_api,
    save_default_fiat_currency_api, save_on_chain_link_scoring_api, save_theme_api,
};
use crate::components::{use_toast, Toast};
use crate::domain::exchange::OnChainLinkScoringSettings;
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
    let mut confirm_destructive = use_signal(|| true);
    let mut confirm_saving = use_signal(|| false);
    let mut scoring = use_signal(OnChainLinkScoringSettings::default);
    let mut scoring_saved = use_signal(OnChainLinkScoringSettings::default);
    let scoring_saving = use_signal(|| false);

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
    let _load_exchange_settings = use_resource(move || async move {
        match get_exchange_settings_api().await {
            Ok(settings) => {
                confirm_destructive.set(settings.confirm_destructive_actions);
                scoring_saved.set(settings.on_chain_link_scoring.clone());
                scoring.set(settings.on_chain_link_scoring);
            }
            Err(error) => {
                toast.error(format!("Could not load exchange settings: {error}"));
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
                }

            }

            div { class: "md3-settings-row",
                section { class: "md3-settings-section",
                    div {
                        h3 { class: "text-title-medium leading-title-medium text-on-surface",
                            "Safety"
                        }
                        p { class: "text-body-medium text-on-surface-variant",
                            "Ask before deleting or replacing local data."
                        }
                    }
                    label { class: "md3-settings-toggle",
                        input {
                            r#type: "checkbox",
                            checked: confirm_destructive(),
                            disabled: *confirm_saving.read(),
                            onchange: move |event| {
                                let enabled = event.checked();
                                let previous = confirm_destructive();
                                confirm_destructive.set(enabled);
                                confirm_saving.set(true);
                                spawn(async move {
                                    match save_confirm_destructive_actions_api(enabled).await {
                                        Ok(()) => {
                                            toast
                                                .success(
                                                    if enabled {
                                                        "Destructive action confirmations enabled."
                                                    } else {
                                                        "Destructive action confirmations disabled."
                                                    },
                                                );
                                        }
                                        Err(error) => {
                                            confirm_destructive.set(previous);
                                            toast
                                                .error(
                                                    format!("Could not save the confirmation setting: {error}"),
                                                );
                                        }
                                    }
                                    confirm_saving.set(false);
                                });
                            },
                        }
                        span { class: "text-body-medium text-on-surface", "Confirm destructive actions" }
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

            section { class: "md3-settings-section",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "On-chain linking"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Weights are relative: higher values give that signal more influence in the 0 to 1 likelihood score. Imported rows auto-link only when the best candidate scores exactly 1.000."
                    }
                }
                div { class: "md3-scoring-grid",
                    for (field, label, helper) in SCORING_FIELDS {
                        label { class: "md3-settings-field",
                            span { class: "text-body-small text-on-surface-variant",
                                "{label}"
                            }
                            div { class: "md3-text-field",
                                input {
                                    class: "md3-input",
                                    r#type: "number",
                                    step: "0.01",
                                    value: "{scoring_value(&scoring(), field)}",
                                    aria_invalid: (!scoring_field_is_valid(&scoring(), field)).to_string(),
                                    oninput: move |event| {
                                        let Ok(value) = event.value().parse::<f64>() else {
                                            return;
                                        };
                                        let mut next = scoring();
                                        set_scoring_value(&mut next, field, value);
                                        queue_scoring_save(
                                            scoring,
                                            scoring_saved,
                                            scoring_saving,
                                            toast,
                                            next,
                                            "On-chain linking weights saved.",
                                        );
                                    },
                                }
                            }
                            span { class: "text-body-small text-on-surface-variant",
                                "{helper}"
                            }
                        }
                    }
                    div { class: "md3-settings-field",
                        span { class: "text-body-small text-on-surface-variant", "Defaults" }
                        button {
                            r#type: "button",
                            class: "md3-button-tonal md3-scoring-reset",
                            onclick: move |_| {
                                queue_scoring_save(
                                    scoring,
                                    scoring_saved,
                                    scoring_saving,
                                    toast,
                                    OnChainLinkScoringSettings::default(),
                                    "On-chain linking weights reset.",
                                );
                            },
                            "Reset"
                        }
                        span { class: "text-body-small text-on-surface-variant",
                            "Restore the default weights and tolerances."
                        }
                    }
                }
                if !scoring().is_valid() {
                    p { class: "text-body-small md3-settings-error",
                        "Weights must be zero or greater and tolerances greater than zero. Changes are saved automatically once every value is valid."
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScoringField {
    HashWeight,
    AmountWeight,
    PaymentOutputWeight,
    FeeAdjustedAmountWeight,
    TimestampWeight,
    DirectionWeight,
    TxShapeWeight,
    UnmatchedWeight,
    AmbiguityWeight,
    AmountToleranceSats,
    TimestampToleranceHours,
}

const SCORING_FIELDS: [(ScoringField, &str, &str); 11] = [
    (
        ScoringField::HashWeight,
        "Hash weight",
        "Exact match between the exchange txid and a cached on-chain txid.",
    ),
    (
        ScoringField::AmountWeight,
        "Balance weight",
        "Broad BTC movement visible through your local addresses.",
    ),
    (
        ScoringField::PaymentOutputWeight,
        "Payment output weight",
        "The most direction-specific output.",
    ),
    (
        ScoringField::FeeAdjustedAmountWeight,
        "Fee-adjusted weight",
        "Amount matches that account for network fees.",
    ),
    (
        ScoringField::TimestampWeight,
        "Timestamp weight",
        "How close the block time is to the exchange timestamp.",
    ),
    (
        ScoringField::DirectionWeight,
        "Direction weight",
        "Whether the flow matches the exchange transaction type.",
    ),
    (
        ScoringField::TxShapeWeight,
        "Transaction shape weight",
        "Ordinary transaction structure versus CoinJoin/batches.",
    ),
    (
        ScoringField::UnmatchedWeight,
        "Unmatched weight",
        "Boost for candidates not already linked.",
    ),
    (
        ScoringField::AmbiguityWeight,
        "Ambiguity weight",
        "How clearly the best candidate separates from the next.",
    ),
    (
        ScoringField::AmountToleranceSats,
        "Amount tolerance sats",
        "Maximum satoshi difference used to grade amount similarity.",
    ),
    (
        ScoringField::TimestampToleranceHours,
        "Timestamp tolerance hours",
        "Time window where timestamp proximity contributes.",
    ),
];

fn scoring_value(settings: &OnChainLinkScoringSettings, field: ScoringField) -> f64 {
    match field {
        ScoringField::HashWeight => settings.hash_weight,
        ScoringField::AmountWeight => settings.amount_weight,
        ScoringField::PaymentOutputWeight => settings.payment_output_weight,
        ScoringField::FeeAdjustedAmountWeight => settings.fee_adjusted_amount_weight,
        ScoringField::TimestampWeight => settings.timestamp_weight,
        ScoringField::DirectionWeight => settings.direction_weight,
        ScoringField::TxShapeWeight => settings.tx_shape_weight,
        ScoringField::UnmatchedWeight => settings.unmatched_weight,
        ScoringField::AmbiguityWeight => settings.ambiguity_weight,
        ScoringField::AmountToleranceSats => settings.amount_tolerance_sats,
        ScoringField::TimestampToleranceHours => settings.timestamp_tolerance_hours,
    }
}

fn set_scoring_value(settings: &mut OnChainLinkScoringSettings, field: ScoringField, value: f64) {
    match field {
        ScoringField::HashWeight => settings.hash_weight = value,
        ScoringField::AmountWeight => settings.amount_weight = value,
        ScoringField::PaymentOutputWeight => settings.payment_output_weight = value,
        ScoringField::FeeAdjustedAmountWeight => settings.fee_adjusted_amount_weight = value,
        ScoringField::TimestampWeight => settings.timestamp_weight = value,
        ScoringField::DirectionWeight => settings.direction_weight = value,
        ScoringField::TxShapeWeight => settings.tx_shape_weight = value,
        ScoringField::UnmatchedWeight => settings.unmatched_weight = value,
        ScoringField::AmbiguityWeight => settings.ambiguity_weight = value,
        ScoringField::AmountToleranceSats => settings.amount_tolerance_sats = value,
        ScoringField::TimestampToleranceHours => settings.timestamp_tolerance_hours = value,
    }
}

/// Whether one scoring field is within the range the domain accepts, so the
/// input can expose `aria-invalid` while the user is editing.
fn scoring_field_is_valid(settings: &OnChainLinkScoringSettings, field: ScoringField) -> bool {
    let value = scoring_value(settings, field);
    match field {
        ScoringField::AmountToleranceSats | ScoringField::TimestampToleranceHours => {
            value.is_finite() && value > 0.0
        }
        _ => value.is_finite() && value >= 0.0,
    }
}

/// Persist valid on-chain scoring changes automatically. The grid has no save
/// button: each valid edit is written, while invalid values stay in the form
/// so the user can finish editing. A single saver runs at a time; edits made
/// while a write is in flight are coalesced into one follow-up request, so the
/// last edit stays authoritative and writes cannot complete out of order. If a
/// write fails, the last persisted values are restored.
fn queue_scoring_save(
    mut scoring: Signal<OnChainLinkScoringSettings>,
    mut saved: Signal<OnChainLinkScoringSettings>,
    mut saving: Signal<bool>,
    mut toast: Toast,
    next: OnChainLinkScoringSettings,
    success_message: &'static str,
) {
    scoring.set(next);

    if !scoring().is_valid() || scoring() == saved() {
        return;
    }
    if *saving.read() {
        // The running saver reads the signal again before it finishes.
        return;
    }

    saving.set(true);

    spawn(async move {
        let mut wrote = false;
        loop {
            let next = scoring();
            if !next.is_valid() || next == saved() {
                break;
            }

            match save_on_chain_link_scoring_api(next.clone()).await {
                Ok(stored) => {
                    saved.set(stored.clone());
                    // Keep an edit typed while the request was in flight.
                    if scoring() == next {
                        scoring.set(stored);
                    }
                    wrote = true;
                }
                Err(error) => {
                    scoring.set(saved());
                    toast.error(format!(
                        "Could not save on-chain linking settings: {error}. The previous values were restored."
                    ));
                    saving.set(false);
                    return;
                }
            }
        }

        saving.set(false);
        if wrote {
            toast.success(success_message);
        }
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
