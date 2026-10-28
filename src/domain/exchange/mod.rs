#![cfg_attr(not(feature = "server"), allow(dead_code))]

//! Exchange ledger domain: accounts, imported transaction rows, CSV column
//! mappings, table queries, and on-chain reconciliation value types.
//!
//! The exchange ledger is independent from the on-chain transaction store.
//! Amounts are normalised to satoshis because only BTC/XBT rows are imported.

pub mod csv;
pub mod matching;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::table::{TablePage, TableQuery};

/// Kind of one exchange ledger row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExchangeTxType {
    Buy,
    Sell,
    Reward,
    Withdrawal,
    Deposit,
    Unknown,
}

impl ExchangeTxType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
            Self::Reward => "reward",
            Self::Withdrawal => "withdrawal",
            Self::Deposit => "deposit",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "buy" => Some(Self::Buy),
            "sell" => Some(Self::Sell),
            "reward" => Some(Self::Reward),
            "withdrawal" => Some(Self::Withdrawal),
            "deposit" => Some(Self::Deposit),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Reconciliation state of one exchange ledger row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchState {
    HashMatch,
    AmountDateMatch,
    ManualMatch,
    Unmatched,
}

impl MatchState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HashMatch => "hash_match",
            Self::AmountDateMatch => "amount_date_match",
            Self::ManualMatch => "manual_match",
            Self::Unmatched => "unmatched",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "hash_match" => Some(Self::HashMatch),
            "amount_date_match" => Some(Self::AmountDateMatch),
            "manual_match" => Some(Self::ManualMatch),
            "unmatched" => Some(Self::Unmatched),
            _ => None,
        }
    }
}

/// Evidence source of a durable on-chain link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnChainLinkSource {
    Hash,
    AmountDate,
    Manual,
}

impl OnChainLinkSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hash => "hash",
            Self::AmountDate => "amount_date",
            Self::Manual => "manual",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "hash" => Some(Self::Hash),
            "amount_date" => Some(Self::AmountDate),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

/// Durable link between an exchange ledger row and cached on-chain data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainLink {
    pub txid: String,
    pub local_addresses: Vec<String>,
    pub source: OnChainLinkSource,
    pub linked_at: i64,
}

/// One exchange account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeAccount {
    pub id: i64,
    pub name: String,
    /// `None` means Manual Import: the ledger was imported without a
    /// matching exchange provider and does not contribute to an address
    /// balance.
    pub provider_id: Option<i64>,
    pub provider_name: Option<String>,
    pub created_at: i64,
    pub last_imported_at: Option<i64>,
    pub last_modified_at: i64,
}

/// Human-readable duplicate row shown by the resolution dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeDuplicateInfo {
    pub transaction_id: i64,
    pub summary: String,
}

/// One exchange transaction together with its derived fiat conversion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeTransaction {
    pub id: i64,
    pub exchange_id: i64,
    pub fingerprint: String,
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    pub amount_sats: i64,
    pub currency: String,
    pub fiat_amount: Option<f64>,
    pub fiat_currency: Option<String>,
    /// Derived at read time from the original fiat value and the current
    /// default currency. Never stored.
    pub converted_fiat_amount: Option<f64>,
    pub converted_fiat_currency: Option<String>,
    pub fee: Option<f64>,
    pub fee_currency: Option<String>,
    pub tx_hash: Option<String>,
    pub description: Option<String>,
    pub raw_data: BTreeMap<String, String>,
    pub match_state: MatchState,
    pub matched_tx_id: Option<String>,
    pub on_chain_link: Option<OnChainLink>,
    pub is_manually_edited: bool,
    pub is_intentional_duplicate: bool,
}

/// One row of the exchanges list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeAccountRow {
    pub id: i64,
    pub name: String,
    /// `None` means Manual Import.
    pub provider_id: Option<i64>,
    pub provider_name: Option<String>,
    /// All ledger rows of the account.
    pub tx_count: i64,
    /// Withdrawals/deposits with a non-unmatched reconciliation state.
    pub onchain_matched_count: i64,
    /// All withdrawals/deposits, whether matched or not.
    pub onchain_total_count: i64,
    pub first_tx_time: Option<i64>,
    pub last_tx_time: Option<i64>,
    pub last_modified_at: i64,
}

/// Global KPI totals of the exchanges list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ExchangeOverviewTotals {
    pub exchange_count: i64,
    /// All ledger rows across every exchange account.
    pub total_transactions: i64,
    /// Matched withdrawals/deposits across every account.
    pub onchain_matched_count: i64,
    /// All withdrawals/deposits across every account.
    pub onchain_total_count: i64,
}

/// One page of exchange accounts plus the global totals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeAccountList {
    pub page: TablePage<ExchangeAccountRow>,
    pub totals: ExchangeOverviewTotals,
}

/// Exchange detail KPIs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExchangeKpis {
    pub bought_sats: i64,
    pub fiat_spent: f64,
    pub sold_sats: i64,
    pub fiat_received: f64,
    pub withdrawn_sats: i64,
    pub deposited_sats: i64,
    pub btc_fees_sats: i64,
    pub unmatched_count: i64,
    pub fiat_currency: String,
}

/// Account detail header plus the KPIs that are independent of the current
/// table page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeDetail {
    pub account: ExchangeAccount,
    pub kpis: ExchangeKpis,
    pub conversion_warning: Option<String>,
}

/// Query of the exchanges list table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExchangeAccountTableQuery {
    #[serde(flatten)]
    pub table: TableQuery,
}

/// Query of one exchange's transaction table.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ExchangeTransactionTableQuery {
    pub exchange_id: i64,
    #[serde(flatten)]
    pub table: TableQuery,
}

/// Manual mapping between CSV headers and canonical exchange fields.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImportDataMapping {
    pub timestamp: String,
    /// `chrono` format string; empty means ISO 8601 / common formats.
    pub timestamp_format: String,
    pub amount: String,
    pub currency: String,
    #[serde(rename = "type")]
    pub tx_type: String,
    pub tx_hash: String,
    pub fiat_amount: String,
    pub fiat_currency: String,
    pub fee: String,
    pub fee_currency: String,
    pub description: String,
}

impl ImportDataMapping {
    pub fn is_complete(&self) -> bool {
        !self.timestamp.trim().is_empty()
            && !self.amount.trim().is_empty()
            && !self.currency.trim().is_empty()
            && !self.tx_type.trim().is_empty()
    }
}

/// One preview row returned while importing a CSV.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDataPreviewRow {
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    pub amount_sats: i64,
    pub currency: String,
    pub converted_fiat_amount: Option<f64>,
    pub converted_fiat_currency: Option<String>,
    pub tx_hash: Option<String>,
}

/// Result of reading and parsing a CSV for preview.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDataPreview {
    pub headers: Vec<String>,
    pub row_count: usize,
    pub detected_parser_id: Option<String>,
    pub detected_parser_name: Option<String>,
    pub parsed_count: usize,
    pub skipped_non_btc: usize,
    pub preview: Vec<ImportDataPreviewRow>,
    pub converted_fiat_currency: String,
    pub conversion_warning: Option<String>,
}

/// Result of importing a CSV into an account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDataSummary {
    pub exchange_id: i64,
    pub created: bool,
    pub inserted: usize,
    pub duplicates: usize,
    pub auto_linked: usize,
    pub skipped_non_btc: usize,
    pub conversion_warning: Option<String>,
}

/// Editable fields of a manually entered or edited ledger row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeTransactionDraft {
    pub timestamp: i64,
    pub tx_type: ExchangeTxType,
    pub amount_sats: i64,
    pub currency: String,
    pub fiat_amount: Option<f64>,
    pub fiat_currency: Option<String>,
    pub fee: Option<f64>,
    pub fee_currency: Option<String>,
    pub tx_hash: Option<String>,
    pub description: Option<String>,
}

/// How an explicit duplicate fingerprint is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExchangeDuplicateChoice {
    KeepBoth,
    Replace,
}

/// Save request of the transaction inspector.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeTransactionSaveRequest {
    pub exchange_id: i64,
    pub transaction_id: Option<i64>,
    pub draft: ExchangeTransactionDraft,
    pub duplicate_choice: Option<ExchangeDuplicateChoice>,
}

/// Save result: either the stored row, or the duplicate that needs a choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExchangeTransactionSaveOutcome {
    Saved(Box<ExchangeTransaction>),
    Duplicate(ExchangeDuplicateInfo),
}

/// One on-chain link candidate shown in the manual linking modal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnChainLinkCandidateView {
    pub txid: String,
    pub local_addresses: Vec<ExchangeCandidateAddress>,
    pub source: OnChainLinkSource,
    pub likeliness: f64,
    pub amount_delta_sats: Option<i64>,
    pub timestamp_delta_seconds: i64,
    pub known_movement_sats: i64,
    pub fee_sats: i64,
    pub block_time: i64,
    pub is_already_linked: bool,
}

/// Address displayed inside one candidate row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeCandidateAddress {
    pub address: String,
    pub label: Option<String>,
}

/// Result of a manual link request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExchangeLinkOutcome {
    Linked(Box<ExchangeTransaction>),
    Duplicate(ExchangeDuplicateInfo),
}

/// Confirmation setting plus reconciliation scoring settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeSettings {
    pub confirm_destructive_actions: bool,
    pub on_chain_link_scoring: OnChainLinkScoringSettings,
}

impl Default for ExchangeSettings {
    fn default() -> Self {
        Self {
            confirm_destructive_actions: true,
            on_chain_link_scoring: OnChainLinkScoringSettings::default(),
        }
    }
}

/// Relative weights and tolerances of the on-chain matching score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OnChainLinkScoringSettings {
    pub hash_weight: f64,
    pub amount_weight: f64,
    pub payment_output_weight: f64,
    pub fee_adjusted_amount_weight: f64,
    pub timestamp_weight: f64,
    pub direction_weight: f64,
    pub tx_shape_weight: f64,
    pub unmatched_weight: f64,
    pub ambiguity_weight: f64,
    pub amount_tolerance_sats: f64,
    pub timestamp_tolerance_hours: f64,
}

impl Default for OnChainLinkScoringSettings {
    fn default() -> Self {
        Self {
            hash_weight: 0.32,
            amount_weight: 0.12,
            payment_output_weight: 0.18,
            fee_adjusted_amount_weight: 0.12,
            timestamp_weight: 0.10,
            direction_weight: 0.08,
            tx_shape_weight: 0.05,
            unmatched_weight: 0.02,
            ambiguity_weight: 0.01,
            amount_tolerance_sats: 5_000.0,
            timestamp_tolerance_hours: 168.0,
        }
    }
}

impl OnChainLinkScoringSettings {
    /// Whether every value is already in the range [`Self::normalized`]
    /// keeps: finite, non-negative weights and finite, positive tolerances.
    /// The settings screen uses this to auto-save only valid edits instead of
    /// silently normalizing them.
    pub fn is_valid(&self) -> bool {
        [
            self.hash_weight,
            self.amount_weight,
            self.payment_output_weight,
            self.fee_adjusted_amount_weight,
            self.timestamp_weight,
            self.direction_weight,
            self.tx_shape_weight,
            self.unmatched_weight,
            self.ambiguity_weight,
        ]
        .into_iter()
        .all(|weight| weight.is_finite() && weight >= 0.0)
            && self.amount_tolerance_sats.is_finite()
            && self.amount_tolerance_sats > 0.0
            && self.timestamp_tolerance_hours.is_finite()
            && self.timestamp_tolerance_hours > 0.0
    }

    /// Normalize user input: weights cannot be negative and tolerances must
    /// remain positive.
    pub fn normalized(&self) -> Self {
        fn non_negative(value: f64) -> f64 {
            if value.is_finite() && value >= 0.0 {
                value
            } else {
                0.0
            }
        }
        fn positive(value: f64, fallback: f64) -> f64 {
            if value.is_finite() && value > 0.0 {
                value
            } else {
                fallback
            }
        }

        let defaults = Self::default();
        Self {
            hash_weight: non_negative(self.hash_weight),
            amount_weight: non_negative(self.amount_weight),
            payment_output_weight: non_negative(self.payment_output_weight),
            fee_adjusted_amount_weight: non_negative(self.fee_adjusted_amount_weight),
            timestamp_weight: non_negative(self.timestamp_weight),
            direction_weight: non_negative(self.direction_weight),
            tx_shape_weight: non_negative(self.tx_shape_weight),
            unmatched_weight: non_negative(self.unmatched_weight),
            ambiguity_weight: non_negative(self.ambiguity_weight),
            amount_tolerance_sats: positive(
                self.amount_tolerance_sats,
                defaults.amount_tolerance_sats,
            ),
            timestamp_tolerance_hours: positive(
                self.timestamp_tolerance_hours,
                defaults.timestamp_tolerance_hours,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scoring_matches_the_utxo_utils_weights() {
        let settings = OnChainLinkScoringSettings::default();
        assert_eq!(settings.hash_weight, 0.32);
        assert_eq!(settings.amount_tolerance_sats, 5_000.0);
        assert_eq!(settings.timestamp_tolerance_hours, 168.0);
    }

    #[test]
    fn scoring_normalization_rejects_negative_weights_and_zero_tolerances() {
        let settings = OnChainLinkScoringSettings {
            hash_weight: -1.0,
            amount_tolerance_sats: 0.0,
            timestamp_tolerance_hours: f64::NAN,
            ..OnChainLinkScoringSettings::default()
        }
        .normalized();
        assert_eq!(settings.hash_weight, 0.0);
        assert_eq!(settings.amount_tolerance_sats, 5_000.0);
        assert_eq!(settings.timestamp_tolerance_hours, 168.0);
    }

    #[test]
    fn scoring_validity_matches_the_normalized_ranges() {
        let defaults = OnChainLinkScoringSettings::default();
        assert!(defaults.is_valid());
        assert!(!OnChainLinkScoringSettings {
            hash_weight: -0.1,
            ..defaults.clone()
        }
        .is_valid());
        assert!(!OnChainLinkScoringSettings {
            amount_tolerance_sats: 0.0,
            ..defaults.clone()
        }
        .is_valid());
        assert!(!OnChainLinkScoringSettings {
            timestamp_tolerance_hours: f64::NAN,
            ..defaults.clone()
        }
        .is_valid());
    }

    #[test]
    fn csv_mapping_is_complete_only_with_required_fields() {
        assert!(!ImportDataMapping::default().is_complete());
        let mapping = ImportDataMapping {
            timestamp: "Date".into(),
            amount: "Amount".into(),
            currency: "Currency".into(),
            tx_type: "Type".into(),
            ..ImportDataMapping::default()
        };
        assert!(mapping.is_complete());
    }
}
