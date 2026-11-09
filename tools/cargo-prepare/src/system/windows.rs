use std::env;
use std::path::{Path, PathBuf};

use crate::context::{run, run_output, which, CommandSpec, Context, Status, Target};

pub fn ensure(ctx: &mut Context) {
    if which("winget").is_none() {
        ctx.record(
            Target::Common,
            "winget",
            Status::Manual,
            false,
            "install or update App Installer from the Microsoft Store, then re-run `cargo prepare`",
        );
        if vs_build_tools_installed() {
            ctx.already_present(
                Target::Desktop,
                "Visual Studio Build Tools",
                "MSVC C++ workload detected",
            );
        } else if ctx.opts.check {
            ctx.missing(
                Target::Desktop,
                "Visual Studio Build Tools",
                "winget is unavailable; install the VCTools workload and Windows SDK manually",
            );
        } else {
            ctx.failed(
                Target::Desktop,
                "Visual Studio Build Tools",
                "winget is unavailable; install the VCTools workload and Windows SDK manually",
            );
        }
        if find_llvm().is_some() {
            ctx.already_present(Target::Web, "LLVM (wasm clang/lld)", "LLVM is installed");
        } else if ctx.opts.check {
            ctx.missing(
                Target::Web,
                "LLVM (wasm clang/lld)",
                "winget is unavailable; install LLVM manually from https://releases.llvm.org",
            );
        } else {
            ctx.failed(
                Target::Web,
                "LLVM (wasm clang/lld)",
                "winget is unavailable; install LLVM manually from https://releases.llvm.org",
            );
        }
        return;
    }

    ensure_vs_build_tools(ctx);
    ensure_webview2(ctx);
    ensure_llvm(ctx);
}

fn ensure_vs_build_tools(ctx: &mut Context) {
    if vs_build_tools_installed() {
        ctx.already_present(
            Target::Desktop,
            "Visual Studio Build Tools",
            "MSVC C++ workload detected",
        );
        return;
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Desktop,
            "Visual Studio Build Tools",
            "install the VCTools workload and Windows SDK",
        );
        return;
    }

    println!("Installing Visual Studio 2022 Build Tools (large download)...");
    let result = winget_install(
        ctx,
        "Microsoft.VisualStudio.2022.BuildTools",
        &[
            "--override",
            "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended",
        ],
        "Visual Studio Build Tools",
    );
    match result {
        Ok(()) => ctx.installed(
            Target::Desktop,
            "Visual Studio Build Tools",
            "VCTools workload installed",
        ),
        Err(error) => ctx.failed(
            Target::Desktop,
            "Visual Studio Build Tools",
            format!("{error}\n       retry: winget install --id Microsoft.VisualStudio.2022.BuildTools --exact"),
        ),
    }
}

fn ensure_webview2(ctx: &mut Context) {
    if winget_installed("Microsoft.EdgeWebView2Runtime") {
        ctx.already_present(
            Target::Desktop,
            "WebView2 runtime",
            "Microsoft.EdgeWebView2Runtime is installed",
        );
        return;
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Desktop,
            "WebView2 runtime",
            "winget install --id Microsoft.EdgeWebView2Runtime --exact",
        );
        return;
    }

    match winget_install(
        ctx,
        "Microsoft.EdgeWebView2Runtime",
        &[],
        "WebView2 runtime",
    ) {
        Ok(()) => ctx.installed(
            Target::Desktop,
            "WebView2 runtime",
            "Microsoft.EdgeWebView2Runtime installed",
        ),
        Err(error) => ctx.failed(
            Target::Desktop,
            "WebView2 runtime",
            format!(
                "{error}\n       retry: winget install --id Microsoft.EdgeWebView2Runtime --exact"
            ),
        ),
    }
}

fn ensure_llvm(ctx: &mut Context) {
    if let Some(prefix) = find_llvm() {
        if let Err(error) = add_user_path(ctx, &prefix.join("bin")) {
            ctx.record(
                Target::Web,
                "LLVM PATH",
                Status::Failed,
                false,
                format!("could not add {} to PATH: {error}", prefix.display()),
            );
        }
        ctx.already_present(
            Target::Web,
            "LLVM (wasm clang/lld)",
            prefix.display().to_string(),
        );
        return;
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Web,
            "LLVM (wasm clang/lld)",
            "winget install --id LLVM.LLVM --exact",
        );
        return;
    }

    match winget_install(ctx, "LLVM.LLVM", &[], "LLVM") {
        Ok(()) => {
            if ctx.opts.dry_run {
                ctx.installed(Target::Web, "LLVM (wasm clang/lld)", "would install LLVM");
                return;
            }
            if let Some(prefix) = find_llvm() {
                if let Err(error) = add_user_path(ctx, &prefix.join("bin")) {
                    ctx.record(
                        Target::Web,
                        "LLVM PATH",
                        Status::Failed,
                        false,
                        format!("could not add {} to PATH: {error}", prefix.display()),
                    );
                }
                ctx.installed(
                    Target::Web,
                    "LLVM (wasm clang/lld)",
                    prefix.display().to_string(),
                );
            } else {
                ctx.failed(
                    Target::Web,
                    "LLVM (wasm clang/lld)",
                    "LLVM was installed but `C:\\Program Files\\LLVM\\bin\\clang.exe` was not found",
                );
            }
        }
        Err(error) => ctx.failed(
            Target::Web,
            "LLVM (wasm clang/lld)",
            format!("{error}\n       retry: winget install --id LLVM.LLVM --exact"),
        ),
    }
}

pub fn find_llvm() -> Option<PathBuf> {
    let candidates = [
        env::var_os("ProgramFiles").map(|path| PathBuf::from(path).join("LLVM")),
        Some(PathBuf::from(r"C:\Program Files\LLVM")),
    ];
    for prefix in candidates.into_iter().flatten() {
        if prefix.join("bin/clang.exe").is_file() {
            return Some(prefix);
        }
    }

    which("clang").and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf))
}

pub fn winget_installed(id: &str) -> bool {
    run_output(&CommandSpec::new("winget").args([
        "list",
        "--id",
        id,
        "--exact",
        "--accept-source-agreements",
    ]))
    .map(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .to_lowercase()
                .contains(&id.to_lowercase())
    })
    .unwrap_or(false)
}

pub fn winget_install(
    ctx: &Context,
    id: &str,
    extra: &[&str],
    description: &str,
) -> Result<(), String> {
    println!("Installing {description} with winget...");
    let mut spec = CommandSpec::new("winget").args([
        "install",
        "--id",
        id,
        "--exact",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    if extra.is_empty() {
        spec = spec.arg("--silent");
    }
    for argument in extra {
        spec = spec.arg(*argument);
    }
    run(ctx, &spec)
}

pub fn user_env(name: &str) -> Option<String> {
    let script = "[Environment]::GetEnvironmentVariable($env:AE_PREPARE_NAME, 'User')";
    let output = run_output(
        &CommandSpec::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("AE_PREPARE_NAME", name),
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn set_user_env(ctx: &Context, name: &str, value: &Path) -> Result<(), String> {
    if ctx.opts.check {
        return Ok(());
    }
    let script = "[Environment]::SetEnvironmentVariable($env:AE_PREPARE_NAME, $env:AE_PREPARE_VALUE, 'User')";
    run(
        ctx,
        &CommandSpec::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("AE_PREPARE_NAME", name)
            .env("AE_PREPARE_VALUE", value.as_os_str()),
    )
}

pub fn add_user_path(ctx: &Context, directory: &Path) -> Result<(), String> {
    if ctx.opts.check {
        return Ok(());
    }
    let script = r#"
$current = [Environment]::GetEnvironmentVariable('Path', 'User')
$parts = @($current -split ';' | Where-Object { $_ -ne '' })
$add = $env:AE_PREPARE_PATH
if ($parts -notcontains $add) {
    $parts += $add
    [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
}
"#;
    run(
        ctx,
        &CommandSpec::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("AE_PREPARE_PATH", directory.as_os_str()),
    )
}

pub fn vs_build_tools_installed() -> bool {
    let vswhere = env::var_os("ProgramFiles(x86)")
        .map(|program_files| {
            PathBuf::from(program_files)
                .join("Microsoft Visual Studio")
                .join("Installer")
                .join("vswhere.exe")
        })
        .filter(|path| path.is_file());

    let Some(vswhere) = vswhere else {
        return false;
    };

    run_output(&CommandSpec::new(vswhere).args([
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property",
        "installationPath",
    ]))
    .map(|output| {
        output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
    })
    .unwrap_or(false)
}
