use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::context::which;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Os {
    Linux,
    Macos,
    Windows,
    Other,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PkgManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Apk,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    Other,
}

pub struct Platform {
    pub os: Os,
    pub arch: String,
    pub distro: String,
    pub distro_like: String,
    pub pkg: Option<PkgManager>,
    pub shell: Shell,
    pub home: Option<PathBuf>,
    pub is_root: bool,
}

impl Platform {
    pub fn detect() -> Self {
        let os = match env::consts::OS {
            "linux" => Os::Linux,
            "macos" => Os::Macos,
            "windows" => Os::Windows,
            _ => Os::Other,
        };
        let arch = env::consts::ARCH.to_string();

        let (distro, distro_like) = if os == Os::Linux {
            read_os_release()
        } else {
            (String::new(), String::new())
        };

        let pkg = if os == Os::Linux {
            detect_package_manager()
        } else {
            None
        };

        let shell = detect_shell();
        let home = env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        let is_root = if os == Os::Windows {
            false
        } else {
            Command::new("id")
                .arg("-u")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|uid| uid.trim() == "0")
                .unwrap_or(false)
        };

        Self {
            os,
            arch,
            distro,
            distro_like,
            pkg,
            shell,
            home,
            is_root,
        }
    }

    pub fn is_linux(&self) -> bool {
        self.os == Os::Linux
    }

    pub fn is_macos(&self) -> bool {
        self.os == Os::Macos
    }

    pub fn is_windows(&self) -> bool {
        self.os == Os::Windows
    }

    pub fn android_supported(&self) -> bool {
        match self.os {
            Os::Linux => self.arch == "x86_64",
            Os::Macos => self.arch == "x86_64" || self.arch == "aarch64",
            Os::Windows => self.arch == "x86_64",
            Os::Other => false,
        }
    }

    pub fn description(&self) -> String {
        let os = match self.os {
            Os::Linux => "Linux",
            Os::Macos => "macOS",
            Os::Windows => "Windows",
            Os::Other => "unknown OS",
        };
        let distro = if self.distro.is_empty() {
            String::new()
        } else {
            format!(" ({})", self.distro)
        };
        format!("{os} {}{distro}", self.arch)
    }

    pub fn profile_file(&self) -> Option<PathBuf> {
        let home = self.home.as_ref()?;
        match self.shell {
            Shell::Bash => Some(home.join(".bashrc")),
            Shell::Zsh => env::var_os("ZDOTDIR")
                .map(PathBuf::from)
                .or_else(|| Some(home.clone()))
                .map(|directory| directory.join(".zshrc")),
            Shell::Fish => Some(home.join(".config/fish/conf.d/address-explorer.fish")),
            Shell::Other => None,
        }
    }

    pub fn package_manager_name(&self) -> &'static str {
        match self.pkg {
            Some(PkgManager::Apt) => "apt",
            Some(PkgManager::Dnf) => "dnf",
            Some(PkgManager::Pacman) => "pacman",
            Some(PkgManager::Zypper) => "zypper",
            Some(PkgManager::Apk) => "apk",
            None => "unknown",
        }
    }
}

fn read_os_release() -> (String, String) {
    let Ok(contents) = fs::read_to_string("/etc/os-release") else {
        return (String::new(), String::new());
    };

    let mut name = String::new();
    let mut id_like = String::new();
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key {
            "PRETTY_NAME" => name = value.to_string(),
            "ID_LIKE" => id_like = value.to_string(),
            _ => {}
        }
    }

    (name, id_like)
}

fn detect_package_manager() -> Option<PkgManager> {
    if which("apt-get").is_some() {
        Some(PkgManager::Apt)
    } else if which("dnf").is_some() {
        Some(PkgManager::Dnf)
    } else if which("pacman").is_some() {
        Some(PkgManager::Pacman)
    } else if which("zypper").is_some() {
        Some(PkgManager::Zypper)
    } else if which("apk").is_some() {
        Some(PkgManager::Apk)
    } else {
        None
    }
}

fn detect_shell() -> Shell {
    let Some(shell) = env::var_os("SHELL") else {
        return Shell::Other;
    };
    let name = PathBuf::from(shell)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    match name.as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        _ => Shell::Other,
    }
}
