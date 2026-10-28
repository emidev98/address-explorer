//! Price trend chart: the BTC close price of one fiat symbol over time,
//! rendered with Apache ECharts as a zoomable line chart.
//!
//! The screen supplies a downsampled window of the stored history; the chart
//! owns every presentation detail, including the `inside` and `slider`
//! dataZoom controls. When the user narrows the visible range the chart
//! reports the new window through `on_zoom` so the screen can re-request
//! finer data; loading, empty, and error states stay with the screen.

use dioxus::prelude::*;
use serde_json::{json, Value};

use super::echarts::EChartsZoomHost;
use super::theme;
use crate::domain::price::Price;

/// Render the historical price trend as a zoomable line chart.
///
/// `points` are the bucketed closes of `[window_start, window_end]` (epoch
/// seconds, oldest first); the axis spans exactly that window so zoom
/// percentages map linearly onto it. `on_zoom` receives the `(start, end)`
/// percentages the user narrowed the window to.
#[component]
pub fn PriceTrendChart(
    points: Vec<Price>,
    window_start: i64,
    window_end: i64,
    symbol: String,
    on_zoom: EventHandler<(f64, f64)>,
    /// One-shot zoom restore token: bumping it returns the visible window to
    /// the full span even when the option itself does not change.
    reset_zoom: u64,
    #[props(default = "Price trend".to_string())] aria_label: String,
) -> Element {
    let option = build_option(&points, window_start, window_end, &symbol).to_string();

    rsx! {
        EChartsZoomHost {
            option,
            aria_label,
            on_datazoom: on_zoom,
            reset_zoom,
        }
    }
}

/// Build the complete ECharts option for the supplied window of closes.
fn build_option(points: &[Price], window_start: i64, window_end: i64, symbol: &str) -> Value {
    // ECharts time axes use milliseconds.
    let data: Vec<Value> = points
        .iter()
        .map(|price| json!([price.close_time * 1000, price.close]))
        .collect();

    let mut tooltip = theme::tooltip("axis");
    tooltip["valueFormatter"] = json!(theme::TOOLTIP_TWO_DECIMALS);

    json!({
        "animation": false,
        "aria": {
            "enabled": true,
        },
        "textStyle": theme::text_style(),
        "grid": {
            "left": 90,
            "right": 24,
            "top": 24,
            "bottom": 72,
        },
        "tooltip": tooltip,
        "xAxis": {
            "type": "time",
            "min": window_start * 1000,
            "max": window_end * 1000,
            "axisLabel": {
                "color": theme::ON_SURFACE_VARIANT,
            },
            "axisLine": {
                "lineStyle": {
                    "color": theme::OUTLINE_VARIANT,
                },
            },
        },
        "yAxis": {
            "type": "value",
            "scale": true,
            "axisLabel": {
                "color": theme::ON_SURFACE_VARIANT,
            },
            "splitLine": {
                "lineStyle": {
                    "color": theme::OUTLINE_VARIANT,
                },
            },
        },
        "dataZoom": [
            {
                "type": "inside",
                "xAxisIndex": 0,
                "start": 0,
                "end": 100,
            },
            {
                "type": "slider",
                "xAxisIndex": 0,
                "start": 0,
                "end": 100,
                "height": 24,
                "borderColor": "transparent",
                "backgroundColor": theme::SURFACE_CONTAINER,
                "fillerColor": theme::alpha("primary", 0.15),
                "handleStyle": {
                    "color": theme::PRIMARY,
                },
                "moveHandleStyle": {
                    "color": theme::PRIMARY,
                },
                "textStyle": {
                    "color": theme::ON_SURFACE_VARIANT,
                },
                "dataBackground": {
                    "lineStyle": {
                        "color": theme::OUTLINE_VARIANT,
                    },
                    "areaStyle": {
                        "color": theme::alpha("primary", 0.2),
                    },
                },
            },
        ],
        "series": [{
            "name": symbol,
            "type": "line",
            "showSymbol": false,
            // Extra client-side downsampling on top of the server-side
            // bucketing keeps rendering cheap on very dense windows.
            "sampling": "lttb",
            "lineStyle": {
                "color": theme::PRIMARY,
                "width": 2,
            },
            "itemStyle": {
                "color": theme::PRIMARY,
            },
            "areaStyle": {
                "color": theme::alpha("primary", 0.15),
            },
            "data": data,
        }],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(close_time: i64, close: f64) -> Price {
        Price {
            symbol: "USD".to_string(),
            close_time,
            close,
        }
    }

    #[test]
    fn data_points_are_epoch_milliseconds_pairs_oldest_first() {
        let option = build_option(&[point(100, 1.5), point(200, 2.5)], 100, 200, "USD");
        let data = option["series"][0]["data"].as_array().expect("series data");

        assert_eq!(data, &[json!([100_000, 1.5]), json!([200_000, 2.5])]);
        assert_eq!(option["series"][0]["name"], "USD");
    }

    #[test]
    fn axis_spans_the_requested_window_so_zoom_maps_linearly() {
        let option = build_option(&[point(100, 1.5)], 100, 900, "EUR");

        assert_eq!(option["xAxis"]["min"], 100_000);
        assert_eq!(option["xAxis"]["max"], 900_000);
    }

    #[test]
    fn zoom_controls_cover_inside_and_slider_at_full_span() {
        let option = build_option(&[], 0, 100, "USD");
        let data_zoom = option["dataZoom"].as_array().expect("dataZoom");

        assert_eq!(data_zoom.len(), 2);
        for entry in data_zoom {
            assert_eq!(entry["start"], 0);
            assert_eq!(entry["end"], 100);
        }
        assert_eq!(data_zoom[0]["type"], "inside");
        assert_eq!(data_zoom[1]["type"], "slider");
    }

    #[test]
    fn tooltip_uses_a_two_decimal_value_formatter() {
        let option = build_option(&[point(100, 1.5)], 100, 200, "USD");

        assert_eq!(
            option["tooltip"]["valueFormatter"],
            theme::TOOLTIP_TWO_DECIMALS
        );
    }

    #[test]
    fn colors_are_theme_tokens_the_host_resolves() {
        let option = build_option(&[point(100, 1.5)], 100, 200, "USD");

        assert_eq!(option["series"][0]["lineStyle"]["color"], theme::PRIMARY);
        assert_eq!(
            option["series"][0]["areaStyle"]["color"],
            theme::alpha("primary", 0.15)
        );
        assert_eq!(
            option["xAxis"]["axisLabel"]["color"],
            theme::ON_SURFACE_VARIANT
        );
        assert_eq!(option["tooltip"]["backgroundColor"], theme::INVERSE_SURFACE);
    }
}
