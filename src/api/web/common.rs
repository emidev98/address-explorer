//! Shared web API dispatch: browser worker RPC in Local mode and full-URL
//! HTTP in Remote mode.

use dioxus_fullstack_core::ServerFnError;
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub struct RemoteRequest<'a> {
    pub method: Method,
    pub path: &'a str,
    pub query: Option<Value>,
    pub body: Option<Value>,
}

pub async fn call<T: DeserializeOwned>(
    endpoint: &str,
    payload: Value,
    remote: RemoteRequest<'_>,
) -> Result<T, ServerFnError> {
    if crate::backend::is_remote() {
        crate::api::remote::call(remote.method, remote.path, remote.query, remote.body).await
    } else {
        crate::web::worker::call(endpoint, payload).await
    }
}

#[macro_export]
macro_rules! web_post_body {
    ($endpoint:literal, $path:expr, $args:tt) => {{
        let payload = serde_json::json!($args);
        $crate::api::web::common::call(
            $endpoint,
            payload.clone(),
            $crate::api::web::common::RemoteRequest {
                method: reqwest::Method::POST,
                path: $path,
                query: None,
                body: Some(payload),
            },
        )
        .await
    }};
}

#[macro_export]
macro_rules! web_get_query {
    ($endpoint:literal, $path:expr, $args:tt) => {{
        let payload = serde_json::json!($args);
        $crate::api::web::common::call(
            $endpoint,
            payload.clone(),
            $crate::api::web::common::RemoteRequest {
                method: reqwest::Method::GET,
                path: $path,
                query: Some(payload),
                body: None,
            },
        )
        .await
    }};
}

#[macro_export]
macro_rules! web_put_body {
    ($endpoint:literal, $path:expr, $args:tt) => {{
        let payload = serde_json::json!($args);
        $crate::api::web::common::call(
            $endpoint,
            payload.clone(),
            $crate::api::web::common::RemoteRequest {
                method: reqwest::Method::PUT,
                path: $path,
                query: None,
                body: Some(payload),
            },
        )
        .await
    }};
}

#[macro_export]
macro_rules! web_delete_query {
    ($endpoint:literal, $path:expr, $args:tt) => {{
        let payload = serde_json::json!($args);
        $crate::api::web::common::call(
            $endpoint,
            payload.clone(),
            $crate::api::web::common::RemoteRequest {
                method: reqwest::Method::DELETE,
                path: $path,
                query: Some(payload),
                body: None,
            },
        )
        .await
    }};
}

#[macro_export]
macro_rules! web_post_query {
    ($endpoint:literal, $path:expr, $args:tt) => {{
        let payload = serde_json::json!($args);
        $crate::api::web::common::call(
            $endpoint,
            payload.clone(),
            $crate::api::web::common::RemoteRequest {
                method: reqwest::Method::POST,
                path: $path,
                query: Some(payload),
                body: None,
            },
        )
        .await
    }};
}
