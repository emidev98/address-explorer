//! API controllers: the `#[server]` entry points of the application.
//!
//! Each module groups the endpoints of one domain. Controllers are thin by
//! contract: they open the database, call
//! exactly one application service operation, and translate errors into
//! `ServerFnError`. They contain no business rules and no SQL.
#[cfg(feature = "server")]
use crate::data::Database;
#[cfg(feature = "server")]
use dioxus::prelude::ServerFnError;

#[macro_use]
pub(crate) mod macros;

#[cfg(all(feature = "web-client", target_arch = "wasm32"))]
pub mod web;

#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/addresses.rs"
)]
pub mod addresses;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/catalogs.rs"
)]
pub mod catalogs;
#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
pub mod cors;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/data.rs"
)]
pub mod data;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/exchanges.rs"
)]
pub mod exchanges;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/flow.rs"
)]
pub mod flow;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/health.rs"
)]
pub mod health;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/prices.rs"
)]
pub mod prices;
#[cfg(any(
    all(feature = "remote-backend", not(target_arch = "wasm32")),
    all(feature = "web-client", target_arch = "wasm32")
))]
pub mod remote;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/settings.rs"
)]
pub mod settings;
#[cfg_attr(
    all(feature = "web-client", target_arch = "wasm32"),
    path = "web/transactions.rs"
)]
pub mod transactions;
pub mod version;
/// Open the application database for one request.
#[cfg(feature = "server")]
pub(crate) fn open_database() -> Result<Database, ServerFnError> {
    Database::open_default().map_err(|error| ServerFnError::new(error.to_string()))
}

#[cfg(all(test, feature = "remote-backend", not(target_arch = "wasm32")))]
mod remote_forward_tests {
    use crate::backend::{BackendConfig, BackendMode};
    use crate::domain::theme::{ThemeMode, ThemePalette, ThemePreference};
    use dioxus::server::axum::{
        routing::{get, post},
        Json, Router,
    };
    use serde_json::{json, Value};

    async fn mock_server() -> String {
        let app = Router::new()
            .route(
                "/api/settings/theme",
                get(|| async {
                    Json(ThemePreference {
                        palette: ThemePalette::Forest,
                        mode: ThemeMode::Light,
                    })
                }),
            )
            .route(
                "/api/addresses/validate",
                post(|Json(body): Json<Value>| async move {
                    assert_eq!(body, json!({ "address": "abc" }));
                    Json(body["address"].clone())
                }),
            )
            .route("/api/data/export", post(|| async { Json("sqlite-backup") }))
            .route(
                "/api/data/import",
                post(|Json(body): Json<Value>| async move {
                    assert_eq!(body, json!({ "file": "c3FsaXRl" }));
                    Json(Value::Null)
                }),
            )
            .route("/api/data/clear", post(|| async { Json(Value::Null) }));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let address = listener.local_addr().expect("mock server address");
        tokio::spawn(async move {
            let _ = dioxus::server::axum::serve(listener, app).await;
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn server_functions_forward_to_the_remote_backend() {
        let base = mock_server().await;
        crate::backend::set_config(BackendConfig {
            version: 1,
            mode: BackendMode::Remote,
            base_url: Some(base),
        });

        let theme = crate::api::settings::get_theme_api()
            .await
            .expect("remote theme");
        assert_eq!(theme.palette, ThemePalette::Forest);
        assert_eq!(theme.mode, ThemeMode::Light);

        let validated = crate::api::addresses::validate_address_api("abc".to_string())
            .await
            .expect("remote validation");
        assert_eq!(validated, "abc");

        let exported = crate::api::data::export_data_api()
            .await
            .expect("remote export");
        assert_eq!(exported, "sqlite-backup");
        crate::api::data::import_data_api("c3FsaXRl".to_string())
            .await
            .expect("remote import");
        crate::api::data::clear_data_api()
            .await
            .expect("remote clear");

        crate::backend::set_config(BackendConfig::default());
    }
}
