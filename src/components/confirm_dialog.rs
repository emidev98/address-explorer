//! Confirmation dialog for destructive actions.
//!
//! Replicates the UTXO Utils `ConfirmModal` on this application's existing
//! `md3-dialog` styling: title, message, one confirm and one cancel action.
//! The dialog closes on cancel, on a scrim click, and on Escape; the caller
//! owns what happens on confirm.

use dioxus::prelude::*;

/// Modal confirmation dialog.
#[component]
pub fn ConfirmDialog(
    title: String,
    message: String,
    confirm_text: String,
    cancel_text: String,
    on_confirm: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| on_cancel.call(()),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_cancel.call(());
                }
            },
            div {
                class: "md3-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "confirm-dialog-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "confirm-dialog-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "{title}"
                }
                p { class: "text-body-medium text-on-surface-variant", "{message}" }

                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        onclick: move |_| on_cancel.call(()),
                        "{cancel_text}"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        autofocus: true,
                        onclick: move |_| on_confirm.call(()),
                        "{confirm_text}"
                    }
                }
            }
        }
    }
}
