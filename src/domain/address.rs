//! Domain types and validation for stored Bitcoin addresses.

#[cfg(feature = "server")]
use std::str::FromStr;

#[cfg(feature = "server")]
use bitcoin::{Address as BitcoinAddress, Network};
use serde::{Deserialize, Serialize};

/// Persisted, SQLite-normalized representation of a mempool address.
///
/// `exchange_id` and `wallet_id` are optional foreign keys into the
/// `exchanges_types` and `wallets_types` catalogs. At most one of them is set:
/// an address is unclassified, owned by a wallet, or owned by an exchange
/// (see `application::addresses` and the triggers added by migration 0006).
/// Exchange addresses are exempt from full transaction indexing and sync via
/// the reverse search in `application::transactions` instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AddressEntity {
    pub address: String,

    pub description: Option<String>,
    pub exchange_id: Option<i64>,
    pub wallet_id: Option<i64>,

    pub chain_funded_txo_count: i64,
    pub chain_funded_txo_sum: i64,
    pub chain_spent_txo_count: i64,
    pub chain_spent_txo_sum: i64,
    pub chain_tx_count: i64,

    pub mempool_funded_txo_count: i64,
    pub mempool_funded_txo_sum: i64,
    pub mempool_spent_txo_count: i64,
    pub mempool_spent_txo_sum: i64,
    pub mempool_tx_count: i64,

    /// Whether transaction indexing stopped on a mempool rate limit and the
    /// stored history is incomplete. The frontend shows a warning and offers
    /// resuming the walk, which continues from the oldest indexed
    /// transaction. Imports and stats refreshes never clear this flag.
    pub tx_index_rate_limited: bool,
}

/// Stored address with the owner catalog names resolved for display.
///
/// The addresses table lists descriptions and owner labels directly; the names
/// exist only for presentation and are absent when an address is unclassified.
// Only the server-side repository constructs list items; web builds merely
// deserialize them through the API client stubs.
#[cfg_attr(not(feature = "server"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressListItem {
    #[serde(flatten)]
    pub entity: AddressEntity,
    pub exchange_name: Option<String>,
    pub wallet_name: Option<String>,
}

/// Result of importing an address and indexing its transactions.
///
/// The import itself succeeded when this value exists; transaction indexing
/// is reported separately: it either completed, stopped early on a mempool
/// rate limit (`address.tx_index_rate_limited`), or failed outright
/// (`sync_error`, e.g. the mempool being unreachable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressImportResult {
    pub address: AddressEntity,
    /// Human-readable indexing failure, `None` when the sync ran.
    pub sync_error: Option<String>,
}

/// Owner classification of one address — the "scope" of the addresses
/// table. The catalog id always identifies the owner; the name is resolved
/// for display and is `None` when the catalog row no longer exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressScope {
    /// Not classified.
    Unassigned,
    /// Owned by a wallet.
    Wallet { id: i64, name: Option<String> },
    /// Owned by an exchange; such addresses sync through the reverse search
    /// instead of a full history walk.
    Exchange { id: i64, name: Option<String> },
}

impl AddressScope {
    /// Build the scope from the stored owner ids and resolved catalog names.
    /// The either-or invariant is enforced behind this boundary, so a
    /// (contradicting) pair of ids resolves to the exchange, matching the
    /// sync behavior keyed on `exchange_id`. Server-side: the mapping of
    /// stored rows to scopes happens in the service layer.
    #[cfg(feature = "server")]
    pub fn new(
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
        exchange_name: Option<String>,
        wallet_name: Option<String>,
    ) -> Self {
        if let Some(id) = exchange_id {
            return Self::Exchange {
                id,
                name: exchange_name,
            };
        }
        if let Some(id) = wallet_id {
            return Self::Wallet {
                id,
                name: wallet_name,
            };
        }
        Self::Unassigned
    }

    /// Whether the scope is an exchange.
    pub fn is_exchange(&self) -> bool {
        matches!(self, Self::Exchange { .. })
    }
}

/// Scope filter of the addresses table: the three states of the Scope
/// column's header toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddressScopeFilter {
    /// Wallets, exchanges, and unclassified addresses (the default).
    #[default]
    All,
    /// Wallet-owned addresses only.
    Wallets,
    /// Exchange-owned addresses only.
    Exchanges,
}

impl AddressScopeFilter {
    /// Singular noun used by the empty-state messages of the page, e.g.
    /// `No wallet data`.
    pub fn empty_noun(self) -> &'static str {
        match self {
            Self::All => "address",
            Self::Wallets => "wallet",
            Self::Exchanges => "exchange",
        }
    }
}

/// Query of the addresses overview endpoint: backend sorting and the scope
/// filter. Sorting follows the shared table contract (Section 5.11 of
/// CONTRIBUTING.md); the overview is not paginated because the KPIs and the
/// holdings chart derive from the full listing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressOverviewQuery {
    /// Key of the sortable column, `None` for the default order (spendable
    /// balance descending).
    pub sort_by: Option<String>,
    /// Sort direction; ignored without `sort_by`.
    pub sort_dir: crate::domain::table::SortDirection,
    /// Scope filter of the rows.
    pub scope: AddressScopeFilter,
}

/// One row of the addresses overview page.
///
/// The overview replicates the UTXO Utils addresses table: label, confirmed
/// transaction count, spendable balance, first/last stored transaction time,
/// the address type, and the owner scope. `spendable_balance_sats` derives
/// from the stored mempool chain statistics, except for exchange-scoped
/// addresses, which report zero until the Exchanges page is available; the
/// transaction figures come from the local `transactions` store and are zero
/// until the address is synced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressOverviewItem {
    pub address: String,
    pub description: Option<String>,
    /// rust-bitcoin address type label (`p2pkh`, `p2sh`, `p2wpkh`, `p2wsh`,
    /// `p2tr`), `None` when the stored string fails to parse.
    pub address_type: Option<String>,
    /// `chain_funded_txo_sum - chain_spent_txo_sum`, in satoshis; zero for
    /// exchange-scoped addresses until the Exchanges page is available.
    pub spendable_balance_sats: i64,
    /// Stored transactions with `confirmed = 1`.
    pub confirmed_tx_count: i64,
    /// `block_time` of the oldest stored confirmed transaction.
    pub first_tx_time: Option<i64>,
    /// `block_time` of the newest stored confirmed transaction.
    pub last_tx_time: Option<i64>,
    /// Owner classification (wallet, exchange, or unclassified).
    pub scope: AddressScope,
    /// Whether transaction indexing stopped on a mempool rate limit; the UI
    /// warns and offers resuming the walk.
    pub tx_index_rate_limited: bool,
}

/// Addresses overview: the page rows plus the KPI totals.
///
/// `items` match the requested sorting and scope filter. The KPI totals are
/// global: they describe every stored address regardless of the filter, so
/// the cards do not change when the table is filtered. The balance total
/// excludes exchange-scoped addresses until the Exchanges page is available.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddressOverview {
    pub items: Vec<AddressOverviewItem>,
    pub address_count: i64,
    /// Confirmed transactions stored across all addresses.
    pub confirmed_tx_count: i64,
    /// Total spendable balance across non-exchange addresses, in satoshis.
    pub spendable_balance_sats: i64,
}

/// Outcome of the import dialog's create-only save: importing an address
/// that is already stored is rejected instead of refreshing it (modifying a
/// stored address goes through the dialog's modify mode).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AddressSaveOutcome {
    /// The address was stored and its transactions indexed.
    Imported(Box<AddressImportResult>),
    /// The address already exists; nothing was stored.
    Duplicate,
}

/// Longest accepted `description` for an imported address.
const MAX_DESCRIPTION_LENGTH: usize = 200;

/// Normalize the optional description of an address for storage.
///
/// Whitespace-only input means "no description". The length cap keeps the
/// addresses table free of unbounded text; the dialog surfaces a shortened,
/// actionable error for longer input.
// Only the server-side import workflow normalizes descriptions today; the
// dialog sends raw input to the server function.
#[cfg_attr(not(feature = "server"), allow(dead_code))]
pub fn normalize_description(description: Option<&str>) -> Result<Option<String>, String> {
    let Some(description) = description else {
        return Ok(None);
    };

    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if trimmed.chars().count() > MAX_DESCRIPTION_LENGTH {
        return Err(format!(
            "The description is limited to {MAX_DESCRIPTION_LENGTH} characters."
        ));
    }

    Ok(Some(trimmed.to_string()))
}

/// Validate and canonicalize a Bitcoin mainnet address with rust-bitcoin.
///
/// This runs on the server only: the browser build does not compile
/// rust-bitcoin's C dependencies, and clients send addresses through
/// `save_address_api`, which performs this validation before any network call.
#[cfg(feature = "server")]
pub fn validate_bitcoin_address(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("Enter a Bitcoin address.".to_string());
    }

    let address = BitcoinAddress::from_str(trimmed)
        .map_err(|_| "Enter a valid Bitcoin address.".to_string())?
        .require_network(Network::Bitcoin)
        .map_err(|_| "The address is not valid on Bitcoin mainnet.".to_string())?;

    Ok(address.to_string())
}

/// Address type label of a stored address (`p2pkh`, `p2sh`, `p2wpkh`,
/// `p2wsh`, `p2tr`), matching the labels UTXO Utils shows in its Type column.
///
/// The type derives from the script embedded in the address string, so no
/// network check is needed and an unparseable stored value yields `None`
/// instead of an error. Server-only, like [`validate_bitcoin_address`].
#[cfg(feature = "server")]
pub fn address_type_label(value: &str) -> Option<&'static str> {
    use bitcoin::address::AddressType;

    let address = BitcoinAddress::from_str(value.trim()).ok()?;
    // The type derives from the embedded script; network validation is
    // irrelevant here, so the unchecked parse is assumed checked first.
    match address.assume_checked().address_type()? {
        AddressType::P2pkh => Some("p2pkh"),
        AddressType::P2sh => Some("p2sh"),
        AddressType::P2wpkh => Some("p2wpkh"),
        AddressType::P2wsh => Some("p2wsh"),
        AddressType::P2tr => Some("p2tr"),
        _ => None,
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::{
        address_type_label, normalize_description, validate_bitcoin_address, AddressScopeFilter,
    };

    #[test]
    fn scope_filters_have_singular_empty_state_nouns() {
        assert_eq!(AddressScopeFilter::All.empty_noun(), "address");
        assert_eq!(AddressScopeFilter::Wallets.empty_noun(), "wallet");
        assert_eq!(AddressScopeFilter::Exchanges.empty_noun(), "exchange");
    }

    #[test]
    fn accepts_mainnet_legacy_and_bech32_addresses() {
        assert_eq!(
            validate_bitcoin_address(" 1BoatSLRHtKNngkdXEeobR76b53LETtpyT ").unwrap(),
            "1BoatSLRHtKNngkdXEeobR76b53LETtpyT"
        );
        assert_eq!(
            validate_bitcoin_address("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq").unwrap(),
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
        );
    }

    #[test]
    fn rejects_empty_invalid_and_testnet_addresses() {
        assert!(validate_bitcoin_address("").is_err());
        assert!(validate_bitcoin_address("not-a-bitcoin-address").is_err());
        assert!(validate_bitcoin_address("mipcBbFg9gMiCh81Kj8tqqdgoZub1ZJRfn").is_err());
    }

    #[test]
    fn address_type_labels_match_the_utxo_utils_type_column() {
        use std::str::FromStr;

        use bitcoin::secp256k1::{Secp256k1, XOnlyPublicKey};
        use bitcoin::{
            Address as BitcoinAddress, CompressedPublicKey, Network, PublicKey, ScriptBuf,
        };

        let public_key = PublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        let compressed = CompressedPublicKey::from_str(
            "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();
        let script = ScriptBuf::from_hex(
            "51210379be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f8179851ae",
        )
        .unwrap();
        let internal_key = XOnlyPublicKey::from_str(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
        )
        .unwrap();

        let p2pkh = BitcoinAddress::p2pkh(public_key, Network::Bitcoin);
        assert_eq!(address_type_label(&p2pkh.to_string()), Some("p2pkh"));

        let p2sh = BitcoinAddress::p2sh(&script, Network::Bitcoin).unwrap();
        assert_eq!(address_type_label(&p2sh.to_string()), Some("p2sh"));

        let p2wpkh = BitcoinAddress::p2wpkh(&compressed, Network::Bitcoin);
        assert_eq!(address_type_label(&p2wpkh.to_string()), Some("p2wpkh"));

        let p2wsh = BitcoinAddress::p2wsh(&script, Network::Bitcoin);
        assert_eq!(address_type_label(&p2wsh.to_string()), Some("p2wsh"));

        let p2tr = BitcoinAddress::p2tr(&Secp256k1::new(), internal_key, None, Network::Bitcoin);
        assert_eq!(address_type_label(&p2tr.to_string()), Some("p2tr"));

        assert_eq!(address_type_label("not-a-bitcoin-address"), None);
    }

    #[test]
    fn descriptions_are_trimmed_empty_means_none_and_capped() {
        assert_eq!(normalize_description(None).unwrap(), None);
        assert_eq!(normalize_description(Some("  ")).unwrap(), None);
        assert_eq!(
            normalize_description(Some("  Binance hot wallet 3 ")).unwrap(),
            Some("Binance hot wallet 3".to_string())
        );

        let too_long = "x".repeat(201);
        assert!(normalize_description(Some(&too_long)).is_err());

        let at_cap = "x".repeat(200);
        assert_eq!(
            normalize_description(Some(&at_cap)).unwrap(),
            Some(at_cap.clone())
        );
    }
}
