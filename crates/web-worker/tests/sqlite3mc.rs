use rusqlite::backup::Backup;
use rusqlite::{Connection, MAIN_DB};
use wasm_bindgen_test::wasm_bindgen_test;

const SPEC: &str = "x'808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f00112233445566778899aabbccddeeff'";

fn keyed(path: &str) -> Connection {
    let _mem = sqlite_wasm_rs::MemVfsUtil::<sqlite_wasm_rs::WasmOsCallback>::new();
    let connection = Connection::open_with_flags_and_vfs(
        path,
        rusqlite::OpenFlags::default(),
        "multipleciphers-memvfs",
    )
    .unwrap();
    connection
        .pragma_update(None, "cipher", "sqlcipher")
        .unwrap();
    connection.pragma_update(None, "legacy", 4).unwrap();
    connection.pragma_update(None, "key", SPEC).unwrap();
    connection
        .pragma_update(None, "memory_security", true)
        .unwrap();
    connection
        .query_row("SELECT COUNT(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap();
    connection
}

#[wasm_bindgen_test]
fn sqlite3mc_sqlcipher_raw_key_round_trip_and_plaintext_export() {
    let path = "sqlite3mc-roundtrip.db";
    {
        let connection = keyed(path);
        connection
            .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (42);")
            .unwrap();
    }
    {
        let connection = keyed(path);
        let value: i64 = connection
            .query_row("SELECT x FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 42);
        let data = connection.serialize(MAIN_DB).unwrap();
        assert!(data.starts_with(b"SQLite format 3\0"));
    }
    {
        let plain = Connection::open("sqlite3mc-plain.db").unwrap();
        plain
            .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (7);")
            .unwrap();
        let mut encrypted = keyed("sqlite3mc-copy.db");
        assert!(Backup::new(&plain, &mut encrypted).is_err());
    }
    {
        let plain = Connection::open(path).unwrap();
        assert!(plain
            .query_row("SELECT x FROM t", [], |row| row.get::<_, i64>(0))
            .is_err());
    }
}

#[wasm_bindgen_test]
fn sqlite3mc_rekey_round_trip() {
    let path = "sqlite3mc-rekey.db";
    {
        let connection = keyed(path);
        connection
            .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (9);")
            .unwrap();
        connection
            .pragma_update(None, "rekey", "x'111111111111111111111111111111111111111111111111111111111111111100112233445566778899aabbccddeeff'")
            .unwrap();
    }
    {
        let connection = Connection::open(path).unwrap();
        connection
            .pragma_update(None, "cipher", "sqlcipher")
            .unwrap();
        connection.pragma_update(None, "legacy", 4).unwrap();
        connection
            .pragma_update(
                None,
                "key",
                "x'111111111111111111111111111111111111111111111111111111111111111100112233445566778899aabbccddeeff'",
            )
            .unwrap();
        let value: i64 = connection
            .query_row("SELECT x FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 9);
    }
}

#[wasm_bindgen_test]
fn sqlite3mc_attach_plaintext_to_encrypted_and_copy() {
    let plain_path = "sqlite3mc-copy-src.db";
    let enc_path = "sqlite3mc-copy-dst.db";
    {
        let plain = Connection::open(plain_path).unwrap();
        plain
            .execute_batch(
                "CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (21); PRAGMA user_version=5;",
            )
            .unwrap();
    }
    {
        let target = keyed(enc_path);
        target.execute_batch("CREATE TABLE t(x INTEGER);").unwrap();
        target
            .execute_batch(&format!(
                "ATTACH DATABASE '{}' AS plaintext KEY '';",
                plain_path
            ))
            .unwrap();
        target
            .execute_batch("INSERT INTO main.t (x) SELECT x FROM plaintext.t;")
            .unwrap();
        let version: i64 = target
            .pragma_query_value(Some("plaintext"), "user_version", |row| row.get(0))
            .unwrap();
        target.pragma_update(None, "user_version", version).unwrap();
        target.execute_batch("DETACH DATABASE plaintext;").unwrap();
    }
    {
        let target = keyed(enc_path);
        let value: i64 = target
            .query_row("SELECT x FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 21);
        let version: i64 = target
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 5);
    }
}

#[wasm_bindgen_test]
fn sqlite3mc_column_list_copy_matches_schema() {
    let plain_path = "sqlite3mc-columns-src.db";
    let enc_path = "sqlite3mc-columns-dst.db";
    {
        let plain = Connection::open(plain_path).unwrap();
        plain
            .execute_batch(
                "CREATE TABLE t(a TEXT, b INTEGER, c BLOB); \
                 INSERT INTO t VALUES ('first', 1, x'00ff');",
            )
            .unwrap();
    }
    {
        let target = keyed(enc_path);
        target
            .execute_batch("CREATE TABLE t(a TEXT, b INTEGER, c BLOB);")
            .unwrap();
        let mut columns = {
            let mut statement = target.prepare("PRAGMA main.table_info(t)").unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        columns.sort();
        let column_list = columns
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        target
            .execute_batch(&format!(
                "ATTACH DATABASE '{plain_path}' AS plaintext KEY ''; \
                 INSERT INTO main.t ({column_list}) SELECT {column_list} FROM plaintext.t; \
                 DETACH DATABASE plaintext;"
            ))
            .unwrap();
    }
    let target = keyed(enc_path);
    let (a, b): (String, i64) = target
        .query_row("SELECT a, b FROM t", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(a, "first");
    assert_eq!(b, 1);
}

#[wasm_bindgen_test]
fn sqlite3mc_wrapper_vfs_is_required_for_keys() {
    let _mem = sqlite_wasm_rs::MemVfsUtil::<sqlite_wasm_rs::WasmOsCallback>::new();

    // The raw custom VFS rejects the key; this is the browser-local failure
    // mode when relaxed-idb is opened without the SQLite3MC wrapper.
    {
        let connection = Connection::open_with_flags_and_vfs(
            "sqlite3mc-raw-memvfs.db",
            rusqlite::OpenFlags::default(),
            "memvfs",
        )
        .unwrap();
        let error = connection.pragma_update(None, "key", SPEC).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Encryption is not supported by the VFS"),
            "unexpected error: {error}"
        );
    }

    // Opening through SQLite3MC's wrapper around the same VFS works.
    let path = "sqlite3mc-wrapped-memvfs.db";
    {
        let connection = Connection::open_with_flags_and_vfs(
            path,
            rusqlite::OpenFlags::default(),
            "multipleciphers-memvfs",
        )
        .unwrap();
        connection
            .pragma_update(None, "cipher", "sqlcipher")
            .unwrap();
        connection.pragma_update(None, "legacy", 4).unwrap();
        connection.pragma_update(None, "key", SPEC).unwrap();
        connection
            .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (31);")
            .unwrap();
    }
    {
        let connection = Connection::open_with_flags_and_vfs(
            path,
            rusqlite::OpenFlags::default(),
            "multipleciphers-memvfs",
        )
        .unwrap();
        connection
            .pragma_update(None, "cipher", "sqlcipher")
            .unwrap();
        connection.pragma_update(None, "legacy", 4).unwrap();
        connection.pragma_update(None, "key", SPEC).unwrap();
        let value: i64 = connection
            .query_row("SELECT x FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 31);
    }
}

#[wasm_bindgen_test]
fn sqlite3mc_reopen_applies_key_before_synchronous() {
    let path = "sqlite3mc-order.db";
    {
        let connection = keyed(path);
        connection
            .execute_batch("CREATE TABLE t(x INTEGER); INSERT INTO t VALUES (37);")
            .unwrap();
    }
    {
        let connection = Connection::open_with_flags_and_vfs(
            path,
            rusqlite::OpenFlags::default(),
            "multipleciphers-memvfs",
        )
        .unwrap();
        let error = connection
            .pragma_update(None, "synchronous", "OFF")
            .unwrap_err();
        assert!(
            error.to_string().contains("file is not a database"),
            "unexpected error: {error}"
        );
    }
    {
        let connection = Connection::open_with_flags_and_vfs(
            path,
            rusqlite::OpenFlags::default(),
            "multipleciphers-memvfs",
        )
        .unwrap();
        connection
            .pragma_update(None, "cipher", "sqlcipher")
            .unwrap();
        connection.pragma_update(None, "legacy", 4).unwrap();
        connection.pragma_update(None, "key", SPEC).unwrap();
        connection
            .pragma_update(None, "synchronous", "OFF")
            .unwrap();
        let value: i64 = connection
            .query_row("SELECT x FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 37);
    }
}
