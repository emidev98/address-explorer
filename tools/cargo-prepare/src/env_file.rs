use std::fs;
use std::path::Path;

use crate::context::Context;
use crate::platform::Shell;

pub const START_MARKER: &str = "# >>> address-explorer prepare >>>";
pub const END_MARKER: &str = "# <<< address-explorer prepare <<<";

pub enum PersistOutcome {
    Written(String),
    AlreadyConfigured(String),
    WouldWrite(String, String),
    NoProfile,
}

pub fn persist_android(
    ctx: &Context,
    sdk: &Path,
    ndk: &Path,
    java_home: &Path,
) -> Result<PersistOutcome, String> {
    let Some(path) = ctx.platform.profile_file() else {
        return Ok(PersistOutcome::NoProfile);
    };

    let block = render_block(ctx.platform.shell, sdk, ndk, java_home);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let already = existing.contains(START_MARKER)
        && existing.contains(&sdk.display().to_string())
        && existing.contains(&ndk.display().to_string())
        && existing.contains(&java_home.display().to_string());

    if already {
        return Ok(PersistOutcome::AlreadyConfigured(
            path.display().to_string(),
        ));
    }

    if ctx.opts.check {
        return Err(format!(
            "{} does not contain the Android environment block\n       add it with `cargo prepare --all`",
            path.display()
        ));
    }

    if ctx.opts.dry_run {
        return Ok(PersistOutcome::WouldWrite(
            path.display().to_string(),
            block,
        ));
    }

    write_block(&path, &block)?;
    Ok(PersistOutcome::Written(path.display().to_string()))
}

pub fn render_block(shell: Shell, sdk: &Path, ndk: &Path, java_home: &Path) -> String {
    let sdk = sdk.display().to_string();
    let ndk = ndk.display().to_string();
    let java_home = java_home.display().to_string();

    match shell {
        Shell::Fish => format!(
            "{START_MARKER}\n\
             set -gx JAVA_HOME '{}'\n\
             set -gx ANDROID_HOME '{}'\n\
             set -gx ANDROID_SDK_ROOT '{}'\n\
             set -gx ANDROID_NDK_HOME '{}'\n\
             set -gx NDK_HOME '{}'\n\
             contains -- \"$ANDROID_HOME/platform-tools\" $PATH; or set -gx PATH $PATH \"$ANDROID_HOME/platform-tools\" \"$ANDROID_HOME/cmdline-tools/latest/bin\"\n\
             {END_MARKER}\n",
            fish_quote(&java_home),
            fish_quote(&sdk),
            fish_quote(&sdk),
            fish_quote(&ndk),
            fish_quote(&ndk),
        ),
        _ => format!(
            "{START_MARKER}\n\
             export JAVA_HOME='{}'\n\
             export ANDROID_HOME='{}'\n\
             export ANDROID_SDK_ROOT='{}'\n\
             export ANDROID_NDK_HOME='{}'\n\
             export NDK_HOME='{}'\n\
             case \":$PATH:\" in\n  *\":$ANDROID_HOME/platform-tools:\"*) ;;\n  *) export PATH=\"$PATH:$ANDROID_HOME/platform-tools:$ANDROID_HOME/cmdline-tools/latest/bin\" ;;\nesac\n\
             {END_MARKER}\n",
            posix_quote(&java_home),
            posix_quote(&sdk),
            posix_quote(&sdk),
            posix_quote(&ndk),
            posix_quote(&ndk),
        ),
    }
}

fn write_block(path: &Path, block: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }

    let existing = fs::read_to_string(path).unwrap_or_default();
    let mut updated = strip_block(&existing);
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.is_empty() {
        updated.push('\n');
    }
    updated.push_str(block);
    fs::write(path, updated).map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn strip_block(contents: &str) -> String {
    let Some(start) = contents.find(START_MARKER) else {
        return contents.to_string();
    };
    let Some(relative_end) = contents[start..].find(END_MARKER) else {
        return contents[..start].to_string();
    };
    let end = start + relative_end + END_MARKER.len();
    let mut result = String::new();
    result.push_str(&contents[..start]);
    result.push_str(contents[end..].trim_start_matches(['\r', '\n']));
    result
}

fn posix_quote(value: &str) -> String {
    value.replace('\'', "'\\''")
}

fn fish_quote(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_the_previous_block_and_keeps_other_lines() {
        let existing = "export EDITOR=vim\n\n# >>> address-explorer prepare >>>\nexport OLD=1\n# <<< address-explorer prepare <<<\n\nexport PAGER=less\n";
        let stripped = strip_block(existing);
        assert!(stripped.contains("export EDITOR=vim"));
        assert!(stripped.contains("export PAGER=less"));
        assert!(!stripped.contains("export OLD=1"));
        assert!(!stripped.contains(START_MARKER));
    }

    #[test]
    fn renders_posix_and_fish_blocks() {
        let sdk = Path::new("/home/user/Android/Sdk");
        let ndk = Path::new("/home/user/Android/Sdk/ndk/25.2.9519653");
        let java = Path::new("/usr/lib/jvm/java-17-openjdk");

        let posix = render_block(Shell::Bash, sdk, ndk, java);
        assert!(posix.contains("export ANDROID_HOME='/home/user/Android/Sdk'"));
        assert!(posix.contains("export JAVA_HOME='/usr/lib/jvm/java-17-openjdk'"));
        assert!(posix.contains(START_MARKER));

        let fish = render_block(Shell::Fish, sdk, ndk, java);
        assert!(fish.contains("set -gx ANDROID_HOME '/home/user/Android/Sdk'"));
        assert!(fish.contains("fish") || fish.contains("set -gx PATH"));
    }

    #[test]
    fn quotes_paths_with_spaces_and_quotes() {
        assert_eq!(
            posix_quote("/tmp/with space/it's here"),
            "/tmp/with space/it'\\''s here"
        );
        assert_eq!(
            fish_quote("/tmp/with space/it's here"),
            "/tmp/with space/it\\'s here"
        );
    }
}
