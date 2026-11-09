//! Export detected tool locations to GitHub Actions environment files.
//!
//! Android SDK/NDK paths and the wasm compiler are discovered while
//! `cargo prepare` runs. GitHub Actions steps are separate processes, so the
//! values must be written to `GITHUB_ENV`/`GITHUB_PATH` to survive into later
//! build steps.

use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use crate::context::Context;

/// Append the detected compiler and Android environment to the GitHub files.
pub fn export(ctx: &Context, include_web_tools: bool) -> Result<(), String> {
    if ctx.opts.dry_run {
        return Ok(());
    }

    let env_file = env::var_os("GITHUB_ENV");
    let path_file = env::var_os("GITHUB_PATH");
    if env_file.is_none() && path_file.is_none() {
        return Ok(());
    }

    let mut assignments = Vec::new();
    if include_web_tools {
        if let Some(tools) = &ctx.web_tools {
            assignments.push(("CC".to_string(), tools.clang.display().to_string()));
            assignments.push(("AR".to_string(), tools.archiver.display().to_string()));
        }
    }
    for (key, value) in &ctx.env_to_persist {
        assignments.push((key.clone(), value.clone()));
    }

    if let Some(path) = env_file {
        append_lines(
            Path::new(&path),
            assignments
                .iter()
                .map(|(key, value)| format!("{key}={value}")),
        )?;
    }

    if let Some(path) = path_file {
        append_lines(
            Path::new(&path),
            ctx.path_to_persist
                .iter()
                .map(|path| path.display().to_string()),
        )?;
    }

    Ok(())
}

fn append_lines(path: &Path, lines: impl IntoIterator<Item = String>) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;

    for line in lines {
        writeln!(file, "{line}")
            .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_environment_lines_without_overwriting() {
        let path = env::temp_dir().join(format!("cargo-prepare-github-env-{}", std::process::id()));
        std::fs::write(&path, "EXISTING=1\n").expect("seed file");

        append_lines(
            &path,
            [
                "CC=/usr/bin/clang".to_string(),
                "AR=/usr/bin/llvm-ar".to_string(),
            ],
        )
        .expect("append");

        let contents = std::fs::read_to_string(&path).expect("read");
        assert!(contents.starts_with("EXISTING=1\n"));
        assert!(contents.contains("CC=/usr/bin/clang\n"));
        assert!(contents.contains("AR=/usr/bin/llvm-ar\n"));

        let _ = std::fs::remove_file(path);
    }
}
