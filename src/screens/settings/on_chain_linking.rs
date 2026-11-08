//! Exchange settings: reconciliation behavior and on-chain linking.
use crate::api::settings::{get_on_chain_link_scoring_api, save_on_chain_link_scoring_api};
use crate::components::{use_toast, PageHeader, Toast};
use crate::domain::exchange::OnChainLinkScoringSettings;
use dioxus::prelude::*;
/// Exchange settings: weights and tolerances for on-chain linking.
#[component]
pub fn SettingsOnChainLinking() -> Element {
    let mut toast = use_toast();
    let mut scoring = use_signal(OnChainLinkScoringSettings::default);
    let mut scoring_saved = use_signal(OnChainLinkScoringSettings::default);
    let scoring_saving = use_signal(|| false);
    let _load_on_chain_link_scoring = use_resource(move || async move {
        match get_on_chain_link_scoring_api().await {
            Ok(settings) => {
                scoring_saved.set(settings.clone());
                scoring.set(settings);
            }
            Err(error) => {
                toast.error(format!("Could not load on-chain linking settings: {error}"));
            }
        }
    });
    rsx! {
        PageHeader { title: "On-chain Linking".to_string() }
        section { class: "md3-settings-section",
            div {
                h3 { class: "text-title-medium leading-title-medium text-on-surface",
                    "On-chain Linking"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Weights are relative: higher values give that signal more influence in the 0 to 1 likelihood score. Imported rows auto-link only when the best candidate scores exactly 1.000."
                }
            }
            div { class: "md3-scoring-grid",
                for (field, label, helper) in SCORING_FIELDS {
                    label { class: "md3-settings-field",
                        span { class: "text-body-small text-on-surface-variant", "{label}" }
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                r#type: "number",
                                step: "0.01",
                                value: "{scoring_value(&scoring(), field)}",
                                aria_invalid: (!
                                                                                                                                        scoring_field_is_valid(& scoring(), field)).to_string(),
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
                        span { class: "text-body-small text-on-surface-variant", "{helper}" }
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
        "Hash mismatch penalty",
        "Penalty when the ledger row has a txid that does not match a candidate. Exact hashes always win.",
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
        ScoringField::FeeAdjustedAmountWeight => {
            settings.fee_adjusted_amount_weight = value;
        }
        ScoringField::TimestampWeight => settings.timestamp_weight = value,
        ScoringField::DirectionWeight => settings.direction_weight = value,
        ScoringField::TxShapeWeight => settings.tx_shape_weight = value,
        ScoringField::UnmatchedWeight => settings.unmatched_weight = value,
        ScoringField::AmbiguityWeight => settings.ambiguity_weight = value,
        ScoringField::AmountToleranceSats => settings.amount_tolerance_sats = value,
        ScoringField::TimestampToleranceHours => {
            settings.timestamp_tolerance_hours = value;
        }
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
                    if scoring() == next {
                        scoring.set(stored);
                    }
                    wrote = true;
                }
                Err(error) => {
                    scoring.set(saved());
                    toast
                        .error(
                            format!(
                                "Could not save on-chain linking settings: {error}. The previous values were restored.",
                            ),
                        );
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
