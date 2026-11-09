use super::*;
use dioxus::prelude::*;
/// Catalog provider name corresponding to a detected CSV parser. The
/// generic/manual parser intentionally has no provider.
pub(crate) fn parser_provider_name(parser_id: &str) -> Option<&'static str> {
    match parser_id {
        "crypto.com" => Some("Crypto.com"),
        "revolut" => Some("Revolut"),
        "binance" => Some("Binance"),
        "kraken" => Some("Kraken"),
        "coinbase" => Some("Coinbase"),
        "okx" => Some("OKX"),
        "bitget" => Some("Bitget"),
        "bybit" => Some("Bybit"),
        _ => None,
    }
}
fn trigger_import_data_preview(
    csv_text: Signal<String>,
    mapping: Signal<ImportDataMapping>,
    mut preview: Signal<Option<Result<ImportDataPreview, String>>>,
    mut loading: Signal<bool>,
) {
    let text = csv_text();
    if text.trim().is_empty() {
        preview.set(None);
        return;
    }
    let mapping = mapping();
    loading.set(true);
    spawn(async move {
        let result = preview_import_data_api(text, Some(mapping))
            .await
            .map_err(|error| error.to_string());
        preview.set(Some(result));
        loading.set(false);
    });
}
/// Import exchange ledger data into a new or existing account.
#[component]
pub fn ImportDataDialog(
    #[props(default)] exchange_id: Option<i64>,
    #[props(default)] exchange_name: Option<String>,
    on_close: EventHandler<()>,
    on_imported: EventHandler<ImportDataSummary>,
) -> Element {
    let mut toast = use_toast();
    let default_currency = use_fiat_currency();
    let default_currency_code = default_currency.code();
    let mut name = use_signal(|| exchange_name.clone().unwrap_or_default());
    let mut provider_id = use_signal(|| Option::<i64>::None);
    let mut provider_touched = use_signal(|| false);
    let mut file_name = use_signal(String::new);
    let mut csv_text = use_signal(String::new);
    let mut mapping = use_signal(ImportDataMapping::default);
    let mut preview = use_signal(|| Option::<Result<ImportDataPreview, String>>::None);
    let mut loading = use_signal(|| false);
    let providers = use_resource(|| async move {
        catalog_options_api(CatalogKind::Exchange.as_str().to_string())
            .await
            .map_err(|error| error.to_string())
    });
    let is_create = exchange_id.is_none();
    let title = if is_create {
        "Import Data".to_string()
    } else {
        format!(
            "Import {} CSV",
            exchange_name.as_deref().unwrap_or("exchange")
        )
    };
    use_effect(move || {
        if !is_create || provider_touched() {
            return;
        }
        let Some(Ok(preview)) = preview() else {
            return;
        };
        let Some(Ok(providers)) = providers() else {
            return;
        };
        let detected_provider = preview
            .detected_parser_id
            .as_deref()
            .and_then(parser_provider_name);
        let next = detected_provider.and_then(|name| {
            providers
                .into_iter()
                .find(|provider| provider.name.eq_ignore_ascii_case(name))
                .map(|provider| provider.id)
        });
        if provider_id() != next {
            provider_id.set(next);
        }
    });
    let on_file = move |(picked_name, text): (String, String)| {
        file_name.set(picked_name.clone());
        csv_text.set(text);
        preview.set(None);
        mapping.set(ImportDataMapping::default());
        if is_create && name().trim().is_empty() {
            name.set(
                picked_name
                    .trim_end_matches(".csv")
                    .trim_end_matches(".txt")
                    .to_string(),
            );
        }
        trigger_import_data_preview(csv_text, mapping, preview, loading);
    };
    let preview_data = preview();
    let preview_ok = matches!(preview_data, Some(Ok(_)));
    let detected = preview_data
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .and_then(|preview| preview.detected_parser_name.clone());
    let manual_mapping = preview_ok && detected.is_none();
    let mut save = move || {
        if is_create && name().trim().is_empty() {
            toast.error("Enter an exchange name.");
            return;
        }
        if !is_create && file_name().is_empty() {
            toast.error("Choose a CSV file to import.");
            return;
        }
        let name_value = name().trim().to_string();
        let provider_value = provider_id();
        let text = csv_text();
        let mapping_value = Some(mapping());
        let exchange_id = exchange_id;
        loading.set(true);
        spawn(async move {
            let result = if text.trim().is_empty() {
                crate::api::exchanges::create_exchange_account_api(name_value, provider_value)
                    .await
                    .map(|account| ImportDataSummary {
                        exchange_id: account.id,
                        created: true,
                        inserted: 0,
                        duplicates: 0,
                        auto_linked: 0,
                        skipped_non_btc: 0,
                        conversion_warning: None,
                    })
                    .map_err(|error| error.to_string())
            } else {
                import_data_api(
                    exchange_id,
                    Some(name_value),
                    provider_value,
                    text,
                    mapping_value,
                )
                .await
                .map_err(|error| error.to_string())
            };
            loading.set(false);
            match result {
                Ok(summary) => on_imported.call(summary),
                Err(error) => toast.error(format!("Could not import data: {error}")),
            }
        });
    };
    let provider_entries = providers().and_then(|state| state.ok()).unwrap_or_default();
    let providers_loading = providers().is_none();
    let auto_fill_entries = provider_entries.clone();
    rsx! {
        Modal {
            title,
            title_id: "import-data-title".to_string(),
            class: "md3-import-data-dialog".to_string(),
            on_close: move |_| on_close.call(()),
            if is_create {
                label { class: "text-body-medium text-on-surface-variant", "Exchange name *" }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{name}",
                        placeholder: "e.g. India OKX 2020, Main Binance 2024, Revolut savings",
                        disabled: * loading
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        .read(),
                        oninput: move |event| name.set(event.value()),
                    }
                }
                label { class: "text-body-medium text-on-surface-variant", "Provider *" }
                CatalogOptionsSelect {
                    kind: CatalogKind::Exchange,
                    value: provider_id(),
                    on_change: move |selected| {
                        provider_touched.set(true);
                        provider_id.set(selected);
                        if name().trim().is_empty() {
                            if let Some(provider) = selected
                                .and_then(|id| {
                                    auto_fill_entries.iter().find(|entry| entry.id == id).cloned()
                                })
                            {
                                name.set(provider.name);
                            }
                        }
                    },
                    disabled: *loading.read(),
                    id: "import-data-provider".to_string(),
                    placeholder: "Manual Import".to_string(),
                    entries: provider_entries.clone(),
                    loading: providers_loading,
                }
            }
            CsvDropzone {
                file_name: file_name(),
                disabled: *loading.read(),
                on_file,
            }
            if let Some(result) = preview_data {
                match result {
                    Err(error) => rsx! {
                        p { class: "text-body-small md3-exchange-error", "{error}" }
                    },
                    Ok(preview) => rsx! {
                        div { class: "md3-import-data-file-info",
                            span { class: "text-body-small text-on-surface-variant",
                                "{preview.row_count} rows · {preview.parsed_count} BTC transactions"
                            }
                            if let Some(name) = preview.detected_parser_name.clone() {
                                span { class: "md3-exchange-chip md3-exchange-chip-success", "Detected: {name}" }
                            } else if !preview.headers.is_empty() {
                                span { class: "md3-exchange-chip md3-exchange-chip-warning",
                                    "Unknown format — manual mapping required"
                                }
                            }
                            if preview.skipped_non_btc > 0 {
                                span { class: "md3-exchange-chip md3-exchange-chip-warning",
                                    "Skipped {preview.skipped_non_btc} non-BTC rows"
                                }
                            }
                        }
                        if let Some(warning) = preview.conversion_warning.clone() {
                            p { class: "text-body-small md3-exchange-warning", "{warning}" }
                        }
                    },
                }
            }
            if manual_mapping {
                p { class: "text-body-small text-on-surface-variant",
                    "Map the CSV columns to the canonical transaction fields."
                }
                div { class: "md3-import-data-mapping",
                    for (field, label, required) in IMPORT_DATA_MAPPING_FIELDS {
                        div { class: "md3-import-data-mapping-row",
                            label { class: "text-body-small text-on-surface-variant",
                                "{label}"
                                if required {
                                    span { class: "md3-import-data-required", " *" }
                                }
                            }
                            div { class: "md3-text-field",
                                select {
                                    class: "md3-input",
                                    value: import_data_mapping_value(&mapping(), field),
                                    disabled: *loading.read(),
                                    onchange: move |event| {
                                        let value = event.value();
                                        mapping
                                            .with_mut(|current| {
                                                set_import_data_mapping_value(current, field, value);
                                            });
                                        trigger_import_data_preview(csv_text, mapping, preview, loading);
                                    },
                                    option { value: "",
                                        if required {
                                            "Required"
                                        } else {
                                            "— Skip —"
                                        }
                                    }
                                    if let Some(Ok(preview)) = preview() {
                                        for header in preview.headers {
                                            option { value: "{header}", "{header}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                label { class: "text-body-small text-on-surface-variant", "Timestamp format (optional)" }
                p { class: "text-body-small text-on-surface-variant",
                    "Use dayjs-style tokens such as YYYY-MM-DD HH:mm:ss, or a chrono format beginning with %."
                }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{mapping().timestamp_format}",
                        placeholder: "YYYY-MM-DD HH:mm:ss",
                        disabled: *
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        loading.read(),
                        onchange: move |event| {
                            mapping
                                .with_mut(|current| {
                                    current.timestamp_format = event.value();
                                });
                            trigger_import_data_preview(csv_text, mapping, preview, loading);
                        },
                    }
                }
            }
            if let Some(Ok(preview)) = preview() {
                if !preview.preview.is_empty() {
                    p { class: "text-body-small text-on-surface-variant",
                        "Preview of the first {preview.preview.len()} of {preview.parsed_count} parsed transactions."
                    }
                    div { class: "md3-table-container",
                        table { class: "md3-table md3-import-data-preview-table",
                            thead {
                                tr {
                                    th { "Date" }
                                    th { "Type" }
                                    th { "Amount" }
                                    th { "Currency" }
                                    th { "Value ({default_currency_code})" }
                                    th { "TX Hash" }
                                }
                            }
                            tbody {
                                for row in preview.preview {
                                    tr {
                                        td { "{civil_time::format_datetime(row.timestamp)}" }
                                        td { "{row.tx_type.as_str()}" }
                                        td { "{format_btc(row.amount_sats)}" }
                                        td { "{row.currency}" }
                                        td {
                                            if let Some(amount) = row.converted_fiat_amount {
                                                "{format_fiat(amount, row.converted_fiat_currency.as_deref().unwrap_or(\"\"))}"
                                            } else {
                                                "—"
                                            }
                                        }
                                        td {
                                            "{row.tx_hash.as_deref().map(format_tx_hash).unwrap_or_else(|| \"—\".to_string())}"
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
                    disabled: *loading.read(),
                    onclick: move | _ |
                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        on_close.call(()),
                    "Cancel"
                }
                button {
                    r#type: "button",
                    class: "md3-button-filled",
                    disabled: *loading.read(),
                    onclick: move |_| save(),
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
