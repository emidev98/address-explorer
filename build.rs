//! Build the browser SharedWorker and copy the stable web assets.
//!
//! This runs only for web builds. Native desktop/mobile and pure-server builds
//! must not require clang or build the worker.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/domain");
    println!("cargo:rerun-if-changed=src/data");
    println!("cargo:rerun-if-changed=src/application");
    println!("cargo:rerun-if-changed=src/clients");
    println!("cargo:rerun-if-changed=crates/core");
    println!("cargo:rerun-if-changed=crates/web-worker");
    println!("cargo:rerun-if-changed=web/worker-entry.js");
    println!("cargo:rerun-if-changed=assets/pricing/btc_usd.bin");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    if env::var("TARGET")
        .map(|target| !target.starts_with("wasm32"))
        .unwrap_or(true)
    {
        return;
    }
    if env::var_os("CARGO_FEATURE_WEB").is_none() {
        return;
    }
    if env::var_os("DOCS_RS").is_some() {
        return;
    }

    if let Err(error) = build_worker() {
        panic!("failed to build the web SharedWorker: {error}");
    }
}

fn build_worker() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let public = root.join("public");
    let target_dir = root.join("target").join("web-worker-build");

    let mut cargo = Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    cargo
        .current_dir(&root)
        .args([
            "build",
            "-p",
            "address-explorer-web-worker",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--locked",
        ])
        .env("CARGO_TARGET_DIR", &target_dir)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    configure_cc(&mut cargo);
    let status = cargo.status()?;
    if !status.success() {
        return Err(format!("cargo build for the worker exited with {status}").into());
    }

    let wasm = target_dir
        .join("wasm32-unknown-unknown")
        .join("release")
        .join("address_explorer_web_worker.wasm");
    if !wasm.is_file() {
        return Err(format!("worker WASM not found at {}", wasm.display()).into());
    }

    let worker_dir = public.join("worker");
    if worker_dir.exists() {
        fs::remove_dir_all(&worker_dir)?;
    }
    fs::create_dir_all(&worker_dir)?;

    let mut bindgen = wasm_bindgen_cli_support::Bindgen::new();
    bindgen
        .input_path(&wasm)
        .out_name("address_explorer_web_worker")
        .no_modules(true)?;
    bindgen.generate(&worker_dir)?;

    fs::copy(
        root.join("web").join("worker-entry.js"),
        public.join("local-worker.js"),
    )?;
    fs::copy(
        root.join("assets").join("pricing").join("btc_usd.bin"),
        public.join("btc_usd.bin"),
    )?;

    Ok(())
}

fn configure_cc(command: &mut Command) {
    if env::var_os("CC").is_none() {
        if command_works("clang", &["--version"]) {
            command.env("CC", "clang");
        } else {
            panic!(
                "the web SharedWorker needs a wasm-capable clang. Install clang and lld, put `clang` on PATH, or set CC."
            );
        }
    }
    if env::var_os("AR").is_none() {
        for candidate in ["llvm-ar", "ar"] {
            if command_works(candidate, &["--version"]) {
                command.env("AR", candidate);
                break;
            }
        }
    }
}

fn command_works(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
