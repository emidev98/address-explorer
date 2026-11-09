//! SharedWorker client used by the browser PWA.
//!
//! One `SharedWorker` is shared by every tab on the origin. Each tab owns a
//! `MessagePort`; requests are correlated with monotonic ids and responses are
//! delivered through oneshot channels.

use address_explorer_core::{
    WorkerHello, WorkerReady, WorkerRequest, WorkerResponse, WORKER_PROTOCOL_VERSION,
};
use dioxus_fullstack_core::ServerFnError;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, MessagePort, SharedWorker};

thread_local! {
    static CLIENT: RefCell<Option<WorkerClient>> = const { RefCell::new(None) };
    static READY: RefCell<Option<WorkerReady>> = const { RefCell::new(None) };
}

type PendingMap =
    std::rc::Rc<RefCell<HashMap<u64, tokio::sync::oneshot::Sender<Result<Value, String>>>>>;

struct WorkerClient {
    port: MessagePort,
    next_id: u64,
    pending: PendingMap,
}

impl WorkerClient {
    fn connect() -> Result<Self, String> {
        let worker = SharedWorker::new("/local-worker.js")
            .map_err(|error| format!("the browser-local worker could not start: {error:?}"))?;
        let port = worker.port();
        let pending: PendingMap = std::rc::Rc::new(RefCell::new(HashMap::new()));
        let pending_for_error = std::rc::Rc::clone(&pending);
        let onerror = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(
            move |event: web_sys::ErrorEvent| {
                let message = event.message();
                for (_, sender) in pending_for_error.borrow_mut().drain() {
                    let _ = sender.send(Err(format!(
                    "the browser-local worker failed: {message}. Rebuild the web app with `dx serve` or `dx build --web`."
                )));
                }
            },
        );
        worker.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
        let pending_for_message = std::rc::Rc::clone(&pending);
        let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            let Ok(response) = serde_json::from_str::<WorkerResponse>(&text) else {
                return;
            };
            if response.id == 0 {
                // A write happened in another tab. Reload so this tab refetches
                // from the shared database instead of showing stale resources.
                if response.ok {
                    if let Some(window) = web_sys::window() {
                        let _ = window.location().reload();
                    }
                }
                return;
            }
            if let Some(sender) = pending_for_message.borrow_mut().remove(&response.id) {
                let value = if response.ok {
                    Ok(response.result.unwrap_or(Value::Null))
                } else {
                    Err(response.error.unwrap_or_else(|| {
                        "the browser-local worker returned an unknown error".to_string()
                    }))
                };
                let _ = sender.send(value);
            }
        });
        port.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
        port.start();
        Ok(Self {
            port,
            next_id: 1,
            pending,
        })
    }

    fn submit(
        &mut self,
        endpoint: &str,
        payload: Value,
    ) -> Result<tokio::sync::oneshot::Receiver<Result<Value, String>>, String> {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.pending.borrow_mut().insert(id, sender);
        let request = WorkerRequest {
            id,
            endpoint: endpoint.to_string(),
            payload,
        };
        let text = serde_json::to_string(&request)
            .map_err(|error| format!("could not encode the worker request: {error}"))?;
        self.port
            .post_message(&JsValue::from_str(&text))
            .map_err(|error| format!("could not send the worker request: {error:?}"))?;
        Ok(receiver)
    }
}

fn with_client<T>(
    action: impl FnOnce(&mut WorkerClient) -> Result<T, String>,
) -> Result<T, String> {
    CLIENT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(WorkerClient::connect()?);
        }
        action(slot.as_mut().expect("worker client initialized"))
    })
}

/// Initialize the worker and wait until the browser database is ready.
pub async fn init(seed_url: &str) -> Result<WorkerReady, String> {
    if let Some(ready) = READY.with(|ready| ready.borrow().clone()) {
        return Ok(ready);
    }

    let app_version = env!("CARGO_PKG_VERSION").to_string();
    let receiver = with_client(|client| {
        client.submit(
            "hello",
            serde_json::to_value(WorkerHello {
                protocol_version: WORKER_PROTOCOL_VERSION,
                app_version,
                seed_url: seed_url.to_string(),
                reset: false,
            })
            .map_err(|error| format!("could not encode the worker hello: {error}"))?,
        )
    })?;

    let value = receiver
        .await
        .map_err(|_| "the browser-local worker stopped before replying".to_string())?
        .map_err(|error| format!("the browser-local database could not start: {error}"))?;
    let ready: WorkerReady = serde_json::from_value(value)
        .map_err(|error| format!("invalid worker ready response: {error}"))?;
    if ready.protocol_version != WORKER_PROTOCOL_VERSION {
        return Err(format!(
            "worker protocol {} does not match this app ({}); reload the application",
            ready.protocol_version, WORKER_PROTOCOL_VERSION
        ));
    }
    READY.with(|slot| *slot.borrow_mut() = Some(ready.clone()));
    Ok(ready)
}

/// Call one endpoint in the browser-local backend.
pub async fn call<T: DeserializeOwned>(endpoint: &str, payload: Value) -> Result<T, ServerFnError> {
    let receiver =
        with_client(|client| client.submit(endpoint, payload)).map_err(ServerFnError::new)?;
    let value = receiver
        .await
        .map_err(|_| ServerFnError::new("the browser-local worker stopped before replying"))?
        .map_err(ServerFnError::new)?;
    serde_json::from_value(value).map_err(|error| {
        ServerFnError::new(format!(
            "the browser-local worker returned an invalid response for {endpoint}: {error}"
        ))
    })
}

/// Whether the browser-local database has completed its handshake.
pub fn is_ready() -> bool {
    READY.with(|ready| ready.borrow().is_some())
}
