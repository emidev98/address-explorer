use std::path::Path;

use crate::context::{capture_stdout, which, CommandSpec, Context, Target};

const XCODE_PATH: &str = "/Applications/Xcode.app/Contents/Developer";

pub fn ensure(ctx: &mut Context) {
    if !ctx.opts.all {
        ctx.skipped(Target::Ios, "iOS toolchain", "not requested; pass --all");
        return;
    }
    if !ctx.platform.is_macos() {
        ctx.unsupported(
            Target::Ios,
            "iOS toolchain",
            "iOS bundles can only be built on macOS",
        );
        return;
    }

    let xcode = Path::new(XCODE_PATH).is_dir();
    if !xcode {
        ctx.manual(
            Target::Ios,
            "Xcode",
            "install Xcode from the App Store, open it once, then re-run `cargo prepare --all`",
        );
        return;
    }

    let version = capture_stdout(&CommandSpec::new("xcodebuild").arg("-version"));
    if version.is_none() {
        ctx.manual(
            Target::Ios,
            "Xcode",
            "Xcode is present but `xcodebuild -version` failed; open Xcode and install its components",
        );
        return;
    }

    let sdk =
        capture_stdout(&CommandSpec::new("xcrun").args(["--sdk", "iphoneos", "--show-sdk-path"]));
    if sdk.is_none() {
        ctx.manual(
            Target::Ios,
            "iOS SDK",
            "the iOS SDK is missing; install it from Xcode → Settings → Platforms",
        );
        return;
    }

    if which("xcodebuild").is_none() {
        ctx.manual(
            Target::Ios,
            "Xcode",
            "xcodebuild is not on PATH; run `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`",
        );
        return;
    }

    ctx.ready(
        Target::Ios,
        "Xcode and iOS SDK",
        version
            .unwrap_or_default()
            .lines()
            .next()
            .unwrap_or("installed")
            .to_string(),
    );

    verify_ios_targets(ctx);
}

fn verify_ios_targets(ctx: &mut Context) {
    let installed = capture_stdout(&CommandSpec::new("rustup").args([
        "target",
        "list",
        "--installed",
        "--toolchain",
        "stable",
    ]))
    .unwrap_or_default();

    let missing: Vec<&str> = [
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
    ]
    .into_iter()
    .filter(|target| !installed.lines().any(|line| line.trim() == *target))
    .collect();

    if missing.is_empty() {
        ctx.ready(
            Target::Ios,
            "iOS Rust targets",
            "aarch64-apple-ios, aarch64-apple-ios-sim, x86_64-apple-ios",
        );
    } else if ctx.opts.check {
        ctx.missing(
            Target::Ios,
            "iOS Rust targets",
            format!("rustup target add {} --toolchain stable", missing.join(" ")),
        );
    } else {
        ctx.failed(
            Target::Ios,
            "iOS Rust targets",
            format!("rustup target add {} --toolchain stable", missing.join(" ")),
        );
    }
}
