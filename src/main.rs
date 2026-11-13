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
#[cfg(feature = "server")]
mod security;
#[cfg(all(target_arch = "wasm32", feature = "web-client"))]
pub mod web;

fn main() {
    #[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
    if handle_cli() {
        return;
    }

    api::version::install_client_header();
    backend::init();

    #[cfg(feature = "server")]
    security::hardening::apply();

    #[cfg(feature = "server")]
    if backend::is_local() {
        if let Err(error) = crate::data::vault::init() {
            eprintln!("database initialization failed: {error}");
        }
    }

    #[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
    {
        run_pure_server();
    }

    #[cfg(any(feature = "desktop", feature = "mobile", not(feature = "server")))]
    {
        dioxus::launch(app::Root);
    }
}

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
fn run_pure_server() -> ! {
    let address = match server_address() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("server configuration error: {error}");
            std::process::exit(1);
        }
    };

    if address.ip().is_loopback() {
        dioxus::serve(|| async move { Ok(build_router()) });
    }

    if let Err(error) = security::tls::ensure() {
        eprintln!("TLS initialization failed: {error}");
        std::process::exit(1);
    }
    if let Err(error) = security::tls::serve(build_router(), address) {
        eprintln!("server failed: {error}");
        std::process::exit(1);
    }
    std::process::exit(0);
}

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
fn build_router() -> dioxus::server::axum::Router {
    use dioxus::server::axum::{extract::DefaultBodyLimit, middleware};

    dioxus::server::router(app::Root)
        .layer(DefaultBodyLimit::max(
            security::body_limit::MAX_BACKUP_REQUEST_BYTES,
        ))
        .layer(middleware::from_fn(api::version::guard))
        .layer(middleware::from_fn(security::body_limit::layer))
        .layer(middleware::from_fn(security::session::guard))
        .layer(middleware::from_fn(security::rate_limit::layer))
        .layer(middleware::from_fn(security::headers::layer))
        .layer(api::cors::layer())
}

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
fn handle_cli() -> bool {
    let Some(command) = std::env::args().nth(1) else {
        return false;
    };
    match command.as_str() {
        "reset-vault" => {
            if let Err(error) = reset_vault_cli() {
                eprintln!("reset-vault failed: {error}");
                std::process::exit(1);
            }
            true
        }
        "--help" | "-h" | "help" => {
            println!(
                "Address Explorer server\n\n\
                 Usage:\n  address-explorer                 start the HTTPS server\n  address-explorer reset-vault     erase the locked database on this host\n\n\
                 reset-vault is the only lost-password recovery path and must be run while the server is stopped."
            );
            true
        }
        _ => false,
    }
}

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
fn reset_vault_cli() -> Result<(), String> {
    use std::io::Write;

    let path = data::Database::default_path().map_err(|error| error.to_string())?;
    if !path.exists() {
        println!("No database exists at {}.", path.display());
        return Ok(());
    }
    println!(
        "This permanently erases the encrypted database at {}.\nType ERASE to continue:",
        path.display()
    );
    let _ = std::io::stdout().flush();
    let mut confirmation = String::new();
    std::io::stdin()
        .read_line(&mut confirmation)
        .map_err(|error| format!("could not read confirmation: {error}"))?;
    if confirmation.trim() != "ERASE" {
        return Err("confirmation did not match ERASE".to_string());
    }
    security::files::best_effort_overwrite_and_remove(&path);
    if let Some(parent) = path.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let candidate = entry.path();
                let is_temp = candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".address-explorer-"));
                if is_temp {
                    security::files::best_effort_overwrite_and_remove(&candidate);
                }
            }
        }
    }
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        let _ = std::fs::remove_file(std::path::PathBuf::from(sidecar));
    }
    println!(
        "The database was erased. Start the server and set a new password before exposing it."
    );
    Ok(())
}

#[cfg(all(feature = "server", not(any(feature = "desktop", feature = "mobile"))))]
fn server_address() -> Result<std::net::SocketAddr, String> {
    let ip = std::env::var("IP").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let address = if ip.contains(':') && !ip.starts_with('[') {
        format!("[{ip}]:{port}")
    } else {
        format!("{ip}:{port}")
    };
    address
        .parse()
        .map_err(|error| format!("invalid IP/PORT ({ip}:{port}): {error}"))
}
