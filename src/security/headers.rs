//! Security response headers for the bundled server and the static PWA.

use dioxus::server::axum::{
    extract::Request,
    http::{header, HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

const CSP: &str = "default-src 'self'; \
script-src 'self' 'wasm-unsafe-eval' 'unsafe-eval'; \
style-src 'self' 'unsafe-inline'; \
img-src 'self' data:; \
font-src 'self'; \
connect-src 'self' https: http://localhost:* http://127.0.0.1:*; \
worker-src 'self' blob:; \
child-src 'self' blob:; \
frame-ancestors 'none'; \
base-uri 'self'; \
form-action 'self'; \
object-src 'none'";

/// Apply the shared browser hardening headers to every response.
pub async fn layer(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    insert(headers, header::CONTENT_SECURITY_POLICY, CSP);
    insert(headers, header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    insert(headers, header::X_FRAME_OPTIONS, "DENY");
    insert(headers, header::REFERRER_POLICY, "no-referrer");
    insert(
        headers,
        HeaderName::from_static("permissions-policy"),
        "camera=(), microphone=(), geolocation=(), payment=(), usb=(), interest-cohort=()",
    );
    insert(
        headers,
        HeaderName::from_static("cross-origin-opener-policy"),
        "same-origin",
    );
    if path.contains("/api/") {
        insert(headers, header::CACHE_CONTROL, "no-store");
    }
    if std::env::var("ADDRESS_EXPLORER_HSTS").as_deref() == Ok("1") {
        insert(
            headers,
            header::STRICT_TRANSPORT_SECURITY,
            "max-age=63072000; includeSubDomains",
        );
    }
    response
}

fn insert(headers: &mut dioxus::server::axum::http::HeaderMap, name: HeaderName, value: &str) {
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name, value);
    }
}
