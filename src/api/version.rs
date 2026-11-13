//! API contract versioning.
//!
//! The client sends [`API_VERSION_HEADER`] on every server-function request.
//! The server rejects mismatched clients with HTTP 426, except for the
//! version-discovery endpoint `GET /api/health`.
#[cfg(not(target_arch = "wasm32"))]
use crate::domain::health::{API_VERSION, API_VERSION_HEADER};

/// Install the client-side API version header used by Dioxus server-function
/// requests. Must run before the first remote call or startup health check.
pub fn install_client_header() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use dioxus::fullstack::HeaderMap;

        let mut headers = HeaderMap::new();
        if let Ok(value) = dioxus::fullstack::http::HeaderValue::from_str(&API_VERSION.to_string())
        {
            headers.insert(
                dioxus::fullstack::http::HeaderName::from_static(API_VERSION_HEADER),
                value,
            );
        }
        dioxus::fullstack::set_request_headers(headers);
    }
}
#[cfg(feature = "server")]
#[allow(dead_code)]
pub async fn guard(
    request: dioxus::server::axum::extract::Request,
    next: dioxus::server::axum::middleware::Next,
) -> dioxus::server::axum::response::Response {
    use dioxus::server::axum::{http::StatusCode, response::IntoResponse, Json};
    use serde::Serialize;

    let path = request.uri().path();
    let is_api = path.starts_with("/api/") || path.contains("/api/");
    if !is_api
        || path.ends_with("/api/health")
        || path.ends_with("/api/tls/ca")
        || request.method() == dioxus::server::axum::http::Method::OPTIONS
    {
        return next.run(request).await;
    }

    let client_version = request
        .headers()
        .get(API_VERSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u32>().ok());

    if client_version == Some(API_VERSION) {
        return next.run(request).await;
    }

    #[derive(Serialize)]
    struct ApiVersionError {
        message: String,
        code: u16,
        data: serde_json::Value,
    }

    let message = match client_version {
        Some(client) => format!("API version mismatch: client {client}, server {API_VERSION}"),
        None => format!(
            "API version header is missing or invalid; this server requires API {API_VERSION}"
        ),
    };

    let body = ApiVersionError {
        message,
        code: StatusCode::UPGRADE_REQUIRED.as_u16(),
        data: serde_json::json!({
            "client_api_version": client_version,
            "server_api_version": API_VERSION,
        }),
    };

    (StatusCode::UPGRADE_REQUIRED, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_header_value_is_stable() {
        assert!(!API_VERSION_HEADER.is_empty());
    }
}

#[cfg(all(test, feature = "server"))]
mod middleware_tests {
    use crate::domain::health::{API_VERSION, API_VERSION_HEADER};
    use dioxus::server::axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware,
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn response_status(uri: &str, version: Option<&str>) -> StatusCode {
        let app = Router::new()
            .route("/api/health", get(|| async { "health" }))
            .route("/api/test", get(|| async { "ok" }))
            .layer(middleware::from_fn(super::guard));

        let mut builder = Request::builder().uri(uri);
        if let Some(version) = version {
            builder = builder.header(API_VERSION_HEADER, version);
        }
        let request = builder.body(Body::empty()).expect("request");
        app.oneshot(request).await.expect("response").status()
    }

    #[tokio::test]
    async fn missing_version_header_is_rejected() {
        assert_eq!(
            response_status("/api/test", None).await,
            StatusCode::UPGRADE_REQUIRED
        );
    }

    #[tokio::test]
    async fn wrong_version_header_is_rejected() {
        assert_eq!(
            response_status("/api/test", Some("999")).await,
            StatusCode::UPGRADE_REQUIRED
        );
    }

    #[tokio::test]
    async fn matching_version_header_is_accepted() {
        let version = API_VERSION.to_string();
        assert_eq!(
            response_status("/api/test", Some(&version)).await,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn health_is_exempt() {
        assert_eq!(response_status("/api/health", None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn options_preflight_is_not_blocked_by_the_version_guard() {
        use dioxus::server::axum::routing::options;

        let app = Router::new()
            .route("/api/test", options(|| async { "preflight" }))
            .layer(middleware::from_fn(super::guard));
        let request = Request::builder()
            .method("OPTIONS")
            .uri("/api/test")
            .body(Body::empty())
            .expect("request");
        let status = app.oneshot(request).await.expect("response").status();
        assert_ne!(status, StatusCode::UPGRADE_REQUIRED);
    }
}
