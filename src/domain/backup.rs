//! Pure rules shared by database export, import, and reset.
//!
//! A backup is a normal SQLite database. These helpers keep the size limit and
//! filename policy identical on web, desktop, mobile, and the remote server.

/// Default filename offered by the export dialog.
pub const DEFAULT_BACKUP_FILE_NAME: &str = "address-explorer.sqlite";

const ALLOWED_EXTENSIONS: [&str; 3] = ["sqlite", "sqlite3", "db"];
const MAX_FILE_NAME_LENGTH: usize = 200;

/// Validate and normalize a user-provided export filename.
///
/// The result is always a plain file name, never a path. A name without an
/// extension gains `.sqlite`; a name with an unsupported extension is rejected
/// so the downloaded file always identifies itself as a SQLite database.
pub fn normalize_backup_file_name(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return Err("Enter a file name.".to_string());
    }
    if trimmed == "." || trimmed == ".." {
        return Err("Enter a valid file name.".to_string());
    }
    if trimmed.contains(['/', '\\', '\0'])
        || trimmed.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        })
    {
        return Err(
            "The file name must not contain path separators or special characters.".to_string(),
        );
    }

    let normalized = match trimmed.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => {
            if ALLOWED_EXTENSIONS
                .iter()
                .any(|allowed| allowed.eq_ignore_ascii_case(extension))
            {
                trimmed.to_string()
            } else {
                return Err("Use a .sqlite, .sqlite3, or .db file name.".to_string());
            }
        }
        Some(_) => return Err("Use a .sqlite, .sqlite3, or .db file name.".to_string()),
        None => format!("{trimmed}.sqlite"),
    };

    if normalized.len() > MAX_FILE_NAME_LENGTH {
        return Err(format!(
            "The file name must be {MAX_FILE_NAME_LENGTH} characters or fewer."
        ));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_extension_gets_the_default_sqlite_extension() {
        assert_eq!(
            normalize_backup_file_name("backup").unwrap(),
            "backup.sqlite"
        );
        assert_eq!(
            normalize_backup_file_name("  report  ").unwrap(),
            "report.sqlite"
        );
    }

    #[test]
    fn accepted_extensions_are_preserved() {
        for name in ["backup.sqlite", "backup.sqlite3", "backup.db", "backup.DB"] {
            assert_eq!(normalize_backup_file_name(name).unwrap(), name);
        }
    }

    #[test]
    fn unknown_extensions_are_rejected_and_trailing_dots_are_trimmed() {
        assert!(normalize_backup_file_name("backup.txt").is_err());
        assert_eq!(
            normalize_backup_file_name("backup.").unwrap(),
            "backup.sqlite"
        );
    }

    #[test]
    fn paths_and_invalid_characters_are_rejected() {
        for name in [
            "",
            ".",
            "..",
            "../evil.sqlite",
            "a/b.sqlite",
            "a\\b.sqlite",
            "a:b.sqlite",
            "a?b.sqlite",
        ] {
            assert!(
                normalize_backup_file_name(name).is_err(),
                "accepted {name:?}"
            );
        }
    }

    #[test]
    fn overly_long_names_are_rejected() {
        let name = "a".repeat(MAX_FILE_NAME_LENGTH + 1);
        assert!(normalize_backup_file_name(&name).is_err());
    }
}
