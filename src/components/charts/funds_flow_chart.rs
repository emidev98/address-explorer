//! Funds-flow Sankey chart.
//!
//! The screen supplies the domain graph; this component owns every ECharts
//! presentation detail, including node colors, labels, tooltips, layout and
//! the taller responsive container. It never fetches or persists data.
use super::echarts::EChartsHost;
use super::theme;
use crate::components::format::{format_btc, format_fiat};
use crate::domain::civil_time::format_datetime;
use crate::domain::flow::{
    FlowGraph, FlowLink, FlowLinkKind, FlowNode, FlowNodeKind, FlowOwnerKind,
};
use crate::domain::price::sats_to_fiat;
use dioxus::prelude::*;
use serde_json::{json, Value};
/// Current-value context for tooltips. Fiat is deliberately a presentation
/// input: the section 3.1 default-currency rules are resolved by the screen,
/// and the flow widths always remain BTC.
#[derive(Debug, Clone, PartialEq)]
pub struct FlowFiatContext {
    pub symbol: String,
    pub btc_price: f64,
}
impl FlowFiatContext {
    fn format(&self, sats: i64) -> String {
        format_fiat(sats_to_fiat(sats, self.btc_price), &self.symbol)
    }
}
/// Render the funds-flow graph as a horizontal Sankey.
#[component]
pub fn FundsFlowChart(
    graph: FlowGraph,
    #[props(default)] fiat: Option<FlowFiatContext>,
    #[props(default = String::new())] aria_label: String,
) -> Element {
    let option = build_option(&graph, fiat.as_ref()).to_string();
    let aria_label = if aria_label.is_empty() {
        format!(
            "Funds flow Sankey with {} nodes, {} links and {} transactions.",
            graph.stats.node_count, graph.stats.link_count, graph.stats.transaction_count,
        )
    } else {
        aria_label
    };
    let height = format!("{}px", (graph.nodes.len() * 22).clamp(560, 1_800));
    rsx! {
        EChartsHost { option, aria_label, height: Some(height) }
    }
}
/// Build the complete ECharts option. Pure so it can be unit-tested without a
/// browser or a Dioxus runtime.
fn build_option(graph: &FlowGraph, fiat: Option<&FlowFiatContext>) -> Value {
    let nodes = graph
        .nodes
        .iter()
        .map(|node| node_data(node, fiat))
        .collect::<Vec<_>>();
    let links = graph
        .links
        .iter()
        .map(|link| link_data(link, graph, fiat))
        .collect::<Vec<_>>();
    let mut tooltip = theme::tooltip("item");
    tooltip["renderMode"] = json!("richText");
    tooltip["formatter"] = json!("@flow-tooltip");
    tooltip["confine"] = json!(true);
    json!(
        { "animation" : false, "aria" : { "enabled" : true, }, "textStyle" :
        theme::text_style(), "tooltip" : tooltip, "series" : [{ "type" : "sankey", "left"
        : 24, "right" : 210, "top" : 18, "bottom" : 18, "nodeWidth" : 14, "nodeGap" : 8,
        "nodeAlign" : "justify", "orient" : "horizontal", "layoutIterations" : 16,
        "draggable" : false, "roam" : true, "scaleLimit" : { "min" : 0.3, "max" : 5, },
        "emphasis" : { "focus" : "adjacency", }, "lineStyle" : { "color" : "gradient",
        "opacity" : 0.24, "curveness" : 0.5, }, "label" : { "color" : theme::ON_SURFACE,
        "fontFamily" : theme::FONT_FAMILY, "fontSize" : 11, "lineHeight" : 16, "width" :
        180, "overflow" : "truncate", }, "data" : nodes, "links" : links, }], }
    )
}
fn node_data(node: &FlowNode, fiat: Option<&FlowFiatContext>) -> Value {
    let (fill, label_color) = node_colors(node);
    let display_value = node.in_sats.max(node.out_sats);
    let label_text = format!("{}\n{}", node.label, format_btc(display_value));
    let show_label = node.kind != FlowNodeKind::Transaction;
    let label_position = if node.out_sats == 0 { "left" } else { "right" };
    let border_color = node
        .owner
        .as_ref()
        .and_then(|owner| owner.brand_color.clone())
        .unwrap_or_else(|| theme::OUTLINE_VARIANT.to_string());
    let border_width = if node
        .owner
        .as_ref()
        .and_then(|owner| owner.brand_color.as_ref())
        .is_some()
    {
        2
    } else {
        1
    };
    json!(
        { "name" : node.id, "itemStyle" : { "color" : fill, "borderColor" : border_color,
        "borderWidth" : border_width, "borderRadius" : 2, }, "label" : { "show" :
        show_label, "formatter" : label_text, "position" : label_position, "color" :
        label_color, "fontFamily" : theme::FONT_FAMILY, "fontSize" : 11, "lineHeight" :
        16, "width" : 180, "overflow" : "truncate", }, "tooltipText" : node_tooltip(node,
        fiat), }
    )
}
fn link_data(link: &FlowLink, graph: &FlowGraph, fiat: Option<&FlowFiatContext>) -> Value {
    let source = graph
        .nodes
        .iter()
        .find(|node| node.id == link.source)
        .map(|node| node.label.as_str())
        .unwrap_or(link.source.as_str());
    let target = graph
        .nodes
        .iter()
        .find(|node| node.id == link.target)
        .map(|node| node.label.as_str())
        .unwrap_or(link.target.as_str());
    json!(
        { "source" : link.source, "target" : link.target, "value" : link.value_sats,
        "tooltipText" : link_tooltip(link, source, target, fiat), }
    )
}
fn node_colors(node: &FlowNode) -> (&'static str, &'static str) {
    match node.kind {
        FlowNodeKind::Transaction => (theme::OUTLINE_VARIANT, theme::ON_SURFACE),
        FlowNodeKind::Exchange => (theme::PRIMARY_CONTAINER, theme::ON_PRIMARY_CONTAINER),
        FlowNodeKind::Wallet => (theme::SECONDARY_CONTAINER, theme::ON_SECONDARY_CONTAINER),
        FlowNodeKind::Address => (theme::TERTIARY_CONTAINER, theme::ON_TERTIARY_CONTAINER),
        FlowNodeKind::ExternalSource
        | FlowNodeKind::ExternalDestination
        | FlowNodeKind::External => (theme::SURFACE_CONTAINER, theme::ON_SURFACE),
        FlowNodeKind::Coinbase => (theme::PRIMARY, theme::ON_PRIMARY),
        FlowNodeKind::Unlinked => (theme::ERROR_CONTAINER, theme::ON_ERROR_CONTAINER),
    }
}
fn node_tooltip(node: &FlowNode, fiat: Option<&FlowFiatContext>) -> String {
    let mut lines = vec![node.label.clone()];
    if node.kind == FlowNodeKind::Transaction {
        lines.push("On-chain transaction".to_string());
    } else {
        lines.push(node_kind_label(node.kind).to_string());
    }
    if let Some(owner) = &node.owner {
        let kind = match owner.kind {
            FlowOwnerKind::Exchange => "Exchange",
            FlowOwnerKind::Wallet => "Wallet",
        };
        lines.push(format!("{kind}: {}", owner.name));
    }
    if let Some(address) = &node.address {
        lines.push(address.clone());
    }
    if node.kind == FlowNodeKind::Transaction {
        lines.push(format!(
            "Inputs {} · Outputs {}",
            format_btc(node.in_sats),
            format_btc(node.out_sats),
        ));
    } else {
        lines.push(format!(
            "In {} · Out {}",
            format_btc(node.in_sats),
            format_btc(node.out_sats),
        ));
        if node.residual_sats() > 0 {
            lines.push(format!("Remaining {}", format_btc(node.residual_sats())));
        }
    }
    if let Some(balance) = node.balance_sats {
        lines.push(format!("Balance {}", format_btc(balance)));
    }
    if let Some(first) = node.first_time {
        lines.push(format!("First activity {}", format_datetime(first)));
    }
    if let Some(last) = node.last_time {
        lines.push(format!("Last activity {}", format_datetime(last)));
    }
    let events = if node.kind == FlowNodeKind::Transaction {
        1
    } else {
        node.transaction_count.max(1)
    };
    lines.push(format!(
        "{events} contributing transaction{}",
        if events == 1 { "" } else { "s" },
    ));
    if let Some(note) = &node.note {
        lines.push(note.clone());
    }
    if let Some(fiat) = fiat {
        let value = node.in_sats.max(node.out_sats);
        if value > 0 {
            lines.push(format!("Current value: {}", fiat.format(value)));
        }
    }
    lines.join("\n")
}
fn link_tooltip(
    link: &FlowLink,
    source: &str,
    target: &str,
    fiat: Option<&FlowFiatContext>,
) -> String {
    let mut lines = vec![format!("{source} → {target}")];
    lines.push(format_btc(link.value_sats));
    if let Some(fiat) = fiat {
        lines.push(format!("Current value: {}", fiat.format(link.value_sats)));
    }
    lines.push(format!(
        "{} event{}",
        link.event_count,
        if link.event_count == 1 { "" } else { "s" },
    ));
    if let Some(first) = link.first_time {
        lines.push(format!("First {}", format_datetime(first)));
    }
    if let Some(last) = link.last_time {
        lines.push(format!("Last {}", format_datetime(last)));
    }
    lines.push(
        match link.kind {
            FlowLinkKind::OnChain => "On-chain flow",
            FlowLinkKind::LedgerUnlinked => "Unmatched exchange ledger",
            FlowLinkKind::Aggregated => "Aggregated flow",
        }
        .to_string(),
    );
    lines.join("\n")
}
fn node_kind_label(kind: FlowNodeKind) -> &'static str {
    match kind {
        FlowNodeKind::Transaction => "Transaction",
        FlowNodeKind::Exchange => "Exchange",
        FlowNodeKind::Wallet => "Wallet",
        FlowNodeKind::Address => "Address",
        FlowNodeKind::ExternalSource => "External source",
        FlowNodeKind::ExternalDestination => "External destination",
        FlowNodeKind::External => "External address",
        FlowNodeKind::Coinbase => "Coinbase / mining",
        FlowNodeKind::Unlinked => "Unlinked ledger",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::flow::{
        FlowEvidenceRef, FlowGraphStats, FlowKpis, FlowNodeKind, FlowOwnerRef,
    };
    fn node(id: &str, kind: FlowNodeKind) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: id.to_string(),
            kind,
            owner: None,
            address: None,
            address_count: 1,
            transaction_count: 1,
            in_sats: 100,
            out_sats: 50,
            balance_sats: None,
            first_time: Some(1_700_000_000),
            last_time: Some(1_700_000_100),
            note: None,
        }
    }
    fn graph() -> FlowGraph {
        let mut source = node("source", FlowNodeKind::ExternalSource);
        source.owner = Some(FlowOwnerRef {
            kind: FlowOwnerKind::Exchange,
            id: Some(1),
            name: "Binance".to_string(),
            brand_color: Some("#F0B90B".to_string()),
        });
        FlowGraph {
            nodes: vec![source, node("tx", FlowNodeKind::Transaction)],
            links: vec![FlowLink {
                source: "source".to_string(),
                target: "tx".to_string(),
                value_sats: 50,
                event_count: 1,
                first_time: Some(1_700_000_000),
                last_time: Some(1_700_000_100),
                kind: FlowLinkKind::OnChain,
                evidence: vec![FlowEvidenceRef {
                    txid: Some("tx".to_string()),
                    ledger_id: None,
                    timestamp: Some(1_700_000_000),
                    value_sats: 50,
                    description: "test".to_string(),
                }],
            }],
            stats: FlowGraphStats {
                node_count: 2,
                link_count: 1,
                transaction_count: 1,
                output_count: 1,
            },
            kpis: FlowKpis::default(),
            exchange_activity: Default::default(),
            warnings: Vec::new(),
        }
    }
    #[test]
    fn option_contains_a_sankey_with_unique_nodes_and_the_tooltip_marker() {
        let option = build_option(&graph(), None);
        assert_eq!(option["series"][0]["type"], "sankey");
        assert_eq!(option["tooltip"]["formatter"], "@flow-tooltip");
        assert_eq!(option["series"][0]["data"].as_array().unwrap().len(), 2);
        assert_eq!(option["series"][0]["links"].as_array().unwrap().len(), 1);
        let names: Vec<&str> = option["series"][0]["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|node| node["name"].as_str().unwrap())
            .collect();
        let unique: std::collections::HashSet<_> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len());
    }
    #[test]
    fn transaction_nodes_are_unlabelled_while_entities_are_labelled() {
        let option = build_option(&graph(), None);
        let data = option["series"][0]["data"].as_array().unwrap();
        assert_eq!(data[0]["label"]["show"], true);
        assert_eq!(data[1]["label"]["show"], false);
    }
    #[test]
    fn theme_tokens_and_provider_accents_are_used() {
        let option = build_option(&graph(), None);
        let data = option["series"][0]["data"].as_array().unwrap();
        assert_eq!(data[0]["itemStyle"]["borderColor"], "#F0B90B");
        assert_eq!(data[0]["itemStyle"]["color"], theme::SURFACE_CONTAINER);
        assert_eq!(data[1]["itemStyle"]["color"], theme::OUTLINE_VARIANT);
    }
    #[test]
    fn tooltips_include_btc_and_current_fiat() {
        let fiat = FlowFiatContext {
            symbol: "EUR".to_string(),
            btc_price: 60_000.0,
        };
        let option = build_option(&graph(), Some(&fiat));
        let text = option["series"][0]["links"][0]["tooltipText"]
            .as_str()
            .unwrap();
        assert!(text.contains("0.0000005"));
        assert!(text.contains("EUR"));
    }
    #[test]
    fn labels_use_the_theme_ui_font() {
        let option = build_option(&graph(), None);
        assert_eq!(
            option["series"][0]["label"]["fontFamily"],
            theme::FONT_FAMILY
        );
        assert_eq!(
            option["series"][0]["data"][0]["label"]["fontFamily"],
            theme::FONT_FAMILY,
        );
    }
}
