//! SQLite backup validation, export, restore, and reset.
//!
//! The browser worker uses the connection validators in this module after it
//! imports a database into its virtual filesystem. The native and server
//! builds additionally use the file-backed export/restore workflow below.

use super::migrations::CURRENT_SCHEMA_VERSION;
#[cfg(not(target_arch = "wasm32"))]
use super::Database;
use super::DatabaseError;
use rusqlite::Connection;
#[cfg(not(target_arch = "wasm32"))]
use rusqlite::MAIN_DB;
use std::collections::HashSet;

/// Tables every Address Explorer database must contain.
pub(crate) const APPLICATION_TABLES: [&str; 13] = [
    "addresses",
    "exchange_accounts",
    "exchange_categories",
    "exchange_transactions",
    "exchanges_types",
    "fiat_exchange_rates",
    "historical_prices",
    "setting",
    "transaction_inputs",
    "transaction_outputs",
    "transactions",
    "wallet_categories",
    "wallets_types",
];

/// True when `bytes` starts with the SQLite file header.
#[cfg(not(target_arch = "wasm32"))]
pub fn has_sqlite_header(bytes: &[u8]) -> bool {
    bytes.starts_with(b"SQLite format 3\0")
}

/// Run SQLite's integrity check and turn a failed check into a clear error.
pub fn integrity_check(connection: &Connection) -> Result<(), DatabaseError> {
    let result: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if result != "ok" {
        return Err(DatabaseError::new(format!(
            "the SQLite integrity check failed: {result}"
        )));
    }
    Ok(())
}

/// Verify that the database has every table this application expects and that
/// its schema version is exactly the current one.
pub fn validate_current_schema(connection: &Connection) -> Result<(), DatabaseError> {
    let version: i32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != CURRENT_SCHEMA_VERSION {
        return Err(DatabaseError::new(format!(
            "database schema version {version} does not match the supported version {CURRENT_SCHEMA_VERSION}"
        )));
    }

    let mut statement =
        connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table'")?;
    let names: HashSet<String> = statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<_, _>>()?;
    let missing = APPLICATION_TABLES
        .iter()
        .copied()
        .filter(|table| !names.contains(*table))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(DatabaseError::new(format!(
            "the database is missing required tables: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

/// Verify that no foreign-key constraint is violated.
pub fn foreign_key_check(connection: &Connection) -> Result<(), DatabaseError> {
    let violations: i64 =
        connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    if violations > 0 {
        return Err(DatabaseError::new(
            "the database has foreign-key violations",
        ));
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
mod files {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    const BUSY_RETRIES: usize = 6;
    const BUSY_SLEEP: Duration = Duration::from_millis(100);

    /// Export a consistent copy of the live database.
    pub fn export_to_bytes(database: &Database) -> Result<Vec<u8>, DatabaseError> {
        let directory = temp_directory(database);
        fs::create_dir_all(&directory)?;
        let temporary = TempPath::new(directory, "export");
        {
            let connection = database.connection()?;
            backup_database(&connection, temporary.path())?;
        }
        let bytes = fs::read(temporary.path())?;
        if !has_sqlite_header(&bytes) {
            return Err(DatabaseError::new(
                "the exported database is not a valid SQLite file",
            ));
        }
        Ok(bytes)
    }

    /// Validate, upgrade, and restore a backup into the live database.
    ///
    /// The live database is backed up first. Any failure restores that safety
    /// copy so a rejected import never destroys the current data.
    pub fn import_from_bytes(database: &Database, bytes: &[u8]) -> Result<(), DatabaseError> {
        if bytes.is_empty() {
            return Err(DatabaseError::new("the selected SQLite file is empty"));
        }
        if !has_sqlite_header(bytes) {
            return Err(DatabaseError::new(
                "the selected file is not a SQLite database",
            ));
        }

        let directory = temp_directory(database);
        fs::create_dir_all(&directory)?;
        let candidate = TempPath::new(directory.clone(), "import");
        fs::write(candidate.path(), bytes)?;
        validate_candidate(candidate.path())?;

        let safety = TempPath::new(directory, "before-import");
        let mut live = database.connection()?;
        backup_database(&live, safety.path())?;
        if let Err(error) = restore_database(&mut live, candidate.path()) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        if let Err(error) = validate_current_schema(&live) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        if let Err(error) = foreign_key_check(&live) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        Ok(())
    }

    /// Replace the live database with a freshly migrated and seeded database.
    pub fn reset_to_defaults(database: &Database) -> Result<(), DatabaseError> {
        let directory = temp_directory(database);
        fs::create_dir_all(&directory)?;
        let fresh = TempPath::new(directory.clone(), "reset");
        {
            let fresh_database = Database::open_path(fresh.path())?;
            let connection = fresh_database.connection()?;
            validate_current_schema(&connection)?;
        }

        let safety = TempPath::new(directory, "before-reset");
        let mut live = database.connection()?;
        backup_database(&live, safety.path())?;
        if let Err(error) = restore_database(&mut live, fresh.path()) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        if let Err(error) = validate_current_schema(&live) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        if let Err(error) = foreign_key_check(&live) {
            let _ = restore_database(&mut live, safety.path());
            return Err(error);
        }
        Ok(())
    }

    fn validate_candidate(path: &Path) -> Result<(), DatabaseError> {
        precheck_candidate(path)?;
        let candidate = Database::open_path(path)?;
        let connection = candidate.connection()?;
        integrity_check(&connection)?;
        validate_current_schema(&connection)?;
        foreign_key_check(&connection)?;
        Ok(())
    }

    /// Reject files that are not an older or current Address Explorer backup
    /// before the migration runner gets a chance to create tables in them.
    fn precheck_candidate(path: &Path) -> Result<(), DatabaseError> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        let version: i32 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version <= 0 {
            return Err(DatabaseError::new(
                "the selected file is not an Address Explorer backup",
            ));
        }
        if version > CURRENT_SCHEMA_VERSION {
            return Err(DatabaseError::new(format!(
                "this backup was created by a newer Address Explorer version (schema {version}; this app supports {CURRENT_SCHEMA_VERSION})"
            )));
        }
        let has_setting: i64 = connection.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'setting'",
            [],
            |row| row.get(0),
        )?;
        if has_setting == 0 {
            return Err(DatabaseError::new(
                "the selected file is not an Address Explorer backup",
            ));
        }
        integrity_check(&connection)?;
        Ok(())
    }

    fn backup_database(connection: &Connection, path: &Path) -> Result<(), DatabaseError> {
        for attempt in 0..BUSY_RETRIES {
            match connection.backup(MAIN_DB, path, None) {
                Ok(()) => return Ok(()),
                Err(error) if is_busy(&error) && attempt + 1 < BUSY_RETRIES => {
                    std::thread::sleep(BUSY_SLEEP);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(DatabaseError::new(
            "the database stayed busy while preparing the backup",
        ))
    }

    fn restore_database(connection: &mut Connection, path: &Path) -> Result<(), DatabaseError> {
        for attempt in 0..BUSY_RETRIES {
            match connection.restore(MAIN_DB, path, None::<fn(rusqlite::backup::Progress)>) {
                Ok(()) => return Ok(()),
                Err(error) if is_busy(&error) && attempt + 1 < BUSY_RETRIES => {
                    std::thread::sleep(BUSY_SLEEP);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Err(DatabaseError::new(
            "the database stayed busy while restoring the backup",
        ))
    }

    fn is_busy(error: &rusqlite::Error) -> bool {
        matches!(
            error,
            rusqlite::Error::SqliteFailure(code, _)
                if matches!(
                    code.code,
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
                )
        )
    }

    fn temp_directory(database: &Database) -> PathBuf {
        if let Some(parent) = database.path().and_then(Path::parent) {
            if !parent.as_os_str().is_empty() {
                return parent.to_path_buf();
            }
        }
        std::env::temp_dir().join("address-explorer")
    }

    struct TempPath {
        path: PathBuf,
    }

    impl TempPath {
        fn new(directory: PathBuf, tag: &str) -> Self {
            let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0);
            let path = directory.join(format!(
                ".address-explorer-{tag}-{}-{timestamp}-{counter}.sqlite",
                std::process::id()
            ));
            let _ = fs::remove_file(&path);
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempPath {
        fn drop(&mut self) {
            remove_related_files(&self.path);
        }
    }

    fn remove_related_files(path: &Path) {
        let _ = fs::remove_file(path);
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut related = OsString::from(path.as_os_str());
            related.push(suffix);
            let _ = fs::remove_file(PathBuf::from(related));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use files::{export_to_bytes, import_from_bytes, reset_to_defaults};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::data::migrations;
    use rusqlite::Connection;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "address-explorer-backup-test-{}-{counter}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_database(path: &Path) -> Database {
        let mut connection = Connection::open(path).unwrap();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        migrations::run_migrations(&mut connection, false).unwrap();
        Database::from_connection(connection)
    }

    fn setting(database: &Database, key: &str) -> Option<String> {
        database
            .connection()
            .unwrap()
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .ok()
    }

    #[test]
    fn fresh_test_database_has_every_required_table() {
        let directory = TestDir::new();
        let database = test_database(&directory.path().join("fresh.sqlite"));
        let connection = database.connection().unwrap();
        validate_current_schema(&connection).unwrap();
        foreign_key_check(&connection).unwrap();
    }

    #[test]
    fn export_import_round_trips_rows() {
        let directory = TestDir::new();
        let source = test_database(&directory.path().join("source.sqlite"));
        source
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value) VALUES ('theme', 'forest')",
                [],
            )
            .unwrap();

        let bytes = export_to_bytes(&source).unwrap();
        assert!(has_sqlite_header(&bytes));

        let target = test_database(&directory.path().join("target.sqlite"));
        import_from_bytes(&target, &bytes).unwrap();
        assert_eq!(setting(&target, "theme").as_deref(), Some("forest"));
    }

    #[test]
    fn import_rejects_a_newer_schema_and_keeps_the_current_database() {
        let directory = TestDir::new();
        let source_path = directory.path().join("newer.sqlite");
        let source = test_database(&source_path);
        source
            .connection()
            .unwrap()
            .pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION + 1)
            .unwrap();
        drop(source);
        let bytes = std::fs::read(&source_path).unwrap();

        let target = test_database(&directory.path().join("target.sqlite"));
        target
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value) VALUES ('theme', 'ocean')",
                [],
            )
            .unwrap();
        let error = import_from_bytes(&target, &bytes).unwrap_err();
        assert!(error.to_string().contains("newer Address Explorer version"));
        assert_eq!(setting(&target, "theme").as_deref(), Some("ocean"));
    }

    #[test]
    fn import_rejects_missing_tables() {
        let directory = TestDir::new();
        let source_path = directory.path().join("missing.sqlite");
        let source = test_database(&source_path);
        source
            .connection()
            .unwrap()
            .execute_batch("DROP TABLE fiat_exchange_rates")
            .unwrap();
        drop(source);
        let bytes = std::fs::read(&source_path).unwrap();

        let target = test_database(&directory.path().join("target.sqlite"));
        let error = import_from_bytes(&target, &bytes).unwrap_err();
        assert!(error.to_string().contains("missing required tables"));
    }

    #[test]
    fn import_rejects_non_sqlite_input() {
        let directory = TestDir::new();
        let target = test_database(&directory.path().join("target.sqlite"));
        let error = import_from_bytes(&target, b"not a sqlite database").unwrap_err();
        assert!(error.to_string().contains("not a SQLite database"));
    }

    #[test]
    fn reset_replaces_user_rows_with_defaults() {
        let directory = TestDir::new();
        let database = test_database(&directory.path().join("reset.sqlite"));
        database
            .connection()
            .unwrap()
            .execute(
                "INSERT INTO setting (key, value) VALUES ('theme', 'forest')",
                [],
            )
            .unwrap();
        reset_to_defaults(&database).unwrap();
        assert_eq!(setting(&database, "theme"), None);
        let connection = database.connection().unwrap();
        validate_current_schema(&connection).unwrap();
    }
}
