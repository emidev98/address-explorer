//! KPI card: a large formatted value with a small subtitle title.
//!
//! Replicates the UTXO Utils `Kpi` card used across the top of its pages:
//! while `loading` is set a placeholder line stands in for the value, and a
//! missing value renders as `-`.

use dioxus::prelude::*;

/// One KPI card of a page header row.
#[component]
pub fn Kpi(title: String, value: Option<String>, #[props(default)] loading: bool) -> Element {
    rsx! {
        section { class: "md3-card md3-card-elevated md3-kpi",
            if loading {
                span {
                    class: "md3-kpi-value text-headline-small leading-headline-small",
                    aria_hidden: "true",
                    "…"
                }
                span { class: "md3-kpi-title text-title-small leading-title-small text-on-surface-variant",
                    "{title}"
                }
            } else {
                span { class: "md3-kpi-value text-headline-small leading-headline-small text-on-surface",
                    "{value.as_deref().unwrap_or(\"-\")}"
                }
                span { class: "md3-kpi-title text-title-small leading-title-small text-on-surface-variant",
                    "{title}"
                }
            }
        }
    }
}
