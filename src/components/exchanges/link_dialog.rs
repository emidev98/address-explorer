use super::*;
use dioxus::prelude::*;
/// Manual on-chain linking modal.
#[component]
pub fn ExchangeLinkDialog(
    transaction: ExchangeTransaction,
    on_close: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut toast = use_toast();
    let mut search = use_signal(String::new);
    let mut loading = use_signal(|| false);
    let mut duplicate = use_signal(|| Option::<(String, ExchangeDuplicateInfo)>::None);
    let tx_id = transaction.id;
    let current_txid = transaction
        .on_chain_link
        .as_ref()
        .map(|link| link.txid.clone())
        .or_else(|| transaction.matched_tx_id.clone());
    let candidates = use_resource(move || {
        let search = search();
        async move {
            exchange_link_candidates_api(tx_id, search)
                .await
                .map_err(|error| error.to_string())
        }
    });
    let link = use_callback(move |(txid, allow_duplicate): (String, bool)| {
        if *loading.read() {
            return;
        }
        loading.set(true);
        spawn(async move {
            match link_exchange_transaction_api(tx_id, txid, allow_duplicate).await {
                Ok(ExchangeLinkOutcome::Linked(_)) => {
                    loading.set(false);
                    duplicate.set(None);
                    on_changed.call(());
                }
                Ok(ExchangeLinkOutcome::Duplicate(info)) => {
                    loading.set(false);
                    duplicate.set(Some((info.transaction_id.to_string(), info)));
                }
                Err(error) => {
                    loading.set(false);
                    toast.error(format!("Could not link the transaction: {error}"));
                }
            }
        });
    });
    let clear = move |_| {
        if *loading.read() {
            return;
        }
        loading.set(true);
        spawn(async move {
            match clear_exchange_transaction_link_api(tx_id).await {
                Ok(_) => {
                    loading.set(false);
                    on_changed.call(());
                }
                Err(error) => {
                    loading.set(false);
                    toast.error(format!("Could not clear the on-chain link: {error}"));
                }
            }
        });
    };
    let candidate_list = candidates();
    let current_txid_for_table = current_txid.clone();
    rsx! {
        Modal {
            title: "Link on-chain transaction".to_string(),
            class: "md3-exchange-link-dialog".to_string(),
            on_close: move |_| on_close.call(()),
            div { class: "md3-exchange-link-summary",
                div {
                    span { class: "text-body-small text-on-surface-variant", "Exchange row" }
                    strong {
                        "{transaction.tx_type.as_str()} · {format_btc(transaction.amount_sats)} · {civil_time::format_datetime(transaction.timestamp)}"
                    }
                }
                div {
                    span { class: "text-body-small text-on-surface-variant", "Current on-chain link" }
                    strong {
                        "{current_txid.as_deref().map(format_tx_hash).unwrap_or_else(|| \"-\".to_string())}"
                    }
                }
            }
            label { class: "text-body-small text-on-surface-variant", "Search cached transactions" }
            div { class: "md3-text-field",
                input {
                    class: "md3-input",
                    value: "{search}",
                    placeholder: "txid, date, address, amount",
                    oninput: move |event| search.set(event.value()),
                }
            }
            match candidate_list {
                None => rsx! {
                    p { class: "text-body-medium text-on-surface-variant", "Loading candidates..." }
                },
                Some(Err(error)) => rsx! {
                    p { class: "text-body-medium md3-exchange-error", "{error}" }
                },
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-body-medium text-on-surface-variant",
                        if search().trim().is_empty() {
                            "No cached candidates found. Sync or add the address that owns the on-chain transaction, then try again."
                        } else {
                            "No cached on-chain transaction matches this search."
                        }
                    }
                },
                Some(Ok(list)) => {
                    let total = list.len();
                    let visible = list.into_iter().take(MAX_LINK_CANDIDATES).collect::<Vec<_>>();
                    rsx! {
                        if total > MAX_LINK_CANDIDATES {
                            p { class: "text-body-small text-on-surface-variant",
                                "Showing the {MAX_LINK_CANDIDATES} best matches; refine the search to see the rest."
                            }
                        }
                        div { class: "md3-table-container md3-exchange-link-table",
                            table { class: "md3-table",
                                thead {
                                    tr {
                                        th { "Link" }
                                        th { "Addresses" }
                                        th { "Likely" }
                                        th { "On-chain tx" }
                                        th { "Timestamp" }
                                        th { "Balance" }
                                    }
                                }
                                tbody {
                                    for candidate in visible {
                                        {
                                            let is_current = current_txid_for_table.as_deref()
                                                == Some(candidate.txid.as_str());
                                            let candidate_txid = candidate.txid.clone();
                                            rsx! {
                                                tr {
                                                    td {
                                                        button {
                                                            r#type: "button",
                                                            class: if is_current { "md3-button-tonal" } else { "md3-button-filled" },
                                                            disabled: is_current || *loading.read(),
                                                            onclick: move |_| {
                                                                link.call((candidate_txid.clone(), false));
                                                            },
                                                            if is_current {
                                                                "Linked"
                                                            } else {
                                                                "Link"
                                                            }
                                                        }
                                                    }
                                                    td {
                                                        div { class: "md3-exchange-address-chips",
                                                            for address in &candidate.local_addresses {
                                                                span { class: "md3-exchange-chip",
                                                                    if let Some(label) = &address.label {
                                                                        "{label} ({format_address(&address.address)})"
                                                                    } else {
                                                                        "{format_address(&address.address)}"
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    td { "{candidate.likeliness:.3}" }
                                                    td { "{format_tx_hash(&candidate.txid)}" }
                                                    td { "{civil_time::format_datetime(candidate.block_time)}" }
                                                    td {
                                                        div { class: "md3-exchange-candidate-balance",
                                                            strong { "{format_btc(candidate.known_movement_sats)}" }
                                                            span { "Fee {format_btc(candidate.fee_sats)}" }
                                                            if let Some(delta) = candidate.amount_delta_sats {
                                                                span { "Delta {delta} sats" }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "md3-dialog-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    onclick: move |_| on_close.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-tonal",
                    disabled: current_txid.is_none()
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            || * loading.read(),
                    onclick: clear,
                    "Clear link"
                }
            }
        }
        if let Some((txid, info)) = duplicate() {
            ConfirmDialog {
                title: "Reuse on-chain transaction"
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .to_string(),
                message: format!(
                    "This on-chain transaction is already linked to another exchange row: {}. Link this row to the same transaction too?",
                    duplicate_summary(&info),
                ),
                confirm_text: "Link anyway".to_string(),
                cancel_text: "Cancel".to_string(),
                on_cancel: move |_| duplicate.set(None),
                on_confirm: move |_| {
                    link.call((txid.clone(), true));
                },
            }
        }
    }
}
