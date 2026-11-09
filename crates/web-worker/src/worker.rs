//! SharedWorker bootstrap and sequential request loop.

use crate::db::BrowserDatabase;
use crate::rpc;
use address_explorer_core::{
    DataChanged, WorkerHello, WorkerReady, WorkerRequest, WorkerResponse, WORKER_PROTOCOL_VERSION,
};
use std::cell::RefCell;
use tokio::sync::mpsc::UnboundedReceiver;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, MessagePort};

struct State {
    db: Option<BrowserDatabase>,
    seed_url: String,
    app_version: String,
    revision: u64,
}

impl Default for State {
    fn default() -> Self {
        Self {
            db: None,
            seed_url: String::new(),
            app_version: "unknown".to_string(),
            revision: 0,
        }
    }
}

thread_local! {
    static PORTS: RefCell<Vec<MessagePort>> = const { RefCell::new(Vec::new()) };
    static SENDER: RefCell<Option<tokio::sync::mpsc::UnboundedSender<(MessagePort, String)>>> =
        const { RefCell::new(None) };
}

pub fn start() -> Result<(), JsValue> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!(
            "address-explorer worker panic: {info}"
        )));
    }));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<(MessagePort, String)>();
    SENDER.with(|sender| *sender.borrow_mut() = Some(tx));
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

async fn run(mut rx: UnboundedReceiver<(MessagePort, String)>) {
    let mut state = State::default();
    while let Some((port, text)) = rx.recv().await {
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
        if state.db.is_none() {
            match BrowserDatabase::open(&hello.seed_url, hello.reset).await {
                Ok(db) => state.db = Some(db),
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

    if request.endpoint == "data.import" || request.endpoint == "data.clear" {
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

async fn replace_local_database(
    state: &mut State,
    endpoint: &str,
    payload: serde_json::Value,
) -> Result<(), String> {
    use base64::Engine;

    let imported = endpoint == "data.import";
    // Decode and validate the payload before detaching the live database so a
    // malformed request cannot leave Local mode without a database.
    let import_bytes = if imported {
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
        Some(bytes)
    } else {
        None
    };

    let Some(old) = state.db.take() else {
        return Err("the browser database is not ready".to_string());
    };
    let seed_url = state.seed_url.clone();
    // Keep a copy for both destructive operations. A rejected import, a failed
    // VFS write, or a failed reopen must never leave the browser without its
    // previous data.
    let backup = match old.export() {
        Ok(bytes) => bytes,
        Err(error) => {
            state.db = Some(old);
            return Err(error);
        }
    };
    let BrowserDatabase { database, util } = old;
    drop(database);

    if let Some(bytes) = import_bytes {
        let delete = match util.delete_db(address_explorer_core::WEB_DATABASE_FILE) {
            Ok(delete) => delete,
            Err(error) => {
                return restore_local_database(
                    state,
                    &util,
                    &backup,
                    &seed_url,
                    format!("could not replace the browser database: {error}"),
                )
                .await;
            }
        };
        if let Err(error) = delete.await {
            return restore_local_database(
                state,
                &util,
                &backup,
                &seed_url,
                format!("could not replace the browser database: {error}"),
            )
            .await;
        }
        let import = match util.import_db(address_explorer_core::WEB_DATABASE_FILE, &bytes) {
            Ok(import) => import,
            Err(error) => {
                return restore_local_database(
                    state,
                    &util,
                    &backup,
                    &seed_url,
                    format!("could not import the SQLite file: {error}"),
                )
                .await;
            }
        };
        if let Err(error) = import.await {
            return restore_local_database(
                state,
                &util,
                &backup,
                &seed_url,
                format!("could not persist the SQLite file: {error}"),
            )
            .await;
        }
    } else {
        let clear = match util.clear_all() {
            Ok(clear) => clear,
            Err(error) => {
                return restore_local_database(
                    state,
                    &util,
                    &backup,
                    &seed_url,
                    format!("could not clear the browser database: {error}"),
                )
                .await;
            }
        };
        if let Err(error) = clear.await {
            return restore_local_database(
                state,
                &util,
                &backup,
                &seed_url,
                format!("could not clear the browser database: {error}"),
            )
            .await;
        }
    }

    let opened = if imported {
        BrowserDatabase::open_imported(&seed_url).await
    } else {
        BrowserDatabase::open(&seed_url, false).await
    };
    match opened {
        Ok(db) => {
            state.db = Some(db);
            Ok(())
        }
        Err(error) => restore_local_database(state, &util, &backup, &seed_url, error).await,
    }
}

async fn restore_local_database(
    state: &mut State,
    util: &sqlite_wasm_vfs::relaxed_idb::RelaxedIdbUtil,
    backup: &[u8],
    seed_url: &str,
    error: String,
) -> Result<(), String> {
    if let Ok(wait) = util.import_db(address_explorer_core::WEB_DATABASE_FILE, backup) {
        let _ = wait.await;
    }
    if let Ok(db) = BrowserDatabase::open(seed_url, false).await {
        state.db = Some(db);
    }
    Err(error)
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
