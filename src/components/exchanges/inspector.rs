use super::*;
use dioxus::prelude::*;
/// Side inspector for adding or editing one ledger transaction.
#[component]
pub fn ExchangeInspector(
    exchange_id: i64,
    #[props(default)] transaction: Option<ExchangeTransaction>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<ExchangeTransaction>,
) -> Element {
    let mut toast = use_toast();
    let default_currency = use_fiat_currency();
    let mut timestamp = use_signal(|| {
        transaction
            .as_ref()
            .map(|row| civil_time::format_short_datetime(row.timestamp))
            .unwrap_or_else(|| civil_time::format_short_datetime(civil_time::now_epoch_seconds()))
    });
    let mut tx_type = use_signal(|| {
        transaction
            .as_ref()
            .map(|row| row.tx_type)
            .unwrap_or(ExchangeTxType::Buy)
    });
    let mut amount = use_signal(|| {
        transaction
            .as_ref()
            .map(|row| sats_to_btc(row.amount_sats))
            .unwrap_or_default()
    });
    let mut fiat_amount = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.fiat_amount)
            .map(|value| format!("{value:.2}"))
            .unwrap_or_default()
    });
    let mut fiat_currency = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.fiat_currency.clone())
            .unwrap_or_else(|| default_currency.code())
    });
    let mut fee = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.fee)
            .map(|value| {
                if transaction
                    .as_ref()
                    .and_then(|row| row.fee_currency.as_deref())
                    .is_some_and(crate::domain::exchange::csv::is_bitcoin_currency)
                {
                    format!("{:.8}", value / 100_000_000.0)
                        .trim_end_matches('0')
                        .trim_end_matches('.')
                        .to_string()
                } else {
                    format!("{value:.2}")
                }
            })
            .unwrap_or_default()
    });
    let mut fee_currency = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.fee_currency.clone())
            .unwrap_or_else(|| "BTC".to_string())
    });
    let mut tx_hash = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.tx_hash.clone())
            .unwrap_or_default()
    });
    let mut description = use_signal(|| {
        transaction
            .as_ref()
            .and_then(|row| row.description.clone())
            .unwrap_or_default()
    });
    let mut loading = use_signal(|| false);
    let mut duplicate = use_signal(|| Option::<ExchangeDuplicateInfo>::None);
    let raw_data = transaction
        .as_ref()
        .map(|row| serde_json::to_string_pretty(&row.raw_data).unwrap_or_default())
        .unwrap_or_default();
    let editing = transaction.is_some();
    let converted = transaction.as_ref().and_then(|row| {
        row.converted_fiat_amount
            .map(|amount| (amount, row.converted_fiat_currency.clone()))
    });
    let converted_fee = transaction.as_ref().and_then(|row| {
        row.converted_fee_amount
            .zip(row.converted_fee_currency.clone())
    });
    let match_state = transaction.as_ref().map(|row| row.match_state);
    let tx_id = transaction.as_ref().map(|row| row.id);
    let submit = use_callback(move |choice: Option<ExchangeDuplicateChoice>| {
        if *loading.read() {
            return;
        }
        let Some(timestamp) = parse_editor_datetime(&timestamp()) else {
            toast.error("Enter the date and time as YYYY-MM-DD HH:mm (UTC).");
            return;
        };
        let Some(amount_sats) = parse_btc_amount(&amount()) else {
            toast.error("Enter a valid BTC amount.");
            return;
        };
        let fee_value = parse_optional_number(&fee());
        let draft = ExchangeTransactionDraft {
            timestamp,
            tx_type: tx_type(),
            amount_sats,
            currency: transaction
                .as_ref()
                .map(|row| row.currency.clone())
                .unwrap_or_else(|| "BTC".to_string()),
            fiat_amount: parse_optional_number(&fiat_amount()),
            fiat_currency: optional_text(&fiat_currency()),
            fee: fee_value,
            fee_currency: optional_text(&fee_currency()),
            tx_hash: optional_text(&tx_hash()),
            description: optional_text(&description()),
        };
        let request = ExchangeTransactionSaveRequest {
            exchange_id,
            transaction_id: tx_id,
            draft,
            duplicate_choice: choice,
        };
        loading.set(true);
        spawn(async move {
            match save_exchange_transaction_api(request).await {
                Ok(ExchangeTransactionSaveOutcome::Saved(row)) => {
                    duplicate.set(None);
                    loading.set(false);
                    on_saved.call(*row);
                }
                Ok(ExchangeTransactionSaveOutcome::Duplicate(info)) => {
                    loading.set(false);
                    duplicate.set(Some(info));
                }
                Err(error) => {
                    loading.set(false);
                    toast.error(format!("Could not save the transaction: {error}"));
                }
            }
        });
    });
    rsx! {
        section { class: "md3-card md3-card-elevated md3-exchange-inspector",
            div { class: "md3-exchange-inspector-header",
                div {
                    h3 { class: "text-title-medium leading-title-medium text-on-surface",
                        if editing {
                            "Edit transaction"
                        } else {
                            "Add transaction"
                        }
                    }
                    p { class: "text-body-small text-on-surface-variant",
                        if editing {
                            "Editing an imported or manual exchange row."
                        } else {
                            "A new manually entered exchange row."
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "md3-icon-button",
                    aria_label: "Close transaction editor",
                    onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            on_close.call(()),
                    Icon { name: IconName::Close, size: 20 }
                }
            }
            if let Some(state) = match_state {
                div { class: "md3-exchange-inspector-state",
                    span { class: "text-body-small text-on-surface-variant", "Match state" }
                    span { class: if state == MatchState::Unmatched { "md3-exchange-chip md3-exchange-chip-warning" } else { "md3-exchange-chip md3-exchange-chip-success" },
                        "{match_label(state)}"
                    }
                }
            }
            if let Some((amount, currency)) = converted.clone() {
                div { class: "md3-exchange-inspector-state",
                    span { class: "text-body-small text-on-surface-variant", "Converted value" }
                    span { class: "text-body-medium text-on-surface",
                        "{format_fiat(amount, currency.as_deref().unwrap_or(\"\"))}"
                    }
                }
            }
            if let Some((amount, currency)) = converted_fee.clone() {
                div { class: "md3-exchange-inspector-state",
                    span { class: "text-body-small text-on-surface-variant", "Converted fee" }
                    span { class: "text-body-medium text-on-surface", "{format_fiat(amount, &currency)}" }
                }
            }
            div { class: "md3-exchange-inspector-form",
                label { class: "text-body-small text-on-surface-variant", "Transaction type" }
                div { class: "md3-text-field",
                    select {
                        class: "md3-input",
                        value: "{tx_type().as_str()}",
                        disabled: *loading.read(),
                        onchange: move |event| {
                            if let Some(value) = ExchangeTxType::from_str(&event.value()) {
                                tx_type.set(value);
                            }
                        },
                        for value in TRANSACTION_TYPES {
                            option { value: "{value.as_str()}", "{type_label(value)}" }
                        }
                    }
                }
                label { class: "text-body-small text-on-surface-variant", "Date and time (UTC)" }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{timestamp}",
                        placeholder: "YYYY-MM-DD HH:mm",
                        disabled: *loading.read(),
                        oninput: move |event| timestamp.set(event.value()),
                    }
                }
                label { class: "text-body-small text-on-surface-variant", "BTC amount" }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{amount}",
                        inputmode: "decimal",
                        disabled: *loading.read(),
                        oninput: move | event |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                amount.set(event.value()),
                    }
                }
                div { class: "md3-exchange-two-cols",
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Value" }
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                value: "{fiat_amount}",
                                inputmode: "decimal",
                                disabled: *loading.read(),
                                oninput: move | event | fiat_amount
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .set(event.value()),
                            }
                        }
                    }
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Value currency" }
                        div { class: "md3-text-field",
                            select {
                                class: "md3-input",
                                value: "{fiat_currency}",
                                disabled: *loading.read(),
                                onchange: move | event | fiat_currency.set(event
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .value()),
                                option {
                                    value: "",
                                    selected: fiat_currency().is_empty(),
                                    "—"
                                }
                                for code in FIAT_CURRENCIES {
                                    option {
                                        value: "{code}",
                                        selected: fiat_currency().as_str() == *code,
                                        "{code}"
                                    }
                                }
                                if !fiat_currency().is_empty()
                                    && !FIAT_CURRENCIES.iter().any(|code| *code == fiat_currency().as_str())
                                {
                                    option {
                                        value: "{fiat_currency}",
                                        selected: true,
                                        "{fiat_currency}"
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some((amount, currency)) = converted.clone() {
                    div { class: "md3-exchange-derived-fiat",
                        span { class: "text-body-small text-on-surface-variant",
                            "Converted automatically"
                        }
                        strong { class: "text-body-medium",
                            "{format_fiat(amount, currency.as_deref().unwrap_or(\"\"))}"
                        }
                    }
                }
                div { class: "md3-exchange-two-cols",
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Fee" }
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                value: "{fee}",
                                inputmode: "decimal",
                                disabled: *loading.read(),
                                oninput: move | event | fee.set(event
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .value()),
                            }
                        }
                    }
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Fee currency" }
                        div { class: "md3-text-field",
                            select {
                                class: "md3-input",
                                value: "{fee_currency}",
                                disabled: *loading.read(),
                                onchange: move | event |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        fee_currency.set(event.value()),
                                option {
                                    value: "",
                                    selected: fee_currency().is_empty(),
                                    "—"
                                }
                                option {
                                    value: "BTC",
                                    selected: fee_currency().as_str() == "BTC",
                                    "BTC"
                                }
                                option {
                                    value: "XBT",
                                    selected: fee_currency().as_str() == "XBT",
                                    "XBT"
                                }
                                for code in FIAT_CURRENCIES {
                                    option {
                                        value: "{code}",
                                        selected: fee_currency().as_str() == *code,
                                        "{code}"
                                    }
                                }
                                if !fee_currency().is_empty() && fee_currency().as_str() != "BTC"
                                    && fee_currency().as_str() != "XBT"
                                    && !FIAT_CURRENCIES.iter().any(|code| *code == fee_currency().as_str())
                                {
                                    option {
                                        value: "{fee_currency}",
                                        selected: true,
                                        "{fee_currency}"
                                    }
                                }
                            }
                        }
                    }
                }
                label { class: "text-body-small text-on-surface-variant", "Transaction hash / txid" }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{tx_hash}",
                        disabled: *loading.read(),
                        oninput: move |event| tx_hash.set(event.value()),
                    }
                }
                label { class: "text-body-small text-on-surface-variant", "Description" }
                div { class: "md3-text-field",
                    textarea {
                        class: "md3-input md3-exchange-textarea",
                        value: "{description}",
                        disabled: * loading
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                .read(),
                        oninput: move |event| description.set(event.value()),
                    }
                }
                if editing && !raw_data.is_empty() {
                    details { class: "md3-exchange-raw-data",
                        summary { class: "text-body-small text-on-surface-variant", "Imported raw data" }
                        pre { "{raw_data}" }
                    }
                }
            }
            div { class: "md3-exchange-inspector-actions",
                button {
                    r#type: "button",
                    class: "md3-button-text",
                    disabled: *loading.read(),
                    onclick: move |_| on_close.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *loading.read(),
                    onclick: move |_| {
                        submit.call(None);
                    },
                    if *loading.read() {
                        "Saving..."
                    } else {
                        "Save"
                    }
                }
            }
        }
        if let Some(info) = duplicate() {
            ExchangeDuplicateDialog {
                duplicate: info,
                on_cancel: move |_| duplicate.set(None),
                on_choice: move | choice | submit
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .call(Some(choice)),
            }
        }
    }
}
fn sats_to_btc(sats: i64) -> String {
    let value = sats as f64 / 100_000_000.0;
    let mut text = format!("{value:.8}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    text
}
fn parse_btc_amount(value: &str) -> Option<i64> {
    let value = value.trim().replace(',', ".");
    let amount = value.parse::<f64>().ok()?;
    if !amount.is_finite() {
        return None;
    }
    Some((amount * 100_000_000.0).round() as i64)
}
fn parse_optional_number(value: &str) -> Option<f64> {
    let value = value.trim().replace(',', ".");
    if value.is_empty() {
        return None;
    }
    value.parse::<f64>().ok().filter(|value| value.is_finite())
}
fn optional_text(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
fn parse_editor_datetime(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.len() < 16 {
        return None;
    }
    let date = civil_time::parse_date(value.get(..10)?)?;
    let hour = value.get(11..13)?.parse::<i64>().ok()?;
    let minute = value.get(14..16)?.parse::<i64>().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(date + hour * 3_600 + minute * 60)
}
