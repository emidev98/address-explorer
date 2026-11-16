//! Build the Tailwind stylesheet for every target, and the browser SharedWorker
//! for web builds.
//!
//! The stylesheet is generated into Cargo's `OUT_DIR` and embedded by the crate,
//! so debug, release, web, desktop, and server builds all ship the same theme.
//! The worker runs only for web builds; native desktop/mobile and pure-server
//! builds must not require clang or build the worker.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Tailwind release pinned for the stylesheet; keep it in step with the CLI
/// that `cargo prepare` installs.
const TAILWIND_VERSION: &str = "v4.1.5";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=crates/core");
    println!("cargo:rerun-if-changed=crates/web-worker");
    println!("cargo:rerun-if-changed=web/worker-entry.js");
    println!("cargo:rerun-if-changed=assets/pricing/btc_usd.bin");
    println!("cargo:rerun-if-changed=styles/tailwind.css");
    println!("cargo:rerun-if-changed=assets/material3-tailwind-palettes.css");
    println!("cargo:rerun-if-env-changed=TAILWIND_BIN");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=Cargo.lock");

    if let Err(error) = write_tailwind_stylesheet() {
        panic!("failed to build the Tailwind stylesheet: {error}");
    }

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

/// Compile `styles/tailwind.css` into `OUT_DIR/tailwind.css`.
///
/// The crate embeds that file, which is why this must run for every target and
/// profile before rustc expands the `include_str!` in `src/app.rs`.
fn write_tailwind_stylesheet() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let output = out_dir.join("tailwind.css");
    let temporary = out_dir.join("tailwind.css.tmp");

    if env::var_os("DOCS_RS").is_some() {
        fs::write(
            &output,
            "/* Tailwind styles are generated outside docs.rs. */\n",
        )?;
        return Ok(());
    }

    let input = root.join("styles").join("tailwind.css");

    if let Some(explicit) = env::var_os("TAILWIND_BIN") {
        let cli = PathBuf::from(explicit);
        run_tailwind(&cli, &root, &input, &temporary)?;
    } else {
        let mut failures = Vec::new();
        let mut built = false;
        for cli in tailwind_candidates() {
            match run_tailwind(&cli, &root, &input, &temporary) {
                Ok(()) => {
                    built = true;
                    break;
                }
                Err(error) => failures.push(format!("{}: {error}", cli.display())),
            }
        }
        if !built {
            return Err(format!(
                "no usable Tailwind CLI found. Run `cargo prepare`, set TAILWIND_BIN, \
                 or run a dx build once so dx downloads it.\n{}",
                failures.join("\n")
            )
            .into());
        }
    }

    let _ = fs::remove_file(&output);
    fs::rename(&temporary, &output)?;
    Ok(())
}

fn run_tailwind(cli: &Path, root: &Path, input: &Path, output: &Path) -> Result<(), String> {
    let result = Command::new(cli)
        .current_dir(root)
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .output()
        .map_err(|error| format!("{}: {error}", cli.display()))?;

    if !result.status.success() {
        let _ = fs::remove_file(output);
        let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("{cli:?} exited with {}", result.status)
        } else {
            stderr
        });
    }

    Ok(())
}

/// Preferred locations for the pinned CLI: the binary dx manages, then `PATH`.
fn tailwind_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(tools) = dx_tools_dir() {
        candidates.push(
            tools
                .join(format!("tailwindcss-{TAILWIND_VERSION}"))
                .join(tailwind_binary_name()),
        );
    }
    candidates.push(PathBuf::from(tailwind_binary_name()));
    candidates
}

fn tailwind_binary_name() -> &'static str {
    if cfg!(windows) {
        "tailwindcss.exe"
    } else {
        "tailwindcss"
    }
}

/// Mirror the directory the Dioxus CLI installs managed tools into.
fn dx_tools_dir() -> Option<PathBuf> {
    if let Some(home) = env::var_os("DX_HOME") {
        return Some(PathBuf::from(home).join("tools"));
    }

    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    let data_dir = if cfg!(any(target_os = "macos", target_os = "windows")) {
        PathBuf::from(home).join(".dx")
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&home).join(".local").join("share"))
            .join(".dx")
    };
    Some(data_dir.join("tools"))
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
    let clang = select_wasm_clang();
    command.env("CC", &clang);

    if let Some(archiver) = select_archiver() {
        command.env("AR", &archiver);
    } else {
        panic!(
            "the web SharedWorker needs an archiver. Install LLVM (for llvm-ar), put it on PATH, or set AR."
        );
    }
}

fn select_wasm_clang() -> OsString {
    if let Some(cc) = env::var_os("CC") {
        if is_wasm_clang(&cc) {
            return cc;
        }
        panic!(
            "CC is set to {}, but it cannot target wasm32. Unset CC or point it at a wasm-capable clang.",
            Path::new(&cc).display()
        );
    }

    for candidate in clang_candidates() {
        if is_wasm_clang(&candidate) {
            return candidate;
        }
    }

    panic!(
        "the web SharedWorker needs a wasm-capable clang. Install clang, lld, and LLVM, put `clang` on PATH, or set CC. On macOS, Homebrew LLVM is detected automatically."
    );
}

fn clang_candidates() -> Vec<OsString> {
    // The vector is only mutated on macOS and Windows, where a platform LLVM
    // is preferred over whatever `clang` happens to be first on PATH.
    #[allow(unused_mut)]
    let mut candidates = vec![OsString::from("clang")];

    #[cfg(target_os = "macos")]
    {
        candidates.insert(0, OsString::from("/usr/local/opt/llvm/bin/clang"));
        candidates.insert(0, OsString::from("/opt/homebrew/opt/llvm/bin/clang"));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            candidates.insert(
                0,
                PathBuf::from(program_files)
                    .join("LLVM")
                    .join("bin")
                    .join("clang.exe")
                    .into_os_string(),
            );
        }
    }

    candidates
}

fn is_wasm_clang(program: &std::ffi::OsStr) -> bool {
    Command::new(program)
        .arg("--print-targets")
        .output()
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("wasm32")
        })
        .unwrap_or(false)
}

fn select_archiver() -> Option<OsString> {
    if let Some(ar) = env::var_os("AR") {
        if command_works(&ar, &["--version"]) {
            return Some(ar);
        }
        panic!(
            "AR is set to {}, but it is not usable. Unset AR or point it at llvm-ar.",
            Path::new(&ar).display()
        );
    }

    if command_works(std::ffi::OsStr::new("llvm-ar"), &["--version"]) {
        return Some(OsString::from("llvm-ar"));
    }

    for candidate in versioned_llvm_ars() {
        if command_works(&candidate, &["--version"]) {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "macos")]
    {
        for prefix in ["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"] {
            let candidate = PathBuf::from(prefix).join("llvm-ar");
            if command_works(&candidate, &["--version"]) {
                return Some(candidate.into_os_string());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(program_files) = env::var_os("ProgramFiles") {
            let candidate = PathBuf::from(program_files)
                .join("LLVM")
                .join("bin")
                .join("llvm-ar.exe");
            if command_works(&candidate, &["--version"]) {
                return Some(candidate.into_os_string());
            }
        }
    }

    if command_works(std::ffi::OsStr::new("ar"), &["--version"]) {
        return Some(OsString::from("ar"));
    }

    None
}

fn versioned_llvm_ars() -> Vec<OsString> {
    let mut found = Vec::new();
    let Some(path) = env::var_os("PATH") else {
        return found;
    };

    for directory in env::split_paths(&path) {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if name.starts_with("llvm-ar-") {
                found.push(entry.path().into_os_string());
            }
        }
    }

    found.sort();
    found.reverse();
    found
}

fn command_works(program: &std::ffi::OsStr, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
