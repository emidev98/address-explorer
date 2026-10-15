#![allow(dead_code, unused_imports)]
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::clients::mempool::bitcoin::types::{Address as MempoolAddress, StatsInfo};

#[cfg(feature = "server")]
use crate::clients::database::Database;
#[cfg(feature = "server")]
use rusqlite::{params, OptionalExtension};

/// Persisted, SQLite-normalized representation of mempool `Address`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AddressEntity {
    pub address: String,

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
}

impl From<MempoolAddress> for AddressEntity {
    fn from(value: MempoolAddress) -> Self {
        Self::from_parts(value.address, value.chain_stats, value.mempool_stats)
    }
}

impl AddressEntity {
    pub fn from_parts(address: String, chain_stats: StatsInfo, mempool_stats: StatsInfo) -> Self {
        Self {
            address,
            chain_funded_txo_count: chain_stats.funded_txo_count,
            chain_funded_txo_sum: chain_stats.funded_txo_sum,
            chain_spent_txo_count: chain_stats.spent_txo_count,
            chain_spent_txo_sum: chain_stats.spent_txo_sum,
            chain_tx_count: chain_stats.tx_count,
            mempool_funded_txo_count: mempool_stats.funded_txo_count,
            mempool_funded_txo_sum: mempool_stats.funded_txo_sum,
            mempool_spent_txo_count: mempool_stats.spent_txo_count,
            mempool_spent_txo_sum: mempool_stats.spent_txo_sum,
            mempool_tx_count: mempool_stats.tx_count,
        }
    }

    pub fn chain_stats(&self) -> StatsInfo {
        StatsInfo {
            funded_txo_count: self.chain_funded_txo_count,
            funded_txo_sum: self.chain_funded_txo_sum,
            spent_txo_count: self.chain_spent_txo_count,
            spent_txo_sum: self.chain_spent_txo_sum,
            tx_count: self.chain_tx_count,
        }
    }

    pub fn mempool_stats(&self) -> StatsInfo {
        StatsInfo {
            funded_txo_count: self.mempool_funded_txo_count,
            funded_txo_sum: self.mempool_funded_txo_sum,
            spent_txo_count: self.mempool_spent_txo_count,
            spent_txo_sum: self.mempool_spent_txo_sum,
            tx_count: self.mempool_tx_count,
        }
    }

    pub fn to_mempool_address(&self) -> MempoolAddress {
        MempoolAddress {
            address: self.address.clone(),
            chain_stats: self.chain_stats(),
            mempool_stats: self.mempool_stats(),
        }
    }
}

#[cfg(feature = "server")]
pub mod repository {
    use super::AddressEntity;
    use crate::clients::database::Database;
    use rusqlite::{params, OptionalExtension};

    pub fn insert_address(
        database: &Database,
        value: &AddressEntity,
    ) -> Result<(), crate::clients::database::DatabaseError> {
        let connection = database.connection()?;
        connection.execute(
            "INSERT INTO addresses (
                address,
                chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                chain_spent_txo_sum, chain_tx_count,
                mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                mempool_spent_txo_sum, mempool_tx_count
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(address) DO UPDATE SET
                chain_funded_txo_count = excluded.chain_funded_txo_count,
                chain_funded_txo_sum = excluded.chain_funded_txo_sum,
                chain_spent_txo_count = excluded.chain_spent_txo_count,
                chain_spent_txo_sum = excluded.chain_spent_txo_sum,
                chain_tx_count = excluded.chain_tx_count,
                mempool_funded_txo_count = excluded.mempool_funded_txo_count,
                mempool_funded_txo_sum = excluded.mempool_funded_txo_sum,
                mempool_spent_txo_count = excluded.mempool_spent_txo_count,
                mempool_spent_txo_sum = excluded.mempool_spent_txo_sum,
                mempool_tx_count = excluded.mempool_tx_count",
            params![
                value.address,
                value.chain_funded_txo_count,
                value.chain_funded_txo_sum,
                value.chain_spent_txo_count,
                value.chain_spent_txo_sum,
                value.chain_tx_count,
                value.mempool_funded_txo_count,
                value.mempool_funded_txo_sum,
                value.mempool_spent_txo_count,
                value.mempool_spent_txo_sum,
                value.mempool_tx_count,
            ],
        )?;
        Ok(())
    }

    pub fn get_address(
        database: &Database,
        address: &str,
    ) -> Result<Option<AddressEntity>, crate::clients::database::DatabaseError> {
        let connection = database.connection()?;
        connection
            .query_row(
                "SELECT address,
                    chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                    chain_spent_txo_sum, chain_tx_count,
                    mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                    mempool_spent_txo_sum, mempool_tx_count
                 FROM addresses WHERE address = ?1",
                [address],
                |row| {
                    Ok(AddressEntity {
                        address: row.get(0)?,
                        chain_funded_txo_count: row.get(1)?,
                        chain_funded_txo_sum: row.get(2)?,
                        chain_spent_txo_count: row.get(3)?,
                        chain_spent_txo_sum: row.get(4)?,
                        chain_tx_count: row.get(5)?,
                        mempool_funded_txo_count: row.get(6)?,
                        mempool_funded_txo_sum: row.get(7)?,
                        mempool_spent_txo_count: row.get(8)?,
                        mempool_spent_txo_sum: row.get(9)?,
                        mempool_tx_count: row.get(10)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn list_addresses(
        database: &Database,
    ) -> Result<Vec<AddressEntity>, crate::clients::database::DatabaseError> {
        let connection = database.connection()?;
        let mut statement = connection.prepare(
            "SELECT address,
                chain_funded_txo_count, chain_funded_txo_sum, chain_spent_txo_count,
                chain_spent_txo_sum, chain_tx_count,
                mempool_funded_txo_count, mempool_funded_txo_sum, mempool_spent_txo_count,
                mempool_spent_txo_sum, mempool_tx_count
             FROM addresses ORDER BY address",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AddressEntity {
                address: row.get(0)?,
                chain_funded_txo_count: row.get(1)?,
                chain_funded_txo_sum: row.get(2)?,
                chain_spent_txo_count: row.get(3)?,
                chain_spent_txo_sum: row.get(4)?,
                chain_tx_count: row.get(5)?,
                mempool_funded_txo_count: row.get(6)?,
                mempool_funded_txo_sum: row.get(7)?,
                mempool_spent_txo_count: row.get(8)?,
                mempool_spent_txo_sum: row.get(9)?,
                mempool_tx_count: row.get(10)?,
            })
        })?;
        let mut values = Vec::new();
        for value in rows {
            values
                .push(value.map_err(|err| {
                    crate::clients::database::DatabaseError::new(err.to_string())
                })?);
        }
        Ok(values)
    }

    pub fn delete_address(
        database: &Database,
        address: &str,
    ) -> Result<(), crate::clients::database::DatabaseError> {
        let connection = database.connection()?;
        connection.execute("DELETE FROM addresses WHERE address = ?1", [address])?;
        Ok(())
    }
}

use dioxus::prelude::ServerFnError;

#[server]
#[get("/api/addresses")]
pub async fn list_addresses_api() -> Result<Vec<AddressEntity>, ServerFnError> {
    let database = Database::open_default().map_err(|err| ServerFnError::new(err.to_string()))?;
    repository::list_addresses(&database).map_err(|err| ServerFnError::new(err.to_string()))
}

#[server]
#[get("/api/addresses/:address")]
pub async fn get_address_api(address: String) -> Result<AddressEntity, ServerFnError> {
    let database = Database::open_default().map_err(|err| ServerFnError::new(err.to_string()))?;
    repository::get_address(&database, &address)
        .map_err(|err| ServerFnError::new(err.to_string()))?
        .ok_or_else(|| ServerFnError::new("address not found"))
}

#[server]
#[post("/api/addresses")]
pub async fn save_address_api(address: String) -> Result<AddressEntity, ServerFnError> {
    let client = crate::clients::mempool::Mempool::new(
        crate::clients::mempool::config::MempoolConfig::default(),
    );
    let mempool_address = client
        .bitcoin
        .addresses
        .get_address(&address)
        .await
        .map_err(|err| ServerFnError::new(err.to_string()))?;
    let entity = AddressEntity::from(mempool_address);

    let database = Database::open_default().map_err(|err| ServerFnError::new(err.to_string()))?;
    repository::insert_address(&database, &entity)
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    Ok(entity)
}

#[server]
#[delete("/api/addresses/:address")]
pub async fn delete_address_api(address: String) -> Result<(), ServerFnError> {
    let database = Database::open_default().map_err(|err| ServerFnError::new(err.to_string()))?;
    repository::delete_address(&database, &address)
        .map_err(|err| ServerFnError::new(err.to_string()))
}
