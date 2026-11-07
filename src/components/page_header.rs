//! Shared screen title row.

use dioxus::prelude::*;

/// Screen heading with the page-level actions passed as children.
#[component]
pub fn PageHeader(title: String, #[props(default)] children: Element) -> Element {
    rsx! {
        div { class: "md3-page-header",
            h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                "{title}"
            }
            {children}
        }
    }
}
