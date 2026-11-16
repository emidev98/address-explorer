use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::context::{download, which, Context, Status, Target};

/// Release pinned by `build.rs`; installed into the Dioxus tools directory.
pub const TAILWIND_VERSION: &str = "v4.1.5";

pub fn ensure(ctx: &mut Context) {
    if let Some(explicit) = env::var_os("TAILWIND_BIN") {
        let path = PathBuf::from(explicit);
        if works(&path) {
            ctx.ready(Target::Common, "Tailwind CLI", path.display().to_string());
        } else {
            ctx.failed(
                Target::Web,
                "Tailwind CLI",
                format!(
                    "TAILWIND_BIN points at {}, but it is not a working Tailwind CLI",
                    path.display()
                ),
            );
        }
        return;
    }

    let Some(binary) = managed_binary() else {
        ctx.record(
            Target::Web,
            "Tailwind CLI",
            Status::Manual,
            true,
            "set DX_HOME or TAILWIND_BIN so Tailwind can be found",
        );
        return;
    };

    if binary.is_file() && works(&binary) {
        ctx.already_present(Target::Common, "Tailwind CLI", binary.display().to_string());
        return;
    }

    if ctx.opts.check {
        if let Some(path) = which("tailwindcss") {
            if works(&path) {
                ctx.ready(Target::Common, "Tailwind CLI", path.display().to_string());
                return;
            }
        }
        ctx.missing(
            Target::Web,
            "Tailwind CLI",
            format!("Tailwind {TAILWIND_VERSION} is not installed; run `cargo prepare`"),
        );
        return;
    }

    if ctx.opts.dry_run {
        ctx.installed(
            Target::Web,
            "Tailwind CLI",
            format!(
                "would install Tailwind {TAILWIND_VERSION} to {}",
                binary.display()
            ),
        );
        return;
    }

    match install(ctx, &binary) {
        Ok(()) => ctx.installed(
            Target::Web,
            "Tailwind CLI",
            format!("Tailwind {TAILWIND_VERSION} at {}", binary.display()),
        ),
        Err(error) => ctx.failed(Target::Common, "Tailwind CLI", error),
    }
}

fn install(ctx: &Context, binary: &Path) -> Result<(), String> {
    let url = download_url().ok_or_else(|| {
        format!(
            "no Tailwind {TAILWIND_VERSION} binary is published for {}-{}; set TAILWIND_BIN",
            env::consts::OS,
            env::consts::ARCH
        )
    })?;

    if let Some(parent) = binary.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }

    println!("Installing Tailwind CSS {TAILWIND_VERSION}...");
    let temporary = binary.with_file_name(format!("{}.download", binary_name()));
    download(ctx, &url, &temporary)?;
    make_executable(&temporary)?;

    let _ = fs::remove_file(binary);
    fs::rename(&temporary, binary)
        .map_err(|error| format!("could not install {}: {error}", binary.display()))?;

    if !works(binary) {
        let _ = fs::remove_file(binary);
        return Err("the downloaded Tailwind CLI did not run".to_string());
    }

    Ok(())
}

fn managed_binary() -> Option<PathBuf> {
    Some(
        dx_tools_dir()?
            .join(format!("tailwindcss-{TAILWIND_VERSION}"))
            .join(binary_name()),
    )
}

fn dx_tools_dir() -> Option<PathBuf> {
    if let Some(home) = env::var_os("DX_HOME") {
        return Some(PathBuf::from(home).join("tools"));
    }

    let home = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE"))?;
    let data_dir = if cfg!(any(target_os = "macos", target_os = "windows")) {
        PathBuf::from(home).join(".dx")
    } else {
        env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(&home).join(".local").join("share"))
            .join(".dx")
    };
    Some(data_dir.join("tools"))
}

fn binary_name() -> &'static str {
    if cfg!(windows) {
        "tailwindcss.exe"
    } else {
        "tailwindcss"
    }
}

fn download_url() -> Option<String> {
    let platform = match env::consts::OS {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => return None,
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "x64",
        // Tailwind publishes no Windows arm64 build; the x64 binary runs there.
        "aarch64" if platform == "windows" => "x64",
        "aarch64" => "arm64",
        _ => return None,
    };
    let musl = if platform == "linux" && cfg!(target_env = "musl") {
        "-musl"
    } else {
        ""
    };
    let suffix = if platform == "windows" { ".exe" } else { "" };
    Some(format!(
        "https://github.com/tailwindlabs/tailwindcss/releases/download/{TAILWIND_VERSION}/tailwindcss-{platform}-{arch}{musl}{suffix}"
    ))
}

fn works(path: &Path) -> bool {
    Command::new(path)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("could not make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_targets_the_pinned_release() {
        let Some(url) = download_url() else {
            return;
        };
        assert!(url.starts_with("https://github.com/tailwindlabs/tailwindcss/releases/"));
        assert!(url.contains("/download/v4.1.5/tailwindcss-"));
    }
}
