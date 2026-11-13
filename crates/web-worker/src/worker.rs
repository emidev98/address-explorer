//! SharedWorker bootstrap and sequential request loop.

use crate::data::encryption::{
    derive_key, generate_salt, is_plaintext_sqlite, VaultKey, SALT_LEN, VAULT_LOCKED_MESSAGE,
};
use crate::db::{apply_sqlcipher_key, open_connection, BrowserDatabase, BrowserDatabaseState};
use crate::domain::encryption::{IdleTimeout, UnlockResult};
use crate::rpc;
use crate::vault::WorkerVault;
use address_explorer_core::{
    DataChanged, WorkerHello, WorkerReady, WorkerRequest, WorkerResponse, WORKER_PROTOCOL_VERSION,
};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::sync::mpsc::UnboundedReceiver;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, MessagePort};
use zeroize::Zeroizing;

const CONVERT_TEMP: &str = "address-explorer-convert.db";
const IMPORT_PLAIN_TEMP: &str = "address-explorer-import-plain.db";
const EXPORT_PLAIN_TEMP: &str = "address-explorer-export-plain.db";
const IMPORT_ENC_TEMP: &str = "address-explorer-import-encrypted.db";
const ENCRYPTED_PAGE_SIZE: usize = 4096;

struct State {
    util: Option<Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>>,
    db: Option<BrowserDatabase>,
    vault: WorkerVault,
    seed_url: String,
    app_version: String,
    revision: u64,
    initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            util: None,
            db: None,
            vault: WorkerVault::default(),
            seed_url: String::new(),
            app_version: "unknown".to_string(),
            revision: 0,
            initialized: false,
        }
    }
}

thread_local! {
    static PORTS: RefCell<Vec<MessagePort>> = const { RefCell::new(Vec::new()) };
    static SENDER: RefCell<Option<tokio::sync::mpsc::UnboundedSender<(MessagePort, String)>>> =
        const { RefCell::new(None) };
    static STATE: RefCell<Option<Rc<RefCell<State>>>> = const { RefCell::new(None) };
}

pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "address-explorer worker panic: {info}"
        )));
    }));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<(MessagePort, String)>();
    SENDER.with(|sender| *sender.borrow_mut() = Some(tx));
    STATE.with(|state| *state.borrow_mut() = Some(Rc::new(RefCell::new(State::default()))));
    wasm_bindgen_futures::spawn_local(async move {
        run(rx).await;
    });
    Ok(())
}

/// Attach one tab's port to the worker. The JavaScript entry calls this for
/// every `connect` event, including ports that arrived while the WASM module
/// was still initializing.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn connect_port(port: MessagePort) -> Result<(), JsValue> {
    let sender = SENDER
        .with(|sender| sender.borrow().clone())
        .ok_or_else(|| JsValue::from_str("the address-explorer worker is not initialized yet"))?;
    let tx = sender.clone();
    let port_for_message = port.clone();
    let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Some(text) = event.data().as_string() {
            let _ = tx.send((port_for_message.clone(), text));
        }
    });
    port.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();
    port.start();
    PORTS.with(|ports| ports.borrow_mut().push(port));
    Ok(())
}

/// Called once per second by the worker entry. It locks an idle vault even
/// when every tab is closed; the drop also closes the SQLite connection.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn vault_tick() {
    let state = STATE.with(|slot| slot.borrow().clone());
    let Some(state) = state else {
        return;
    };
    let Ok(mut state) = state.try_borrow_mut() else {
        return;
    };
    if !state.vault.lock_if_idle() {
        return;
    }
    state.db = None;
    state.revision = state.revision.saturating_add(1);
    broadcast_all(&serde_json::json!({ "vault_locked": true }));
}

// The single-threaded worker serializes every request through this borrow.
// Holding it across the request future is intentional: the idle tick must not
// drop the connection in the middle of an operation.
#[allow(clippy::await_holding_refcell_ref)]
async fn run(mut rx: UnboundedReceiver<(MessagePort, String)>) {
    let Some(state) = STATE.with(|slot| slot.borrow().clone()) else {
        return;
    };
    while let Some((port, text)) = rx.recv().await {
        let Ok(mut state) = state.try_borrow_mut() else {
            continue;
        };
        handle_message(&mut state, &port, &text).await;
    }
}

async fn handle_message(state: &mut State, port: &MessagePort, text: &str) {
    let request: WorkerRequest = match serde_json::from_str(text) {
        Ok(request) => request,
        Err(error) => {
            post(
                port,
                &WorkerResponse::failure(0, format!("invalid worker request: {error}")),
            );
            return;
        }
    };

    if request.endpoint == "hello" {
        let hello: WorkerHello = match serde_json::from_value(request.payload) {
            Ok(hello) => hello,
            Err(error) => {
                post(
                    port,
                    &WorkerResponse::failure(
                        request.id,
                        format!("invalid worker hello payload: {error}"),
                    ),
                );
                return;
            }
        };
        if hello.protocol_version != WORKER_PROTOCOL_VERSION {
            post(
                port,
                &WorkerResponse::failure(
                    request.id,
                    format!(
                        "worker protocol {} does not match this app ({}); reload the application",
                        hello.protocol_version, WORKER_PROTOCOL_VERSION
                    ),
                ),
            );
            return;
        }

        state.app_version = hello.app_version.clone();
        if state.seed_url.is_empty() {
            state.seed_url = hello.seed_url.clone();
        }
        if !state.initialized {
            match initialize_database(state, &hello).await {
                Ok(()) => state.initialized = true,
                Err(error) => {
                    post(port, &WorkerResponse::failure(request.id, error));
                    return;
                }
            }
        }

        post(
            port,
            &WorkerResponse::success(
                request.id,
                serde_json::to_value(WorkerReady {
                    protocol_version: WORKER_PROTOCOL_VERSION,
                    app_version: state.app_version.clone(),
                    database: address_explorer_core::WEB_DATABASE_FILE.to_string(),
                })
                .unwrap_or(serde_json::Value::Null),
            ),
        );
        return;
    }

    if request.endpoint.starts_with("encryption.") {
        let result = handle_encryption(state, &request.endpoint, request.payload).await;
        match result {
            Ok(value) => post(port, &WorkerResponse::success(request.id, value)),
            Err(error) => post(port, &WorkerResponse::failure(request.id, error)),
        }
        return;
    }

    if !state.vault.unlocked() {
        let message = if state.vault.configured() {
            VAULT_LOCKED_MESSAGE
        } else {
            "no database password is set; create one before using the application"
        };
        post(port, &WorkerResponse::failure(request.id, message));
        return;
    }

    if request.endpoint == "data.export_encrypted" {
        let result = export_encrypted_backup(state, &request.payload).await;
        match result {
            Ok(value) => post(port, &WorkerResponse::success(request.id, value)),
            Err(error) => post(port, &WorkerResponse::failure(request.id, error)),
        }
        return;
    }

    if request.endpoint == "data.import_encrypted" || request.endpoint == "data.clear" {
        match replace_local_database(state, &request.endpoint, request.payload).await {
            Ok(()) => {
                state.revision = state.revision.saturating_add(1);
                broadcast(
                    port,
                    &serde_json::to_value(DataChanged {
                        revision: state.revision,
                    })
                    .unwrap_or(serde_json::Value::Null),
                );
                post(
                    port,
                    &WorkerResponse::success(request.id, serde_json::Value::Null),
                );
            }
            Err(error) => post(port, &WorkerResponse::failure(request.id, error)),
        }
        return;
    }

    let Some(db) = state.db.as_mut() else {
        post(
            port,
            &WorkerResponse::failure(
                request.id,
                "the browser database is not ready; send hello before using the local backend",
            ),
        );
        return;
    };

    match rpc::dispatch(db, &request.endpoint, request.payload).await {
        Ok(result) => {
            if rpc::is_write(&request.endpoint) {
                if let Err(error) = db.wait_committed().await {
                    post(port, &WorkerResponse::failure(request.id, error));
                    return;
                }
                state.revision = state.revision.saturating_add(1);
                broadcast(
                    port,
                    &serde_json::to_value(DataChanged {
                        revision: state.revision,
                    })
                    .unwrap_or(serde_json::Value::Null),
                );
            }
            post(port, &WorkerResponse::success(request.id, result));
        }
        Err(error) => post(port, &WorkerResponse::failure(request.id, error)),
    }
}

async fn initialize_database(state: &mut State, hello: &WorkerHello) -> Result<(), String> {
    let util = BrowserDatabase::install(hello.reset).await?;
    let _ = delete_named_file(&util, IMPORT_PLAIN_TEMP).await;
    let _ = delete_named_file(&util, EXPORT_PLAIN_TEMP).await;
    let _ = delete_named_file(&util, IMPORT_ENC_TEMP).await;
    let _ = delete_named_file(&util, CONVERT_TEMP).await;
    match BrowserDatabase::inspect(&util)? {
        BrowserDatabaseState::Encrypted(salt) => {
            state.vault.set_locked(salt);
            state.util = Some(util);
            state.db = None;
        }
        BrowserDatabaseState::Missing | BrowserDatabaseState::Plaintext => {
            // A password is mandatory. The plaintext file, when present, is
            // encrypted in place by the setup flow before any data is served.
            state.vault.set_needs_setup();
            state.util = Some(util);
            state.db = None;
        }
    }
    Ok(())
}

async fn handle_encryption(
    state: &mut State,
    endpoint: &str,
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    match endpoint {
        "encryption.status" => encode(state.vault.status()),
        "encryption.setup" => {
            let password: String = arg(&payload, "password")?;
            let password = Zeroizing::new(password);
            encode(setup_encryption(state, &password).await?)
        }
        "encryption.unlock" => {
            let password: String = arg(&payload, "password")?;
            let password = Zeroizing::new(password);
            encode(unlock_encryption(state, &password).await?)
        }
        "encryption.lock" => lock_encryption(state).await,
        "encryption.touch" => {
            state.vault.touch();
            encode(())
        }
        "encryption.change_password" => {
            let current: String = arg(&payload, "current_password")?;
            let new: String = arg(&payload, "new_password")?;
            let current = Zeroizing::new(current);
            let new = Zeroizing::new(new);
            encode(change_password(state, &current, &new).await?)
        }
        "encryption.save_idle_timeout" => {
            let timeout: IdleTimeout = arg(&payload, "timeout")?;
            save_idle_timeout(state, timeout).await
        }
        "encryption.reset" => {
            let confirmation: String = arg(&payload, "confirmation")?;
            if confirmation.trim() != "ERASE" {
                return Err("type ERASE to confirm erasing the encrypted database".to_string());
            }
            let result = reset_encryption(state).await;
            if result.is_ok() {
                state.revision = state.revision.saturating_add(1);
                broadcast_all(
                    &serde_json::to_value(DataChanged {
                        revision: state.revision,
                    })
                    .unwrap_or(serde_json::Value::Null),
                );
            }
            encode(result?)
        }
        other => Err(format!("unknown worker endpoint: {other}")),
    }
}

async fn setup_encryption(state: &mut State, password: &str) -> Result<UnlockResult, String> {
    if state.vault.configured() {
        return Err("encryption is already configured".to_string());
    }
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    // Open (or create) the plaintext database so an upgrade keeps its policy,
    // timeout, and data. It is converted and replaced before the vault opens.
    let plaintext = BrowserDatabase::open_with_util(&state.seed_url, util.clone(), None, false)
        .await
        .map_err(|error| format!("could not prepare the database: {error}"))?;
    let policy = crate::security::policy::load_from_database(&plaintext.database)
        .map_err(|error| error.to_string())?;
    crate::domain::security_settings::validate_password(password, policy.password_min_length)?;
    let timeout =
        crate::data::repositories::settings::load_encryption_idle_timeout(&plaintext.database)
            .unwrap_or_default();
    let plaintext_bytes = plaintext.export_plaintext()?;
    drop(plaintext);

    let salt = generate_salt()?;
    let key = derive_key(password, &salt)?;
    let encrypted = match encrypted_snapshot_from_plaintext(
        &util,
        address_explorer_core::WEB_DATABASE_FILE,
        &key,
        &salt,
    )
    .await
    {
        Ok(bytes) => bytes,
        Err(error) => {
            let _ = restore_plaintext(state, &plaintext_bytes).await;
            return Err(error);
        }
    };

    if let Err(error) = replace_main_with_encrypted(&util, &encrypted).await {
        let _ = restore_plaintext(state, &plaintext_bytes).await;
        return Err(error);
    }
    let new_db = match BrowserDatabase::open_with_util(
        &state.seed_url,
        util.clone(),
        Some((&key, &salt)),
        false,
    )
    .await
    {
        Ok(db) => db,
        Err(error) => {
            let _ = restore_plaintext(state, &plaintext_bytes).await;
            return Err(error);
        }
    };
    let settings_result = crate::data::repositories::settings::save_encryption_idle_timeout(
        &new_db.database,
        timeout,
    )
    .map_err(|error| error.to_string());
    state.vault.set_unlocked(key, salt, timeout);
    state.db = Some(new_db);
    if let Err(error) = settings_result {
        let _ = state.db.as_ref().map(|db| db.wait_committed());
        return Err(error);
    }
    Ok(UnlockResult::local(state.vault.status()))
}

async fn unlock_encryption(state: &mut State, password: &str) -> Result<UnlockResult, String> {
    if !state.vault.locked() {
        if state.vault.unlocked() {
            return Ok(UnlockResult::local(state.vault.status()));
        }
        return Err("no database password is set".to_string());
    }
    if password.is_empty() {
        return Err("incorrect password".to_string());
    }
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let salt = state
        .vault
        .salt()
        .ok_or_else(|| "the encrypted database header is invalid".to_string())?;
    let key = derive_key(password, &salt)?;
    let db = BrowserDatabase::open_with_util(&state.seed_url, util, Some((&key, &salt)), false)
        .await
        .map_err(|_| "incorrect password or damaged database".to_string())?;
    let timeout = crate::data::repositories::settings::load_encryption_idle_timeout(&db.database)
        .unwrap_or_default();
    crate::security::policy::load_from_database(&db.database).map_err(|error| error.to_string())?;
    state.vault.set_unlocked(key, salt, timeout);
    state.db = Some(db);
    Ok(UnlockResult::local(state.vault.status()))
}

async fn lock_encryption(state: &mut State) -> Result<serde_json::Value, String> {
    if !state.vault.unlocked() {
        return encode(state.vault.status());
    }
    let salt = state
        .vault
        .salt()
        .ok_or_else(|| "the encrypted database header is invalid".to_string())?;
    if let Some(db) = state.db.as_ref() {
        db.wait_committed().await?;
    }
    if let Some(db) = state.db.take() {
        drop(db);
    }
    state.vault.set_locked(salt);
    crate::security::policy::reset_to_defaults();
    encode(state.vault.status())
}

async fn change_password(
    state: &mut State,
    current_password: &str,
    new_password: &str,
) -> Result<UnlockResult, String> {
    if !state.vault.unlocked() {
        return Err("the database is locked".to_string());
    }
    if current_password.is_empty() {
        return Err("the current password must not be empty".to_string());
    }
    crate::domain::security_settings::validate_password(
        new_password,
        crate::security::policy::current().password_min_length,
    )?;
    let old_salt = state
        .vault
        .salt()
        .ok_or_else(|| "the encrypted database header is invalid".to_string())?;
    let old_key = state
        .vault
        .key()
        .ok_or_else(|| "the database is locked".to_string())?;
    let candidate = derive_key(current_password, &old_salt)?;
    if !old_key.matches(&candidate) {
        return Err("incorrect password".to_string());
    }
    let new_salt = generate_salt()?;
    let new_key = derive_key(new_password, &new_salt)?;
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let safety = db.export_encrypted()?;
    {
        let connection = db
            .database
            .connection()
            .map_err(|error| error.to_string())?;
        let spec = Zeroizing::new(new_key.key_spec(&new_salt));
        connection
            .pragma_update(None, "rekey", spec.as_str())
            .map_err(|error| error.to_string())?;
    }
    db.wait_committed().await?;
    state.db = None;

    match BrowserDatabase::open_with_util(
        &state.seed_url,
        util.clone(),
        Some((&new_key, &new_salt)),
        false,
    )
    .await
    {
        Ok(new_db) => {
            state
                .vault
                .set_unlocked(new_key, new_salt, state.vault.idle_timeout());
            state.db = Some(new_db);
            Ok(UnlockResult::local(state.vault.status()))
        }
        Err(error) => {
            let _ =
                restore_encrypted_database(&util, &state.seed_url, &safety, &old_key, &old_salt)
                    .await;
            Err(format!("the password change failed: {error}"))
        }
    }
}

async fn save_idle_timeout(
    state: &mut State,
    timeout: IdleTimeout,
) -> Result<serde_json::Value, String> {
    if !state.vault.unlocked() {
        return Err(VAULT_LOCKED_MESSAGE.to_string());
    }
    {
        let db = state
            .db
            .as_ref()
            .ok_or_else(|| "the browser database is not ready".to_string())?;
        crate::data::repositories::settings::save_encryption_idle_timeout(&db.database, timeout)
            .map_err(|error| error.to_string())?;
        db.wait_committed().await?;
    }
    state.vault.set_idle_timeout(timeout);
    if state.vault.lock_if_idle() {
        state.db = None;
    }
    encode(state.vault.status())
}

async fn reset_encryption(state: &mut State) -> Result<UnlockResult, String> {
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    state.db = None;
    delete_database_file(&util).await?;
    let _ = delete_named_file(&util, CONVERT_TEMP).await;
    let _ = delete_named_file(&util, IMPORT_PLAIN_TEMP).await;
    let _ = delete_named_file(&util, EXPORT_PLAIN_TEMP).await;
    let _ = delete_named_file(&util, IMPORT_ENC_TEMP).await;
    state.vault.set_needs_setup();
    crate::security::policy::reset_to_defaults();
    Ok(UnlockResult::local(state.vault.status()))
}

async fn replace_local_database(
    state: &mut State,
    endpoint: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    use base64::Engine;

    let import_bytes = if endpoint == "data.import_encrypted" {
        let encoded: String = serde_json::from_value(
            payload
                .get("file")
                .cloned()
                .ok_or_else(|| "missing `file` argument".to_string())?,
        )
        .map_err(|error| format!("invalid `file` argument: {error}"))?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid base64 SQLite file: {error}"))?;
        if bytes.is_empty() {
            return Err("the selected SQLite file is empty".to_string());
        }
        let passphrase: String = serde_json::from_value(
            payload
                .get("passphrase")
                .cloned()
                .ok_or_else(|| "missing `passphrase` argument".to_string())?,
        )
        .map_err(|error| format!("invalid `passphrase` argument: {error}"))?;
        Some(decrypt_imported_backup(state, &bytes, &passphrase).await?)
    } else {
        None
    };

    replace_encrypted_database(state, import_bytes).await
}

async fn replace_encrypted_database(
    state: &mut State,
    import_bytes: Option<Vec<u8>>,
) -> Result<(), String> {
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let key = state
        .vault
        .key()
        .ok_or_else(|| "the database is locked".to_string())?;
    let salt = state
        .vault
        .salt()
        .ok_or_else(|| "the encrypted database header is invalid".to_string())?;
    let old = state
        .db
        .take()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let safety = match old.export_encrypted() {
        Ok(bytes) => bytes,
        Err(error) => {
            state.db = Some(old);
            return Err(error);
        }
    };
    drop(old);

    let result = match import_bytes {
        Some(bytes) => import_plaintext_into_encrypted(state, &util, &bytes, &key, &salt).await,
        None => reset_encrypted_database(state, &util, &key, &salt).await,
    };

    if let Err(error) = result {
        match restore_encrypted_database(&util, &state.seed_url, &safety, &key, &salt).await {
            Ok(restored) => {
                state.db = Some(restored);
                state
                    .vault
                    .set_unlocked(key, salt, state.vault.idle_timeout());
            }
            Err(restore_error) => {
                state.vault.set_locked(salt);
                return Err(format!(
                    "{error}; restoring the previous database also failed: {restore_error}"
                ));
            }
        }
        return Err(error);
    }
    Ok(())
}

async fn import_plaintext_into_encrypted(
    state: &mut State,
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    bytes: &[u8],
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), String> {
    if !is_plaintext_sqlite(bytes) {
        return Err("the selected file is not a plaintext SQLite database".to_string());
    }
    delete_named_file(util, IMPORT_PLAIN_TEMP).await?;
    let import = util
        .import_db(IMPORT_PLAIN_TEMP, bytes)
        .map_err(|error| format!("could not import the SQLite file: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the SQLite file: {error}"))?;
    let plaintext_db = BrowserDatabase::open_named(
        &state.seed_url,
        util.clone(),
        IMPORT_PLAIN_TEMP,
        None,
        true,
        true,
    )
    .await;
    let plaintext_db = match plaintext_db {
        Ok(db) => db,
        Err(error) => {
            let _ = delete_named_file(util, IMPORT_PLAIN_TEMP).await;
            return Err(error);
        }
    };
    drop(plaintext_db);
    let encrypted = encrypted_snapshot_from_plaintext(util, IMPORT_PLAIN_TEMP, key, salt).await;
    let _ = delete_named_file(util, IMPORT_PLAIN_TEMP).await;
    let encrypted = encrypted?;
    replace_main_with_encrypted(util, &encrypted).await?;
    let db =
        BrowserDatabase::open_with_util(&state.seed_url, util.clone(), Some((key, salt)), false)
            .await?;
    let timeout = crate::data::repositories::settings::load_encryption_idle_timeout(&db.database)
        .unwrap_or_default();
    crate::data::repositories::settings::save_security_settings(
        &db.database,
        &crate::security::policy::current(),
    )
    .map_err(|error| error.to_string())?;
    db.wait_committed().await?;
    state.vault.set_unlocked(key.clone(), *salt, timeout);
    state.db = Some(db);
    Ok(())
}

async fn reset_encrypted_database(
    state: &mut State,
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<(), String> {
    delete_database_file(util).await?;
    let db =
        BrowserDatabase::open_with_util(&state.seed_url, util.clone(), Some((key, salt)), false)
            .await?;
    let timeout = crate::data::repositories::settings::load_encryption_idle_timeout(&db.database)
        .unwrap_or_default();
    crate::data::repositories::settings::save_security_settings(
        &db.database,
        &crate::security::policy::current(),
    )
    .map_err(|error| error.to_string())?;
    db.wait_committed().await?;
    state.vault.set_unlocked(key.clone(), *salt, timeout);
    state.db = Some(db);
    Ok(())
}

async fn restore_plaintext(state: &mut State, bytes: &[u8]) -> Result<(), String> {
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    delete_database_file(&util).await?;
    let import = util
        .import_db(address_explorer_core::WEB_DATABASE_FILE, bytes)
        .map_err(|error| format!("could not restore the plaintext database: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the plaintext database: {error}"))?;
    state.db = None;
    state.vault.set_needs_setup();
    Ok(())
}

async fn restore_encrypted_database(
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    seed_url: &str,
    bytes: &[u8],
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<BrowserDatabase, String> {
    delete_database_file(util).await?;
    let import = util
        .import_db_unchecked(
            address_explorer_core::WEB_DATABASE_FILE,
            bytes,
            ENCRYPTED_PAGE_SIZE,
        )
        .map_err(|error| format!("could not restore the encrypted database: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the encrypted database: {error}"))?;
    BrowserDatabase::open_with_util(seed_url, util.clone(), Some((key, salt)), false).await
}

async fn export_encrypted_backup(
    state: &State,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    use base64::Engine;
    let passphrase: String = serde_json::from_value(
        payload
            .get("passphrase")
            .cloned()
            .ok_or_else(|| "missing `passphrase` argument".to_string())?,
    )
    .map_err(|error| format!("invalid `passphrase` argument: {error}"))?;
    crate::domain::security_settings::validate_password(
        &passphrase,
        crate::security::policy::current().password_min_length,
    )?;
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let db = state
        .db
        .as_ref()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    db.wait_committed().await?;
    let plaintext = db.export_plaintext()?;
    delete_named_file(&util, EXPORT_PLAIN_TEMP).await?;
    let import = util
        .import_db(EXPORT_PLAIN_TEMP, &plaintext)
        .map_err(|error| format!("could not prepare the backup: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the backup: {error}"))?;
    let salt = generate_salt()?;
    let key = derive_key(&passphrase, &salt)?;
    let encrypted = encrypted_snapshot_from_plaintext(&util, EXPORT_PLAIN_TEMP, &key, &salt).await;
    let _ = delete_named_file(&util, EXPORT_PLAIN_TEMP).await;
    let encrypted = encrypted?;
    Ok(serde_json::Value::String(
        base64::engine::general_purpose::STANDARD.encode(encrypted),
    ))
}

async fn decrypt_imported_backup(
    state: &State,
    bytes: &[u8],
    passphrase: &str,
) -> Result<Vec<u8>, String> {
    let util = state
        .util
        .clone()
        .ok_or_else(|| "the browser database is not ready".to_string())?;
    let salt = crate::data::encryption::salt_from_header(bytes)
        .map_err(|_| "the selected file is not an encrypted backup".to_string())?;
    let key = derive_key(passphrase, &salt)?;
    delete_named_file(&util, IMPORT_ENC_TEMP).await?;
    let import = util
        .import_db_unchecked(IMPORT_ENC_TEMP, bytes, ENCRYPTED_PAGE_SIZE)
        .map_err(|error| format!("could not read the encrypted backup: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the encrypted backup: {error}"))?;
    let db = BrowserDatabase::open_named(
        &state.seed_url,
        util.clone(),
        IMPORT_ENC_TEMP,
        Some((&key, &salt)),
        false,
        false,
    )
    .await
    .map_err(|_| "the backup passphrase is incorrect or the file is damaged".to_string())?;
    let plaintext = db.export_plaintext()?;
    drop(db);
    let _ = delete_named_file(&util, IMPORT_ENC_TEMP).await;
    Ok(plaintext)
}

async fn encrypted_snapshot_from_plaintext(
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    plaintext_filename: &str,
    key: &VaultKey,
    salt: &[u8; SALT_LEN],
) -> Result<Vec<u8>, String> {
    delete_named_file(util, CONVERT_TEMP).await?;
    let mut connection = open_connection(CONVERT_TEMP, true)?;
    apply_sqlcipher_key(&connection, key, salt)?;
    connection
        .pragma_update(None, "synchronous", "OFF")
        .map_err(|error| format!("could not configure the encrypted copy: {error}"))?;
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|error| format!("could not configure the encrypted copy: {error}"))?;
    crate::data::migrations::run_migrations(&mut connection, false)
        .map_err(|error| format!("could not create the encrypted schema: {error}"))?;
    crate::data::backup::copy_plaintext_into_encrypted(plaintext_filename, &mut connection)
        .map_err(|error| {
            format!("could not copy the database into the encrypted vault: {error}")
        })?;
    crate::data::backup::validate_current_schema(&connection)
        .map_err(|error| format!("the encrypted schema is invalid: {error}"))?;
    let wait = util
        .wait_committed(CONVERT_TEMP)
        .map_err(|error| format!("could not queue the encrypted copy commit: {error}"))?;
    wait.await
        .map_err(|error| format!("could not persist the encrypted copy: {error}"))?;
    let bytes = util
        .export_db(CONVERT_TEMP)
        .map_err(|error| format!("could not export the encrypted copy: {error}"))?;
    drop(connection);
    delete_named_file(util, CONVERT_TEMP).await?;
    Ok(bytes)
}

async fn replace_main_with_encrypted(
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    bytes: &[u8],
) -> Result<(), String> {
    delete_database_file(util).await?;
    let import = util
        .import_db_unchecked(
            address_explorer_core::WEB_DATABASE_FILE,
            bytes,
            ENCRYPTED_PAGE_SIZE,
        )
        .map_err(|error| format!("could not store the encrypted database: {error}"))?;
    import
        .await
        .map_err(|error| format!("could not persist the encrypted database: {error}"))?;
    Ok(())
}

async fn delete_named_file(
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
    filename: &str,
) -> Result<(), String> {
    if !util.exists(filename) {
        return Ok(());
    }
    let delete = util
        .delete_db(filename)
        .map_err(|error| format!("could not replace {filename}: {error}"))?;
    delete
        .await
        .map_err(|error| format!("could not persist the replacement of {filename}: {error}"))
}

async fn delete_database_file(
    util: &Rc<sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil>,
) -> Result<(), String> {
    delete_named_file(util, address_explorer_core::WEB_DATABASE_FILE).await
}

fn arg<T: serde::de::DeserializeOwned>(
    payload: &serde_json::Value,
    name: &str,
) -> Result<T, String> {
    serde_json::from_value(
        payload
            .get(name)
            .cloned()
            .unwrap_or(serde_json::Value::Null),
    )
    .map_err(|error| format!("invalid `{name}` argument: {error}"))
}

fn encode<T: serde::Serialize>(value: T) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|error| format!("could not encode the response: {error}"))
}

fn post(port: &MessagePort, response: &WorkerResponse) {
    if let Ok(text) = serde_json::to_string(response) {
        let _ = port.post_message(&JsValue::from_str(&text));
    }
}

fn broadcast(source: &MessagePort, value: &serde_json::Value) {
    let response = WorkerResponse::success(0, value.clone());
    let Ok(text) = serde_json::to_string(&response) else {
        return;
    };
    PORTS.with(|ports| {
        for port in ports.borrow().iter() {
            if port != source {
                let _ = port.post_message(&JsValue::from_str(&text));
            }
        }
    });
}

fn broadcast_all(value: &serde_json::Value) {
    let response = WorkerResponse::success(0, value.clone());
    let Ok(text) = serde_json::to_string(&response) else {
        return;
    };
    PORTS.with(|ports| {
        for port in ports.borrow().iter() {
            let _ = port.post_message(&JsValue::from_str(&text));
        }
    });
}
