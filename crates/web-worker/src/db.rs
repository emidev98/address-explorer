//! Browser database lifecycle: relaxed-idb VFS, migrations, seeds, export.

use crate::data::{backup, migrations, seeds, Database};
use address_explorer_core::WEB_DATABASE_FILE;
use rusqlite::Connection;
use sqlite_wasm_vfs::relaxed_idb::{
    install as install_idb_vfs, RelaxedIdbCfg, RelaxedIdbCfgBuilder, RelaxedIdbUtil,
};

/// The worker's single database handle plus the VFS administration API.
pub struct BrowserDatabase {
    pub database: Database,
    pub util: RelaxedIdbUtil,
}

impl BrowserDatabase {
    /// Install the VFS and open the persistent database.
    ///
    /// The seed asset is fetched only when the database is new or the seed
    /// migration is still pending. The commit barrier is awaited before the
    /// database is reported ready.
    pub async fn open(seed_url: &str, reset: bool) -> Result<Self, String> {
        Self::open_inner(seed_url, reset, false).await
    }

    /// Open a database that was just imported. The extra integrity and
    /// foreign-key checks make a rejected backup recoverable before the worker
    /// restores the previous database.
    pub async fn open_imported(seed_url: &str) -> Result<Self, String> {
        Self::open_inner(seed_url, false, true).await
    }

    async fn open_inner(
        seed_url: &str,
        reset: bool,
        validate_imported: bool,
    ) -> Result<Self, String> {
        let cfg: RelaxedIdbCfg = RelaxedIdbCfgBuilder::new()
            .vfs_name("address-explorer")
            .clear_on_init(reset)
            .build();
        let util = install_idb_vfs::<sqlite_wasm_rs::WasmOsCallback>(&cfg, true)
            .await
            .map_err(|error| format!("could not install the browser SQLite VFS: {error}"))?;

        let mut connection = Connection::open(WEB_DATABASE_FILE)
            .map_err(|error| format!("could not open the browser database: {error}"))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| format!("could not enable foreign keys: {error}"))?;
        connection
            .pragma_update(None, "synchronous", "OFF")
            .map_err(|error| format!("could not set synchronous=OFF: {error}"))?;

        let version: i32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(|error| format!("could not read the browser schema version: {error}"))?;
        if validate_imported {
            backup::integrity_check(&connection)
                .map_err(|error| format!("the imported SQLite file is invalid: {error}"))?;
            if version <= 0 {
                return Err("the selected file is not an Address Explorer backup".to_string());
            }
            let has_setting: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'setting'",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| format!("could not inspect the imported SQLite file: {error}"))?;
            if has_setting == 0 {
                return Err("the selected file is not an Address Explorer backup".to_string());
            }
        }
        if version < 15 {
            let text = fetch_seed(seed_url).await?;
            seeds::set_packed_pricing_text(text);
        }
        migrations::run_migrations(&mut connection, true)
            .map_err(|error| format!("browser database migration failed: {error}"))?;
        backup::validate_current_schema(&connection)
            .map_err(|error| format!("the browser database schema is invalid: {error}"))?;
        if validate_imported {
            backup::foreign_key_check(&connection)
                .map_err(|error| format!("the imported SQLite file is invalid: {error}"))?;
        }
        let wait = util
            .wait_committed(WEB_DATABASE_FILE)
            .map_err(|error| format!("could not queue the browser commit barrier: {error}"))?;
        wait.await
            .map_err(|error| format!("could not persist the browser database: {error}"))?;

        Ok(Self {
            database: Database::from_connection(connection),
            util,
        })
    }

    /// Wait for previously queued IndexedDB writes to finish.
    pub async fn wait_committed(&self) -> Result<(), String> {
        let wait = self
            .util
            .wait_committed(WEB_DATABASE_FILE)
            .map_err(|error| format!("could not queue the browser commit barrier: {error}"))?;
        wait.await
            .map_err(|error| format!("could not persist the browser database: {error}"))
    }

    /// Export the live database as a normal SQLite file.
    pub fn export(&self) -> Result<Vec<u8>, String> {
        self.util
            .export_db(WEB_DATABASE_FILE)
            .map_err(|error| format!("could not export the browser database: {error}"))
    }
}

async fn fetch_seed(seed_url: &str) -> Result<String, String> {
    let response = reqwest::get(seed_url)
        .await
        .map_err(|error| format!("could not fetch the bundled pricing seed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "could not fetch the bundled pricing seed: HTTP {}",
            response.status()
        ));
    }
    response
        .text()
        .await
        .map_err(|error| format!("could not read the bundled pricing seed: {error}"))
}
