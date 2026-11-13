//! Private file creation, permission hardening, and stale temp cleanup.

use crate::domain::security_settings::ExportFileMode;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const PRIVATE_FILE_MODE: u32 = 0o600;
const PRIVATE_DIR_MODE: u32 = 0o700;
const READ_ONLY_FILE_MODE: u32 = 0o400;
const TEMP_PREFIX: &str = ".address-explorer-";

/// Create a directory and, when it is new, restrict it to the owner on Unix.
pub fn ensure_private_dir(path: &Path) -> Result<(), String> {
    let existed = path.exists();
    fs::create_dir_all(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))?;
    #[cfg(unix)]
    if !existed {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
            .map_err(|error| format!("could not restrict {}: {error}", path.display()))?;
    }
    Ok(())
}

/// Restrict an existing directory to the owner on Unix.
#[allow(dead_code)]
pub fn restrict_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .map_err(|error| format!("could not restrict {}: {error}", path.display()))?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Create a new file with owner-only permissions.
pub fn create_private_file(path: &Path) -> Result<fs::File, String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(PRIVATE_FILE_MODE);
    options
        .open(path)
        .map_err(|error| format!("could not create {}: {error}", path.display()))
}

/// Write a file with owner-only permissions, replacing an existing file.
pub fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            ensure_private_dir(parent)?;
        }
    }
    let temporary = temporary_sibling(path);
    let _ = fs::remove_file(&temporary);
    {
        let mut file = create_private_file(&temporary)?;
        file.write_all(bytes)
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        file.sync_all()
            .map_err(|error| format!("could not flush {}: {error}", temporary.display()))?;
    }
    set_private_file(&temporary)?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("could not replace {}: {error}", path.display())
    })?;
    set_private_file(path)
}

/// Restrict an existing file to the owner.
pub fn set_private_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .map_err(|error| format!("could not restrict {}: {error}", path.display()))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

/// Apply the configured export mode after the file is complete.
pub fn apply_export_mode(path: &Path, mode: ExportFileMode) -> Result<(), String> {
    #[cfg(unix)]
    {
        let unix_mode = match mode {
            ExportFileMode::OwnerReadWrite => PRIVATE_FILE_MODE,
            ExportFileMode::OwnerReadOnly => READ_ONLY_FILE_MODE,
        };
        fs::set_permissions(path, fs::Permissions::from_mode(unix_mode))
            .map_err(|error| format!("could not set export permissions: {error}"))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
    Ok(())
}

/// Best-effort overwrite before unlinking a plaintext database or safety copy.
/// Filesystems with copy-on-write, snapshots, or wear leveling may retain old
/// blocks; this reduces but does not eliminate recoverable remnants.
pub fn best_effort_overwrite_and_remove(path: &Path) {
    if !path.is_file() {
        let _ = fs::remove_file(path);
        return;
    }
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new().read(true).write(true).open(path)?;
        let length = file.metadata()?.len();
        let zeros = [0u8; 64 * 1024];
        let mut written = 0u64;
        while written < length {
            let remaining = (length - written).min(zeros.len() as u64) as usize;
            file.write_all(&zeros[..remaining])?;
            written += remaining as u64;
        }
        file.sync_all()
    })();
    if let Err(error) = result {
        eprintln!("could not overwrite {}: {error}", path.display());
    }
    let _ = fs::remove_file(path);
}

/// Remove old Address Explorer temporary files left by a crash.
pub fn cleanup_stale_temp_files(directory: &Path, older_than_hours: u64) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let cutoff = Duration::from_secs(older_than_hours.saturating_mul(3600).max(3600));
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(TEMP_PREFIX) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        if now.duration_since(modified).unwrap_or_default() < cutoff {
            continue;
        }
        best_effort_overwrite_and_remove(&path);
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut sidecar = path.as_os_str().to_os_string();
            sidecar.push(suffix);
            let _ = fs::remove_file(PathBuf::from(sidecar));
        }
    }
}

fn temporary_sibling(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    parent.join(format!(
        ".address-explorer-{name}-{}.tmp",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "address-explorer-files-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap(); // allow-unwrap: test setup.
        path
    }

    #[cfg(unix)]
    #[test]
    fn private_files_and_directories_are_owner_only() {
        let directory = temp_dir("permissions");
        let _ = fs::remove_dir_all(&directory);
        ensure_private_dir(&directory).unwrap(); // allow-unwrap: test setup.
        let file = directory.join("secret");
        write_private(&file, b"secret").unwrap(); // allow-unwrap: test setup.
        let directory_mode = fs::metadata(&directory).unwrap(); // allow-unwrap: test setup.
        assert_eq!(directory_mode.permissions().mode() & 0o777, 0o700);
        let file_mode = fs::metadata(&file).unwrap(); // allow-unwrap: test setup.
        assert_eq!(file_mode.permissions().mode() & 0o777, 0o600);
        apply_export_mode(&file, ExportFileMode::OwnerReadOnly).unwrap(); // allow-unwrap: test setup.
        let exported_mode = fs::metadata(&file).unwrap(); // allow-unwrap: test setup.
        assert_eq!(exported_mode.permissions().mode() & 0o777, 0o400);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn stale_temp_cleanup_removes_old_files() {
        let directory = temp_dir("cleanup");
        let old = directory.join(".address-explorer-old.sqlite");
        fs::write(&old, b"old").unwrap(); // allow-unwrap: test setup.
        let now = SystemTime::now() - Duration::from_secs(7200);
        let _ = filetime_set(&old, now);
        cleanup_stale_temp_files(&directory, 1);
        assert!(!old.exists());
        let _ = fs::remove_dir_all(directory);
    }

    fn filetime_set(path: &Path, time: SystemTime) -> std::io::Result<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        file.set_modified(time)
    }
}
