use crate::context::{run, run_output, CommandSpec, Context, Target};
use crate::platform::PkgManager;

const DESKTOP_DEBIAN: &[&str] = &[
    "build-essential",
    "pkg-config",
    "libssl-dev",
    "libgtk-3-dev",
    "libwebkit2gtk-4.1-dev",
    "libasound2-dev",
    "libudev-dev",
    "libayatana-appindicator3-dev",
    "libxdo-dev",
    "libglib2.0-dev",
    "curl",
    "unzip",
];
const WEB_DEBIAN: &[&str] = &["clang", "lld"];

const DESKTOP_FEDORA: &[&str] = &[
    "gcc",
    "gcc-c++",
    "make",
    "pkgconf-pkg-config",
    "openssl-devel",
    "gtk3-devel",
    "webkit2gtk4.1-devel",
    "libsoup3-devel",
    "alsa-lib-devel",
    "systemd-devel",
    "libayatana-appindicator-gtk3-devel",
    "libxdo-devel",
    "glib2-devel",
    "curl",
    "unzip",
];
const WEB_FEDORA: &[&str] = &["clang", "lld"];

const DESKTOP_ARCH: &[&str] = &[
    "base-devel",
    "pkgconf",
    "openssl",
    "gtk3",
    "webkit2gtk-4.1",
    "libsoup3",
    "alsa-lib",
    "systemd-libs",
    "libayatana-appindicator",
    "xdotool",
    "glib2",
    "curl",
    "unzip",
];
const WEB_ARCH: &[&str] = &["clang", "lld"];

const DESKTOP_SUSE: &[&str] = &[
    "gcc",
    "gcc-c++",
    "make",
    "pkg-config",
    "libopenssl-devel",
    "gtk3-devel",
    "webkit2gtk4.1-devel",
    "libsoup3-devel",
    "alsa-devel",
    "libudev-devel",
    "libayatana-appindicator3-devel",
    "libxdo-devel",
    "glib2-devel",
    "curl",
    "unzip",
];
const WEB_SUSE: &[&str] = &["clang", "lld"];

const DESKTOP_ALPINE: &[&str] = &[
    "build-base",
    "pkgconf",
    "openssl-dev",
    "gtk+3.0-dev",
    "webkit2gtk-4.1-dev",
    "libsoup3-dev",
    "alsa-lib-dev",
    "eudev-dev",
    "libayatana-appindicator-dev",
    "libxdo-dev",
    "glib-dev",
    "curl",
    "unzip",
];
const WEB_ALPINE: &[&str] = &["clang", "lld"];

pub fn ensure(ctx: &mut Context) {
    let Some(pkg) = ctx.platform.pkg else {
        let is_nixos = ctx.platform.distro.to_lowercase().contains("nixos")
            || ctx.platform.distro_like.to_lowercase().contains("nixos");
        let detail = if is_nixos {
            "NixOS is not managed by cargo prepare. Use: nix-shell -p gcc pkg-config openssl gtk3 webkitgtk_4_1 libsoup_3 alsa-lib udev libayatana-appindicator libxdo glib clang lld llvm nodejs"
                .to_string()
        } else {
            format!(
                "no supported package manager found. Install the desktop/web packages listed in docs/BUILDING.md manually (dx doctor lists: {})",
                dx_doctor_linux_packages()
            )
        };
        ctx.manual(Target::Desktop, "Linux desktop packages", detail);
        return;
    };

    ensure_group(
        ctx,
        pkg,
        desktop_packages(pkg),
        Target::Desktop,
        "Linux desktop packages",
    );
    ensure_group(
        ctx,
        pkg,
        web_packages(pkg),
        Target::Web,
        "Linux web build packages",
    );
}

fn ensure_group(ctx: &mut Context, pkg: PkgManager, packages: &[&str], target: Target, name: &str) {
    let missing: Vec<&str> = packages
        .iter()
        .copied()
        .filter(|package| !installed(pkg, package))
        .collect();

    if ctx.opts.check {
        if missing.is_empty() {
            ctx.ready(
                target,
                name,
                format!("all {} packages are installed", packages.len()),
            );
        } else {
            ctx.missing(
                target,
                name,
                format!(
                    "missing: {}\n       install with: {}",
                    missing.join(" "),
                    install_command(pkg, &missing)
                ),
            );
        }
        return;
    }

    if missing.is_empty() {
        ctx.already_present(
            target,
            name,
            format!("all {} packages are installed", packages.len()),
        );
        return;
    }

    println!(
        "Installing {} {} with {}...",
        missing.len(),
        name,
        ctx.platform.package_manager_name()
    );
    match install(ctx, pkg, &missing) {
        Ok(()) => ctx.installed(target, name, format!("installed: {}", missing.join(" "))),
        Err(error) => ctx.failed(
            target,
            name,
            format!("{error}\n       retry: {}", install_command(pkg, &missing)),
        ),
    }
}

fn desktop_packages(pkg: PkgManager) -> &'static [&'static str] {
    match pkg {
        PkgManager::Apt => DESKTOP_DEBIAN,
        PkgManager::Dnf => DESKTOP_FEDORA,
        PkgManager::Pacman => DESKTOP_ARCH,
        PkgManager::Zypper => DESKTOP_SUSE,
        PkgManager::Apk => DESKTOP_ALPINE,
    }
}

fn web_packages(pkg: PkgManager) -> &'static [&'static str] {
    match pkg {
        PkgManager::Apt => WEB_DEBIAN,
        PkgManager::Dnf => WEB_FEDORA,
        PkgManager::Pacman => WEB_ARCH,
        PkgManager::Zypper => WEB_SUSE,
        PkgManager::Apk => WEB_ALPINE,
    }
}

pub fn install_extra(ctx: &mut Context, packages: &[&str]) -> Result<(), String> {
    let Some(pkg) = ctx.platform.pkg else {
        return Err("no supported Linux package manager found".to_string());
    };
    install(ctx, pkg, packages)
}

pub fn planned_packages(pkg: PkgManager) -> Vec<&'static str> {
    let mut packages = desktop_packages(pkg).to_vec();
    packages.extend(web_packages(pkg).iter().copied());
    packages
}

fn installed(pkg: PkgManager, package: &str) -> bool {
    match pkg {
        PkgManager::Apt => {
            run_output(&CommandSpec::new("dpkg-query").args(["-W", "-f=${Status}", package]))
                .ok()
                .map(|output| {
                    output.status.success()
                        && String::from_utf8_lossy(&output.stdout).contains("install ok installed")
                })
                .unwrap_or(false)
        }
        PkgManager::Dnf | PkgManager::Zypper => {
            run_output(&CommandSpec::new("rpm").args(["-q", package]))
                .map(|output| output.status.success())
                .unwrap_or(false)
        }
        PkgManager::Pacman => run_output(&CommandSpec::new("pacman").args(["-Q", package]))
            .map(|output| output.status.success())
            .unwrap_or(false),
        PkgManager::Apk => run_output(&CommandSpec::new("apk").args(["info", "-e", package]))
            .map(|output| output.status.success())
            .unwrap_or(false),
    }
}

fn install(ctx: &mut Context, pkg: PkgManager, packages: &[&str]) -> Result<(), String> {
    if pkg == PkgManager::Apt && !ctx.apt_updated {
        run(
            ctx,
            &CommandSpec::new("apt-get")
                .args(["update"])
                .env("DEBIAN_FRONTEND", "noninteractive")
                .privileged(),
        )?;
        ctx.apt_updated = true;
    }

    let mut spec = match pkg {
        PkgManager::Apt => CommandSpec::new("apt-get")
            .args(["install", "-y", "--no-install-recommends"])
            .env("DEBIAN_FRONTEND", "noninteractive"),
        PkgManager::Dnf => CommandSpec::new("dnf").args(["install", "-y"]),
        PkgManager::Pacman => CommandSpec::new("pacman").args(["-S", "--needed", "--noconfirm"]),
        PkgManager::Zypper => CommandSpec::new("zypper").args(["install", "-y"]),
        PkgManager::Apk => CommandSpec::new("apk").args(["add", "--no-cache"]),
    };
    for package in packages {
        spec = spec.arg(*package);
    }
    run(ctx, &spec.privileged())
}

fn install_command(pkg: PkgManager, packages: &[&str]) -> String {
    match pkg {
        PkgManager::Apt => format!(
            "sudo apt-get update && sudo apt-get install -y --no-install-recommends {}",
            packages.join(" ")
        ),
        PkgManager::Dnf => format!("sudo dnf install -y {}", packages.join(" ")),
        PkgManager::Pacman => format!("sudo pacman -S --needed --noconfirm {}", packages.join(" ")),
        PkgManager::Zypper => format!("sudo zypper install -y {}", packages.join(" ")),
        PkgManager::Apk => format!("sudo apk add --no-cache {}", packages.join(" ")),
    }
}

fn dx_doctor_linux_packages() -> &'static str {
    "libwebkit2gtk-4.1-dev libgtk-3-dev libasound2-dev libudev-dev libayatana-appindicator3-dev libxdo-dev libglib2.0-dev"
}
