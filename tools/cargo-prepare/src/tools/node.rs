use crate::context::{capture_stdout, which, CommandSpec, Context, Target};
use crate::platform::PkgManager;
use crate::system::{linux, macos, windows};

const MINIMUM_NODE_MAJOR: u32 = 20;

pub fn ensure(ctx: &mut Context) {
    let node = which("node");
    let npm = which("npm");
    let npx = which("npx");

    let node_version = node
        .as_ref()
        .and_then(|path| capture_stdout(&CommandSpec::new(path).arg("--version")));
    let node_major = node_version.as_deref().and_then(parse_major);

    let ready = node_version.is_some()
        && node_major.is_some_and(|major| major >= MINIMUM_NODE_MAJOR)
        && npm.is_some()
        && npx.is_some();

    if ready {
        ctx.already_present(
            Target::Web,
            "Node.js / npm",
            node_version.unwrap_or_default(),
        );
    } else {
        if ctx.opts.check {
            ctx.missing(
                Target::Web,
                "Node.js / npm",
                format!(
                    "Node.js {MINIMUM_NODE_MAJOR}+ with npm and npx is required by the Node tooling"
                ),
            );
            return;
        }

        if let Err(error) = install_node(ctx) {
            ctx.failed(Target::Web, "Node.js / npm", error);
            return;
        }

        if ctx.opts.dry_run {
            ctx.installed(Target::Web, "Node.js / npm", "would install Node.js LTS");
            return;
        }

        let node = which("node");
        let node_version = node
            .as_ref()
            .and_then(|path| capture_stdout(&CommandSpec::new(path).arg("--version")));
        let node_major = node_version.as_deref().and_then(parse_major);

        if node_version.is_none()
            || node_major.is_none_or(|major| major < MINIMUM_NODE_MAJOR)
            || which("npm").is_none()
            || which("npx").is_none()
        {
            ctx.failed(
                Target::Web,
                "Node.js / npm",
                format!(
                    "installed Node.js is missing or older than {MINIMUM_NODE_MAJOR}; install a current LTS from https://nodejs.org"
                ),
            );
            return;
        }

        ctx.installed(
            Target::Web,
            "Node.js / npm",
            node_version.unwrap_or_default(),
        );
    }
}

fn install_node(ctx: &mut Context) -> Result<(), String> {
    match ctx.platform.os {
        crate::platform::Os::Linux => {
            let Some(pkg) = ctx.platform.pkg else {
                return Err("no supported Linux package manager found".to_string());
            };
            let packages: &[&str] = match pkg {
                PkgManager::Apt
                | PkgManager::Dnf
                | PkgManager::Pacman
                | PkgManager::Zypper
                | PkgManager::Apk => &["nodejs", "npm"],
            };
            println!("Installing Node.js and npm...");
            linux::install_extra(ctx, packages)?;
            if ctx.opts.dry_run {
                return Ok(());
            }
            if which("node").is_none() {
                return Err(
                    "node is still missing after installation; install a current LTS from https://nodejs.org"
                        .to_string(),
                );
            }
            Ok(())
        }
        crate::platform::Os::Macos => {
            macos::ensure_formula(ctx, "node", "Node.js / npm", Target::Web)
        }
        crate::platform::Os::Windows => {
            if !windows::winget_installed("OpenJS.NodeJS.LTS") {
                windows::winget_install(ctx, "OpenJS.NodeJS.LTS", &[], "Node.js LTS")?;
            }
            if ctx.opts.dry_run {
                return Ok(());
            }
            add_windows_node_path();
            Ok(())
        }
        crate::platform::Os::Other => Err("unsupported operating system".to_string()),
    }
}

fn add_windows_node_path() {
    use std::path::PathBuf;

    let candidates = [
        std::env::var_os("ProgramFiles").map(|path| PathBuf::from(path).join("nodejs")),
        std::env::var_os("ProgramFiles(x86)").map(|path| PathBuf::from(path).join("nodejs")),
        std::env::var_os("LOCALAPPDATA")
            .map(|path| PathBuf::from(path).join("Programs").join("nodejs")),
    ];
    for directory in candidates.into_iter().flatten() {
        if directory.join("node.exe").is_file() {
            crate::context::append_path(&directory);
            break;
        }
    }
}

fn parse_major(version: &str) -> Option<u32> {
    version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .next()
        .and_then(|major| major.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_node_versions() {
        assert_eq!(parse_major("v26.13.0"), Some(26));
        assert_eq!(parse_major("20.1.2"), Some(20));
        assert_eq!(parse_major("not-a-version"), None);
    }
}
