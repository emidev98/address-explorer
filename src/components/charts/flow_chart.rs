//! Flow chart component rendered with Apache ECharts.
//!
//! The component receives the domain data (nodes and links) and owns every
//! presentation detail: layer assignment, node positions, colors, arrows,
//! labels, tooltip, and ECharts option construction.

use std::collections::{BTreeMap, HashMap, HashSet};

use dioxus::prelude::*;
use serde_json::{json, Value};

use super::echarts::EChartsHost;
use super::theme;

/// Node colors by layer, as `(fill, label)` pairs of the Material 3 strong
/// chromatic roles; the paired `on-*` color keeps the node label contrast
/// the palette guarantees in both appearance modes.
const NODE_COLORS: [(&str, &str); 3] = [
    (theme::PRIMARY, theme::ON_PRIMARY),
    (theme::SECONDARY, theme::ON_SECONDARY),
    (theme::TERTIARY, theme::ON_TERTIARY),
];
const COLUMN_SPACING: f64 = 240.0;
const ROW_SPACING: f64 = 180.0;

/// A node in a flow chart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowNode {
    /// Stable identifier used by links. Must be unique within the chart.
    pub id: String,
    /// Human-readable label displayed inside the node.
    pub label: String,
}

impl FlowNode {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// A directed link between two [`FlowNode`] values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlowLink {
    /// Identifier of the source node.
    pub source: String,
    /// Identifier of the target node.
    pub target: String,
    /// Optional label drawn on the link. Empty strings are not displayed.
    pub label: String,
}

impl FlowLink {
    pub fn new(
        source: impl Into<String>,
        target: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
            label: label.into(),
        }
    }
}

/// Render a flow chart from nodes and directed links.
///
/// The component computes a left-to-right layered layout from the link
/// topology. Nodes that cannot be reached from a root stay in the first
/// layer, and cyclic input is handled without unbounded iteration.
#[component]
pub fn FlowChart(
    nodes: Vec<FlowNode>,
    links: Vec<FlowLink>,
    #[props(default = "Flow chart".to_string())] aria_label: String,
) -> Element {
    let option = build_option(&nodes, &links).to_string();

    rsx! {
        div { class: "md3-chart-scroll",
            EChartsHost { option, aria_label }
        }
    }
}

/// Build the complete ECharts option for the supplied flow data.
fn build_option(nodes: &[FlowNode], links: &[FlowLink]) -> Value {
    let positions = layered_positions(nodes, links);

    let node_data: Vec<Value> = nodes
        .iter()
        .map(|node| {
            let (x, y, layer) = positions.get(&node.id).copied().unwrap_or((0.0, 0.0, 0));
            let (fill, label_color) = NODE_COLORS[layer % NODE_COLORS.len()];

            json!({
                "id": node.id,
                "name": node.label,
                "x": x,
                "y": y,
                "itemStyle": {
                    "color": fill,
                },
                "label": {
                    "color": label_color,
                },
            })
        })
        .collect();

    let link_data: Vec<Value> = links
        .iter()
        .filter(|link| positions.contains_key(&link.source) && positions.contains_key(&link.target))
        .map(|link| {
            let mut data = json!({
                "source": link.source,
                "target": link.target,
            });

            if !link.label.is_empty() {
                data["value"] = json!(link.label);
            }

            data
        })
        .collect();

    json!({
        "animationDuration": 400,
        "animationDurationUpdate": 250,
        "aria": {
            "enabled": true,
        },
        "textStyle": theme::text_style(),
        "tooltip": theme::tooltip("item"),
        "series": [{
            "type": "graph",
            "layout": "none",
            "coordinateSystem": "view",
            "left": "6%",
            "right": "6%",
            "top": "8%",
            "bottom": "8%",
            "symbol": "roundRect",
            "symbolSize": [160, 48],
            "data": node_data,
            "links": link_data,
            "label": {
                "show": true,
                "position": "inside",
                "fontSize": 12,
                "fontWeight": 500,
                "formatter": "{b}",
            },
            "edgeSymbol": ["none", "arrow"],
            "edgeSymbolSize": [0, 10],
            "lineStyle": {
                "color": theme::OUTLINE,
                "width": 2,
                "opacity": 0.8,
                "curveness": 0.05,
            },
            "edgeLabel": {
                "show": true,
                "formatter": "{c}",
                "fontSize": 11,
                "color": theme::ON_SURFACE_VARIANT,
                "backgroundColor": theme::alpha("surface", 0.9),
                "padding": [2, 5],
                "borderRadius": 4,
            },
            "itemStyle": {
                "borderColor": theme::SURFACE,
                "borderWidth": 2,
            },
            "emphasis": {
                "focus": "adjacency",
                "lineStyle": {
                    "width": 3,
                },
            },
        }],
    })
}

/// Assign every node to a layer and a position.
///
/// Layers are based on the longest path from a root node, which keeps every
/// link pointing to the right. Nodes in the same layer are vertically centered
/// around the tallest column. The returned tuple is `(x, y, layer)`.
fn layered_positions(nodes: &[FlowNode], links: &[FlowLink]) -> HashMap<String, (f64, f64, usize)> {
    let node_ids: HashSet<&str> = nodes.iter().map(|node| node.id.as_str()).collect();
    let valid_links: Vec<(&str, &str)> = links
        .iter()
        .filter(|link| {
            node_ids.contains(link.source.as_str()) && node_ids.contains(link.target.as_str())
        })
        .map(|link| (link.source.as_str(), link.target.as_str()))
        .collect();

    let mut layers: HashMap<&str, usize> = nodes.iter().map(|node| (node.id.as_str(), 0)).collect();

    // Longest-path layering. Reading from a snapshot of the previous pass
    // avoids cascading updates inside one pass, and the bounded pass count
    // keeps cyclic input from looping forever.
    for _ in 1..nodes.len() {
        let previous = layers.clone();
        let mut changed = false;

        for &(source, target) in &valid_links {
            let next_layer = previous[source] + 1;
            if next_layer > layers[target] {
                layers.insert(target, next_layer);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    let mut columns: BTreeMap<usize, Vec<&FlowNode>> = BTreeMap::new();
    for node in nodes {
        columns
            .entry(layers[node.id.as_str()])
            .or_default()
            .push(node);
    }

    let tallest_column = columns.values().map(Vec::len).max().unwrap_or(1);
    let mut positions = HashMap::with_capacity(nodes.len());

    for (layer, column) in columns {
        let vertical_offset = (tallest_column - column.len()) as f64 * ROW_SPACING / 2.0;

        for (row, node) in column.into_iter().enumerate() {
            positions.insert(
                node.id.clone(),
                (
                    layer as f64 * COLUMN_SPACING,
                    vertical_offset + row as f64 * ROW_SPACING,
                    layer,
                ),
            );
        }
    }

    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, label: &str) -> FlowNode {
        FlowNode::new(id, label)
    }

    fn link(source: &str, target: &str, label: &str) -> FlowLink {
        FlowLink::new(source, target, label)
    }

    #[test]
    fn nodes_are_positioned_by_longest_path_layer() {
        let nodes = vec![
            node("wallet", "Wallet"),
            node("mempool", "Mempool"),
            node("address", "Address"),
        ];
        let links = vec![
            link("wallet", "mempool", "broadcast"),
            link("wallet", "address", ""),
            link("mempool", "address", "confirmed"),
        ];

        let positions = layered_positions(&nodes, &links);

        assert_eq!(positions["wallet"].2, 0);
        assert_eq!(positions["mempool"].2, 1);
        assert_eq!(positions["address"].2, 2);
        assert_eq!(positions["wallet"].0, 0.0);
        assert_eq!(positions["mempool"].0, COLUMN_SPACING);
        assert_eq!(positions["address"].0, 2.0 * COLUMN_SPACING);
    }

    #[test]
    fn nodes_in_a_column_are_vertically_centered() {
        let nodes = vec![
            node("root", "Root"),
            node("left", "Left"),
            node("right", "Right"),
        ];
        let links = vec![link("root", "left", ""), link("root", "right", "")];

        let positions = layered_positions(&nodes, &links);

        assert_eq!(positions["left"].1, 0.0);
        assert_eq!(positions["right"].1, ROW_SPACING);
    }

    #[test]
    fn links_use_node_ids_while_labels_use_display_names() {
        let nodes = vec![node("wallet", "Wallet"), node("mempool", "Mempool")];
        let links = vec![link("wallet", "mempool", "broadcast")];

        let option = build_option(&nodes, &links);
        let data = option["series"][0]["data"]
            .as_array()
            .expect("node data array");
        let links = option["series"][0]["links"]
            .as_array()
            .expect("link data array");

        assert_eq!(data[0]["id"], "wallet");
        assert_eq!(data[0]["name"], "Wallet");
        assert_eq!(links[0]["source"], "wallet");
        assert_eq!(links[0]["target"], "mempool");
        assert_eq!(links[0]["value"], "broadcast");
    }

    #[test]
    fn links_with_missing_nodes_are_ignored() {
        let nodes = vec![node("wallet", "Wallet")];
        let links = vec![link("wallet", "missing", "broadcast")];

        let option = build_option(&nodes, &links);
        let links = option["series"][0]["links"]
            .as_array()
            .expect("link data array");

        assert!(links.is_empty());
    }

    #[test]
    fn cyclic_links_finish_with_bounded_layers() {
        let nodes = vec![node("a", "A"), node("b", "B")];
        let links = vec![link("a", "b", ""), link("b", "a", "")];

        let positions = layered_positions(&nodes, &links);

        assert_eq!(positions.len(), 2);
        assert!(positions["a"].2 < nodes.len());
        assert!(positions["b"].2 < nodes.len());
    }

    #[test]
    fn empty_data_produces_an_empty_series() {
        let option = build_option(&[], &[]);
        let data = option["series"][0]["data"]
            .as_array()
            .expect("node data array");

        assert!(data.is_empty());
    }

    #[test]
    fn nodes_and_edges_use_paired_theme_color_tokens() {
        let nodes = vec![
            node("a", "A"),
            node("b", "B"),
            node("c", "C"),
            node("d", "D"),
        ];
        let links = vec![link("a", "b", ""), link("b", "c", ""), link("c", "d", "")];

        let option = build_option(&nodes, &links);
        let data = option["series"][0]["data"].as_array().expect("node data");

        // Layers cycle the strong chromatic roles with their on-colors.
        assert_eq!(data[0]["itemStyle"]["color"], theme::PRIMARY);
        assert_eq!(data[0]["label"]["color"], theme::ON_PRIMARY);
        assert_eq!(data[1]["itemStyle"]["color"], theme::SECONDARY);
        assert_eq!(data[1]["label"]["color"], theme::ON_SECONDARY);
        assert_eq!(data[2]["itemStyle"]["color"], theme::TERTIARY);
        assert_eq!(data[3]["itemStyle"]["color"], theme::PRIMARY);

        assert_eq!(option["series"][0]["lineStyle"]["color"], theme::OUTLINE);
        assert_eq!(
            option["series"][0]["edgeLabel"]["backgroundColor"],
            theme::alpha("surface", 0.9)
        );
    }
}
