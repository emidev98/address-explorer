mod cli;
mod context;
mod env_file;
mod platform;
mod report;
mod system;
mod tools;

use std::io::{self, Write};
use std::process::ExitCode;

use context::{run, CommandSpec, Context, Target};
use platform::Platform;

fn main() -> ExitCode {
    match cli::parse() {
        Ok(cli::Parse::Help) => {
            print!("{}", cli::help());
            ExitCode::SUCCESS
        }
        Ok(cli::Parse::Version) => {
            println!("{}", cli::version());
            ExitCode::SUCCESS
        }
        Ok(cli::Parse::Run(options)) => {
            let platform = Platform::detect();
            let mut ctx = Context::new(options, platform);

            println!("Address Explorer development setup");
            println!("Host: {}", ctx.platform.description());
            println!(
                "Mode: {}{}",
                if ctx.opts.check {
                    "check only"
                } else if ctx.opts.dry_run {
                    "dry run"
                } else {
                    "install"
                },
                if ctx.opts.all {
                    " (desktop + web + mobile)"
                } else {
                    " (desktop + web)"
                }
            );

            if ctx.opts.check || ctx.opts.dry_run {
                // Verification modes never prompt.
            } else if !ctx.opts.yes {
                if !context::is_tty() {
                    eprintln!(
                        "\nRefusing to change the system without a terminal. Re-run with `--yes`, or use `--check`/`--dry-run`."
                    );
                    return ExitCode::from(2);
                }
                print_summary(&ctx);
                if !confirm() {
                    println!("Aborted.");
                    return ExitCode::from(2);
                }
            }

            tools::rust::ensure(&mut ctx);
            system::ensure(&mut ctx);
            tools::dioxus::ensure(&mut ctx);
            tools::node::ensure(&mut ctx);
            tools::webtoolchain::ensure(&mut ctx);
            tools::android::ensure(&mut ctx);
            tools::ios::ensure(&mut ctx);

            if ctx.opts.verify_build && !ctx.opts.check && !ctx.opts.dry_run {
                verify_build(&mut ctx);
            }

            report::print(&ctx);

            if ctx.opts.dry_run {
                println!("\nDry run complete. No changes were made.");
                return ExitCode::SUCCESS;
            }

            if ctx.has_required_problem() {
                eprintln!(
                    "\nSome required steps failed or need manual action. Review the report above, fix them, then re-run `cargo prepare --check`."
                );
                return ExitCode::FAILURE;
            }

            if ctx.opts.check {
                println!("\nEnvironment check complete.");
            } else {
                println!("\nEnvironment is ready.");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::from(2)
        }
    }
}

fn print_summary(ctx: &Context) {
    println!("\nThe following changes will be made:");
    println!("  - ensure the Rust stable toolchain, rustfmt, clippy, and required targets");
    println!("  - install the platform build dependencies for desktop and web");
    if ctx.platform.is_linux() {
        if let Some(pkg) = ctx.platform.pkg {
            let packages = system::linux::planned_packages(pkg);
            println!(
                "  - install {} packages with {}: {}",
                packages.len(),
                ctx.platform.package_manager_name(),
                packages.join(" ")
            );
        }
    } else if ctx.platform.is_macos() {
        println!("  - ensure Xcode Command Line Tools, Homebrew, and Homebrew LLVM/lld");
    } else if ctx.platform.is_windows() {
        println!("  - ensure Visual Studio Build Tools, Windows SDK, WebView2, and LLVM");
    }
    println!("  - install Dioxus CLI 0.7.10");
    println!("  - install Node.js LTS and warm the Tailwind 4.1.5 cache");
    println!("  - verify the wasm clang/lld/archiver toolchain");

    if ctx.opts.all {
        if ctx.platform.android_supported() {
            println!("  - install JDK 17, Android SDK 34, build-tools 34, NDK 25.2.9519653, CMake, and Android Rust targets");
            if let Some(profile) = ctx.platform.profile_file() {
                println!(
                    "  - persist Android environment variables to {}",
                    profile.display()
                );
            } else if ctx.platform.is_windows() {
                println!(
                    "  - persist Android environment variables to the Windows user environment"
                );
            } else {
                println!(
                    "  - print Android environment exports (no supported shell profile detected)"
                );
            }
        } else {
            println!(
                "  - Android is not supported on {} {}; it will be reported as unsupported",
                if ctx.platform.is_linux() {
                    "Linux"
                } else {
                    "this host"
                },
                ctx.platform.arch
            );
        }
        if ctx.platform.is_macos() {
            println!("  - verify Xcode and the iOS SDK; add the iOS Rust targets");
        }
    }

    println!("\nSystem package installs use sudo on Linux/macOS and UAC on Windows.");
    if ctx.opts.all && ctx.platform.android_supported() {
        println!("Android SDK licenses are accepted when `--all` is used.");
    }
}

fn confirm() -> bool {
    print!("\nContinue? [y/N] ");
    let _ = io::stdout().flush();
    let mut answer = String::new();
    if io::stdin().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn verify_build(ctx: &mut Context) {
    println!("\nRunning dx check for the installed targets...");
    check_platform(ctx, "web", Target::Web);
    check_platform(ctx, "desktop", Target::Desktop);
}

fn check_platform(ctx: &mut Context, platform: &str, target: Target) {
    let spec = CommandSpec::new("dx").args(["check", "--platform", platform]);
    match run(ctx, &spec) {
        Ok(()) => ctx.ready(
            target,
            format!("dx check --platform {platform}"),
            "compile check passed",
        ),
        Err(error) => ctx.failed(target, format!("dx check --platform {platform}"), error),
    }
}
