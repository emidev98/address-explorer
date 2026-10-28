//! Low-level Apache ECharts host for chart components.
//!
//! Chart components build their own ECharts option from their business data
//! and pass the serialized option here. Only this module talks to the ECharts
//! JavaScript API, so chart components never have to handle initialization,
//! resizing, updates, or disposal themselves.

use std::sync::atomic::{AtomicU64, Ordering};

use dioxus::prelude::*;

const ECHARTS_JS: Asset = asset!("/assets/vendor/echarts/echarts.min.js");
const ECHARTS_LICENSE: Asset = asset!("/assets/vendor/echarts/LICENSE");
const ECHARTS_D3_LICENSE: Asset = asset!("/assets/vendor/echarts/LICENSE-d3");

static NEXT_CHART_ID: AtomicU64 = AtomicU64::new(1);

fn next_chart_id() -> String {
    format!("echarts-{}", NEXT_CHART_ID.fetch_add(1, Ordering::Relaxed))
}

/// Render an ECharts option in a responsive container.
///
/// `option` must be a valid ECharts option serialized as JSON. Chart
/// components own option construction; this host only owns the ECharts
/// lifecycle.
#[component]
pub(crate) fn EChartsHost(option: String, aria_label: String) -> Element {
    let chart_id = use_hook(next_chart_id);

    use_effect(use_reactive((&option,), {
        let chart_id = chart_id.clone();
        let aria_label = aria_label.clone();
        move |(option,)| {
            document::eval(&render_script(&chart_id, &aria_label, &option));
        }
    }));

    use_drop({
        let chart_id = chart_id.clone();
        move || {
            document::eval(&dispose_script(&chart_id));
        }
    });

    rsx! {
        document::Script { src: ECHARTS_JS, defer: true }
        document::Link { rel: "license", href: ECHARTS_LICENSE }
        document::Link { rel: "license", href: ECHARTS_D3_LICENSE }
        div {
            id: "{chart_id}",
            class: "md3-chart",
            role: "img",
            aria_label: "{aria_label}",
        }
    }
}

fn render_script(chart_id: &str, aria_label: &str, option: &str) -> String {
    let chart_id = serde_json::to_string(chart_id).unwrap_or_else(|_| "\"\"".to_string());
    let aria_label = serde_json::to_string(aria_label).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        r#"
(() => {{
  const id = {chart_id};
  const label = {aria_label};
  const option = {option};

  if (!globalThis.__addressExplorerECharts) {{
    globalThis.__addressExplorerECharts = {{
      charts: Object.create(null),
      observers: Object.create(null),
      timers: Object.create(null),
      options: Object.create(null),
      debounces: Object.create(null),
      probe: null,
    }};
  }}
  const registry = globalThis.__addressExplorerECharts;

  // Chart options reference app theme colors as `var(--md-sys-color-*)`
  // tokens with an optional `~alpha` suffix. Canvas fill styles cannot parse
  // CSS variables (nor the `light-dark()` the palettes use), so each token is
  // resolved through the computed `color` of a hidden probe element, which
  // always yields a canvas-ready `rgb()` value for the current palette and
  // appearance mode.
  const MD3_TOKEN = /^var\((--[\w-]+)(?:~([0-9.]+))?\)$/;

  const resolveColor = (value) => {{
    const match = MD3_TOKEN.exec(value);
    if (!match) {{
      return value;
    }}
    if (!registry.probe) {{
      const probe = document.createElement("div");
      probe.style.display = "none";
      document.body.appendChild(probe);
      registry.probe = probe;
    }}
    registry.probe.style.color = `var(${{match[1]}})`;
    const color = getComputedStyle(registry.probe).color;
    if (match[2] === undefined) {{
      return color;
    }}
    const rgb = /^rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(color);
    return rgb ? `rgba(${{rgb[1]}}, ${{rgb[2]}}, ${{rgb[3]}}, ${{match[2]}})` : color;
  }};

  const resolveThemeColors = (node) => {{
    if (typeof node === "string") {{
      return resolveColor(node);
    }}
    if (Array.isArray(node)) {{
      return node.map(resolveThemeColors);
    }}
    if (node && typeof node === "object") {{
      const resolved = {{}};
      for (const key of Object.keys(node)) {{
        resolved[key] = resolveThemeColors(node[key]);
      }}
      return resolved;
    }}
    return node;
  }};

  // Chart options stay plain JSON, so a tooltip cannot embed a JavaScript
  // function directly. Instead an option may name one of these formatters;
  // the host swaps the marker for the implementation before applying it.
  const VALUE_FORMATTERS = {{
    "@two-decimals": (value) => {{
      // Line series can pass the whole data item; the close is its last
      // dimension.
      const raw = Array.isArray(value) ? value[value.length - 1] : value;
      const number = Number(raw);
      return Number.isFinite(number) ? number.toFixed(2) : String(raw);
    }},
  }};

  const resolveValueFormatters = (node) => {{
    if (Array.isArray(node)) {{
      return node.map(resolveValueFormatters);
    }}
    if (node && typeof node === "object") {{
      const resolved = {{}};
      for (const key of Object.keys(node)) {{
        const value = node[key];
        if (
          key === "valueFormatter" &&
          typeof value === "string" &&
          VALUE_FORMATTERS[value]
        ) {{
          resolved[key] = VALUE_FORMATTERS[value];
        }} else {{
          resolved[key] = resolveValueFormatters(value);
        }}
      }}
      return resolved;
    }}
    return node;
  }};

  const applyOption = (chartId) => {{
    const chart = registry.charts[chartId];
    const chartOption = registry.options[chartId];
    if (chart && chartOption) {{
      chart.setOption(
        resolveValueFormatters(resolveThemeColors(chartOption)),
        true
      );
    }}
  }};

  // Palette or mode switches restyle the document through the `data-theme`
  // and `data-mode` attributes (and the system color scheme when the mode is
  // `system`); re-resolve every chart's colors so they follow.
  if (!registry.themeObserver) {{
    const reapplyTheme = () => {{
      for (const chartId of Object.keys(registry.charts)) {{
        applyOption(chartId);
      }}
    }};
    registry.themeObserver = new MutationObserver(reapplyTheme);
    registry.themeObserver.observe(document.documentElement, {{
      attributes: true,
      attributeFilter: ["data-theme", "data-mode"],
    }});
    globalThis
      .matchMedia("(prefers-color-scheme: dark)")
      .addEventListener("change", reapplyTheme);
  }}

  // Keep the newest option even when the ECharts bundle is still loading.
  registry.options[id] = option;

  // Hydration keeps SSR-rendered attributes, and the server-side chart id
  // counter runs ahead of the client-side one, so the element's DOM id may
  // not match this client's id. The aria-label is identical in both
  // renders, so it anchors the fallback lookup.
  const findElement = () => {{
    const byId = document.getElementById(id);
    if (byId && byId.getAttribute("aria-label") === label) {{
      return byId;
    }}
    return Array.from(document.querySelectorAll("div.md3-chart"))
      .find((candidate) => candidate.getAttribute("aria-label") === label) || null;
  }};

  const resize = () => {{
    const chart = registry.charts[id];
    if (chart) {{
      chart.resize();
    }}
  }};

  const apply = () => {{
    const element = findElement();
    if (!element || typeof globalThis.echarts === "undefined") {{
      return false;
    }}

    let chart = registry.charts[id];
    if (!chart) {{
      chart = globalThis.echarts.init(element, null, {{ renderer: "canvas" }});
      registry.charts[id] = chart;

      if (typeof ResizeObserver === "function") {{
        const observer = new ResizeObserver(resize);
        observer.observe(element);
        registry.observers[id] = observer;
      }} else {{
        globalThis.addEventListener("resize", resize);
        registry.observers[id] = {{
          disconnect: () => globalThis.removeEventListener("resize", resize),
        }};
      }}
    }}

    chart.setOption(
      resolveValueFormatters(resolveThemeColors(registry.options[id])),
      true
    );
    chart.resize();
    return true;
  }};

  if (apply() || registry.timers[id]) {{
    return;
  }}

  // The script tag is inserted in the document head, so it may still be
  // loading when this effect runs.
  let attempts = 0;
  registry.timers[id] = setInterval(() => {{
    attempts += 1;
    if (apply() || attempts >= 200) {{
      clearInterval(registry.timers[id]);
      delete registry.timers[id];
    }}
  }}, 50);
}})();
"#
    )
}

/// Zoom-capable variant of [`EChartsHost`]: identical rendering lifecycle,
/// plus a persistent eval that forwards debounced ECharts `datazoom` events
/// to Rust as `(start, end)` percentages of the current axis window. Chart
/// components use it to re-request finer data when the user narrows the
/// visible range.
#[component]
pub(crate) fn EChartsZoomHost(
    option: String,
    aria_label: String,
    on_datazoom: EventHandler<(f64, f64)>,
    /// One-shot zoom restore token: bumping it returns the visible window to
    /// the full span, even when the option itself does not change.
    reset_zoom: u64,
) -> Element {
    let chart_id = use_hook(next_chart_id);

    use_effect(use_reactive((&option,), {
        let chart_id = chart_id.clone();
        let aria_label = aria_label.clone();
        move |(option,)| {
            document::eval(&render_script(&chart_id, &aria_label, &option));
        }
    }));

    use_effect(use_reactive((&reset_zoom,), {
        let chart_id = chart_id.clone();
        move |(reset_zoom,)| {
            if reset_zoom > 0 {
                document::eval(&reset_zoom_script(&chart_id));
            }
        }
    }));

    // One persistent eval per chart lifetime: it waits for the chart to
    // appear in the registry, subscribes to `datazoom`, and stays alive so
    // the channel keeps delivering. The handlers it captures are Copy
    // handles (signals, resources), so running once is enough.
    use_effect({
        let chart_id = chart_id.clone();
        move || {
            let mut eval = document::eval(&zoom_listener_script(&chart_id));
            spawn(async move {
                while let Ok(window) = eval.recv::<(f64, f64)>().await {
                    on_datazoom.call(window);
                }
            });
        }
    });

    use_drop({
        let chart_id = chart_id.clone();
        move || {
            document::eval(&dispose_script(&chart_id));
        }
    });

    rsx! {
        document::Script { src: ECHARTS_JS, defer: true }
        document::Link { rel: "license", href: ECHARTS_LICENSE }
        document::Link { rel: "license", href: ECHARTS_D3_LICENSE }
        div {
            id: "{chart_id}",
            class: "md3-chart",
            role: "img",
            aria_label: "{aria_label}",
        }
    }
}

/// Subscribe to the chart's `datazoom` events and forward them, debounced, as
/// `[start, end]` percentages. The trailing never-resolving promise keeps the
/// eval task alive so its channel stays open for the lifetime of the chart.
fn zoom_listener_script(chart_id: &str) -> String {
    let chart_id = serde_json::to_string(chart_id).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        r#"
(async () => {{
  const id = {chart_id};
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  let chart = null;
  for (let attempts = 0; attempts < 200 && !chart; attempts += 1) {{
    const registry = globalThis.__addressExplorerECharts;
    chart = (registry && registry.charts[id]) || null;
    if (!chart) {{
      await sleep(50);
    }}
  }}
  if (!chart) {{
    return;
  }}
  const registry = globalThis.__addressExplorerECharts;

  // The debounce timer lives in the registry (not this closure) so the
  // reset-zoom script can cancel a gesture that is still pending when the
  // user asks for the full span back.
  chart.on("datazoom", (event) => {{
    const info = (event.batch && event.batch[0]) || event;
    const start = typeof info.start === "number" ? info.start : 0;
    const end = typeof info.end === "number" ? info.end : 100;
    if (registry.debounces[id]) {{
      clearTimeout(registry.debounces[id]);
    }}
    registry.debounces[id] = setTimeout(() => {{
      delete registry.debounces[id];
      dioxus.send([start, end]);
    }}, 300);
  }});

  await new Promise(() => {{}});
}})();
"#
    )
}

/// Restore the chart's visible window to the full span, cancelling any zoom
/// gesture still sitting in the debounce timer. ECharts applies `inside` and
/// `slider` zooms client-side before the debounced event reaches Rust, so a
/// reset that arrives within the debounce would otherwise leave the chart
/// zoomed (no option changes) and the stale event would re-apply the zoom
/// right after. The `datazoom` event this action fires describes the full
/// span, which the Rust side already ignores.
fn reset_zoom_script(chart_id: &str) -> String {
    let chart_id = serde_json::to_string(chart_id).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        r#"
(() => {{
  const registry = globalThis.__addressExplorerECharts;
  if (!registry) {{
    return;
  }}

  const id = {chart_id};

  if (registry.debounces && registry.debounces[id]) {{
    clearTimeout(registry.debounces[id]);
    delete registry.debounces[id];
  }}

  const chart = registry.charts[id];
  if (!chart) {{
    return;
  }}
  chart.dispatchAction({{ type: "dataZoom", start: 0, end: 100 }});
}})();
"#
    )
}

fn dispose_script(chart_id: &str) -> String {
    let chart_id = serde_json::to_string(chart_id).unwrap_or_else(|_| "\"\"".to_string());

    format!(
        r#"
(() => {{
  const registry = globalThis.__addressExplorerECharts;
  if (!registry) {{
    return;
  }}

  const id = {chart_id};

  if (registry.timers[id]) {{
    clearInterval(registry.timers[id]);
    delete registry.timers[id];
  }}

  if (registry.debounces && registry.debounces[id]) {{
    clearTimeout(registry.debounces[id]);
    delete registry.debounces[id];
  }}

  const observer = registry.observers[id];
  if (observer) {{
    observer.disconnect();
    delete registry.observers[id];
  }}

  const chart = registry.charts[id];
  if (chart) {{
    chart.dispose();
    delete registry.charts[id];
  }}

  delete registry.options[id];
}})();
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_script_embeds_the_chart_id_and_option() {
        let script = render_script("echarts-7", "Flow chart", r#"{"series":[]}"#);

        assert!(script.contains(r#"const id = "echarts-7";"#));
        assert!(script.contains(r#"const label = "Flow chart";"#));
        assert!(script.contains(r#"const option = {"series":[]};"#));
        assert!(script.trim_start().starts_with("(() => {"));
        assert!(script.trim_end().ends_with("})();"));
    }

    #[test]
    fn render_script_falls_back_to_the_aria_label_for_hydrated_elements() {
        let script = render_script("echarts-7", "Flow chart", "{}");

        // Hydrated pages keep the SSR-rendered id, so the client-side id
        // lookup may miss; the aria-label is stable across both renders.
        assert!(script.contains("const findElement = () => {"));
        assert!(script.contains("candidate.getAttribute(\"aria-label\") === label"));
    }

    #[test]
    fn render_script_resolves_theme_color_tokens_against_a_probe_element() {
        let script = render_script("echarts-7", "Flow chart", "{}");

        // Tokens are `var(--md-sys-color-*)` strings with an optional
        // `~alpha` suffix, resolved through the computed `color` of a hidden
        // probe element so `light-dark()` and `var()` chains become
        // canvas-ready `rgb()` values.
        assert!(script.contains("const MD3_TOKEN = /^var\\((--[\\w-]+)(?:~([0-9.]+))?\\)$/;"));
        assert!(script.contains("const resolveThemeColors = (node) => {"));
        assert!(script.contains("probe.style.display = \"none\";"));
        assert!(script.contains("getComputedStyle(registry.probe).color"));
        assert!(script.contains("resolveValueFormatters(resolveThemeColors("));
    }

    #[test]
    fn render_script_resolves_declarative_value_formatters() {
        let script = render_script("echarts-7", "Price trend", "{}");

        assert!(script.contains(r#""@two-decimals": (value) => {"#));
        assert!(script.contains("number.toFixed(2)"));
        assert!(script.contains("const resolveValueFormatters = (node) => {"));
        assert!(script.contains(r#"key === "valueFormatter""#));
    }

    #[test]
    fn render_script_reapplies_options_when_the_theme_changes() {
        let script = render_script("echarts-7", "Flow chart", "{}");

        assert!(script.contains("new MutationObserver(reapplyTheme)"));
        assert!(script.contains("attributeFilter: [\"data-theme\", \"data-mode\"]"));
        assert!(script.contains("matchMedia(\"(prefers-color-scheme: dark)\")"));
    }

    #[test]
    fn dispose_script_embeds_the_chart_id() {
        let script = dispose_script("echarts-7");

        assert!(script.contains(r#"const id = "echarts-7";"#));
        assert!(script.contains("chart.dispose();"));
    }

    #[test]
    fn zoom_listener_script_subscribes_and_forwards_debounced_events() {
        let script = zoom_listener_script("echarts-9");

        assert!(script.contains(r#"const id = "echarts-9";"#));
        assert!(script.contains(r#"chart.on("datazoom""#));
        assert!(script.contains("dioxus.send([start, end])"));
        // The debounce timer lives in the registry so the reset-zoom script
        // can cancel a gesture still pending when the reset arrives.
        assert!(script.contains("clearTimeout(registry.debounces[id])"));
        assert!(script.contains("registry.debounces[id] = setTimeout("));
        // The task stays alive so the channel keeps delivering.
        assert!(script.contains("await new Promise(() => {});"));
    }

    #[test]
    fn reset_zoom_script_cancels_pending_gestures_and_restores_the_full_span() {
        let script = reset_zoom_script("echarts-9");

        assert!(script.contains(r#"const id = "echarts-9";"#));
        // A gesture still sitting in the debounce timer must never reach
        // Rust: it would re-apply the window the reset just left.
        assert!(script.contains("clearTimeout(registry.debounces[id])"));
        assert!(script.contains("delete registry.debounces[id]"));
        // The visible window returns to the full span even when the option
        // does not change, so no `setOption` would restore it otherwise.
        assert!(
            script.contains(r#"chart.dispatchAction({ type: "dataZoom", start: 0, end: 100 });"#)
        );
    }
}
