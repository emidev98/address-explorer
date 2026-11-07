//! Exchange-specific dialogs: Import Data, account edit, transaction
//! inspector, duplicate resolution, and manual on-chain linking.

mod account_dialog;
mod duplicate;
mod import_data;
mod inspector;
mod link_dialog;

pub use account_dialog::ExchangeAccountDialog;
pub(super) use duplicate::{duplicate_summary, ExchangeDuplicateDialog};
pub use import_data::ImportDataDialog;
pub use inspector::ExchangeInspector;
pub use link_dialog::ExchangeLinkDialog;

use super::icon::{Icon, IconName};
use super::Modal;

use crate::api::catalogs::catalog_options_api;
use crate::api::exchanges::{
    clear_exchange_transaction_link_api, exchange_link_candidates_api, import_data_api,
    link_exchange_transaction_api, preview_import_data_api, save_exchange_transaction_api,
};
use crate::components::format::{format_address, format_btc, format_fiat, format_tx_hash};
use crate::components::{
    use_fiat_currency, use_toast, CatalogOptionsSelect, ConfirmDialog, CsvDropzone,
};
use crate::domain::catalog::CatalogKind;
use crate::domain::civil_time;
use crate::domain::exchange::{
    ExchangeAccount, ExchangeDuplicateChoice, ExchangeDuplicateInfo, ExchangeLinkOutcome,
    ExchangeTransaction, ExchangeTransactionDraft, ExchangeTransactionSaveOutcome,
    ExchangeTransactionSaveRequest, ExchangeTxType, ImportDataMapping, ImportDataPreview,
    ImportDataSummary, MatchState,
};
use crate::domain::price::FIAT_CURRENCIES;
use crate::domain::table::DEFAULT_PAGE_SIZE;

/// Row cap of the link-candidate table: the picker ranks the best matches and
/// the search field narrows the rest.
const MAX_LINK_CANDIDATES: usize = DEFAULT_PAGE_SIZE as usize;

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
    (ImportDataMappingField::FiatAmount, "Value", false),
    (
        ImportDataMappingField::FiatCurrency,
        "Value currency",
        false,
    ),
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

/// Human-readable summary of one ledger CSV import.
pub fn import_data_summary_message(summary: &ImportDataSummary) -> String {
    if summary.inserted == 0 && summary.created {
        return format!(
            "Exchange account created.{}",
            if summary.skipped_non_btc > 0 {
                format!(" Skipped {} non-BTC rows.", summary.skipped_non_btc)
            } else {
                String::new()
            }
        );
    }
    let mut message = format!(
        "{} {} transaction{}{}{}.",
        if summary.created {
            "Account created and imported"
        } else {
            "Imported"
        },
        summary.inserted,
        if summary.inserted == 1 { "" } else { "s" },
        if summary.auto_linked > 0 {
            format!(", auto-linked {}", summary.auto_linked)
        } else {
            String::new()
        },
        if summary.duplicates > 0 {
            format!(
                ", skipped {} {}",
                summary.duplicates,
                if summary.duplicates == 1 {
                    "duplicate"
                } else {
                    "duplicates"
                }
            )
        } else {
            String::new()
        }
    );
    if summary.skipped_non_btc > 0 {
        message.push_str(&format!(
            " Skipped {} non-BTC {}.",
            summary.skipped_non_btc,
            if summary.skipped_non_btc == 1 {
                "row"
            } else {
                "rows"
            }
        ));
    }
    message
}

#[cfg(test)]
mod tests {
    use super::import_data::parser_provider_name;
    use super::import_data_summary_message;
    use crate::domain::exchange::ImportDataSummary;

    #[test]
    fn import_summary_handles_created_and_partial_imports() {
        let summary = ImportDataSummary {
            exchange_id: 1,
            created: true,
            inserted: 0,
            duplicates: 0,
            auto_linked: 0,
            skipped_non_btc: 2,
            conversion_warning: None,
        };
        assert_eq!(
            import_data_summary_message(&summary),
            "Exchange account created. Skipped 2 non-BTC rows."
        );
        let summary = ImportDataSummary {
            inserted: 1,
            skipped_non_btc: 0,
            created: false,
            ..summary
        };
        assert_eq!(
            import_data_summary_message(&summary),
            "Imported 1 transaction."
        );
    }

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
