use dioxus::prelude::*;

use crate::api::prices::update_price_api;
use crate::components::format::format_fiat;
use crate::components::use_toast;
use crate::domain::civil_time::format_datetime;
use crate::domain::price::Price;

/// Modal dialog that edits the close of one stored price. The update goes
/// through `update_price_api`, which validates the value like any external
/// input and never creates rows: editing only changes existing history.
#[component]
pub fn EditPriceDialog(
    price: Price,
    on_close: EventHandler<()>,
    on_saved: EventHandler<Price>,
) -> Element {
    let toast = use_toast();
    let mut value = use_signal(|| price.close.to_string());
    let mut loading = use_signal(|| false);

    let close = move |_| on_close.call(());
    let save = {
        let price = price.clone();
        move |_| {
            let mut toast = toast;
            if *loading.read() {
                return;
            }
            let parsed = value().trim().replace(',', ".").parse::<f64>();
            let Ok(close_value) = parsed else {
                toast.error("Enter a positive number, e.g. 66226.07.");
                return;
            };
            if !close_value.is_finite() || close_value <= 0.0 {
                toast.error("The price must be a positive number.");
                return;
            }

            loading.set(true);
            let price = price.clone();
            spawn(async move {
                match update_price_api(price.symbol.clone(), price.close_time, close_value).await {
                    Ok(updated) => {
                        loading.set(false);
                        on_saved.call(updated);
                    }
                    Err(error) => {
                        loading.set(false);
                        toast.error(format!("Could not update the price: {error}"));
                    }
                }
            });
        }
    };

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: close,
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "edit-price-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "edit-price-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Modify price"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Update the {price.symbol} close of {format_datetime(price.close_time)} (UTC), currently {format_fiat(price.close, &price.symbol)}."
                }

                label {
                    class: "text-body-medium text-on-surface-variant",
                    r#for: "edit-price-input",
                    "New price ({price.symbol})"
                }
                div { class: "md3-text-field",
                    input {
                        id: "edit-price-input",
                        class: "md3-input",
                        r#type: "text",
                        inputmode: "decimal",
                        value: "{value}",
                        autofocus: true,
                        oninput: move |event| value.set(event.value()),
                        disabled: *loading.read(),
                    }
                }

                div { class: "md3-dialog-actions",
                    button {
                        r#type: "button",
                        class: "md3-button-text",
                        onclick: close,
                        disabled: *loading.read(),
                        "Cancel"
                    }
                    button {
                        r#type: "button",
                        class: "md3-button-filled",
                        onclick: save,
                        disabled: *loading.read(),
                        if *loading.read() {
                            "Saving..."
                        } else {
                            "Save"
                        }
                    }
                }
            }
        }
    }
}
