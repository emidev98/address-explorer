//! Connection creation and SQLite configuration.
//!
//! `Database` owns the single connection behind a mutex so repository calls
//! stay serialized and blocking work never reaches the UI path.
#[cfg(any(not(target_arch = "wasm32"), test))]
use super::migrations::run_migrations;
use rusqlite::Connection;
#[cfg(not(target_arch = "wasm32"))]
use std::fs;
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
                    fs::create_dir_all(parent)?;
                }
                return Ok(path);
            }
        }
        if cfg!(debug_assertions) {
            let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join("db");
            fs::create_dir_all(&directory)?;
            return Ok(directory.join(DB_FILE_NAME));
        }
        if let Some(legacy) = dirs::home_dir().map(|home| home.join(APP_NAME).join(DB_FILE_NAME)) {
            if legacy.is_file() {
                return Ok(legacy);
            }
        }
        let directory = dirs::data_dir()
            .or_else(dirs::home_dir)
            .map(|base| base.join(APP_NAME))
            .ok_or_else(|| {
                DatabaseError::new("could not determine the application-data directory")
            })?;
        fs::create_dir_all(&directory)?;
        Ok(directory.join(DB_FILE_NAME))
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let path = path.as_ref().to_path_buf();
        let mut connection = Connection::open(&path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        run_migrations(&mut connection, true)?;
        Ok(Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
            path: Some(path),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_default() -> Result<Self, DatabaseError> {
        Self::open_path(Self::default_path()?)
    }

    /// Wrap an already configured connection. The browser worker uses this
    /// after installing the relaxed-idb VFS and running migrations. Tests use
    /// it to build file-backed databases without importing the large seeds.
    #[cfg(any(target_arch = "wasm32", test))]
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dev_default_path_uses_target_db() {
        let path = Database::default_path().unwrap();
        assert!(path.ends_with(DB_FILE_NAME));
        assert!(path.to_string_lossy().contains("target/db"));
    }
}
