//! CORS policy for the pure server.
//!
//! The remote web PWA is a cross-origin client. Allowed origins must be
//! configured explicitly; there is no permissive wildcard default.

/// Environment variable with a comma-separated list of allowed origins.
pub const CORS_ORIGINS_ENV: &str = "ADDRESS_EXPLORER_CORS_ORIGINS";

/// Build the CORS layer from `ADDRESS_EXPLORER_CORS_ORIGINS`.
///
/// An unset or empty variable leaves cross-origin requests without CORS
/// headers, so only the same origin (for example a PWA served by this server)
/// is allowed.
pub fn layer() -> tower_http::cors::CorsLayer {
    use dioxus::server::axum::http::{HeaderName, HeaderValue, Method};
    use tower_http::cors::{AllowOrigin, CorsLayer};

    let origins = std::env::var(CORS_ORIGINS_ENV)
        .ok()
        .into_iter()
        .flat_map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .filter_map(|origin| match HeaderValue::from_str(origin) {
                    Ok(value) => Some(value),
                    Err(error) => {
                        eprintln!("ignoring invalid {CORS_ORIGINS_ENV} origin {origin}: {error}");
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let mut layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static(crate::domain::health::API_VERSION_HEADER),
        ]);

    if !origins.is_empty() {
        layer = layer.allow_origin(AllowOrigin::list(origins));
    }
    layer
}
