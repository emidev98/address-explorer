use crate::context::{capture_stdout, run, which, CommandSpec, Context, Target};

pub const DIOXUS_CLI_VERSION: &str = "0.7.10";

pub fn ensure(ctx: &mut Context) {
    let current = capture_stdout(&CommandSpec::new("dx").arg("--version"));

    if let Some(version) = &current {
        if version.contains(DIOXUS_CLI_VERSION) {
            ctx.already_present(Target::Common, "Dioxus CLI", version.clone());
            return;
        }
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Common,
            "Dioxus CLI",
            format!(
                "dx {DIOXUS_CLI_VERSION} is required; install with: cargo install dioxus-cli --version {DIOXUS_CLI_VERSION} --locked"
            ),
        );
        return;
    }

    println!("Installing Dioxus CLI {DIOXUS_CLI_VERSION} (this can take several minutes)...");

    let installed = if which("cargo-binstall").is_some() {
        let spec = CommandSpec::new("cargo")
            .args(["binstall", "--no-confirm"])
            .arg(format!("dioxus-cli@{DIOXUS_CLI_VERSION}"));
        run(ctx, &spec).is_ok()
    } else {
        false
    };

    let installed = installed
        || run(
            ctx,
            &CommandSpec::new("cargo").args([
                "install",
                "dioxus-cli",
                "--version",
                DIOXUS_CLI_VERSION,
                "--locked",
                "--force",
            ]),
        )
        .is_ok();

    if ctx.opts.dry_run {
        ctx.installed(
            Target::Common,
            "Dioxus CLI",
            format!("would install {DIOXUS_CLI_VERSION}"),
        );
        return;
    }

    if !installed {
        ctx.failed(
            Target::Common,
            "Dioxus CLI",
            format!(
                "could not install dx {DIOXUS_CLI_VERSION}\n       retry: cargo install dioxus-cli --version {DIOXUS_CLI_VERSION} --locked --force"
            ),
        );
        return;
    }

    match capture_stdout(&CommandSpec::new("dx").arg("--version")) {
        Some(version) if version.contains(DIOXUS_CLI_VERSION) => {
            ctx.installed(Target::Common, "Dioxus CLI", version);
        }
        Some(version) => ctx.failed(
            Target::Common,
            "Dioxus CLI",
            format!(
                "installed dx reports `{version}`, expected {DIOXUS_CLI_VERSION}; check your PATH order"
            ),
        ),
        None => ctx.failed(
            Target::Common,
            "Dioxus CLI",
            "dx is not available after installation; check that ~/.cargo/bin is on PATH",
        ),
    }
}
