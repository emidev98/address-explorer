//! Address service: import, list, and delete stored Bitcoin addresses.
//!
//! The service owns the address workflow: input validation, the owner
//! classification rules, the mempool stats fetch, and the persistence of the
//! imported address. SQL stays in `data::repositories::addresses`; the
//! mempool wire model is mapped to the domain entity at this boundary.

use crate::application::transactions::TransactionService;
use crate::clients::mempool::bitcoin::types::Address as MempoolAddress;
use crate::clients::mempool::error::MempoolError;
use crate::clients::mempool::{config::MempoolConfig, Mempool};
use crate::data::repositories::{addresses as address_repository, catalogs as catalog_repository};
use crate::data::{Database, DatabaseError};
use crate::domain::address::{
    address_type_label, normalize_description, validate_bitcoin_address, AddressEntity,
    AddressImportResult, AddressListItem, AddressOverview, AddressOverviewItem,
    AddressOverviewQuery, AddressScope,
};

/// Failures of the address service.
#[derive(Debug, thiserror::Error)]
pub enum AddressServiceError {
    /// User-facing validation failure (address, description, or owner).
    #[error("{0}")]
    Validation(String),
    /// The import dialog is create-only: the address is already stored and
    /// must be changed through the modify flow instead.
    #[error("address already exists")]
    AlreadyExists,
    #[error("address not found")]
    NotFound,
    #[error("address was not stored")]
    NotStored,
    /// Client sort key outside the repository's allowlist.
    #[error("unknown sort column: {0}")]
    InvalidSortColumn(String),
    #[error("database access failed: {0}")]
    Database(#[from] DatabaseError),
    #[error("mempool request failed: {0}")]
    Mempool(#[from] MempoolError),
}

/// Facade over the addresses repository and the mempool client.
pub struct AddressService {
    database: Database,
    mempool: Mempool,
}

impl AddressService {
    /// Connect the mempool client with its default configuration.
    pub fn connect(database: Database) -> Self {
        Self {
            database,
            mempool: Mempool::new(MempoolConfig::default()),
        }
    }

    /// Stored addresses with the owner catalog names resolved for display.
    pub fn list_addresses(&self) -> Result<Vec<AddressListItem>, AddressServiceError> {
        Ok(address_repository::list_address_items(&self.database)?)
    }

    /// Addresses overview: the page rows plus the KPI totals.
    ///
    /// The rows are sorted and scope-filtered in the backend (Section 5.11
    /// of CONTRIBUTING.md); the sort key is validated against the
    /// repository's allowlist before any SQL is built. The KPI totals are
    /// global and ignore the filter. No network access.
    pub fn addresses_overview(
        &self,
        query: &AddressOverviewQuery,
    ) -> Result<AddressOverview, AddressServiceError> {
        let sort = match query.sort_by.as_deref() {
            None => None,
            Some(key) => Some((
                address_repository::AddressSortColumn::from_key(key)
                    .ok_or_else(|| AddressServiceError::InvalidSortColumn(key.to_string()))?,
                query.sort_dir,
            )),
        };

        let rows = address_repository::list_overview_rows(
            &self.database,
            &address_repository::AddressOverviewFilter {
                sort,
                scope: query.scope,
            },
        )?;
        let (address_count, confirmed_tx_count, spendable_balance_sats) =
            address_repository::overview_totals(&self.database)?;

        Ok(AddressOverview {
            items: rows.iter().map(overview_item).collect(),
            address_count,
            confirmed_tx_count,
            spendable_balance_sats,
        })
    }

    /// One stored address by its base58/bech32 string.
    pub fn get_address(&self, address: &str) -> Result<AddressEntity, AddressServiceError> {
        address_repository::get_address(&self.database, address)?
            .ok_or(AddressServiceError::NotFound)
    }

    /// Delete a stored address; its transactions cascade away with it.
    pub fn delete_address(&self, address: &str) -> Result<(), AddressServiceError> {
        Ok(address_repository::delete_address(&self.database, address)?)
    }

    /// Import an address and index its transaction history.
    ///
    /// This is the use case behind the import dialog's confirm action. The
    /// import is create-only: an address that is already stored is rejected
    /// with [`AddressServiceError::AlreadyExists`] without touching it —
    /// changing a stored address goes through [`Self::update_address`]. A
    /// new address is validated and stored first, then the transaction
    /// service indexes its history (the reverse search for exchange-owned
    /// addresses). Transaction indexing never fails the import — the
    /// address is already stored at that point — so its outcome is
    /// reported inside the result: complete, paused on a mempool rate
    /// limit (`tx_index_rate_limited` on the returned entity), or failed
    /// (`sync_error`).
    pub async fn import_address_and_sync(
        &self,
        address: &str,
        description: Option<&str>,
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
    ) -> Result<AddressImportResult, AddressServiceError> {
        let canonical =
            validate_bitcoin_address(address).map_err(AddressServiceError::Validation)?;
        if address_repository::get_address(&self.database, &canonical)?.is_some() {
            return Err(AddressServiceError::AlreadyExists);
        }

        let entity = self
            .import_address(&canonical, description, exchange_id, wallet_id)
            .await?;

        let sync = TransactionService::connect(self.database.clone())
            .sync_address_transactions(&entity.address)
            .await;
        let sync_error = sync.err().map(|error| error.to_string());

        // Re-read the row so the result carries the rate-limit marker the
        // sync may have set on the address.
        let address = address_repository::get_address(&self.database, &entity.address)?
            .ok_or(AddressServiceError::NotStored)?;

        Ok(AddressImportResult {
            address,
            sync_error,
        })
    }

    /// Update the description and owner classification of a stored address.
    ///
    /// This is the use case behind the dialog's modify mode: only the
    /// editable details change — the mempool statistics and the indexed
    /// transactions are untouched (the sync action refreshes those). No
    /// network access.
    pub fn update_address(
        &self,
        address: &str,
        description: Option<&str>,
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
    ) -> Result<AddressEntity, AddressServiceError> {
        let address = validate_bitcoin_address(address).map_err(AddressServiceError::Validation)?;
        let description =
            normalize_description(description).map_err(AddressServiceError::Validation)?;
        validate_owner(&self.database, exchange_id, wallet_id)
            .map_err(AddressServiceError::Validation)?;

        if !address_repository::update_address_details(
            &self.database,
            &address,
            description.as_deref(),
            exchange_id,
            wallet_id,
        )? {
            return Err(AddressServiceError::NotFound);
        }

        address_repository::get_address(&self.database, &address)?
            .ok_or(AddressServiceError::NotStored)
    }

    /// Validate, fetch, and store an address with its optional classification.
    ///
    /// Returns the stored row so the caller observes the persisted
    /// description and owner reference, including preserved values on
    /// re-import of an already-classified address.
    pub async fn import_address(
        &self,
        address: &str,
        description: Option<&str>,
        exchange_id: Option<i64>,
        wallet_id: Option<i64>,
    ) -> Result<AddressEntity, AddressServiceError> {
        let address = validate_bitcoin_address(address).map_err(AddressServiceError::Validation)?;
        let description =
            normalize_description(description).map_err(AddressServiceError::Validation)?;
        validate_owner(&self.database, exchange_id, wallet_id)
            .map_err(AddressServiceError::Validation)?;

        let mempool_address = self.mempool.bitcoin.addresses.get_address(&address).await?;
        let mut entity = entity_from_mempool(mempool_address);
        entity.description = description;
        entity.exchange_id = exchange_id;
        entity.wallet_id = wallet_id;

        address_repository::insert_address(&self.database, &entity)?;

        address_repository::get_address(&self.database, &address)?
            .ok_or(AddressServiceError::NotStored)
    }
}

/// Validate and canonicalize an address without storing it.
///
/// The import dialog calls this before pasting a clipboard value so invalid
/// text never reaches the dialog input.
pub fn validate_address(value: &str) -> Result<String, AddressServiceError> {
    validate_bitcoin_address(value).map_err(AddressServiceError::Validation)
}

/// Map one repository overview row to the page's item: the spendable
/// balance derives from the stored mempool chain statistics and the address
/// type from the address string; everything else rides along.
fn overview_item(row: &address_repository::AddressOverviewRow) -> AddressOverviewItem {
    let entity = &row.entity;
    AddressOverviewItem {
        address: entity.address.clone(),
        description: entity.description.clone(),
        address_type: address_type_label(&entity.address).map(str::to_string),
        spendable_balance_sats: row.spendable_balance_sats,
        confirmed_tx_count: row.confirmed_tx_count,
        first_tx_time: row.first_tx_time,
        last_tx_time: row.last_tx_time,
        scope: AddressScope::new(
            entity.exchange_id,
            entity.wallet_id,
            row.exchange_name.clone(),
            row.wallet_name.clone(),
        ),
        tx_index_rate_limited: entity.tx_index_rate_limited,
    }
}

/// Map the mempool wire model into the domain entity. This is the only place
/// where the external address schema crosses into the application.
fn entity_from_mempool(value: MempoolAddress) -> AddressEntity {
    AddressEntity {
        address: value.address,
        description: None,
        exchange_id: None,
        wallet_id: None,
        chain_funded_txo_count: value.chain_stats.funded_txo_count,
        chain_funded_txo_sum: value.chain_stats.funded_txo_sum,
        chain_spent_txo_count: value.chain_stats.spent_txo_count,
        chain_spent_txo_sum: value.chain_stats.spent_txo_sum,
        chain_tx_count: value.chain_stats.tx_count,
        mempool_funded_txo_count: value.mempool_stats.funded_txo_count,
        mempool_funded_txo_sum: value.mempool_stats.funded_txo_sum,
        mempool_spent_txo_count: value.mempool_stats.spent_txo_count,
        mempool_spent_txo_sum: value.mempool_stats.spent_txo_sum,
        mempool_tx_count: value.mempool_stats.tx_count,
        // Fresh mappings start unflagged; the sync walk sets the flag and
        // stats refreshes never clear it.
        tx_index_rate_limited: false,
    }
}

/// Validate the optional exchange/wallet owner reference of an address.
///
/// An address belongs to at most one owner (`exchange_id`, `wallet_id`), both
/// ids must reference existing catalog rows, and no id means "unclassified".
/// The database foreign keys and the migration 0006 triggers enforce the same
/// invariants behind this boundary.
fn validate_owner(
    database: &Database,
    exchange_id: Option<i64>,
    wallet_id: Option<i64>,
) -> Result<(), String> {
    if exchange_id.is_some() && wallet_id.is_some() {
        return Err(
            "An address can belong to either an exchange or a wallet, not both.".to_string(),
        );
    }

    if let Some(id) = exchange_id {
        let exists = catalog_repository::exchange_type_exists(database, id)
            .map_err(|err| err.to_string())?;
        if !exists {
            return Err(format!("Unknown exchange type with id {id}."));
        }
    }

    if let Some(id) = wallet_id {
        let exists =
            catalog_repository::wallet_type_exists(database, id).map_err(|err| err.to_string())?;
        if !exists {
            return Err(format!("Unknown wallet type with id {id}."));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clients::mempool::bitcoin::types::StatsInfo;

    #[test]
    fn overview_item_maps_the_repository_row() {
        let mut entity = AddressEntity {
            address: "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq".to_string(),
            description: Some("Savings".to_string()),
            ..AddressEntity::default()
        };
        entity.chain_funded_txo_sum = 150_000;
        entity.chain_spent_txo_sum = 50_000;

        let mut row = address_repository::AddressOverviewRow {
            entity,
            exchange_name: None,
            wallet_name: Some("Ledger".to_string()),
            confirmed_tx_count: 3,
            first_tx_time: Some(1_600_000_000),
            last_tx_time: Some(1_700_000_000),
            spendable_balance_sats: 100_000,
        };
        row.entity.wallet_id = Some(1);

        let item = overview_item(&row);

        assert_eq!(item.address, "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq");
        assert_eq!(item.description.as_deref(), Some("Savings"));
        assert_eq!(item.address_type.as_deref(), Some("p2wpkh"));
        assert_eq!(item.spendable_balance_sats, 100_000);
        assert_eq!(item.confirmed_tx_count, 3);
        assert_eq!(item.first_tx_time, Some(1_600_000_000));
        assert_eq!(item.last_tx_time, Some(1_700_000_000));
        assert_eq!(
            item.scope,
            AddressScope::Wallet {
                id: 1,
                name: Some("Ledger".to_string())
            }
        );
        assert!(!item.scope.is_exchange());
        assert!(!item.tx_index_rate_limited);
    }

    #[test]
    fn scope_prefers_the_exchange_on_a_contradicting_owner_pair() {
        // The either-or invariant is enforced behind this boundary, but a
        // contradicting pair must still resolve deterministically — matching
        // the sync behavior keyed on `exchange_id`.
        let scope = AddressScope::new(Some(1), Some(2), Some("Binance".to_string()), None);

        assert!(scope.is_exchange());
    }

    #[test]
    fn overview_item_uses_the_zeroed_repository_balance_for_exchanges() {
        let mut entity = AddressEntity {
            address: "bc1qexchange".to_string(),
            exchange_id: Some(1),
            ..AddressEntity::default()
        };
        entity.chain_funded_txo_sum = 150_000;
        entity.chain_spent_txo_sum = 50_000;

        let row = address_repository::AddressOverviewRow {
            entity,
            exchange_name: Some("Binance".to_string()),
            wallet_name: None,
            confirmed_tx_count: 4,
            first_tx_time: None,
            last_tx_time: None,
            spendable_balance_sats: 0,
        };

        let item = overview_item(&row);

        assert!(item.scope.is_exchange());
        assert_eq!(item.spendable_balance_sats, 0);
        assert_eq!(item.confirmed_tx_count, 4);
    }

    #[test]
    fn owner_validation_rejects_conflicting_and_unknown_ids() {
        let database = Database::open_in_memory().unwrap();

        // Both owners at once violate the either-or invariant.
        assert!(validate_owner(&database, Some(1), Some(2)).is_err());

        // Unknown ids must reference existing catalog rows.
        assert!(validate_owner(&database, Some(999), None).is_err());
        assert!(validate_owner(&database, None, Some(999)).is_err());

        // Valid single-owner and unclassified combinations are accepted.
        assert!(validate_owner(&database, Some(1), None).is_ok());
        assert!(validate_owner(&database, None, Some(18)).is_ok());
        assert!(validate_owner(&database, None, None).is_ok());
    }

    #[test]
    fn mempool_wire_model_maps_into_the_domain_entity() {
        let wire = MempoolAddress {
            address: "bc1qexample".to_string(),
            chain_stats: StatsInfo {
                funded_txo_count: 1,
                funded_txo_sum: 2,
                spent_txo_count: 3,
                spent_txo_sum: 4,
                tx_count: 5,
            },
            mempool_stats: StatsInfo {
                funded_txo_count: 6,
                funded_txo_sum: 7,
                spent_txo_count: 8,
                spent_txo_sum: 9,
                tx_count: 10,
            },
        };

        let entity = entity_from_mempool(wire);

        assert_eq!(entity.address, "bc1qexample");
        assert_eq!(entity.chain_funded_txo_count, 1);
        assert_eq!(entity.chain_funded_txo_sum, 2);
        assert_eq!(entity.chain_spent_txo_count, 3);
        assert_eq!(entity.chain_spent_txo_sum, 4);
        assert_eq!(entity.chain_tx_count, 5);
        assert_eq!(entity.mempool_funded_txo_count, 6);
        assert_eq!(entity.mempool_funded_txo_sum, 7);
        assert_eq!(entity.mempool_spent_txo_count, 8);
        assert_eq!(entity.mempool_spent_txo_sum, 9);
        assert_eq!(entity.mempool_tx_count, 10);
        assert_eq!(entity.description, None);
        assert_eq!(entity.exchange_id, None);
        assert_eq!(entity.wallet_id, None);
    }
}
