//! Application entry point and composition root: module wiring, one-time
//! backend/database initialization, and the Dioxus launch.
mod api;
mod app;
#[cfg(feature = "server")]
mod application;
mod backend;
mod clients;
mod components;
#[cfg(feature = "server")]
mod data;
mod domain;
mod navigation;
mod screens;
#[cfg(all(target_arch = "wasm32", feature = "web-client"))]
pub mod web;

fn main() {
    api::version::install_client_header();
    backend::init();

    #[cfg(feature = "server")]
    if backend::is_local() {
        if let Err(error) = crate::data::Database::open_default() {
            eprintln!("database initialization failed: {error}");
        }
    }

    #[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
    {
        dioxus::serve(|| async move {
            let router = dioxus::server::router(app::Root)
                .layer(dioxus::server::axum::extract::DefaultBodyLimit::disable())
                .layer(dioxus::server::axum::middleware::from_fn(
                    api::version::guard,
                ))
                .layer(api::cors::layer());
            Ok(router)
        });
    }

    #[cfg(any(feature = "desktop", feature = "mobile", not(feature = "server")))]
    {
        dioxus::launch(app::Root);
    }
}
