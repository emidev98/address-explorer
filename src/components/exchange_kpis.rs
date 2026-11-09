//! Shared KPI row of the exchanges list and exchange detail screens.
use crate::components::format::{format_btc, format_fiat};
use crate::components::Kpi;
use crate::domain::exchange::{ExchangeKpiMetric, ExchangeKpis};
use dioxus::prelude::*;
/// State of the total-exchange-count card shown by the exchanges list. The
/// exchange detail screen keeps the default [`Self::Hidden`], since a single
/// account's screen has no total to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExchangeCountKpi {
    /// The card is not part of the grid.
    #[default]
    Hidden,
    /// The list is still loading, so the count is not known yet.
    Loading,
    /// The list failed to load, so no count is available.
    Unavailable,
    /// Number of exchange accounts across all pages.
    Count(u64),
}
impl ExchangeCountKpi {
    /// Rendered card value; `None` hides the card.
    fn value(self) -> Option<String> {
        match self {
            Self::Hidden => None,
            Self::Loading => Some("…".to_string()),
            Self::Unavailable => Some("-".to_string()),
            Self::Count(count) => Some(count.to_string()),
        }
    }
}
/// One amount card of the exchange KPI row. The kind owns the card title and
/// the noun used for the ledger entries counted in its details overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AmountKpiKind {
    Bought,
    Sold,
    Withdrawn,
    Deposited,
    Rewarded,
    Fees,
}
impl AmountKpiKind {
    fn title(self) -> &'static str {
        match self {
            Self::Bought => "Bought",
            Self::Sold => "Sold",
            Self::Withdrawn => "Withdrawn",
            Self::Deposited => "Deposited",
            Self::Rewarded => "Rewarded",
            Self::Fees => "Fees",
        }
    }
    /// Singular and plural names of the ledger entries that contribute to the
    /// metric.
    fn count_nouns(self) -> (&'static str, &'static str) {
        match self {
            Self::Bought => ("purchase", "purchases"),
            Self::Sold => ("sale", "sales"),
            Self::Withdrawn => ("withdrawal", "withdrawals"),
            Self::Deposited => ("deposit", "deposits"),
            Self::Rewarded => ("reward", "rewards"),
            Self::Fees => ("transaction", "transactions"),
        }
    }
}
/// The six ledger amount cards plus the matched on-chain ratio, optionally
/// preceded by the total count of exchanges. Hovering an amount card's help
/// button previews the default-currency value and ledger count over the
/// base content; clicking it pins them open until it is clicked again.
///
/// Every card owns its own data slot, so a missing metric renders the shared
/// `-` placeholder and a loading row renders `…`. The row therefore needs no
/// separate placeholder definitions to keep in sync with the real cards.
#[component]
pub fn ExchangeKpiGrid(
    kpis: Option<ExchangeKpis>,
    #[props(default)] exchange_count: ExchangeCountKpi,
    #[props(default)] loading: bool,
) -> Element {
    let count_value = exchange_count.value();
    let grid_class = if count_value.is_some() {
        "md3-kpi-grid md3-kpi-grid-eight"
    } else {
        "md3-kpi-grid md3-kpi-grid-seven"
    };
    let currency = kpis
        .as_ref()
        .map(|kpis| kpis.fiat_currency.clone())
        .unwrap_or_default();
    rsx! {
        div { class: grid_class,
            if let Some(value) = count_value {
                Kpi { title: "Exchanges".to_string(), value: Some(value) }
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Bought,
                metric: kpis.as_ref().map(|kpis| kpis.bought),
                currency: currency.clone(),
                loading,
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Sold,
                metric: kpis.as_ref().map(|kpis| kpis.sold),
                currency: currency.clone(),
                loading,
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Withdrawn,
                metric: kpis
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .as_ref().map(| kpis | kpis.withdrawn),
                currency: currency.clone(),
                loading,
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Deposited,
                metric: kpis.as_ref().map(|
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        kpis | kpis.deposited),
                currency: currency.clone(),
                loading,
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Rewarded,
                metric: kpis.as_ref().map(| kpis | kpis
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .rewarded),
                currency: currency.clone(),
                loading,
            }
            ExchangeAmountKpi {
                kind: AmountKpiKind::Fees,
                metric: kpis.as_ref().map(|kpis| kpis.fees),
                currency,
                loading,
            }
            ExchangeOnChainKpi {
                matched: kpis.as_ref().map(| kpis | kpis
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .onchain_matched.count),
                total: kpis.as_ref().map(| kpis | kpis
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .onchain_total_count),
                loading,
            }
        }
    }
}
/// One BTC amount card. Its metric is the card's own data slot: `None` while
/// the row loads or when the row failed, which [`Kpi`] renders as `…` or `-`.
#[component]
fn ExchangeAmountKpi(
    kind: AmountKpiKind,
    metric: Option<ExchangeKpiMetric>,
    #[props(default)] currency: String,
    #[props(default)] loading: bool,
) -> Element {
    let (btc, fiat, count) = match metric {
        Some(metric) => {
            let (btc, fiat, count) = amount_values(metric, &currency, kind.count_nouns());
            (Some(btc), Some(fiat), Some(count))
        }
        None => (None, None, None),
    };
    rsx! {
        Kpi {
            title: kind.title().to_string(),
            value: btc,
            secondary_value: fiat,
            supporting_value: count,
            loading,
        }
    }
}
/// The matched on-chain operations card. It shows the matched count over the
/// total deposits and withdrawals and has no details overlay.
#[component]
fn ExchangeOnChainKpi(
    #[props(default)] matched: Option<i64>,
    #[props(default)] total: Option<i64>,
    #[props(default)] loading: bool,
) -> Element {
    let value = matched
        .zip(total)
        .map(|(matched, total)| onchain_value(matched, total));
    rsx! {
        Kpi {
            title: "matched on chain operations".to_string(),
            value,
            loading,
        }
    }
}
fn amount_values(
    metric: ExchangeKpiMetric,
    currency: &str,
    count_nouns: (&str, &str),
) -> (String, String, String) {
    let (singular, plural) = count_nouns;
    (
        format_btc(metric.sats),
        format_fiat(metric.fiat, currency),
        count_label(metric.count, singular, plural),
    )
}
fn onchain_value(matched: i64, total: i64) -> String {
    format!("{matched} / {total}")
}
fn count_label(count: i64, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("1 {singular}")
    } else {
        format!("{count} {plural}")
    }
}
#[cfg(test)]
mod tests {
    use super::{amount_values, count_label, onchain_value, AmountKpiKind, ExchangeCountKpi};
    use crate::domain::exchange::ExchangeKpiMetric;
    #[test]
    fn count_labels_are_pluralized() {
        assert_eq!(count_label(0, "withdrawal", "withdrawals"), "0 withdrawals");
        assert_eq!(count_label(1, "withdrawal", "withdrawals"), "1 withdrawal");
        assert_eq!(count_label(2, "withdrawal", "withdrawals"), "2 withdrawals");
    }
    #[test]
    fn amount_card_count_nouns_name_each_metric() {
        assert_eq!(
            AmountKpiKind::Bought.count_nouns(),
            ("purchase", "purchases")
        );
        assert_eq!(AmountKpiKind::Sold.count_nouns(), ("sale", "sales"));
        assert_eq!(
            AmountKpiKind::Withdrawn.count_nouns(),
            ("withdrawal", "withdrawals"),
        );
        assert_eq!(
            AmountKpiKind::Deposited.count_nouns(),
            ("deposit", "deposits")
        );
        assert_eq!(AmountKpiKind::Rewarded.count_nouns(), ("reward", "rewards"));
        assert_eq!(
            AmountKpiKind::Fees.count_nouns(),
            ("transaction", "transactions")
        );
    }
    #[test]
    fn exchange_count_card_maps_every_state_to_a_value() {
        assert_eq!(ExchangeCountKpi::Hidden.value(), None);
        assert_eq!(ExchangeCountKpi::Loading.value(), Some("…".to_string()));
        assert_eq!(ExchangeCountKpi::Unavailable.value(), Some("-".to_string()));
        assert_eq!(ExchangeCountKpi::Count(12).value(), Some("12".to_string()));
    }
    #[test]
    fn amount_cards_format_btc_the_default_currency_and_the_ledger_count() {
        let metric = ExchangeKpiMetric {
            sats: 100_000_000,
            fiat: 60_000.0,
            count: 1,
        };
        assert_eq!(
            amount_values(metric, "USD", AmountKpiKind::Bought.count_nouns()),
            (
                "1 ₿".to_string(),
                "$60,000.00".to_string(),
                "1 purchase".to_string()
            ),
        );
        assert_eq!(
            amount_values(metric, "EUR", AmountKpiKind::Fees.count_nouns()),
            (
                "1 ₿".to_string(),
                "60,000.00 EUR".to_string(),
                "1 transaction".to_string(),
            ),
        );
    }
    #[test]
    fn onchain_card_formats_the_matched_ratio() {
        assert_eq!(onchain_value(0, 0), "0 / 0");
        assert_eq!(onchain_value(2, 5), "2 / 5");
    }
}
