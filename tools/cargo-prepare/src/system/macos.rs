use std::path::{Path, PathBuf};

use crate::context::{
    append_path, capture_stdout, run, run_output, which, CommandSpec, Context, Status, Target,
};

const HOMEBREW_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh";

pub fn ensure(ctx: &mut Context) {
    ensure_xcode_clt(ctx);
    ensure_homebrew(ctx);
    ensure_llvm(ctx);
}

fn ensure_xcode_clt(ctx: &mut Context) {
    let installed = which("xcode-select")
        .map(|select| run_output_ok(&CommandSpec::new(select).arg("-p")))
        .unwrap_or(false);

    if ctx.opts.check {
        if installed {
            ctx.ready(
                Target::Desktop,
                "Xcode Command Line Tools",
                "xcode-select -p succeeded",
            );
        } else {
            ctx.missing(
                Target::Desktop,
                "Xcode Command Line Tools",
                "run: xcode-select --install",
            );
        }
        return;
    }

    if installed {
        ctx.already_present(
            Target::Desktop,
            "Xcode Command Line Tools",
            "xcode-select -p succeeded",
        );
        return;
    }

    println!("Starting the Xcode Command Line Tools installer...");
    if ctx.opts.dry_run {
        println!("  + xcode-select --install");
        ctx.record(
            Target::Desktop,
            "Xcode Command Line Tools",
            Status::Installed,
            true,
            "would start the macOS installer",
        );
        return;
    }
    match run(ctx, &CommandSpec::new("xcode-select").arg("--install")) {
        Ok(()) => ctx.manual(
            Target::Desktop,
            "Xcode Command Line Tools",
            "finish the macOS installer, then re-run `cargo prepare`",
        ),
        Err(error) => ctx.failed(
            Target::Desktop,
            "Xcode Command Line Tools",
            format!("{error}\n       retry: xcode-select --install"),
        ),
    }
}

fn ensure_homebrew(ctx: &mut Context) {
    if let Some(brew) = find_brew() {
        ctx.brew = Some(brew.clone());
        if let Some(bin) = brew.parent() {
            append_path(bin);
        }
        ctx.already_present(Target::Common, "Homebrew", brew.display().to_string());
        return;
    }

    if ctx.opts.check {
        ctx.record(
            Target::Common,
            "Homebrew",
            Status::Missing,
            false,
            "install from https://brew.sh or run `cargo prepare`",
        );
        return;
    }

    println!("Installing Homebrew (this may ask for your password)...");
    let script = std::env::temp_dir().join("address-explorer-homebrew-install.sh");
    let download = CommandSpec::new("curl")
        .args(["--fail", "--location", "--retry", "3", "--output"])
        .arg(&script)
        .arg(HOMEBREW_INSTALL_URL);
    if let Err(error) = run(ctx, &download) {
        ctx.record(
            Target::Common,
            "Homebrew",
            Status::Failed,
            false,
            format!("{error}\n       install manually from https://brew.sh"),
        );
        return;
    }

    if ctx.opts.dry_run {
        println!("  + /bin/bash {} (NONINTERACTIVE=1)", script.display());
        ctx.brew = Some(PathBuf::from("/opt/homebrew/bin/brew"));
        ctx.installed(Target::Common, "Homebrew", "would install Homebrew");
        return;
    }

    match run(
        ctx,
        &CommandSpec::new("/bin/bash")
            .arg(&script)
            .env("NONINTERACTIVE", "1"),
    ) {
        Ok(()) => {
            if let Some(brew) = find_brew() {
                ctx.brew = Some(brew.clone());
                if let Some(bin) = brew.parent() {
                    append_path(bin);
                }
                ctx.installed(Target::Common, "Homebrew", brew.display().to_string());
            } else {
                ctx.record(
                    Target::Common,
                    "Homebrew",
                    Status::Failed,
                    false,
                    "the installer completed but `brew` was not found in /opt/homebrew or /usr/local",
                );
            }
        }
        Err(error) => ctx.record(
            Target::Common,
            "Homebrew",
            Status::Failed,
            false,
            format!("{error}\n       install manually from https://brew.sh"),
        ),
    }
}

fn ensure_llvm(ctx: &mut Context) {
    let Some(brew) = ctx.brew.clone().or_else(find_brew) else {
        ctx.manual(
            Target::Web,
            "Homebrew LLVM (wasm clang/lld)",
            "install Homebrew, then run: brew install llvm lld",
        );
        return;
    };
    ctx.brew = Some(brew.clone());

    let toolchain = discover_llvm(&brew);

    if ctx.opts.check {
        match toolchain {
            Some(toolchain) => ctx.ready(
                Target::Web,
                "Homebrew LLVM (wasm clang/lld)",
                toolchain.prefix.display().to_string(),
            ),
            None => ctx.missing(
                Target::Web,
                "Homebrew LLVM (wasm clang/lld)",
                "run: brew install llvm lld",
            ),
        }
        return;
    }

    if let Some(toolchain) = toolchain {
        ctx.llvm_prefix = Some(toolchain.prefix);
        ctx.already_present(
            Target::Web,
            "Homebrew LLVM (wasm clang/lld)",
            "llvm and lld are installed",
        );
        return;
    }

    println!("Installing Homebrew LLVM and lld for the web SharedWorker...");
    if ctx.opts.dry_run {
        println!("  + {} install llvm lld", brew.display());
        ctx.llvm_prefix = Some(PathBuf::from("/opt/homebrew/opt/llvm"));
        ctx.installed(
            Target::Web,
            "Homebrew LLVM (wasm clang/lld)",
            "would install llvm and lld",
        );
        return;
    }
    match run(
        ctx,
        &CommandSpec::new(&brew).args(["install", "llvm", "lld"]),
    ) {
        // `brew --prefix` can fail right after an install, so the keg location
        // is checked directly as well.
        Ok(()) => match discover_llvm(&brew) {
            Some(toolchain) => {
                ctx.llvm_prefix = Some(toolchain.prefix.clone());
                ctx.installed(
                    Target::Web,
                    "Homebrew LLVM (wasm clang/lld)",
                    toolchain.prefix.display().to_string(),
                );
            }
            None => ctx.failed(
                Target::Web,
                "Homebrew LLVM (wasm clang/lld)",
                "brew install completed but no Homebrew clang, llvm-ar, and wasm-ld were found under \
                 /opt/homebrew/opt or /usr/local/opt",
            ),
        },
        Err(error) => ctx.failed(
            Target::Web,
            "Homebrew LLVM (wasm clang/lld)",
            format!("{error}\n       retry: brew install llvm lld"),
        ),
    }
}

struct LlvmToolchain {
    prefix: PathBuf,
}

fn discover_llvm(brew: &Path) -> Option<LlvmToolchain> {
    let llvm_prefixes = formula_prefixes(brew, "llvm");
    let lld_prefixes = formula_prefixes(brew, "lld");

    let prefix = llvm_prefixes.iter().find(|prefix| {
        prefix.join("bin/clang").is_file() && prefix.join("bin/llvm-ar").is_file()
    })?;

    let wasm_ld = llvm_prefixes
        .iter()
        .chain(lld_prefixes.iter())
        .map(|candidate| candidate.join("bin/wasm-ld"))
        .any(|path| path.is_file())
        || which("wasm-ld").is_some();
    if !wasm_ld {
        return None;
    }

    Some(LlvmToolchain {
        prefix: prefix.clone(),
    })
}

fn formula_prefixes(brew: &Path, formula: &str) -> Vec<PathBuf> {
    let mut prefixes = Vec::new();
    if let Some(prefix) = formula_prefix(brew, formula) {
        prefixes.push(prefix);
    }
    prefixes.push(PathBuf::from(format!("/opt/homebrew/opt/{formula}")));
    prefixes.push(PathBuf::from(format!("/usr/local/opt/{formula}")));
    prefixes
}

fn formula_prefix(brew: &Path, formula: &str) -> Option<PathBuf> {
    match run_output(&CommandSpec::new(brew).args(["--prefix", formula])) {
        Ok(output) if output.status.success() => {
            let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!prefix.is_empty()).then(|| PathBuf::from(prefix))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if !stderr.is_empty() {
                eprintln!("`brew --prefix {formula}` failed: {stderr}");
            }
            None
        }
        Err(error) => {
            eprintln!("could not run `brew --prefix {formula}`: {error}");
            None
        }
    }
}

pub fn find_brew() -> Option<PathBuf> {
    which("brew")
        .or_else(|| {
            let candidate = PathBuf::from("/opt/homebrew/bin/brew");
            candidate.is_file().then_some(candidate)
        })
        .or_else(|| {
            let candidate = PathBuf::from("/usr/local/bin/brew");
            candidate.is_file().then_some(candidate)
        })
}

fn run_output_ok(spec: &CommandSpec) -> bool {
    crate::context::run_output(spec)
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub fn homebrew_prefix(brew: &Path, formula: &str) -> Option<PathBuf> {
    let prefix = capture_stdout(&CommandSpec::new(brew).args(["--prefix", formula]))?;
    if prefix.is_empty() {
        None
    } else {
        Some(PathBuf::from(prefix))
    }
}

pub fn ensure_formula(
    ctx: &mut Context,
    formula: &str,
    description: &str,
    target: Target,
) -> Result<(), String> {
    let Some(brew) = ctx.brew.clone().or_else(find_brew) else {
        return Err("Homebrew is not available".to_string());
    };
    ctx.brew = Some(brew.clone());

    if formula_installed(&brew, formula) {
        ctx.already_present(target, description, formula);
        return Ok(());
    }

    if ctx.opts.check {
        ctx.missing(target, description, format!("run: brew install {formula}"));
        return Ok(());
    }

    run(ctx, &CommandSpec::new(&brew).args(["install", formula]))?;
    if ctx.opts.dry_run {
        ctx.installed(target, description, format!("would install {formula}"));
        return Ok(());
    }
    if formula_installed(&brew, formula) {
        ctx.installed(target, description, formula);
        Ok(())
    } else {
        Err(format!(
            "brew install {formula} did not register the formula"
        ))
    }
}

fn formula_installed(brew: &Path, formula: &str) -> bool {
    run_output(&CommandSpec::new(brew).args(["list", "--versions", formula]))
        .map(|output| output.status.success())
        .unwrap_or(false)
}
