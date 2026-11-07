//! Shared Material 3 checkbox.
//!
//! The component renders a native checkbox so keyboard interaction, form
//! semantics, and assistive technology keep working, then draws the Material 3
//! container, state layer, and check/dash marks on top of it. Use it for every
//! checkbox instead of styling a native input in a screen.

use dioxus::prelude::*;

const ICON_CHECK: &str = "M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z";
// A 10 px wide, 2 px tall mark in the 18 px icon box.
const ICON_DASH: &str = "M5.333 10.667h13.334v2.666H5.333z";

/// A Material 3 checkbox with an optional visible label.
///
/// `label` renders text to the right of the control. When no visible label is
/// appropriate, such as a table row selector, pass `aria_label` instead so the
/// native input still has an accessible name. `indeterminate` displays the
/// mixed dash and is intended for a parent checkbox whose children are only
/// partially selected; callers should pass `checked: false` in that state.
#[component]
pub fn Checkbox(
    checked: bool,
    on_change: EventHandler<bool>,
    #[props(default)] label: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default)] disabled: bool,
    #[props(default)] indeterminate: bool,
) -> Element {
    // `indeterminate` is a DOM property rather than an HTML attribute, so RSX
    // cannot set it directly. Mirror the visual mixed state onto every native
    // checkbox after render so screen readers announce the true mixed state.
    use_effect(use_reactive((&indeterminate,), |(_indeterminate,)| {
        document::eval(
            "document.querySelectorAll('.md3-checkbox-input').forEach((input) => { \
             const control = input.closest('.md3-checkbox'); \
             input.indeterminate = control?.classList.contains('md3-checkbox-indeterminate') ?? false; \
             });",
        );
    }));

    let class = if indeterminate {
        "md3-checkbox md3-checkbox-indeterminate"
    } else {
        "md3-checkbox"
    };

    rsx! {
        label { class,
            span { class: "md3-checkbox-control",
                input {
                    class: "md3-checkbox-input",
                    r#type: "checkbox",
                    checked,
                    disabled,
                    aria_label,
                    aria_checked: indeterminate.then_some("mixed"),
                    onchange: move |event| on_change.call(event.checked()),
                }
                span { class: "md3-checkbox-state-layer", aria_hidden: "true" }
                span { class: "md3-checkbox-box", aria_hidden: "true",
                    svg { class: "md3-checkbox-icon", view_box: "0 0 24 24",
                        path { class: "md3-checkbox-check", d: ICON_CHECK }
                        path { class: "md3-checkbox-dash", d: ICON_DASH }
                    }
                }
            }
            if let Some(label) = label {
                span { class: "md3-checkbox-label", "{label}" }
            }
        }
    }
}
