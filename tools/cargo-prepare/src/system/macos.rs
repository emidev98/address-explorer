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

    let prefix = capture_stdout(&CommandSpec::new(&brew).args(["prefix", "llvm"]))
        .filter(|prefix| !prefix.is_empty())
        .map(PathBuf::from);

    let clang = prefix.as_ref().map(|prefix| prefix.join("bin/clang"));
    let llvm_ar = prefix.as_ref().map(|prefix| prefix.join("bin/llvm-ar"));
    let lld_prefix = capture_stdout(&CommandSpec::new(&brew).args(["prefix", "lld"]))
        .filter(|prefix| !prefix.is_empty())
        .map(PathBuf::from);
    let wasm_ld = lld_prefix
        .as_ref()
        .map(|prefix| prefix.join("bin/wasm-ld"))
        .filter(|path| path.is_file())
        .or_else(|| prefix.as_ref().map(|prefix| prefix.join("bin/wasm-ld")))
        .filter(|path| path.is_file());

    let ready = clang.as_ref().is_some_and(|path| path.is_file())
        && llvm_ar.as_ref().is_some_and(|path| path.is_file())
        && wasm_ld.is_some();

    if ctx.opts.check {
        if ready {
            ctx.ready(
                Target::Web,
                "Homebrew LLVM (wasm clang/lld)",
                prefix.unwrap_or_default().display().to_string(),
            );
        } else {
            ctx.missing(
                Target::Web,
                "Homebrew LLVM (wasm clang/lld)",
                "run: brew install llvm lld",
            );
        }
        return;
    }

    if ready {
        ctx.llvm_prefix = prefix;
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
        Ok(()) => {
            let prefix = capture_stdout(&CommandSpec::new(&brew).args(["prefix", "llvm"]))
                .filter(|prefix| !prefix.is_empty())
                .map(PathBuf::from);
            if let Some(prefix) = prefix {
                ctx.llvm_prefix = Some(prefix.clone());
                ctx.installed(
                    Target::Web,
                    "Homebrew LLVM (wasm clang/lld)",
                    prefix.display().to_string(),
                );
            } else {
                ctx.failed(
                    Target::Web,
                    "Homebrew LLVM (wasm clang/lld)",
                    "brew install completed but `brew --prefix llvm` failed",
                );
            }
        }
        Err(error) => ctx.failed(
            Target::Web,
            "Homebrew LLVM (wasm clang/lld)",
            format!("{error}\n       retry: brew install llvm lld"),
        ),
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
    let prefix = capture_stdout(&CommandSpec::new(brew).args(["prefix", formula]))?;
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
