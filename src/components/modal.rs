//! Shared modal dialog shell.
use dioxus::prelude::*;
/// Modal scrim and dialog container. Dialogs render their form and actions as
/// children; the shell owns the scrim click, Escape handling, and ARIA roles.
#[component]
pub fn Modal(
    #[props(default)] title: Option<String>,
    #[props(default)] title_id: Option<String>,
    #[props(default)] aria_label: Option<String>,
    #[props(default = "dialog".to_string())] role: String,
    #[props(default)] class: String,
    #[props(default = true)] dismissible: bool,
    on_close: EventHandler<()>,
    #[props(default)] children: Element,
) -> Element {
    let dialog_class = if class.is_empty() {
        "md3-dialog".to_string()
    } else {
        format!("md3-dialog {class}")
    };
    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| {
                if dismissible {
                    on_close.call(());
                }
            },
            onkeydown: move |event| {
                if dismissible && event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: dialog_class,
                role,
                aria_modal: "true",
                aria_labelledby: title_id.clone(),
                aria_label,
                onclick: move |event| event.stop_propagation(),
                if let Some(title) = title {
                    h2 {
                        id: title_id.clone().unwrap_or_default(),
                        class: "text-headline-medium leading-headline-medium text-on-surface",
                        "{title}"
                    }
                }
                {children}
            }
        }
    }
}
