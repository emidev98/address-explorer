use dioxus::prelude::*;

use crate::components::{FlowChart, FlowLink, FlowNode};

/// Flow screen: demo transaction lifecycle rendered with Apache ECharts.
#[component]
pub fn Flow() -> Element {
    let nodes = vec![
        FlowNode::new("wallet", "Wallet"),
        FlowNode::new("mempool", "Mempool"),
        FlowNode::new("confirmed", "Confirmed block"),
        FlowNode::new("dropped", "Dropped"),
        FlowNode::new("address", "Address"),
        FlowNode::new("report", "Flow report"),
    ];

    let links = vec![
        FlowLink::new("wallet", "mempool", "broadcast"),
        FlowLink::new("mempool", "confirmed", "mined"),
        FlowLink::new("mempool", "dropped", "evicted"),
        FlowLink::new("confirmed", "address", "credit"),
        FlowLink::new("address", "report", "summarize"),
    ];

    rsx! {
        section { id: "flow", class: "md3-screen",
            h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                "Flow"
            }
            p { class: "text-body-medium text-on-surface-variant",
                "A demo transaction lifecycle rendered with Apache ECharts. Replace the sample data with real address flows."
            }

            section { class: "md3-chart-card",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        "Transaction lifecycle"
                    }
                    p { class: "text-body-medium text-on-surface-variant",
                        "Hover over or tap a node to highlight its connections."
                    }
                }

                FlowChart {
                    nodes,
                    links,
                    aria_label: "Transaction lifecycle flow chart".to_string(),
                }
            }
        }
    }
}
