use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::context::{run_output, which, CommandSpec, Context, Target, WebTools};
use crate::system::{linux, macos, windows};

pub fn ensure(ctx: &mut Context) {
    let clang = find_clang(ctx);
    let mut archiver = find_archiver(ctx);
    let wasm_ld = find_wasm_ld(ctx);

    if archiver.is_none() && ctx.platform.is_linux() && !ctx.opts.check {
        match linux::install_extra(ctx, &["llvm"]) {
            Ok(()) => {
                archiver = if ctx.opts.dry_run {
                    Some(PathBuf::from("llvm-ar"))
                } else {
                    find_archiver(ctx)
                };
            }
            Err(error) => {
                ctx.failed(
                    Target::Web,
                    "Web wasm archiver",
                    format!("{error}\n       retry: install LLVM (llvm-ar)"),
                );
                return;
            }
        }
    }

    let Some(clang) = clang else {
        let mut message = if ctx.opts.check {
            "no wasm-capable clang found; install clang/lld/LLVM and put clang on PATH".to_string()
        } else {
            "no wasm-capable clang found after installing packages; install clang/lld/LLVM, then re-run".to_string()
        };
        if let Some(cc) = env::var_os("CC") {
            message.push_str(&format!(
                "\n       CC is set to {}; unset it or point it at a wasm-capable clang",
                Path::new(&cc).display()
            ));
        }
        if ctx.opts.check {
            ctx.missing(Target::Web, "Web wasm toolchain", message);
        } else {
            ctx.failed(Target::Web, "Web wasm toolchain", message);
        }
        return;
    };

    let Some(archiver) = archiver else {
        let mut message = "no archiver found; install LLVM (llvm-ar) or set AR".to_string();
        if let Some(ar) = env::var_os("AR") {
            message.push_str(&format!(
                "\n       AR is set to {}; unset it or point it at llvm-ar",
                Path::new(&ar).display()
            ));
        }
        if ctx.opts.check {
            ctx.missing(Target::Web, "Web wasm archiver", message);
        } else {
            ctx.failed(Target::Web, "Web wasm archiver", message);
        }
        return;
    };

    if let Err(error) = probe(ctx, &clang, &archiver) {
        if ctx.opts.check {
            ctx.missing(Target::Web, "Web wasm toolchain", error);
        } else {
            ctx.failed(Target::Web, "Web wasm toolchain", error);
        }
        return;
    }

    ctx.web_tools = Some(WebTools {
        clang: clang.clone(),
        archiver: archiver.clone(),
        wasm_ld: wasm_ld.clone(),
    });
    ctx.ready(
        Target::Web,
        "Web wasm toolchain",
        format!(
            "clang: {}; archiver: {}{}",
            clang.display(),
            archiver.display(),
            wasm_ld
                .map(|path| format!("; wasm-ld: {}", path.display()))
                .unwrap_or_default()
        ),
    );
}

fn find_clang(ctx: &Context) -> Option<PathBuf> {
    if let Some(cc) = env::var_os("CC") {
        let path = PathBuf::from(cc);
        if is_wasm_clang(&path) {
            return Some(path);
        }
        return None;
    }

    clang_candidates(ctx)
        .into_iter()
        .find(|candidate| is_wasm_clang(candidate))
}

fn clang_candidates(ctx: &Context) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = which("clang") {
        candidates.push(path);
    }
    if let Some(prefix) = &ctx.llvm_prefix {
        candidates.push(prefix.join("bin/clang"));
    }
    if ctx.platform.is_macos() {
        candidates.push(PathBuf::from("/opt/homebrew/opt/llvm/bin/clang"));
        candidates.push(PathBuf::from("/usr/local/opt/llvm/bin/clang"));
    }
    if ctx.platform.is_windows() {
        if let Some(prefix) = windows::find_llvm() {
            candidates.push(prefix.join("bin/clang.exe"));
        }
    }
    candidates
}

fn find_archiver(ctx: &Context) -> Option<PathBuf> {
    if let Some(ar) = env::var_os("AR") {
        let path = PathBuf::from(ar);
        if command_works(&path) {
            return Some(path);
        }
        return None;
    }

    if let Some(path) = which("llvm-ar") {
        return Some(path);
    }
    for path in versioned_llvm_ars() {
        if command_works(&path) {
            return Some(path);
        }
    }
    if let Some(prefix) = &ctx.llvm_prefix {
        let path = prefix.join("bin/llvm-ar");
        if command_works(&path) {
            return Some(path);
        }
    }
    if ctx.platform.is_macos() {
        for prefix in ["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"] {
            let path = PathBuf::from(prefix).join("llvm-ar");
            if command_works(&path) {
                return Some(path);
            }
        }
    }
    if ctx.platform.is_windows() {
        if let Some(prefix) = windows::find_llvm() {
            let path = prefix.join("bin/llvm-ar.exe");
            if command_works(&path) {
                return Some(path);
            }
        }
    }
    which("ar")
}

fn versioned_llvm_ars() -> Vec<PathBuf> {
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
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found.reverse();
    found
}

fn find_wasm_ld(ctx: &Context) -> Option<PathBuf> {
    if let Some(path) = which("wasm-ld") {
        return Some(path);
    }
    if let Some(prefix) = &ctx.llvm_prefix {
        let path = prefix.join("bin/wasm-ld");
        if path.is_file() {
            return Some(path);
        }
    }
    if ctx.platform.is_macos() {
        for prefix in ["/opt/homebrew/opt/llvm/bin", "/usr/local/opt/llvm/bin"] {
            let path = PathBuf::from(prefix).join("wasm-ld");
            if path.is_file() {
                return Some(path);
            }
        }
        if let Some(brew) = macos::find_brew() {
            if let Some(prefix) = macos::homebrew_prefix(&brew, "lld") {
                let path = prefix.join("bin/wasm-ld");
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    if ctx.platform.is_windows() {
        if let Some(prefix) = windows::find_llvm() {
            let path = prefix.join("bin/wasm-ld.exe");
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

fn is_wasm_clang(path: &Path) -> bool {
    run_output(&CommandSpec::new(path).arg("--print-targets"))
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("wasm32")
        })
        .unwrap_or(false)
}

fn command_works(path: &Path) -> bool {
    run_output(&CommandSpec::new(path).arg("--version"))
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn probe(ctx: &Context, clang: &Path, archiver: &Path) -> Result<(), String> {
    let directory = env::temp_dir().join(format!("cargo-prepare-wasm-{}", std::process::id()));
    let source = directory.join("probe.c");
    let object = directory.join("probe.o");
    let archive = directory.join("probe.a");

    if ctx.opts.dry_run {
        println!(
            "  + {} --target=wasm32-unknown-unknown -c {} -o {}",
            clang.display(),
            source.display(),
            object.display()
        );
        println!(
            "  + {} rcs {} {}",
            archiver.display(),
            archive.display(),
            object.display()
        );
        return Ok(());
    }

    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    fs::write(
        &source,
        "int address_explorer_wasm_probe(void) { return 0; }\n",
    )
    .map_err(|error| format!("could not write {}: {error}", source.display()))?;

    let compile = run_output(
        &CommandSpec::new(clang)
            .args(["--target=wasm32-unknown-unknown", "-c"])
            .arg(&source)
            .arg("-o")
            .arg(&object),
    )?;
    if !compile.status.success() {
        let stderr = String::from_utf8_lossy(&compile.stderr);
        let _ = fs::remove_dir_all(&directory);
        return Err(format!(
            "wasm probe compile failed with {}: {}",
            compile.status,
            stderr.trim()
        ));
    }

    let archive_result = run_output(
        &CommandSpec::new(archiver)
            .arg("rcs")
            .arg(&archive)
            .arg(&object),
    )?;
    let ok = archive_result.status.success() && archive.is_file();
    let stderr = String::from_utf8_lossy(&archive_result.stderr)
        .trim()
        .to_string();
    let _ = fs::remove_dir_all(&directory);

    if ok {
        Ok(())
    } else {
        Err(format!(
            "wasm archiver probe failed with {}{}",
            archive_result.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            }
        ))
    }
}
