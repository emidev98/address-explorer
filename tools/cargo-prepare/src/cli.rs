use std::env;
use std::ffi::OsString;

#[derive(Debug, Default)]
pub struct Options {
    pub all: bool,
    pub check: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub non_interactive: bool,
    pub verify_build: bool,
    pub verbose: bool,
}

pub enum Parse {
    Run(Options),
    Help,
    Version,
}

pub fn parse() -> Result<Parse, String> {
    parse_from(env::args_os().skip(1))
}

fn parse_from<I>(args: I) -> Result<Parse, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter().peekable();

    // Cargo invokes an external subcommand as `cargo-prepare prepare ...`.
    if let Some(first) = args.peek() {
        if first == "prepare" {
            args.next();
        }
    }

    let mut options = Options::default();
    for arg in args {
        match arg.to_string_lossy().as_ref() {
            "--all" => options.all = true,
            "--check" => options.check = true,
            "--dry-run" => options.dry_run = true,
            "--yes" | "-y" => options.yes = true,
            "--non-interactive" => options.non_interactive = true,
            "--verify-build" => options.verify_build = true,
            "--verbose" | "-v" => options.verbose = true,
            "--help" | "-h" => return Ok(Parse::Help),
            "--version" | "-V" => return Ok(Parse::Version),
            other => {
                return Err(format!(
                    "unknown argument `{other}`\n\nRun `cargo prepare --help` for usage."
                ));
            }
        }
    }

    if options.check && options.dry_run {
        return Err("`--check` and `--dry-run` cannot be combined".to_string());
    }

    if options.non_interactive && !options.yes && !options.check && !options.dry_run {
        return Err(
            "`--non-interactive` needs `--yes` to install, or use `--check`/`--dry-run`"
                .to_string(),
        );
    }

    Ok(Parse::Run(options))
}

pub fn help() -> &'static str {
    "Install the development toolchains and system dependencies for Address Explorer.

USAGE:
    cargo prepare [OPTIONS]

OPTIONS:
    --all                Also install Android everywhere supported and iOS on macOS
    --check              Verify the environment without installing anything
    --dry-run            Print every planned command without running it
    -y, --yes            Skip the confirmation prompt
        --non-interactive  Never prompt; requires --yes for installs
        --verify-build   Run dx check for web and desktop after installing
    -v, --verbose        Stream command output
    -h, --help           Print this help
    -V, --version        Print the cargo-prepare version

The default installs the host desktop and web toolchain. Cargo itself must
already be available to run this command; it can add the stable toolchain,
components, targets, system packages, the Dioxus CLI, Node, LLVM, Android SDK,
and iOS checks.
"
}

pub fn version() -> String {
    format!("cargo-prepare {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<Options, String> {
        match parse_from(args.iter().map(OsString::from))? {
            Parse::Run(options) => Ok(options),
            Parse::Help | Parse::Version => Err("expected run options".to_string()),
        }
    }

    #[test]
    fn accepts_the_cargo_subcommand_prefix() {
        let options = run(&["prepare", "--all"]).expect("parse");
        assert!(options.all);
    }

    #[test]
    fn parses_all_flags() {
        let options = run(&["--all", "--yes", "--verbose", "--verify-build"]).expect("parse");
        assert!(options.all);
        assert!(options.yes);
        assert!(options.verbose);
        assert!(options.verify_build);
    }

    #[test]
    fn rejects_conflicting_modes() {
        assert!(run(&["--check", "--dry-run"]).is_err());
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(run(&["--bogus"]).is_err());
    }

    #[test]
    fn non_interactive_requires_yes_or_a_read_only_mode() {
        assert!(run(&["--non-interactive"]).is_err());
        assert!(run(&["--non-interactive", "--check"]).is_ok());
        assert!(run(&["--non-interactive", "--yes"]).is_ok());
    }
}
