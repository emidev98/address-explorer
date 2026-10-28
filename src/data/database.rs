//! Connection creation and SQLite configuration.
//!
//! `Database` owns the single connection behind a mutex so repository calls
//! stay serialized and blocking work never reaches the UI path (Section 6.2
//! of CONTRIBUTING.md).

use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use super::migrations::run_migrations;

const APP_NAME: &str = "address-exporer";
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
/// every access serialized (Section 6.2 of CONTRIBUTING.md).
#[derive(Clone)]
pub struct Database {
    connection: std::sync::Arc<Mutex<Connection>>,
}

impl Database {
    /// Resolve the database file location.
    ///
    /// In debug builds this resolves to `<workspace>/target/db/<file>`.
    /// In release builds it resolves to the user's home directory under a folder
    /// named after the application.
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
            Ok(directory.join(DB_FILE_NAME))
        } else {
            let home = dirs::home_dir().ok_or_else(|| {
                DatabaseError::new("could not determine the user's home directory")
            })?;
            let directory = home.join(APP_NAME);
            fs::create_dir_all(&directory)?;
            Ok(directory.join(DB_FILE_NAME))
        }
    }

    pub fn open_path(path: impl AsRef<Path>) -> Result<Self, DatabaseError> {
        let mut connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        run_migrations(&mut connection, true)?;
        Ok(Self {
            connection: std::sync::Arc::new(Mutex::new(connection)),
        })
    }

    pub fn open_default() -> Result<Self, DatabaseError> {
        Self::open_path(Self::default_path()?)
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
        })
    }

    pub fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::new("database connection lock was poisoned"))
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
