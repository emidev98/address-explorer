use std::path::Path;

use crate::context::{run_output, which, CommandSpec, Context, Status, Target};

pub fn print(ctx: &Context) {
    println!("\n== Capability summary ==");
    for target in [Target::Desktop, Target::Web, Target::Android, Target::Ios] {
        println!("  {:<8} {}", target.label(), capability(ctx, target));
    }

    println!("\n== Details ==");
    for result in &ctx.results {
        let detail = result.detail.replace('\n', "\n             ");
        println!(
            "  [{}] {}: {}{} — {}",
            result.target.label(),
            result.name,
            result.status.label(),
            if result.required { "" } else { " (optional)" },
            detail
        );
    }

    println!("\n== Versions ==");
    version("rustc", &["--version"]);
    version("cargo", &["--version"]);
    version("rustup", &["--version"]);
    version("dx", &["--version"]);
    version("node", &["--version"]);
    version("npm", &["--version"]);
    version("clang", &["--version"]);
    version("wasm-ld", &["--version"]);
    if which("llvm-ar").is_some() {
        version("llvm-ar", &["--version"]);
    } else {
        version("ar", &["--version"]);
    }
    if let Some(home) = &ctx.java_home {
        let java = if cfg!(windows) {
            home.join("bin/java.exe")
        } else {
            home.join("bin/java")
        };
        version_path("java", &java, &["-version"]);
    } else {
        version("java", &["-version"]);
    }
    if let Some(sdk) = &ctx.android_sdk {
        let adb = sdk
            .join("platform-tools")
            .join(if cfg!(windows) { "adb.exe" } else { "adb" });
        version_path("adb", &adb, &["--version"]);
    } else {
        version("adb", &["--version"]);
    }
    version("xcodebuild", &["-version"]);
    if let Some(tools) = &ctx.web_tools {
        println!("  web clang  {}", tools.clang.display());
        println!("  web ar     {}", tools.archiver.display());
        if let Some(wasm_ld) = &tools.wasm_ld {
            println!("  web wasm-ld {}", wasm_ld.display());
        }
    }

    println!("\n== dx doctor ==");
    if ctx.opts.dry_run {
        println!("  would run `dx doctor`");
    } else if which("dx").is_some() {
        match run_output(&CommandSpec::new("dx").arg("doctor")) {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stdout.trim().is_empty() {
                    println!("{}", stdout.trim_end());
                }
                if !stderr.trim().is_empty() {
                    println!("{}", stderr.trim_end());
                }
            }
            Err(error) => println!("  could not run dx doctor: {error}"),
        }
    } else {
        println!("  dx is not installed");
    }
}

fn capability(ctx: &Context, target: Target) -> &'static str {
    let results: Vec<_> = ctx
        .results
        .iter()
        .filter(|result| result.target == target)
        .collect();
    let common: Vec<_> = ctx
        .results
        .iter()
        .filter(|result| result.target == Target::Common)
        .collect();

    if results.is_empty() {
        return "not requested";
    }
    if results
        .iter()
        .all(|result| result.status == Status::Skipped)
    {
        return "not requested";
    }
    if results
        .iter()
        .all(|result| result.status == Status::Unsupported)
    {
        return "unsupported";
    }

    let problem = results
        .iter()
        .chain(common.iter())
        .any(|result| result.required && result.status.is_problem());

    if problem {
        "blocked"
    } else {
        "ready"
    }
}

fn version(program: &str, args: &[&str]) {
    let Some(path) = which(program) else {
        println!("  {program:<10} not found");
        return;
    };
    version_path(program, &path, args);
}

fn version_path(label: &str, path: &Path, args: &[&str]) {
    if !path.is_file() {
        println!("  {label:<10} not found");
        return;
    }
    let output = run_output(&CommandSpec::new(path).args(args));
    let Ok(output) = output else {
        println!("  {label:<10} not available");
        return;
    };
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("installed");
    println!("  {label:<10} {line}");
}
