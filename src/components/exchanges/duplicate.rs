use super::*;
use dioxus::prelude::*;

pub(crate) fn duplicate_summary(duplicate: &ExchangeDuplicateInfo) -> String {
    let amount = if crate::domain::exchange::csv::is_bitcoin_currency(&duplicate.currency) {
        format_btc(duplicate.amount_sats)
    } else {
        format!("{} {}", duplicate.amount_sats, duplicate.currency)
    };
    let hash = duplicate
        .tx_hash
        .as_deref()
        .map(format_tx_hash)
        .or_else(|| duplicate.description.clone())
        .unwrap_or_else(|| "no hash".to_string());
    format!(
        "{} {} · {} · {}",
        duplicate.tx_type.as_str(),
        amount,
        civil_time::format_datetime(duplicate.timestamp),
        hash,
    )
}

/// Three-way duplicate fingerprint resolution.
#[component]
pub fn ExchangeDuplicateDialog(
    duplicate: ExchangeDuplicateInfo,
    on_cancel: EventHandler<()>,
    on_choice: EventHandler<ExchangeDuplicateChoice>,
) -> Element {
    rsx! {
        Modal {
            title: "Duplicate transaction".to_string(),
            on_close: move |_| on_cancel.call(()),
            p { class: "text-body-medium text-on-surface-variant",
                "Another row already has the same fingerprint: {duplicate_summary(&duplicate)}. Choose whether to replace the existing row, keep both rows as an intentional duplicate, or cancel."
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    onclick: move |_| on_choice.call(ExchangeDuplicateChoice::KeepBoth),
                    "Add duplicated row"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    onclick: move |_| on_choice.call(ExchangeDuplicateChoice::Replace),
                    "Replace existing row"
                }
            }
        }
    }
}
