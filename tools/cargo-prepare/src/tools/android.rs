use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::context::{
    append_path, capture_stdout, run, run_output, which, CommandSpec, Context, Status, Target,
};
use crate::env_file::{self, PersistOutcome};
use crate::platform::{Os, PkgManager};
use crate::system::{linux, macos, windows};

const CMDLINE_TOOLS_BASE: &str = "https://dl.google.com/android/repository";
const NDK_VERSION: &str = "25.2.9519653";
const SDK_PACKAGES: &[&str] = &[
    "platform-tools",
    "platforms;android-34",
    "build-tools;34.0.0",
    "ndk;25.2.9519653",
    "cmake;3.22.1",
];

pub fn ensure(ctx: &mut Context) {
    if !ctx.opts.all {
        ctx.skipped(
            Target::Android,
            "Android toolchain",
            "not requested; pass --all",
        );
        return;
    }

    if !ctx.platform.android_supported() {
        ctx.unsupported(
            Target::Android,
            "Android toolchain",
            format!(
                "Android SDK/NDK host tools are not available for {} {}; use an x86_64 Linux host/container",
                os_name(ctx.platform.os),
                ctx.platform.arch
            ),
        );
        return;
    }

    let sdk = sdk_root(ctx);
    ctx.android_sdk = Some(sdk.clone());

    let Some(java_home) = ensure_jdk(ctx) else {
        return;
    };
    ctx.java_home = Some(java_home.clone());

    let Some(sdkmanager) = ensure_cmdline_tools(ctx, &sdk) else {
        return;
    };

    check_or_install_packages(ctx, &sdk, &sdkmanager, &java_home);

    let ndk = sdk.join("ndk").join(NDK_VERSION);
    ctx.android_ndk = Some(ndk.clone());
    apply_env(ctx, &sdk, &ndk, &java_home);
    persist_env(ctx, &sdk, &ndk, &java_home);
    verify_adb(ctx, &sdk);
}

fn os_name(os: Os) -> &'static str {
    match os {
        Os::Linux => "Linux",
        Os::Macos => "macOS",
        Os::Windows => "Windows",
        Os::Other => "unknown",
    }
}

fn sdk_root(ctx: &Context) -> PathBuf {
    for key in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Some(value) = env::var_os(key) {
            let path = PathBuf::from(value);
            if !path.as_os_str().is_empty() {
                return path;
            }
        }
    }

    match ctx.platform.os {
        Os::Macos => home_dir().join("Library/Android/sdk"),
        Os::Windows => env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join("AppData/Local"))
            .join("Android/Sdk"),
        _ => home_dir().join("Android/Sdk"),
    }
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn ensure_jdk(ctx: &mut Context) -> Option<PathBuf> {
    if let Some(home) = java_home_candidate(ctx) {
        if java_major(&home).is_some_and(|major| major >= 17) {
            ctx.already_present(Target::Android, "JDK 17", home.display().to_string());
            return Some(home);
        }
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Android,
            "JDK 17",
            "install OpenJDK 17 (or set JAVA_HOME) for the Android SDK",
        );
        return None;
    }

    println!("Installing JDK 17...");
    let result = install_jdk(ctx);
    if let Err(error) = result {
        ctx.failed(Target::Android, "JDK 17", error);
        return None;
    }

    if ctx.opts.dry_run {
        ctx.installed(Target::Android, "JDK 17", "would install JDK 17");
        return Some(PathBuf::from("<jdk-17>"));
    }

    let Some(home) = java_home_candidate(ctx) else {
        ctx.failed(
            Target::Android,
            "JDK 17",
            "JDK 17 was installed but JAVA_HOME could not be detected; set JAVA_HOME manually",
        );
        return None;
    };

    if !java_major(&home).is_some_and(|major| major >= 17) {
        ctx.failed(
            Target::Android,
            "JDK 17",
            format!("{} does not look like JDK 17+", home.display()),
        );
        return None;
    }

    ctx.installed(Target::Android, "JDK 17", home.display().to_string());
    Some(home)
}

fn java_home_candidate(ctx: &Context) -> Option<PathBuf> {
    let usable = |path: &Path| {
        path.join("bin/java").is_file() && java_major(path).is_some_and(|major| major >= 17)
    };

    if let Some(home) = env::var_os("JAVA_HOME") {
        let path = PathBuf::from(home);
        if usable(&path) {
            return Some(path);
        }
    }

    if ctx.platform.is_macos() {
        if let Some(home) =
            capture_stdout(&CommandSpec::new("/usr/libexec/java_home").args(["-v", "17"]))
        {
            let path = PathBuf::from(home);
            if usable(&path) {
                return Some(path);
            }
        }
    }

    if ctx.platform.is_linux() {
        for root in ["/usr/lib/jvm", "/usr/java"] {
            let Ok(entries) = fs::read_dir(root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("17"))
                    && usable(&path)
                {
                    return Some(path);
                }
            }
        }
    }

    if let Some(java) = which("java") {
        if let Some(home) = java_home_from_binary(&java) {
            if usable(&home) {
                return Some(home);
            }
        }
        if let Some(parent) = java.parent().and_then(Path::parent) {
            if usable(parent) {
                return Some(parent.to_path_buf());
            }
        }
    }

    if ctx.platform.is_windows() {
        return find_windows_jdk();
    }

    None
}

fn install_jdk(ctx: &mut Context) -> Result<(), String> {
    match ctx.platform.os {
        Os::Linux => {
            let Some(pkg) = ctx.platform.pkg else {
                return Err("no supported Linux package manager found".to_string());
            };
            let package = match pkg {
                PkgManager::Apt => "openjdk-17-jdk",
                PkgManager::Dnf => "java-17-openjdk-devel",
                PkgManager::Pacman => "jdk17-openjdk",
                PkgManager::Zypper => "java-17-openjdk-devel",
                PkgManager::Apk => "openjdk17",
            };
            linux::install_extra(ctx, &[package])
        }
        Os::Macos => {
            macos::ensure_formula(ctx, "openjdk@17", "JDK 17", Target::Android)?;
            if ctx.opts.dry_run {
                return Ok(());
            }
            let brew = ctx
                .brew
                .clone()
                .or_else(macos::find_brew)
                .ok_or_else(|| "Homebrew is not available".to_string())?;
            let prefix = macos::homebrew_prefix(&brew, "openjdk@17")
                .ok_or_else(|| "openjdk@17 was not installed by Homebrew".to_string())?;
            let home = prefix.join("libexec/openjdk.jdk/Contents/Home");
            if !home.join("bin/java").is_file() {
                return Err(format!(
                    "expected a JDK at {}, but bin/java is missing",
                    home.display()
                ));
            }
            env::set_var("JAVA_HOME", &home);
            Ok(())
        }
        Os::Windows => {
            if !windows::winget_installed("EclipseAdoptium.Temurin.17.JDK") {
                windows::winget_install(ctx, "EclipseAdoptium.Temurin.17.JDK", &[], "JDK 17")?;
            }
            if ctx.opts.dry_run {
                return Ok(());
            }
            let home = find_windows_jdk().ok_or_else(|| {
                "JDK 17 was installed but no Temurin 17 directory was found".to_string()
            })?;
            env::set_var("JAVA_HOME", &home);
            Ok(())
        }
        Os::Other => Err("unsupported operating system".to_string()),
    }
}

fn find_windows_jdk() -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(program_files) = env::var_os("ProgramFiles") {
        let program_files = PathBuf::from(program_files);
        roots.push(program_files.join("Eclipse Adoptium"));
        roots.push(program_files.join("Java"));
    }
    if let Some(program_files) = env::var_os("ProgramFiles(x86)") {
        let program_files = PathBuf::from(program_files);
        roots.push(program_files.join("Eclipse Adoptium"));
        roots.push(program_files.join("Java"));
    }

    let mut candidates = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.join("bin/java.exe").is_file() {
                candidates.push(path);
            }
        }
    }

    candidates.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    let preferred = candidates
        .iter()
        .find(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().contains("17"))
        })
        .cloned();
    preferred.or_else(|| candidates.into_iter().next())
}

fn java_major(home: &Path) -> Option<u32> {
    let java = home.join("bin/java");
    let output = run_output(&CommandSpec::new(&java).arg("-version")).ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let first = text.lines().next()?;
    let start = first.find('"')?;
    let rest = &first[start + 1..];
    let end = rest.find('"')?;
    let version = &rest[..end];
    let major = version.split('.').next()?.parse().ok()?;
    if major == 1 {
        version.split('.').nth(1)?.parse().ok()
    } else {
        Some(major)
    }
}

fn java_home_from_binary(java: &Path) -> Option<PathBuf> {
    let output =
        run_output(&CommandSpec::new(java).args(["-XshowSettings:properties", "-version"])).ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    text.lines().find_map(|line| {
        line.trim()
            .strip_prefix("java.home =")
            .map(|value| PathBuf::from(value.trim()))
            .filter(|path| path.join("bin/java").is_file())
    })
}

fn ensure_cmdline_tools(ctx: &mut Context, sdk: &Path) -> Option<PathBuf> {
    let sdkmanager =
        sdk.join("cmdline-tools")
            .join("latest")
            .join("bin")
            .join(if ctx.platform.is_windows() {
                "sdkmanager.bat"
            } else {
                "sdkmanager"
            });

    if sdkmanager.is_file() {
        ctx.already_present(
            Target::Android,
            "Android command-line tools",
            sdkmanager.display().to_string(),
        );
        return Some(sdkmanager);
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Android,
            "Android command-line tools",
            "install with `cargo prepare --all`",
        );
        return None;
    }

    let Some((url, sha1)) = cmdline_tools_artifact(ctx) else {
        ctx.unsupported(
            Target::Android,
            "Android command-line tools",
            "no Google command-line tools archive is available for this host",
        );
        return None;
    };

    println!("Downloading Android command-line tools...");
    if ctx.opts.dry_run {
        println!("  + download {url}");
        println!(
            "  + extract to {}",
            sdk.join("cmdline-tools/latest").display()
        );
        ctx.installed(
            Target::Android,
            "Android command-line tools",
            "would install the Google command-line tools",
        );
        return Some(sdkmanager);
    }

    let zip = env::temp_dir().join("address-explorer-commandlinetools.zip");
    if let Err(error) = download(ctx, &url, &zip) {
        ctx.failed(Target::Android, "Android command-line tools", error);
        return None;
    }
    if let Err(error) = verify_sha1(&zip, sha1) {
        ctx.failed(Target::Android, "Android command-line tools", error);
        return None;
    }

    if let Err(error) = fs::create_dir_all(sdk) {
        ctx.failed(
            Target::Android,
            "Android command-line tools",
            format!("could not create {}: {error}", sdk.display()),
        );
        return None;
    }
    // Keep the extraction on the SDK filesystem so the final rename is atomic.
    let extract = sdk.join(format!(".cmdline-tools-extract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&extract);
    if let Err(error) = extract_zip(ctx, &zip, &extract) {
        ctx.failed(Target::Android, "Android command-line tools", error);
        return None;
    }

    let extracted = extract.join("cmdline-tools");
    let destination = sdk.join("cmdline-tools").join("latest");
    let result = (|| -> Result<(), String> {
        fs::create_dir_all(sdk.join("cmdline-tools"))
            .map_err(|error| format!("could not create {}: {error}", sdk.display()))?;
        if destination.exists() {
            fs::remove_dir_all(&destination)
                .map_err(|error| format!("could not replace {}: {error}", destination.display()))?;
        }
        fs::rename(&extracted, &destination).map_err(|error| {
            format!(
                "could not move {} to {}: {error}",
                extracted.display(),
                destination.display()
            )
        })?;
        Ok(())
    })();

    let _ = fs::remove_dir_all(&extract);
    let _ = fs::remove_file(&zip);

    if let Err(error) = result {
        ctx.failed(Target::Android, "Android command-line tools", error);
        return None;
    }
    if !sdkmanager.is_file() {
        ctx.failed(
            Target::Android,
            "Android command-line tools",
            format!("sdkmanager was not found at {}", sdkmanager.display()),
        );
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(&sdkmanager) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            let _ = fs::set_permissions(&sdkmanager, permissions);
        }
    }

    ctx.installed(
        Target::Android,
        "Android command-line tools",
        sdkmanager.display().to_string(),
    );
    Some(sdkmanager)
}

fn cmdline_tools_artifact(ctx: &Context) -> Option<(String, &'static str)> {
    let arch = ctx.platform.arch.as_str();
    let (file, sha1) = match ctx.platform.os {
        Os::Linux if arch == "x86_64" => (
            "commandlinetools-linux-16111833_latest.zip",
            "e025545c62a8e64c7559119566a569fb1dec5f60",
        ),
        Os::Macos if arch == "x86_64" => (
            "commandlinetools-mac_x86_64-16111833_latest.zip",
            "112cf9618794a997ff273537d55bee02c22abffe",
        ),
        Os::Macos if arch == "aarch64" => (
            "commandlinetools-mac_arm64-16111833_latest.zip",
            "ad03dc49bfacfd52c110b14104ea548b8a07e830",
        ),
        Os::Windows if arch == "x86_64" => (
            "commandlinetools-win-16111833_latest.zip",
            "57d04f2d75eb8e8fffc5000a987e5de4b5a63e9d",
        ),
        _ => return None,
    };
    Some((format!("{CMDLINE_TOOLS_BASE}/{file}"), sha1))
}

fn check_or_install_packages(ctx: &mut Context, sdk: &Path, sdkmanager: &Path, java_home: &Path) {
    let listed =
        sdkmanager_output(sdk, sdkmanager, java_home, &["--list_installed"]).unwrap_or_default();
    let missing: Vec<&str> = SDK_PACKAGES
        .iter()
        .copied()
        .filter(|package| !listed.lines().any(|line| line.contains(package)))
        .collect();

    if missing.is_empty() {
        ctx.already_present(
            Target::Android,
            "Android SDK packages",
            SDK_PACKAGES.join(", "),
        );
        return;
    }

    if ctx.opts.check {
        ctx.missing(
            Target::Android,
            "Android SDK packages",
            format!("missing: {}", missing.join(", ")),
        );
        return;
    }

    if let Err(error) = accept_licenses(ctx, sdk, sdkmanager, java_home) {
        ctx.failed(Target::Android, "Android SDK licenses", error);
        return;
    }

    println!("Installing Android SDK packages (platform 34, build-tools, NDK, CMake)...");
    let mut spec = sdkmanager_spec(sdkmanager, java_home, sdk);
    for package in &missing {
        spec = spec.arg(*package);
    }
    match run(ctx, &spec) {
        Ok(()) => ctx.installed(Target::Android, "Android SDK packages", missing.join(", ")),
        Err(error) => ctx.failed(
            Target::Android,
            "Android SDK packages",
            format!(
                "{error}\n       retry: {} --sdk_root={} {}",
                sdkmanager.display(),
                sdk.display(),
                missing.join(" ")
            ),
        ),
    }
}

fn accept_licenses(
    ctx: &mut Context,
    sdk: &Path,
    sdkmanager: &Path,
    java_home: &Path,
) -> Result<(), String> {
    if ctx.opts.check {
        return Ok(());
    }
    println!("Accepting Android SDK licenses...");
    let spec = sdkmanager_spec(sdkmanager, java_home, sdk)
        .arg("--licenses")
        .yes_stdin();
    run(ctx, &spec)
}

fn sdkmanager_output(
    sdk: &Path,
    sdkmanager: &Path,
    java_home: &Path,
    args: &[&str],
) -> Option<String> {
    let mut spec = sdkmanager_spec(sdkmanager, java_home, sdk);
    for arg in args {
        spec = spec.arg(*arg);
    }
    let output = run_output(&spec).ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn sdkmanager_spec(sdkmanager: &Path, java_home: &Path, sdk: &Path) -> CommandSpec {
    let mut spec = if cfg!(windows) {
        CommandSpec::new("cmd").arg("/C").arg(sdkmanager)
    } else {
        CommandSpec::new(sdkmanager)
    };
    spec = spec
        .arg(format!("--sdk_root={}", sdk.display()))
        .env("JAVA_HOME", java_home);
    spec
}

fn apply_env(ctx: &mut Context, sdk: &Path, ndk: &Path, java_home: &Path) {
    let values = [
        ("JAVA_HOME", java_home.to_path_buf()),
        ("ANDROID_HOME", sdk.to_path_buf()),
        ("ANDROID_SDK_ROOT", sdk.to_path_buf()),
        ("ANDROID_NDK_HOME", ndk.to_path_buf()),
        ("NDK_HOME", ndk.to_path_buf()),
    ];

    for (key, value) in &values {
        env::set_var(key, value);
        ctx.env_to_persist
            .push((key.to_string(), value.display().to_string()));
    }

    let directories = [
        sdk.join("platform-tools"),
        sdk.join("cmdline-tools").join("latest").join("bin"),
    ];
    for directory in &directories {
        append_path(directory);
        if !ctx.path_to_persist.iter().any(|path| path == directory) {
            ctx.path_to_persist.push(directory.clone());
        }
    }

    ctx.ready(
        Target::Android,
        "Android environment variables",
        format!(
            "ANDROID_HOME={}; JAVA_HOME={}",
            sdk.display(),
            java_home.display()
        ),
    );
}

fn persist_env(ctx: &mut Context, sdk: &Path, ndk: &Path, java_home: &Path) {
    if ctx.platform.is_windows() {
        let values = [
            ("JAVA_HOME", java_home.to_path_buf()),
            ("ANDROID_HOME", sdk.to_path_buf()),
            ("ANDROID_SDK_ROOT", sdk.to_path_buf()),
            ("ANDROID_NDK_HOME", ndk.to_path_buf()),
            ("NDK_HOME", ndk.to_path_buf()),
        ];
        if ctx.opts.check {
            let expected = [
                ("JAVA_HOME", java_home),
                ("ANDROID_HOME", sdk),
                ("ANDROID_NDK_HOME", ndk),
            ];
            let missing: Vec<&str> = expected
                .iter()
                .filter(|(name, value)| {
                    windows::user_env(name)
                        .map(|current| current != value.display().to_string())
                        .unwrap_or(true)
                })
                .map(|(name, _)| *name)
                .collect();
            if missing.is_empty() {
                ctx.ready(
                    Target::Android,
                    "Android environment persistence",
                    "Windows user environment is configured",
                );
            } else {
                ctx.missing(
                    Target::Android,
                    "Android environment persistence",
                    format!(
                        "missing or stale Windows user variables: {}; run `cargo prepare --all`",
                        missing.join(", ")
                    ),
                );
            }
            return;
        }

        for (key, value) in &values {
            if let Err(error) = windows::set_user_env(ctx, key, value) {
                ctx.failed(
                    Target::Android,
                    "Android environment persistence",
                    format!("could not set {key}: {error}"),
                );
                return;
            }
        }
        for directory in [
            sdk.join("platform-tools"),
            sdk.join("cmdline-tools").join("latest").join("bin"),
        ] {
            if let Err(error) = windows::add_user_path(ctx, &directory) {
                ctx.failed(
                    Target::Android,
                    "Android environment persistence",
                    format!("could not add {} to PATH: {error}", directory.display()),
                );
                return;
            }
        }
        ctx.env_written = true;
        ctx.installed(
            Target::Android,
            "Android environment persistence",
            "Windows user environment updated; open a new terminal",
        );
        return;
    }

    match env_file::persist_android(ctx, sdk, ndk, java_home) {
        Ok(PersistOutcome::Written(path)) => {
            ctx.env_written = true;
            ctx.installed(
                Target::Android,
                "Android environment persistence",
                format!("wrote the marked block to {path}"),
            );
        }
        Ok(PersistOutcome::AlreadyConfigured(path)) => {
            ctx.already_present(Target::Android, "Android environment persistence", path);
        }
        Ok(PersistOutcome::WouldWrite(path, block)) => {
            ctx.ready(
                Target::Android,
                "Android environment persistence",
                format!("would write to {path}:\n{block}"),
            );
        }
        Ok(PersistOutcome::NoProfile) => {
            let block = env_file::render_block(ctx.platform.shell, sdk, ndk, java_home);
            println!("\nAdd these exports to your shell profile:\n{block}");
            ctx.record(
                Target::Android,
                "Android environment persistence",
                Status::Manual,
                false,
                "unsupported shell; exports printed above and not persisted",
            );
        }
        Err(error) => {
            if ctx.opts.check {
                ctx.missing(Target::Android, "Android environment persistence", error);
            } else {
                ctx.failed(Target::Android, "Android environment persistence", error);
            }
        }
    }
}

fn verify_adb(ctx: &mut Context, sdk: &Path) {
    let adb = sdk
        .join("platform-tools")
        .join(if ctx.platform.is_windows() {
            "adb.exe"
        } else {
            "adb"
        });
    if ctx.opts.dry_run {
        ctx.ready(
            Target::Android,
            "adb",
            format!("would install {}", adb.display()),
        );
        return;
    }
    if adb.is_file() {
        ctx.ready(Target::Android, "adb", adb.display().to_string());
    } else if ctx.opts.check {
        ctx.missing(Target::Android, "adb", "platform-tools are not installed");
    } else {
        ctx.failed(
            Target::Android,
            "adb",
            format!(
                "{} is missing after installing platform-tools",
                adb.display()
            ),
        );
    }
}

fn download(ctx: &Context, url: &str, destination: &Path) -> Result<(), String> {
    if ctx.platform.is_windows() {
        let script = "Invoke-WebRequest -Uri $env:AE_PREPARE_URL -OutFile $env:AE_PREPARE_DEST";
        return run(
            ctx,
            &CommandSpec::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("AE_PREPARE_URL", url)
                .env("AE_PREPARE_DEST", destination.as_os_str()),
        );
    }

    let curl = CommandSpec::new("curl")
        .args(["--fail", "--location", "--retry", "3", "--output"])
        .arg(destination)
        .arg(url);
    if run(ctx, &curl).is_ok() {
        return Ok(());
    }

    println!("curl failed; retrying the download with wget...");
    run(
        ctx,
        &CommandSpec::new("wget").arg("-O").arg(destination).arg(url),
    )
}

fn extract_zip(ctx: &Context, zip: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("could not create {}: {error}", destination.display()))?;

    if ctx.platform.is_windows() {
        let script = "Expand-Archive -LiteralPath $env:AE_PREPARE_ZIP -DestinationPath $env:AE_PREPARE_DEST -Force";
        return run(
            ctx,
            &CommandSpec::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("AE_PREPARE_ZIP", zip.as_os_str())
                .env("AE_PREPARE_DEST", destination.as_os_str()),
        );
    }

    if which("unzip").is_none() {
        return Err("unzip is required to extract the Android command-line tools".to_string());
    }
    run(
        ctx,
        &CommandSpec::new("unzip")
            .args(["-q", "-o"])
            .arg(zip)
            .arg("-d")
            .arg(destination),
    )
}

fn verify_sha1(path: &Path, expected: &str) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let actual = hex(&sha1(&data));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "SHA-1 mismatch for {}: expected {expected}, got {actual}",
            path.display()
        ))
    }
}

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.as_chunks::<64>().0 {
        let mut w = [0u32; 80];
        for (index, word) in chunk.as_chunks::<4>().0.iter().take(16).enumerate() {
            w[index] = u32::from_be_bytes(*word);
        }
        for index in 16..80 {
            w[index] = (w[index - 3] ^ w[index - 8] ^ w[index - 14] ^ w[index - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (index, word) in w.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut output = [0u8; 20];
    for (index, word) in h.iter().enumerate() {
        output[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    output
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            hex(&sha1(b"The quick brown fox jumps over the lazy dog")),
            "2fd4e1c67a2d28fced849ee1bb76e7391b93eb12"
        );
    }
}
