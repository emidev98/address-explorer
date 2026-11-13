//! Browser database lifecycle: relaxed-idb VFS, migrations, seeds, export.

use crate::data::encryption::{
    is_plaintext_sqlite, salt_from_header, EncryptionError, VaultKey, SALT_LEN,
};
use crate::data::{backup, migrations, seeds, Database};
use address_explorer_core::WEB_DATABASE_FILE;
use rusqlite::{Connection, OpenFlags, MAIN_DB};
use sqlite_wasm_vfs::relaxed_idb::{
    install as install_idb_vfs, RelaxedIdbCfg, RelaxedIdbCfgBuilder, RelaxedIdbUtil,
};
use std::rc::Rc;

/// SQLite3MC wraps the real VFS to add page-codec support. The base VFS is
/// registered by the worker as `address-explorer`; SQLite3MC auto-creates this
/// wrapper when the prefixed name is opened.
pub(crate) const ENCRYPTED_VFS_NAME: &str = "multipleciphers-address-explorer";

/// Open a browser database. Encrypted databases must use the SQLite3MC VFS
/// wrapper; opening them through the raw relaxed-idb VFS rejects `PRAGMA key`.
pub(crate) fn open_connection(filename: &str, encrypted: bool) -> Result<Connection, String> {
    if encrypted {
        Connection::open_with_flags_and_vfs(filename, OpenFlags::default(), ENCRYPTED_VFS_NAME)
            .map_err(|error| format!("could not open the encrypted browser database: {error}"))
    } else {
        Connection::open(filename)
            .map_err(|error| format!("could not open the browser database: {error}"))
    }
}

/// What the VFS contains before the worker opens a SQLite connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserDatabaseState {
    Missing,
    Plaintext,
    Encrypted([u8; SALT_LEN]),
}

/// The worker's single database handle plus the VFS administration API.
pub struct BrowserDatabase {
    pub database: Database,
    pub util: Rc<RelaxedIdbUtil>,
    pub filename: String,
    pub encrypted: bool,
}

impl BrowserDatabase {
    /// Install the VFS without opening a database. Encrypted vaults inspect
    /// the file first, so a locked database is never opened without its key.
    pub async fn install(reset: bool) -> Result<Rc<RelaxedIdbUtil>, String> {
        let cfg: RelaxedIdbCfg = RelaxedIdbCfgBuilder::new()
            .vfs_name("address-explorer")
            .clear_on_init(reset)
            .build();
        install_idb_vfs::<sqlite_wasm_rs::WasmOsCallback>(&cfg, true)
            .await
            .map(Rc::new)
            .map_err(|error| format!("could not install the browser SQLite VFS: {error}"))
    }

    /// Inspect the stored VFS file without opening it.
    pub fn inspect(util: &RelaxedIdbUtil) -> Result<BrowserDatabaseState, String> {
        if !util.exists(WEB_DATABASE_FILE) {
            return Ok(BrowserDatabaseState::Missing);
        }
        let bytes = util
            .export_db(WEB_DATABASE_FILE)
            .map_err(|error| format!("could not inspect the browser database: {error}"))?;
        if is_plaintext_sqlite(&bytes) {
            return Ok(BrowserDatabaseState::Plaintext);
        }
        match salt_from_header(&bytes) {
            Ok(salt) => Ok(BrowserDatabaseState::Encrypted(salt)),
            Err(EncryptionError::InvalidHeader) => {
                // A damaged/truncated encrypted file must not be mistaken for
                // a new plaintext database. A zero salt makes every unlock
                // fail while leaving the destructive reset path available.
                Ok(BrowserDatabaseState::Encrypted([0u8; SALT_LEN]))
            }
            Err(_) => Ok(BrowserDatabaseState::Encrypted([0u8; SALT_LEN])),
        }
    }

    /// Open the browser database with an optional raw SQLCipher key.
    pub async fn open_with_util(
        seed_url: &str,
        util: Rc<RelaxedIdbUtil>,
        key: Option<(&VaultKey, &[u8; SALT_LEN])>,
        validate_imported: bool,
    ) -> Result<Self, String> {
        Self::open_named(
            seed_url,
            util,
            WEB_DATABASE_FILE,
            key,
            true,
            validate_imported,
        )
        .await
    }

    /// Open a named database file with an optional raw SQLCipher key.
    pub async fn open_named(
        seed_url: &str,
        util: Rc<RelaxedIdbUtil>,
        filename: &str,
        key: Option<(&VaultKey, &[u8; SALT_LEN])>,
        run_migrations: bool,
        validate_imported: bool,
    ) -> Result<Self, String> {
        let encrypted = key.is_some();
        let mut connection = open_connection(filename, encrypted)?;
        // The key must be applied before any pragma that can touch encrypted
        // pages; otherwise SQLite reports "file is not a database".
        if let Some((key, salt)) = key {
            apply_sqlcipher_key(&connection, key, salt)?;
        }
        connection
            .pragma_update(None, "synchronous", "OFF")
            .map_err(|error| format!("could not set synchronous=OFF: {error}"))?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| format!("could not enable foreign keys: {error}"))?;

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
            backup::validate_source_schema(&connection)
                .map_err(|error| format!("the imported SQLite schema is not trusted: {error}"))?;
        }
        if run_migrations {
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
        }
        let wait = util
            .wait_committed(filename)
            .map_err(|error| format!("could not queue the browser commit barrier: {error}"))?;
        wait.await
            .map_err(|error| format!("could not persist the browser database: {error}"))?;

        Ok(Self {
            database: Database::from_connection(connection),
            util,
            filename: filename.to_string(),
            encrypted,
        })
    }

    /// Wait for previously queued IndexedDB writes to finish.
    pub async fn wait_committed(&self) -> Result<(), String> {
        let wait = self
            .util
            .wait_committed(&self.filename)
            .map_err(|error| format!("could not queue the browser commit barrier: {error}"))?;
        wait.await
            .map_err(|error| format!("could not persist the browser database: {error}"))
    }

    /// Export a normal, unencrypted SQLite snapshot.
    ///
    /// For an encrypted database the copy is made into an in-memory plaintext
    /// SQLite connection first, so the VFS never stores a plaintext export.
    pub fn export_plaintext(&self) -> Result<Vec<u8>, String> {
        if !self.encrypted {
            return self
                .util
                .export_db(&self.filename)
                .map_err(|error| format!("could not export the browser database: {error}"));
        }
        let source = self
            .database
            .connection()
            .map_err(|error| format!("could not read the browser database: {error}"))?;
        let data = source
            .serialize(MAIN_DB)
            .map_err(|error| format!("could not export the browser database: {error}"))?;
        Ok(data.to_vec())
    }

    /// Export the encrypted VFS file for an internal safety copy.
    pub fn export_encrypted(&self) -> Result<Vec<u8>, String> {
        self.util
            .export_db(&self.filename)
            .map_err(|error| format!("could not export the browser database: {error}"))
    }
}

pub(crate) fn apply_sqlcipher_key(
    connection: &Connection,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), String> {
    connection
        .pragma_update(None, "cipher", "sqlcipher")
        .map_err(|error| format!("could not select SQLCipher: {error}"))?;
    connection
        .pragma_update(None, "legacy", 4)
        .map_err(|error| format!("could not select SQLCipher 4: {error}"))?;
    let spec = zeroize::Zeroizing::new(key.key_spec(salt));
    connection
        .pragma_update(None, "key", spec.as_str())
        .map_err(|error| format!("could not apply the database key: {error}"))?;
    connection
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| "incorrect password or damaged database".to_string())?;
    Ok(())
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
