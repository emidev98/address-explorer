//! Exchange-specific dialogs: Import Data, account edit, transaction
//! inspector, duplicate resolution, and manual on-chain linking.

use dioxus::html::HasFileData;
use dioxus::prelude::*;

use crate::api::catalogs::list_exchange_types_api;
use crate::api::exchanges::{
    clear_exchange_transaction_link_api, exchange_link_candidates_api, import_data_api,
    link_exchange_transaction_api, preview_import_data_api, save_exchange_transaction_api,
};
use crate::components::format::{format_address, format_btc, format_fiat};
use crate::components::{use_toast, ConfirmDialog};
use crate::domain::civil_time;
use crate::domain::exchange::{
    ExchangeAccount, ExchangeDuplicateChoice, ExchangeDuplicateInfo, ExchangeLinkOutcome,
    ExchangeTransaction, ExchangeTransactionDraft, ExchangeTransactionSaveOutcome,
    ExchangeTransactionSaveRequest, ExchangeTxType, ImportDataMapping, ImportDataPreview,
    ImportDataSummary, MatchState,
};
use crate::domain::price::FIAT_CURRENCIES;

const TRANSACTION_TYPES: [ExchangeTxType; 6] = [
    ExchangeTxType::Buy,
    ExchangeTxType::Sell,
    ExchangeTxType::Reward,
    ExchangeTxType::Withdrawal,
    ExchangeTxType::Deposit,
    ExchangeTxType::Unknown,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportDataMappingField {
    Timestamp,
    Amount,
    Currency,
    TxType,
    TxHash,
    FiatAmount,
    FiatCurrency,
    Fee,
    FeeCurrency,
    Description,
}

const IMPORT_DATA_MAPPING_FIELDS: [(ImportDataMappingField, &str, bool); 10] = [
    (ImportDataMappingField::Timestamp, "Timestamp", true),
    (ImportDataMappingField::Amount, "Amount", true),
    (ImportDataMappingField::Currency, "Currency", true),
    (ImportDataMappingField::TxType, "Transaction type", true),
    (
        ImportDataMappingField::TxHash,
        "Transaction hash / txid",
        false,
    ),
    (ImportDataMappingField::FiatAmount, "Fiat amount", false),
    (ImportDataMappingField::FiatCurrency, "Fiat currency", false),
    (ImportDataMappingField::Fee, "Fee", false),
    (ImportDataMappingField::FeeCurrency, "Fee currency", false),
    (ImportDataMappingField::Description, "Description", false),
];

fn import_data_mapping_value(mapping: &ImportDataMapping, field: ImportDataMappingField) -> String {
    match field {
        ImportDataMappingField::Timestamp => mapping.timestamp.clone(),
        ImportDataMappingField::Amount => mapping.amount.clone(),
        ImportDataMappingField::Currency => mapping.currency.clone(),
        ImportDataMappingField::TxType => mapping.tx_type.clone(),
        ImportDataMappingField::TxHash => mapping.tx_hash.clone(),
        ImportDataMappingField::FiatAmount => mapping.fiat_amount.clone(),
        ImportDataMappingField::FiatCurrency => mapping.fiat_currency.clone(),
        ImportDataMappingField::Fee => mapping.fee.clone(),
        ImportDataMappingField::FeeCurrency => mapping.fee_currency.clone(),
        ImportDataMappingField::Description => mapping.description.clone(),
    }
}

fn set_import_data_mapping_value(
    mapping: &mut ImportDataMapping,
    field: ImportDataMappingField,
    value: String,
) {
    match field {
        ImportDataMappingField::Timestamp => mapping.timestamp = value,
        ImportDataMappingField::Amount => mapping.amount = value,
        ImportDataMappingField::Currency => mapping.currency = value,
        ImportDataMappingField::TxType => mapping.tx_type = value,
        ImportDataMappingField::TxHash => mapping.tx_hash = value,
        ImportDataMappingField::FiatAmount => mapping.fiat_amount = value,
        ImportDataMappingField::FiatCurrency => mapping.fiat_currency = value,
        ImportDataMappingField::Fee => mapping.fee = value,
        ImportDataMappingField::FeeCurrency => mapping.fee_currency = value,
        ImportDataMappingField::Description => mapping.description = value,
    }
}

fn type_label(tx_type: ExchangeTxType) -> &'static str {
    tx_type.as_str()
}

fn match_label(state: MatchState) -> &'static str {
    match state {
        MatchState::HashMatch => "Hash match",
        MatchState::AmountDateMatch => "Amount + date",
        MatchState::ManualMatch => "Manual match",
        MatchState::Unmatched => "Unmatched",
    }
}

/// Catalog provider name corresponding to a detected CSV parser. The
/// generic/manual parser intentionally has no provider.
fn parser_provider_name(parser_id: &str) -> Option<&'static str> {
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

fn read_selected_file(
    file: dioxus::html::FileData,
    on_file: EventHandler<(String, String)>,
    mut error: Signal<Option<String>>,
) {
    let name = file.name();
    error.set(None);
    spawn(async move {
        match file.read_string().await {
            Ok(text) => on_file.call((name, text)),
            Err(err) => error.set(Some(err.to_string())),
        }
    });
}

/// Drop zone shared by the CSV import dialog.
#[component]
pub fn ImportDataDropzone(
    file_name: String,
    disabled: bool,
    on_file: EventHandler<(String, String)>,
) -> Element {
    let local_error = use_signal(|| Option::<String>::None);
    let accept = ".csv,.txt";

    rsx! {
        div {
            class: "md3-import-data-dropzone",
            ondragover: move |event| event.prevent_default(),
            ondrop: move |event| {
                event.prevent_default();
                if let Some(file) = event.files().into_iter().next() {
                    read_selected_file(file, on_file, local_error);
                }
            },
            label { class: "md3-import-data-dropzone-inner",
                input {
                    r#type: "file",
                    accept: "{accept}",
                    disabled,
                    class: "md3-import-data-file-input",
                    onchange: move |event| {
                        if let Some(file) = event.files().into_iter().next() {
                            read_selected_file(file, on_file, local_error);
                        }
                    },
                }
                span { class: "md3-import-data-dropzone-icon", "⇧" }
                strong {
                    if file_name.is_empty() {
                        "Choose a CSV file"
                    } else {
                        "Replace CSV file"
                    }
                }
                span { class: "text-body-small text-on-surface-variant",
                    if file_name.is_empty() {
                        "Drop a CSV export here or click to browse"
                    } else {
                        "{file_name}"
                    }
                }
            }
            if let Some(error) = local_error() {
                p { class: "text-body-small md3-exchange-error", "{error}" }
            }
        }
    }
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
    let mut name = use_signal(|| exchange_name.clone().unwrap_or_default());
    let mut provider_id = use_signal(|| Option::<i64>::None);
    let mut provider_touched = use_signal(|| false);
    let mut file_name = use_signal(String::new);
    let mut csv_text = use_signal(String::new);
    let mut mapping = use_signal(ImportDataMapping::default);
    let mut preview = use_signal(|| Option::<Result<ImportDataPreview, String>>::None);
    let mut loading = use_signal(|| false);
    let providers = use_resource(|| async move {
        list_exchange_types_api()
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

    // A detected built-in parser selects its catalog provider while the user
    // has not made an explicit provider choice. Manual Import remains the
    // fallback for unknown formats and the generic parser.
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
                crate::api::exchanges::create_exchange_api(name_value, provider_value)
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

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| on_close.call(()),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog md3-import-data-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "import-data-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "import-data-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "{title}"
                }

                if is_create {
                    label { class: "text-body-medium text-on-surface-variant", "Exchange name *" }
                    div { class: "md3-text-field",
                        input {
                            class: "md3-input",
                            value: "{name}",
                            placeholder: "e.g. India OKX 2020, Main Binance 2024, Revolut savings",
                            disabled: *loading.read(),
                            oninput: move |event| name.set(event.value()),
                        }
                    }
                    label { class: "text-body-medium text-on-surface-variant", "Provider *" }
                    div { class: "md3-text-field",
                        select {
                            class: "md3-input",
                            value: provider_id().map(|id| id.to_string()).unwrap_or_default(),
                            disabled: *loading.read(),
                            onchange: move |event| {
                                let selected = event.value().parse::<i64>().ok();
                                provider_touched.set(true);
                                provider_id.set(selected);
                                if name().trim().is_empty() {
                                    if let Some(Ok(list)) = providers() {
                                        if let Some(provider) = selected
                                            .and_then(|id| { list.into_iter().find(|entry| entry.id == id) })
                                        {
                                            name.set(provider.name);
                                        }
                                    }
                                }
                            },
                            option { value: "", "Manual Import" }
                            if let Some(Ok(list)) = providers() {
                                for provider in list {
                                    option { value: "{provider.id}", "{provider.name}" }
                                }
                            }
                        }
                    }
                }

                ImportDataDropzone {
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
                                        "Skipped {preview.skipped_non_btc} non-BTC row(s)"
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
                    label { class: "text-body-small text-on-surface-variant",
                        "Timestamp format (optional)"
                    }
                    p { class: "text-body-small text-on-surface-variant",
                        "Use dayjs-style tokens such as YYYY-MM-DD HH:mm:ss, or a chrono format beginning with %."
                    }
                    div { class: "md3-text-field",
                        input {
                            class: "md3-input",
                            value: "{mapping().timestamp_format}",
                            placeholder: "YYYY-MM-DD HH:mm:ss",
                            disabled: *loading.read(),
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
                                        th { "Fiat" }
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
                        onclick: move |_| on_close.call(()),
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
}

/// Edit an exchange account's name and provider.
#[component]
pub fn ExchangeAccountDialog(
    account: ExchangeAccount,
    on_close: EventHandler<()>,
    on_saved: EventHandler<ExchangeAccount>,
) -> Element {
    let mut toast = use_toast();
    let mut name = use_signal(|| account.name.clone());
    let mut provider_id = use_signal(|| account.provider_id);
    let mut loading = use_signal(|| false);
    let providers = use_resource(|| async move {
        list_exchange_types_api()
            .await
            .map_err(|error| error.to_string())
    });
    let account_id = account.id;

    let save = move |_| {
        if name().trim().is_empty() {
            toast.error("Enter an exchange name.");
            return;
        }
        let name_value = name().trim().to_string();
        let provider_value = provider_id();
        loading.set(true);
        spawn(async move {
            match crate::api::exchanges::update_exchange_api(account_id, name_value, provider_value)
                .await
            {
                Ok(updated) => on_saved.call(updated),
                Err(error) => toast.error(format!("Could not update the exchange: {error}")),
            }
            loading.set(false);
        });
    };

    rsx! {
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| on_close.call(()),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "exchange-edit-title",
                onclick: move |event| event.stop_propagation(),
                h2 {
                    id: "exchange-edit-title",
                    class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Edit exchange"
                }
                label { class: "text-body-medium text-on-surface-variant", "Exchange name *" }
                div { class: "md3-text-field",
                    input {
                        class: "md3-input",
                        value: "{name}",
                        disabled: *loading.read(),
                        oninput: move |event| name.set(event.value()),
                    }
                }
                label { class: "text-body-medium text-on-surface-variant", "Provider *" }
                div { class: "md3-text-field",
                    select {
                        class: "md3-input",
                        value: provider_id().map(|id| id.to_string()).unwrap_or_default(),
                        disabled: *loading.read(),
                        onchange: move |event| provider_id.set(event.value().parse::<i64>().ok()),
                        option { value: "", "Manual Import" }
                        if let Some(Ok(list)) = providers() {
                            for provider in list {
                                option { value: "{provider.id}", "{provider.name}" }
                            }
                        }
                    }
                }
                div { class: "md3-dialog-actions",
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
                        onclick: save,
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

/// Three-way duplicate fingerprint resolution.
#[component]
pub fn ExchangeDuplicateDialog(
    duplicate: ExchangeDuplicateInfo,
    on_cancel: EventHandler<()>,
    on_choice: EventHandler<ExchangeDuplicateChoice>,
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
                onclick: move |event| event.stop_propagation(),
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Duplicate transaction"
                }
                p { class: "text-body-medium text-on-surface-variant",
                    "Another row already has the same fingerprint: {duplicate.summary}. Choose whether to replace the existing row, keep both rows as an intentional duplicate, or cancel."
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
}

fn format_tx_hash(hash: &str) -> String {
    if hash.len() <= 18 {
        hash.to_string()
    } else {
        format!("{}...{}", &hash[..10], &hash[hash.len() - 8..])
    }
}

/// Side inspector for adding or editing one ledger transaction.
#[component]
pub fn ExchangeInspector(
    exchange_id: i64,
    #[props(default)] transaction: Option<ExchangeTransaction>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<ExchangeTransaction>,
) -> Element {
    let mut toast = use_toast();
    let mut timestamp = use_signal(|| {
        transaction
            .as_ref()
            .map(|row| format_editor_datetime(row.timestamp))
            .unwrap_or_else(|| format_editor_datetime(now()))
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
            .unwrap_or_else(|| "USD".to_string())
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
        let fee_units = if fee_currency().as_str().eq_ignore_ascii_case("BTC")
            || fee_currency().as_str().eq_ignore_ascii_case("XBT")
        {
            fee_value.map(|value| (value * 100_000_000.0).round())
        } else {
            fee_value
        };
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
            fee: fee_units,
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
                    onclick: move |_| on_close.call(()),
                    {icon(ICON_CLOSE)}
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
                    span { class: "text-body-small text-on-surface-variant", "Converted fiat" }
                    span { class: "text-body-medium text-on-surface",
                        "{format_fiat(amount, currency.as_deref().unwrap_or(\"\"))}"
                    }
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
                        oninput: move |event| amount.set(event.value()),
                    }
                }
                div { class: "md3-exchange-two-cols",
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Fiat amount" }
                        div { class: "md3-text-field",
                            input {
                                class: "md3-input",
                                value: "{fiat_amount}",
                                inputmode: "decimal",
                                disabled: *loading.read(),
                                oninput: move |event| fiat_amount.set(event.value()),
                            }
                        }
                    }
                    div {
                        label { class: "text-body-small text-on-surface-variant", "Fiat currency" }
                        div { class: "md3-text-field",
                            select {
                                class: "md3-input",
                                value: "{fiat_currency}",
                                disabled: *loading.read(),
                                onchange: move |event| fiat_currency.set(event.value()),
                                option { value: "", "—" }
                                for code in FIAT_CURRENCIES {
                                    option { value: "{code}", "{code}" }
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
                                oninput: move |event| fee.set(event.value()),
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
                                onchange: move |event| fee_currency.set(event.value()),
                                option { value: "", "—" }
                                option { value: "BTC", "BTC" }
                                option { value: "XBT", "XBT" }
                                for code in FIAT_CURRENCIES {
                                    option { value: "{code}", "{code}" }
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
                        disabled: *loading.read(),
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
                on_choice: move |choice| submit.call(Some(choice)),
            }
        }
    }
}

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
        div {
            class: "md3-dialog-scrim",
            role: "presentation",
            onclick: move |_| on_close.call(()),
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "md3-dialog md3-exchange-link-dialog",
                role: "dialog",
                aria_modal: "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "text-headline-medium leading-headline-medium text-on-surface",
                    "Link on-chain transaction"
                }
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
                    Some(Ok(list)) => rsx! {
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
                                    for candidate in list {
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
                    },
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
                        disabled: current_txid.is_none() || *loading.read(),
                        onclick: clear,
                        "Clear link"
                    }
                }
            }
        }
        if let Some((txid, info)) = duplicate() {
            ConfirmDialog {
                title: "Reuse on-chain transaction".to_string(),
                message: format!(
                    "This on-chain transaction is already linked to another exchange row: {}. Link this row to the same transaction too?",
                    info.summary,
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

fn format_editor_datetime(timestamp: i64) -> String {
    let formatted = civil_time::format_datetime(timestamp);
    formatted.get(..16).unwrap_or(&formatted).to_string()
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

fn now() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1_000.0) as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0)
    }
}

const ICON_CLOSE: &str = "M19 6.41 17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z";

fn icon(path: &str) -> Element {
    rsx! {
        svg {
            view_box: "0 0 24 24",
            width: "20",
            height: "20",
            fill: "currentColor",
            path { d: "{path}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser_provider_name;

    #[test]
    fn detected_parsers_map_to_catalog_providers() {
        for (parser, provider) in [
            ("crypto.com", "Crypto.com"),
            ("revolut", "Revolut"),
            ("binance", "Binance"),
            ("kraken", "Kraken"),
            ("coinbase", "Coinbase"),
            ("okx", "OKX"),
            ("bitget", "Bitget"),
            ("bybit", "Bybit"),
        ] {
            assert_eq!(parser_provider_name(parser), Some(provider));
        }
        assert_eq!(parser_provider_name("generic-trades"), None);
        assert_eq!(parser_provider_name("unknown"), None);
    }
}
