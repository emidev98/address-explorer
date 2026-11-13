//! Connection creation and SQLite configuration.
//!
//! `Database` owns the single connection behind a mutex so repository calls
//! stay serialized and blocking work never reaches the UI path.
#[cfg(not(target_arch = "wasm32"))]
use super::encryption::{VaultKey, SALT_LEN};
#[cfg(any(not(target_arch = "wasm32"), test))]
use super::migrations::run_migrations;
#[cfg(not(target_arch = "wasm32"))]
use crate::security::files::{create_private_file, ensure_private_dir, set_private_file};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
const APP_NAME: &str = "address-explorer";
#[cfg(not(target_arch = "wasm32"))]
const DB_FILE_NAME: &str = "app.db";
#[derive(Debug, Clone)]
pub struct DatabaseError {
    message: String,
}
impl DatabaseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
impl std::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for DatabaseError {}
impl From<rusqlite::Error> for DatabaseError {
    fn from(value: rusqlite::Error) -> Self {
        Self::new(value.to_string())
    }
}
impl From<std::io::Error> for DatabaseError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}
/// Connection metadata and thread-safe ownership wrapper.
///
/// Cloning shares the same mutex-protected connection, so application
/// services can compose (for example the address service delegating a
/// transaction sync) without opening a second connection; the mutex keeps
/// every access serialized.
#[derive(Clone)]
pub struct Database {
    connection: std::sync::Arc<Mutex<Connection>>,
    path: Option<PathBuf>,
}
impl Database {
    /// Resolve the database file location.
    ///
    /// Debug builds use `<workspace>/target/db/<file>`. Release builds use the
    /// platform application-data directory, falling back to the legacy
    /// `~/address-explorer/<file>` when that database already exists.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn default_path() -> Result<PathBuf, DatabaseError> {
        if let Ok(path) = std::env::var("DATABASE_PATH") {
            if !path.trim().is_empty() {
                let path = PathBuf::from(path);
                if let Some(parent) = path.parent() {
                    ensure_private_dir(parent).map_err(DatabaseError::new)?;
                }
                return Ok(path);
            }
        }
        if cfg!(debug_assertions) {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("db");
            ensure_private_dir(&directory).map_err(DatabaseError::new)?;
            crate::security::files::restrict_dir(&directory).map_err(DatabaseError::new)?;
            return Ok(directory.join(DB_FILE_NAME));
        }
        if let Some(legacy) = dirs::home_dir().map(|home| home.join(APP_NAME).join(DB_FILE_NAME)) {
            if legacy.is_file() {
                if let Some(parent) = legacy.parent() {
                    let _ = crate::security::files::restrict_dir(parent);
                }
                return Ok(legacy);
            }
        }
        let directory = dirs::data_dir()
            .or_else(dirs::home_dir)
            .map(|base| base.join(APP_NAME))
            .ok_or_else(|| {
                DatabaseError::new("could not determine the application-data directory")
            })?;
        ensure_private_dir(&directory).map_err(DatabaseError::new)?;
        crate::security::files::restrict_dir(&directory).map_err(DatabaseError::new)?;
        Ok(directory.join(DB_FILE_NAME))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_path_buf();
        prepare_database_file(&path)?;
        let mut connection = Connection::open(&path)?;
        set_private_file(&path).map_err(DatabaseError::new)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "secure_delete", true)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        run_migrations(&mut connection, true)?;
        Ok(Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
            path: Some(path),
        })
    }

    /// Open an existing or new SQLCipher database with a raw Argon2id key.
    ///
    /// The key pragma must run before any statement that reads the schema.
    /// SQLCipher raw-key mode bypasses its own PBKDF2, so unlocking once and
    /// passing this key per request stays cheap.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path_with_key(
        path: impl AsRef<Path>,
        key: &VaultKey,
        salt: &[u8; SALT_LEN],
    ) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_path_buf();
        prepare_database_file(&path)?;
        let mut connection = open_connection_with_key(&path, key, salt)?;
        set_private_file(&path).map_err(DatabaseError::new)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        run_migrations(&mut connection, true)?;
        Ok(Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
            path: Some(path),
        })
    }

    /// Wrap an already configured connection. The browser worker uses this
    /// after installing the relaxed-idb VFS and running migrations. Tests use
    /// it to build file-backed databases without importing the large seeds.
    pub fn from_connection(connection: Connection) -> Self {
        Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
            path: None,
        }
    }
    /// Open an in-memory database with schema migrations applied. Seed
    /// migrations are intentionally skipped: they import on-disk reference
    /// datasets, so running them per test would make every database test
    /// pay a multi-megabyte import.
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let mut connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", true)?;
        run_migrations(&mut connection, false)?;
        Ok(Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
            path: None,
        })
    }
    pub fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::new("database connection lock was poisoned"))
    }

    /// File path of a persistent database, when it has one.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}

/// Open a SQLCipher connection with the public salt and derived raw key.
/// Used for temporary conversion targets as well as normal opens.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_connection_with_key(
    path: &Path,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<Connection, DatabaseError> {
    prepare_database_file(path)?;
    let connection = Connection::open(path)?;
    set_private_file(path).map_err(DatabaseError::new)?;
    apply_sqlcipher_key(&connection, key, salt)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "secure_delete", true)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(connection)
}

#[cfg(not(target_arch = "wasm32"))]
fn prepare_database_file(path: &Path) -> Result<(), DatabaseError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_private_dir(parent).map_err(DatabaseError::new)?;
        }
    }
    match create_private_file(path) {
        Ok(_) => Ok(()),
        Err(_) if path.exists() => Ok(()),
        Err(error) => Err(DatabaseError::new(error)),
    }
}

/// Configure an already open SQLCipher connection. The compatibility pragma
/// must precede the key pragma; the schema query verifies the key.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn apply_sqlcipher_key(
    connection: &Connection,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), DatabaseError> {
    connection.pragma_update(None, "cipher_compatibility", 4)?;
    let spec = zeroize::Zeroizing::new(key.key_spec(salt));
    connection.pragma_update(None, "key", spec.as_str())?;
    connection.query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(())
}
#[cfg(test)]
pub(crate) static DEFAULT_PATH_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dev_default_path_uses_target_db() {
        let _guard = DEFAULT_PATH_TEST_LOCK.lock().unwrap(); // allow-unwrap: test-only lock.
        let path = Database::default_path().unwrap(); // allow-unwrap: test setup.
        assert!(path.ends_with(DB_FILE_NAME));
        assert!(path.to_string_lossy().contains("target/db"));
    }
}
