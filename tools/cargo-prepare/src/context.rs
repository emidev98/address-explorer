use std::env;
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use crate::cli::Options;
use crate::platform::{Os, Platform};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Common,
    Desktop,
    Web,
    Android,
    Ios,
}

impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Target::Common => "common",
            Target::Desktop => "desktop",
            Target::Web => "web",
            Target::Android => "android",
            Target::Ios => "ios",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Ready,
    Installed,
    AlreadyPresent,
    Skipped,
    Missing,
    Failed,
    Manual,
    Unsupported,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Ready => "ready",
            Status::Installed => "installed",
            Status::AlreadyPresent => "already present",
            Status::Skipped => "skipped",
            Status::Missing => "missing",
            Status::Failed => "failed",
            Status::Manual => "manual action",
            Status::Unsupported => "unsupported",
        }
    }

    pub fn is_problem(self) -> bool {
        matches!(self, Status::Missing | Status::Failed | Status::Manual)
    }
}

pub struct StepResult {
    pub target: Target,
    pub name: String,
    pub status: Status,
    pub required: bool,
    pub detail: String,
}

#[derive(Clone, Debug)]
pub struct WebTools {
    pub clang: PathBuf,
    pub archiver: PathBuf,
    pub wasm_ld: Option<PathBuf>,
}

pub struct Context {
    pub opts: Options,
    pub platform: Platform,
    pub results: Vec<StepResult>,
    pub apt_updated: bool,
    pub brew: Option<PathBuf>,
    pub llvm_prefix: Option<PathBuf>,
    pub web_tools: Option<WebTools>,
    pub android_sdk: Option<PathBuf>,
    pub android_ndk: Option<PathBuf>,
    pub java_home: Option<PathBuf>,
    pub env_to_persist: Vec<(String, String)>,
    pub path_to_persist: Vec<PathBuf>,
    pub env_written: bool,
}

impl Context {
    pub fn new(opts: Options, platform: Platform) -> Self {
        Self {
            opts,
            platform,
            results: Vec::new(),
            apt_updated: false,
            brew: None,
            llvm_prefix: None,
            web_tools: None,
            android_sdk: None,
            android_ndk: None,
            java_home: None,
            env_to_persist: Vec::new(),
            path_to_persist: Vec::new(),
            env_written: false,
        }
    }

    pub fn record(
        &mut self,
        target: Target,
        name: impl Into<String>,
        status: Status,
        required: bool,
        detail: impl Into<String>,
    ) {
        self.results.push(StepResult {
            target,
            name: name.into(),
            status,
            required,
            detail: detail.into(),
        });
    }

    pub fn ready(&mut self, target: Target, name: impl Into<String>, detail: impl Into<String>) {
        self.record(target, name, Status::Ready, true, detail);
    }

    pub fn installed(
        &mut self,
        target: Target,
        name: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.record(target, name, Status::Installed, true, detail);
    }

    pub fn already_present(
        &mut self,
        target: Target,
        name: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.record(target, name, Status::AlreadyPresent, true, detail);
    }

    pub fn missing(&mut self, target: Target, name: impl Into<String>, detail: impl Into<String>) {
        self.record(target, name, Status::Missing, true, detail);
    }

    pub fn failed(&mut self, target: Target, name: impl Into<String>, detail: impl Into<String>) {
        self.record(target, name, Status::Failed, true, detail);
    }

    pub fn manual(&mut self, target: Target, name: impl Into<String>, detail: impl Into<String>) {
        self.record(target, name, Status::Manual, true, detail);
    }

    pub fn unsupported(
        &mut self,
        target: Target,
        name: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.record(target, name, Status::Unsupported, false, detail);
    }

    pub fn skipped(&mut self, target: Target, name: impl Into<String>, detail: impl Into<String>) {
        self.record(target, name, Status::Skipped, false, detail);
    }

    pub fn has_required_problem(&self) -> bool {
        self.results
            .iter()
            .any(|result| result.required && result.status.is_problem())
    }
}

pub struct CommandSpec {
    pub program: OsString,
    pub args: Vec<OsString>,
    pub envs: Vec<(OsString, OsString)>,
    pub privileged: bool,
    pub yes_stdin: bool,
}

impl CommandSpec {
    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            envs: Vec::new(),
            privileged: false,
            yes_stdin: false,
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }

    pub fn privileged(mut self) -> Self {
        self.privileged = true;
        self
    }

    pub fn yes_stdin(mut self) -> Self {
        self.yes_stdin = true;
        self
    }
}

pub fn run(ctx: &Context, spec: &CommandSpec) -> Result<(), String> {
    if ctx.opts.check {
        return Err(format!(
            "internal error: tried to change the system while checking: {}",
            command_line(&spec.program, &spec.args)
        ));
    }

    let (program, args) = effective_command(ctx, spec);
    let line = command_line(&program, &args);

    if ctx.opts.dry_run {
        println!("  + {line}");
        return Ok(());
    }

    println!("  > {line}");
    let mut command = Command::new(&program);
    command.args(&args);
    for (key, value) in &spec.envs {
        command.env(key, value);
    }

    if spec.yes_stdin {
        command.stdin(Stdio::piped());
        command.stdout(if ctx.opts.verbose {
            Stdio::inherit()
        } else {
            Stdio::piped()
        });
        command.stderr(if ctx.opts.verbose {
            Stdio::inherit()
        } else {
            Stdio::piped()
        });
        let mut child = command
            .spawn()
            .map_err(|error| format!("could not start `{line}`: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            let answers = "y\n".repeat(500);
            let _ = stdin.write_all(answers.as_bytes());
        }
        let output = child
            .wait_with_output()
            .map_err(|error| format!("could not wait for `{line}`: {error}"))?;
        if output.status.success() {
            return Ok(());
        }
        print_output(&output);
        return Err(format!("`{line}` failed with {}", output.status));
    }

    if ctx.opts.verbose {
        let status = command
            .status()
            .map_err(|error| format!("could not start `{line}`: {error}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("`{line}` failed with {status}"))
        }
    } else {
        let output = command
            .output()
            .map_err(|error| format!("could not start `{line}`: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            print_output(&output);
            Err(format!("`{line}` failed with {}", output.status))
        }
    }
}

pub fn run_output(spec: &CommandSpec) -> Result<Output, String> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    for (key, value) in &spec.envs {
        command.env(key, value);
    }
    command.output().map_err(|error| {
        format!(
            "could not run `{}`: {error}",
            command_line(&spec.program, &spec.args)
        )
    })
}

pub fn capture_stdout(spec: &CommandSpec) -> Option<String> {
    let output = run_output(spec).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn which(program: &str) -> Option<PathBuf> {
    let path = Path::new(program);
    if path.components().count() > 1 {
        return is_executable(path).then(|| path.to_path_buf());
    }

    let path_var = env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .map(|extension| extension.to_string())
            .collect()
    } else {
        Vec::new()
    };

    for directory in env::split_paths(&path_var) {
        let candidate = directory.join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
        for extension in &extensions {
            let candidate = directory.join(format!("{program}{extension}"));
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }

    None
}

fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

pub fn append_path(directory: &Path) {
    let mut parts = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        parts.extend(env::split_paths(&path));
    }
    if !parts.iter().any(|part| part == directory) {
        parts.push(directory.to_path_buf());
    }
    if let Ok(joined) = env::join_paths(parts) {
        env::set_var("PATH", joined);
    }
}

pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

pub fn command_line(program: &OsStr, args: &[OsString]) -> String {
    let mut parts = vec![quote(program)];
    parts.extend(args.iter().map(|arg| quote(arg)));
    parts.join(" ")
}

fn quote(value: &OsStr) -> String {
    let text = value.to_string_lossy();
    if text.is_empty()
        || text
            .chars()
            .any(|character| character.is_whitespace() || "\"'\\$`&|;<>()".contains(character))
    {
        format!("{text:?}")
    } else {
        text.into_owned()
    }
}

fn effective_command(ctx: &Context, spec: &CommandSpec) -> (OsString, Vec<OsString>) {
    if spec.privileged && ctx.platform.os != Os::Windows && !ctx.platform.is_root {
        let mut args = Vec::new();
        if ctx.opts.non_interactive {
            args.push(OsString::from("-n"));
        }
        args.push(spec.program.clone());
        args.extend(spec.args.iter().cloned());
        (OsString::from("sudo"), args)
    } else {
        (spec.program.clone(), spec.args.clone())
    }
}

fn print_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.trim().is_empty() {
        eprintln!("{}", stdout.trim_end());
    }
    if !stderr.trim().is_empty() {
        eprintln!("{}", stderr.trim_end());
    }
}
