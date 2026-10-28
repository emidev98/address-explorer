//! Application entry point and composition root: module wiring, one-time
//! database initialization, and the Dioxus launch.

mod api;
mod app;
#[cfg(feature = "server")]
mod application;
mod clients;
mod components;
#[cfg(feature = "server")]
mod data;
mod domain;
mod screens;

fn main() {
    #[cfg(feature = "server")]
    {
        if let Err(error) = crate::data::Database::open_default() {
            eprintln!("database initialization failed: {error}");
        }
    }

    dioxus::launch(app::App);
}
