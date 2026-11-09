//! KPI cards: the generic card with an optional details overlay plus the
//! balance card shared by the addresses and flow screens.
//!
//! The base card always shows the primary value and its title. Cards that
//! carry detail values (the default-currency equivalent and the ledger entry
//! count of the exchange ledger KPIs) render a help button in the top-right
//! corner. Hovering the help button previews the details, and clicking it
//! pins the overlay open until it is clicked again. Only the help button
//! reveals the overlay, so moving the pointer over the rest of the card does
//! not change what is shown.
use crate::components::format::{format_btc, format_fiat};
use crate::domain::price::sats_to_fiat;
use dioxus::prelude::*;
/// One KPI card of a page header row.
#[component]
pub fn Kpi(
    title: String,
    value: Option<String>,
    #[props(default)] secondary_value: Option<String>,
    #[props(default)] supporting_value: Option<String>,
    #[props(default)] loading: bool,
) -> Element {
    let primary = if loading {
        "…".to_string()
    } else {
        value.unwrap_or_else(|| "-".to_string())
    };
    let has_details = secondary_value.is_some() || supporting_value.is_some();
    let mut details_open = use_signal(|| false);
    let mut help_hovered = use_signal(|| false);
    let help_label = format!("KPI details for {title}");
    let details_visible = has_details && (details_open() || help_hovered());
    rsx! {
        section { class: if details_visible { "md3-card md3-card-elevated md3-kpi md3-kpi-details-open" } else { "md3-card md3-card-elevated md3-kpi" },
            span { class: "md3-kpi-value text-headline-small leading-headline-small text-on-surface",
                "{primary}"
            }
            span { class: "md3-kpi-title text-title-small leading-title-small text-on-surface-variant",
                "{title}"
            }
            if has_details {
                button {
                    r#type: "button",
                    class: "md3-kpi-help",
                    aria_label: "{help_label}",
                    aria_expanded: details_open().to_string(),
                    onmouseenter: move |_| help_hovered.set(true),
                    onmouseleave: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        help_hovered.set(false),
                    onclick: move |_| {
                        if details_open() {
                            details_open.set(false);
                            help_hovered.set(false);
                        } else {
                            details_open.set(true);
                        }
                    },
                    "?"
                }
                div { class: "md3-kpi-details",
                    if let Some(secondary) = &secondary_value {
                        div { class: "md3-kpi-detail-row",
                            span { class: "md3-kpi-detail-value", "{secondary}" }
                        }
                    }
                    if let Some(supporting) = &supporting_value {
                        div { class: "md3-kpi-detail-row",
                            span { class: "md3-kpi-detail-value md3-kpi-detail-value-supporting",
                                "{supporting}"
                            }
                        }
                    }
                }
            }
        }
    }
}
/// Balance card of the addresses and flow screens.
///
/// The BTC balance is the primary value and the default-currency equivalent
/// lives in the details overlay, the same way the exchange amount cards show
/// their fiat values. This keeps the fiat balance from needing a card of its
/// own.
#[component]
pub fn BalanceKpi(
    #[props(default)] balance_sats: Option<i64>,
    #[props(default)] latest_price: Option<f64>,
    #[props(default)] currency: String,
    #[props(default)] loading: bool,
) -> Element {
    let (value, secondary_value) = balance_values(balance_sats, latest_price, &currency);
    rsx! {
        Kpi {
            title: "Balance".to_string(),
            value,
            secondary_value,
            loading,
        }
    }
}
/// Primary and detail values of the balance card. A missing latest price keeps
/// the details available and renders `-`, matching the unavailable state the
/// separate fiat card used to show; a missing balance hides the details.
fn balance_values(
    balance_sats: Option<i64>,
    latest_price: Option<f64>,
    currency: &str,
) -> (Option<String>, Option<String>) {
    match balance_sats {
        Some(sats) => (
            Some(format_btc(sats)),
            Some(
                latest_price
                    .map(|price| format_fiat(sats_to_fiat(sats, price), currency))
                    .unwrap_or_else(|| "-".to_string()),
            ),
        ),
        None => (None, None),
    }
}
#[cfg(test)]
mod tests {
    use super::balance_values;
    #[test]
    fn balance_values_show_btc_and_the_default_currency_value() {
        assert_eq!(
            balance_values(Some(100_000_000), Some(60_000.0), "USD"),
            (Some("1 ₿".to_string()), Some("$60,000.00".to_string())),
        );
        assert_eq!(
            balance_values(Some(100_000_000), Some(60_000.0), "EUR"),
            (Some("1 ₿".to_string()), Some("60,000.00 EUR".to_string())),
        );
    }
    #[test]
    fn balance_values_report_an_unavailable_fiat_value_as_a_dash() {
        assert_eq!(
            balance_values(Some(100_000_000), None, "USD"),
            (Some("1 ₿".to_string()), Some("-".to_string())),
        );
    }
    #[test]
    fn balance_values_hide_details_without_a_balance() {
        assert_eq!(balance_values(None, Some(60_000.0), "USD"), (None, None));
    }
}
