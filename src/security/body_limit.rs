//! Per-endpoint request-body limits.
//!
//! The middleware buffers API bodies so chunked requests cannot bypass the
//! limit, then reconstructs the request for the generated server functions.

use crate::security::policy;
use dioxus::server::axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

/// Hard ceiling accepted by the axum extractor. The middleware still applies
/// the smaller configured limit for every request.
pub const MAX_BACKUP_REQUEST_BYTES: usize = 720 * 1024 * 1024;

pub async fn layer(request: Request, next: Next) -> Response {
    if !request.uri().path().contains("/api/")
        || matches!(
            *request.method(),
            Method::GET | Method::HEAD | Method::OPTIONS
        )
    {
        return next.run(request).await;
    }
    let is_import = request.uri().path().ends_with("/api/data/import")
        || request.uri().path().ends_with("/api/data/import-encrypted");
    let settings = policy::current();
    let limit = if is_import {
        settings.max_backup_request_bytes() as usize
    } else {
        settings.max_json_body_bytes as usize
    }
    .min(MAX_BACKUP_REQUEST_BYTES);

    if let Some(length) = request
        .headers()
        .get(dioxus::server::axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
    {
        if length > limit as u64 {
            return too_large(limit);
        }
    }

    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, limit).await {
        Ok(bytes) => bytes,
        Err(_) => return too_large(limit),
    };
    let request = Request::from_parts(parts, Body::from(bytes));
    next.run(request).await
}

fn too_large(limit: usize) -> Response {
    let body = Json(serde_json::json!({
        "message": format!("request body exceeds the {limit}-byte limit"),
        "code": StatusCode::PAYLOAD_TOO_LARGE.as_u16(),
        "data": null,
    }));
    (StatusCode::PAYLOAD_TOO_LARGE, body).into_response()
}
