//! Holdings distribution chart: a treemap of the spendable balance per
//! address, rendered with Apache ECharts.
//!
//! Replicates the UTXO Utils "Aggregated coins by address" treemap: one tile
//! per address with a non-zero spendable balance, sized by the balance in
//! satoshis, sorted descending, colored by balance range, and labeled with
//! the BTC value and its share of the total. The component owns every
//! presentation detail; the screen only supplies the business data.
use super::super::format::format_btc;
use super::echarts::EChartsHost;
use super::theme;
use dioxus::prelude::*;
use serde_json::{json, Value};
const CHART_TITLE: &str = "Aggregated coins by address";
/// Color ranges of the treemap tiles, in satoshis, as `(fill, label)` pairs
/// of Material 3 theme roles with escalating emphasis: the paired `on-*`
/// color keeps the label contrast the palette guarantees in both appearance
/// modes. Anything at or above the last range's lower bound uses the last
/// pair.
const COLOR_RANGES: [(i64, &str, &str); 4] = [
    (0, theme::SECONDARY_CONTAINER, theme::ON_SECONDARY_CONTAINER),
    (
        1_000,
        theme::TERTIARY_CONTAINER,
        theme::ON_TERTIARY_CONTAINER,
    ),
    (
        100_000,
        theme::PRIMARY_CONTAINER,
        theme::ON_PRIMARY_CONTAINER,
    ),
    (100_000_000, theme::PRIMARY, theme::ON_PRIMARY),
];
/// One tile of the holdings distribution treemap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingSlice {
    /// Display label of the address (its description or shortened address).
    pub label: String,
    /// Spendable balance of the address, in satoshis.
    pub balance_sats: i64,
}
/// Render the "Aggregated coins by address" treemap.
///
/// Zero-balance slices are dropped (the UTXO Utils chart ignores them), and
/// the remaining slices are sorted by balance descending. An empty input
/// renders an empty treemap; the screen decides whether to show the chart
/// at all.
#[component]
pub fn HoldingsDistributionChart(
    entries: Vec<HoldingSlice>,
    #[props(default = CHART_TITLE.to_string())] aria_label: String,
) -> Element {
    let option = build_option(&entries).to_string();
    rsx! {
        EChartsHost { option, aria_label }
    }
}
/// Build the complete ECharts option for the supplied holdings.
fn build_option(entries: &[HoldingSlice]) -> Value {
    let mut slices: Vec<&HoldingSlice> = entries
        .iter()
        .filter(|entry| entry.balance_sats != 0)
        .collect();
    slices.sort_by(|left, right| {
        right
            .balance_sats
            .cmp(&left.balance_sats)
            .then_with(|| left.label.cmp(&right.label))
    });
    let total: i128 = slices.iter().map(|entry| entry.balance_sats as i128).sum();
    let data: Vec<Value> = slices
        .iter()
        .map(|entry| {
            let share = if total > 0 {
                entry.balance_sats as f64 / total as f64 * 100.0
            } else {
                0.0
            };
            let value = format!("{} ({share:.2}%)", format_btc(entry.balance_sats));
            let (fill, label_color) = tile_colors(entry.balance_sats);
            json!(
                { "name" : entry.label, "value" : entry.balance_sats, "itemStyle" : {
                "color" : fill, }, "label" : { "formatter" : format!("{{b}}\n{value}"),
                "color" : label_color, "fontFamily" : theme::FONT_FAMILY, }, "tooltip" :
                { "formatter" : format!("{}\n{value}", entry.label), }, }
            )
        })
        .collect();
    json!(
        { "animation" : false, "aria" : { "enabled" : true, }, "textStyle" :
        theme::text_style(), "title" : { "text" : CHART_TITLE, "left" : "center", "top" :
        0, "textStyle" : { "fontSize" : 14, "fontFamily" : theme::FONT_HEADING, "color" :
        theme::ON_SURFACE, }, }, "tooltip" : theme::tooltip("item"), "series" : [{ "type"
        : "treemap", "left" : 0, "right" : 0, "top" : 36, "bottom" : 0, "roam" : false,
        "nodeClick" : false, "breadcrumb" : { "show" : false, }, "itemStyle" : {
        "borderColor" : theme::SURFACE_CONTAINER_LOW, "borderWidth" : 2, "gapWidth" : 2,
        }, "label" : { "show" : true, "position" : "inside", "fontSize" : 12,
        "fontFamily" : theme::FONT_FAMILY, "lineHeight" : 20, "overflow" : "truncate", },
        "data" : data, }], }
    )
}
/// Tile fill and label colors for a balance, following the escalating theme
/// color ranges.
fn tile_colors(balance_sats: i64) -> (&'static str, &'static str) {
    COLOR_RANGES
        .iter()
        .rev()
        .find(|(from, _, _)| balance_sats >= *from)
        .map(|(_, fill, label)| (*fill, *label))
        .unwrap_or((COLOR_RANGES[0].1, COLOR_RANGES[0].2))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn slice(label: &str, balance_sats: i64) -> HoldingSlice {
        HoldingSlice {
            label: label.to_string(),
            balance_sats,
        }
    }
    #[test]
    fn zero_balances_are_dropped_and_tiles_sorted_descending() {
        let entries = vec![
            slice("small", 500),
            slice("empty", 0),
            slice("large", 150_000),
        ];
        let option = build_option(&entries);
        let data = option["series"][0]["data"].as_array().expect("tile data");
        assert_eq!(data.len(), 2);
        assert_eq!(data[0]["name"], "large");
        assert_eq!(data[0]["value"], 150_000);
        assert_eq!(data[1]["name"], "small");
        assert_eq!(data[1]["value"], 500);
    }
    #[test]
    fn tiles_use_the_escalating_theme_color_scale() {
        assert_eq!(
            tile_colors(0),
            (theme::SECONDARY_CONTAINER, theme::ON_SECONDARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(999),
            (theme::SECONDARY_CONTAINER, theme::ON_SECONDARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(1_000),
            (theme::TERTIARY_CONTAINER, theme::ON_TERTIARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(99_999),
            (theme::TERTIARY_CONTAINER, theme::ON_TERTIARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(100_000),
            (theme::PRIMARY_CONTAINER, theme::ON_PRIMARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(99_999_999),
            (theme::PRIMARY_CONTAINER, theme::ON_PRIMARY_CONTAINER),
        );
        assert_eq!(
            tile_colors(100_000_000),
            (theme::PRIMARY, theme::ON_PRIMARY)
        );
        assert_eq!(
            tile_colors(10_000_000_000),
            (theme::PRIMARY, theme::ON_PRIMARY)
        );
        assert_eq!(
            tile_colors(-1),
            (theme::SECONDARY_CONTAINER, theme::ON_SECONDARY_CONTAINER),
        );
    }
    #[test]
    fn tile_labels_carry_the_btc_value_and_total_share() {
        let entries = vec![slice("savings", 150_000_000), slice("spending", 50_000_000)];
        let option = build_option(&entries);
        let data = option["series"][0]["data"].as_array().expect("tile data");
        assert_eq!(data[0]["label"]["formatter"], "{b}\n1.5 ₿ (75.00%)");
        assert_eq!(data[1]["label"]["formatter"], "{b}\n0.5 ₿ (25.00%)");
        assert_eq!(data[0]["tooltip"]["formatter"], "savings\n1.5 ₿ (75.00%)");
        assert_eq!(data[0]["label"]["color"], theme::ON_PRIMARY);
        assert_eq!(data[1]["label"]["color"], theme::ON_PRIMARY_CONTAINER);
    }
    #[test]
    fn tile_labels_keep_a_gap_between_the_name_and_the_amount() {
        let entries = vec![slice("Wallet 3", 150_000_000)];
        let option = build_option(&entries);
        assert_eq!(option["series"][0]["label"]["fontSize"], 12);
        assert_eq!(option["series"][0]["label"]["lineHeight"], 20);
    }
    #[test]
    fn empty_input_produces_an_empty_treemap() {
        let option = build_option(&[]);
        let data = option["series"][0]["data"].as_array().expect("tile data");
        assert!(data.is_empty());
        assert_eq!(option["title"]["text"], CHART_TITLE);
    }
    #[test]
    fn chart_text_uses_the_theme_font_tokens() {
        let option = build_option(&[slice("Wallet", 1)]);
        assert_eq!(
            option["title"]["textStyle"]["fontFamily"],
            theme::FONT_HEADING
        );
        assert_eq!(
            option["series"][0]["label"]["fontFamily"],
            theme::FONT_FAMILY
        );
    }
}
