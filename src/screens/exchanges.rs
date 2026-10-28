use dioxus::prelude::*;

/// Exchanges screen.
#[component]
pub fn Exchanges() -> Element {
    rsx! {
        section { id: "exchanges", class: "md3-screen",
            h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                "Exchanges"
            }
            p { class: "text-body-medium text-on-surface-variant", "Exchanges screen content." }
        }
    }
}
