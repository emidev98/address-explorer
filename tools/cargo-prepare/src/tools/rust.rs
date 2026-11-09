use crate::context::{capture_stdout, run, which, CommandSpec, Context, Target};

const ANDROID_TARGETS: &[&str] = &[
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "i686-linux-android",
    "x86_64-linux-android",
];
const IOS_TARGETS: &[&str] = &[
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "x86_64-apple-ios",
];

pub fn ensure(ctx: &mut Context) {
    let Some(rustup) = which("rustup") else {
        ctx.manual(
            Target::Common,
            "rustup",
            "install Rust from https://rustup.rs, then re-run `cargo prepare`",
        );
        return;
    };
    if which("cargo").is_none() {
        ctx.manual(
            Target::Common,
            "Cargo",
            "install Rust from https://rustup.rs, then re-run `cargo prepare`",
        );
        return;
    }

    ensure_toolchain(ctx, &rustup);
    ensure_components(ctx, &rustup);
    ensure_targets(ctx, &rustup);

    if ctx.platform.is_windows() {
        ensure_msvc_toolchain(ctx);
    }
}

fn ensure_toolchain(ctx: &mut Context, rustup: &std::path::Path) {
    let installed = capture_stdout(&CommandSpec::new(rustup).args(["toolchain", "list"]))
        .map(|list| list.lines().any(|line| line.starts_with("stable")))
        .unwrap_or(false);

    if installed {
        ctx.already_present(
            Target::Common,
            "Rust stable toolchain",
            "stable is installed",
        );
        return;
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Common,
            "Rust stable toolchain",
            "rustup toolchain install stable --profile minimal",
        );
        return;
    }

    match run(
        ctx,
        &CommandSpec::new(rustup).args(["toolchain", "install", "stable", "--profile", "minimal"]),
    ) {
        Ok(()) => ctx.installed(Target::Common, "Rust stable toolchain", "stable"),
        Err(error) => ctx.failed(
            Target::Common,
            "Rust stable toolchain",
            format!("{error}\n       retry: rustup toolchain install stable --profile minimal"),
        ),
    }
}

fn ensure_components(ctx: &mut Context, rustup: &std::path::Path) {
    let installed = capture_stdout(&CommandSpec::new(rustup).args([
        "component",
        "list",
        "--installed",
        "--toolchain",
        "stable",
    ]))
    .unwrap_or_default();

    for component in ["rustfmt", "clippy"] {
        if installed
            .lines()
            .any(|line| line.trim_start().starts_with(component))
        {
            ctx.already_present(
                Target::Common,
                format!("Rust component {component}"),
                "installed",
            );
            continue;
        }

        if ctx.opts.check {
            ctx.missing(
                Target::Common,
                format!("Rust component {component}"),
                format!("rustup component add {component} --toolchain stable"),
            );
            continue;
        }

        match run(
            ctx,
            &CommandSpec::new(rustup).args([
                "component",
                "add",
                component,
                "--toolchain",
                "stable",
            ]),
        ) {
            Ok(()) => ctx.installed(
                Target::Common,
                format!("Rust component {component}"),
                "installed",
            ),
            Err(error) => ctx.failed(
                Target::Common,
                format!("Rust component {component}"),
                format!(
                    "{error}\n       retry: rustup component add {component} --toolchain stable"
                ),
            ),
        }
    }
}

fn ensure_targets(ctx: &mut Context, rustup: &std::path::Path) {
    let installed = capture_stdout(&CommandSpec::new(rustup).args([
        "target",
        "list",
        "--installed",
        "--toolchain",
        "stable",
    ]))
    .unwrap_or_default();

    let mut groups: Vec<(&str, Target, Vec<&str>)> =
        vec![("Web target", Target::Web, vec!["wasm32-unknown-unknown"])];

    if ctx.opts.all && ctx.platform.android_supported() {
        groups.push((
            "Android Rust targets",
            Target::Android,
            ANDROID_TARGETS.to_vec(),
        ));
    }
    if ctx.opts.all && ctx.platform.is_macos() {
        groups.push(("iOS Rust targets", Target::Ios, IOS_TARGETS.to_vec()));
    }

    for (name, target, targets) in groups {
        let missing: Vec<&str> = targets
            .iter()
            .copied()
            .filter(|target| !installed.lines().any(|line| line.trim() == *target))
            .collect();

        if missing.is_empty() {
            ctx.already_present(target, name, targets.join(", "));
            continue;
        }

        if ctx.opts.check {
            ctx.missing(
                target,
                name,
                format!("rustup target add {} --toolchain stable", missing.join(" ")),
            );
            continue;
        }

        let mut spec = CommandSpec::new(rustup).arg("target").arg("add");
        for target_name in &missing {
            spec = spec.arg(*target_name);
        }
        spec = spec.args(["--toolchain", "stable"]);
        match run(ctx, &spec) {
            Ok(()) => ctx.installed(target, name, format!("added {}", missing.join(", "))),
            Err(error) => ctx.failed(
                target,
                name,
                format!(
                    "{error}\n       retry: rustup target add {} --toolchain stable",
                    missing.join(" ")
                ),
            ),
        }
    }
}

fn ensure_msvc_toolchain(ctx: &mut Context) {
    let host = capture_stdout(&CommandSpec::new("rustc").arg("-vV"))
        .and_then(|output| {
            output
                .lines()
                .find_map(|line| line.strip_prefix("host: ").map(str::to_string))
        })
        .unwrap_or_default();

    if host.contains("msvc") {
        ctx.ready(Target::Common, "Rust MSVC host toolchain", host);
    } else {
        ctx.manual(
            Target::Common,
            "Rust MSVC host toolchain",
            format!(
                "active host is `{host}`; install the MSVC toolchain with: rustup toolchain install stable-x86_64-pc-windows-msvc"
            ),
        );
    }
}
