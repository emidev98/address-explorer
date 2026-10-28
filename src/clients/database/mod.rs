#![allow(dead_code, unused_imports)]

use rusqlite::{Connection, Transaction};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

const APP_NAME: &str = "AddressExplorer";
const DB_FILE_NAME: &str = "app.db";

const MIGRATIONS: &[(&str, &str)] = &[(
    "0001_hello_world",
    include_str!("migrations/0001_hello_world.sql"),
),(
    "0002_addresses",
    include_str!("migrations/0002_addresses.sql"),
)];

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
pub struct Database {
    connection: Mutex<Connection>,
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
        run_migrations(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_default() -> Result<Self, DatabaseError> {
        Self::open_path(Self::default_path()?)
    }

    pub fn open_in_memory() -> Result<Self, DatabaseError> {
        let mut connection = Connection::open_in_memory()?;
        connection.pragma_update(None, "foreign_keys", true)?;
        run_migrations(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::new("database connection lock was poisoned"))
    }
}

fn run_migrations(connection: &mut Connection) -> Result<(), DatabaseError> {
    let current_version: i32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DatabaseError::from)?;

    for (index, (_name, sql)) in MIGRATIONS.iter().enumerate() {
        let version = (index + 1) as i32;
        if current_version >= version {
            continue;
        }

        let transaction: Transaction<'_> = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(DatabaseError::from)?;
        transaction
            .execute_batch(sql)
            .map_err(DatabaseError::from)?;
        transaction
            .pragma_update(None, "user_version", version)
            .map_err(DatabaseError::from)?;
        transaction.commit().map_err(DatabaseError::from)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_world_migration_runs() {
        let database = Database::open_in_memory().unwrap();
        let connection = database.connection().unwrap();
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM hello_world", [], |row| row.get(0))
            .unwrap();
        assert!(count >= 1);
    }

    #[test]
    fn dev_default_path_uses_target_db() {
        let path = Database::default_path().unwrap();
        assert!(path.ends_with(DB_FILE_NAME));
        assert!(path.to_string_lossy().contains("target/db"));
    }
}
