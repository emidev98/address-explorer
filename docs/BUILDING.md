# Building Address Explorer

This page describes the development environment, the bundle matrix, and the
`cargo prepare` command. The repository rules for code, tests, and formatting
remain in [`CONTRIBUTING.md`](../CONTRIBUTING.md).

## Quick start

Cargo must already be available to run the command. Install Rust from
<https://rustup.rs> if necessary, then from the repository root:

```sh
cargo prepare          # desktop + web toolchain for this operating system
cargo prepare --all    # also Android everywhere supported, and iOS on macOS
```

Useful verification modes:

```sh
cargo prepare --check                  # report what is missing; no changes
cargo prepare --dry-run                # print every planned command
cargo prepare --all --dry-run          # inspect the full mobile plan
cargo prepare --all --yes              # skip the confirmation prompt
cargo prepare --verify-build           # run dx check web/desktop afterwards
```

`--check` exits non-zero when a requested tool is missing or needs manual
action. `--dry-run` always leaves the system untouched. Host capabilities that
cannot exist (for example Android on Linux arm64) are reported as
**unsupported** and do not fail the command.

## What the command installs

| Host | `cargo prepare` | `cargo prepare --all` adds |
| --- | --- | --- |
| **Linux x86_64** | Rust stable with rustfmt/clippy, `wasm32-unknown-unknown`, Dioxus CLI 0.7.10, wasm `clang` + `lld`, Node.js LTS + Tailwind 4.1.5 cache, GTK3/WebKitGTK 4.1/libsoup3/OpenSSL development libraries, GCC, pkg-config, `unzip`, `curl` | JDK 17, Android command-line tools, SDK platform 34, build-tools 34.0.0, NDK 25.2.9519653, CMake 3.22.1, `adb`, Android Rust targets, Android environment variables |
| **Linux arm64** | Same desktop/web tools | Android is reported as unsupported (Google's SDK/NDK host tools are x86_64-only); no iOS |
| **macOS** | Xcode Command Line Tools, Rust stable with rustfmt/clippy, `wasm32-unknown-unknown`, Dioxus CLI 0.7.10, Homebrew, Homebrew LLVM + lld, Node.js + Tailwind cache | JDK 17, Android SDK/NDK/targets/environment; iOS Xcode and SDK verification plus iOS Rust targets |
| **Windows x86_64** | Visual Studio Build Tools with the C++ workload, Windows SDK, WebView2 runtime, LLVM (clang/llvm-ar/wasm-ld), Rust MSVC with rustfmt/clippy, `wasm32-unknown-unknown`, Dioxus CLI 0.7.10, Node.js + Tailwind cache | JDK 17, Android SDK/NDK/targets/environment |

Environment variables for Android are persisted to the Windows user
environment, or to a marked block in the detected Unix shell profile
(bash/zsh/fish). Other shells receive the exports printed to the terminal.

### Android versions

- JDK 17
- Android SDK platform 34, build-tools 34.0.0
- NDK 25.2.9519653 (the version used by the Dioxus 0.7 mobile guide)
- CMake 3.22.1
- Google command-line tools build 16111833
- Rust targets: `aarch64-linux-android`, `armv7-linux-androideabi`,
  `i686-linux-android`, `x86_64-linux-android`

No emulator system image is installed. Use Android Studio or `sdkmanager` to
add one if you want to run on an emulator.

## Bundle matrix

Native desktop bundles are built on their matching host. Web is one deployable
PWA directory; the optional Remote server is a separate host-native binary.
Android builds on supported hosts, and iOS requires macOS.

| Bundle / artifact | Linux host | macOS host | Windows host | Required development tools (brief) |
| --- | --- | --- | --- | --- |
| **Linux desktop** — `.deb`, `.rpm`, `.AppImage` | Yes | No | No | Rust stable, `dx`, GCC/`build-essential`, `pkg-config`, GTK3 + WebKitGTK 4.1 + libsoup3 + OpenSSL dev libraries |
| **macOS desktop** — `.app`, `.dmg` | No | Yes | No | Rust stable, `dx`, Xcode Command Line Tools |
| **Windows desktop** — `.msi`, `.exe` (NSIS) | No | No | Yes | Rust MSVC, `dx`, Visual Studio Build Tools + Windows SDK, WebView2; `dx` downloads WiX/NSIS |
| **Web PWA** — static `public/` with SharedWorker WASM; Local and Remote modes | Yes | Yes | Yes | Rust `wasm32-unknown-unknown`, `dx`, wasm-capable `clang`, `lld`/`wasm-ld`, a working archiver (`llvm-ar` or `ar`), Node/npm for Tailwind; no external `wasm-bindgen-cli` |
| **Web/Remote server** — native `server` binary | Yes | Yes | Yes | Rust stable with the `server` feature, host C toolchain; OpenSSL dev libraries on Linux; build on the deployment OS family |
| **Android** — `.apk`, `.aab` | Yes (x86_64) | Yes (x86_64/arm64) | Yes (x86_64) | JDK 17, Android SDK 34 + build-tools 34 + NDK + CMake, `adb`, Rust Android targets, `dx` |
| **iOS** — `.app`, `.ipa` | No | Yes | No | macOS, Xcode + iOS SDK, Rust iOS targets, `dx` |

Notes:

- `dx bundle` builds native desktop bundles only for the host platform. Use
  native CI runners for other operating systems instead of cross-building.
- `dx` automatically downloads `linuxdeploy`, WiX, NSIS, the WebView2
  bootstrapper, `wasm-bindgen`, `wasm-opt`, and its bundled Tailwind binary.
  `cargo prepare` does not install those separately.
- Signing, notarization, and store credentials are out of scope. Keep them out
  of the repository.
- The web PWA supports latest 2026+ Chrome (desktop/Android), Edge, Firefox,
  and Safari (macOS/iOS). SharedWorker and service-worker support are required.
- Remote web clients need the server's `ADDRESS_EXPLORER_CORS_ORIGINS`
  allowlist. A server that serves the PWA itself needs no CORS entry.
- Windows arm64 has not been validated; use an x86_64 Windows host.

## Web PWA and SharedWorker

The root `build.rs` makes web builds self-contained. For a wasm32 web build it:

1. Builds `address-explorer-web-worker` in release mode with a separate
   `CARGO_TARGET_DIR`.
2. Generates the classic worker with `wasm-bindgen-cli-support` in
   `no_modules` mode. No external `wasm-bindgen-cli` is required.
3. Writes `public/local-worker.js`, `public/worker/*`, and
   `public/btc_usd.bin`.

Those generated files are gitignored. Native desktop/mobile and pure-server
builds do not build the worker and do not require wasm clang.

The worker needs a wasm-capable clang and an archiver. The build script looks
for:

1. `CC` and `AR` when set by the user;
2. `clang` / `llvm-ar` on `PATH`;
3. versioned tools such as `llvm-ar-21` on `PATH`;
4. Homebrew LLVM on macOS, or `C:\Program Files\LLVM\bin` on Windows.

If `CC` or `AR` is set to a native-only compiler, unset it or point it at a
wasm-capable LLVM tool. `cargo prepare --check` probes the toolchain with a tiny
wasm compile and archive.

### Web commands

```sh
# Development: both commands build the SharedWorker and stable seed.
dx serve
dx serve --web

# Deployable PWA
dx build --web --release
# or
dx bundle --web --release --no-default-features --features web

# Optional Remote server
cargo build --release --no-default-features --features server
```

The deployable `public/` directory includes the app shell, hashed JS/WASM,
`local-worker.js`, `worker/*`, the manifest, service worker, icons, fonts,
vendored ECharts, and the stable `btc_usd.bin`. Serve it over HTTPS or
`localhost`, and rewrite unknown paths to `index.html` for the client-side
router.

## Native and mobile commands

```sh
# Desktop
dx bundle --desktop --release --no-default-features

# Android
dx bundle --android --release --no-default-features

# iOS
dx bundle --ios --release --no-default-features

# Pure server binary
cargo build --release --no-default-features --features server
```

`desktop` and `mobile` already include `server` and `remote-backend`; do not
add those features again. These native commands must not build the web worker
or require clang.

## Platform notes and troubleshooting

### Linux

The desktop libraries are the set reported by `dx doctor`:
`libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libasound2-dev`, `libudev-dev`,
`libayatana-appindicator3-dev`, `libxdo-dev`, and `libglib2.0-dev`, plus the
compiler, pkg-config, and OpenSSL development packages. `cargo prepare` maps
these to apt, dnf, pacman, zypper, or apk. NixOS and unknown distributions get
a package list and shell command to run manually.

Android is not natively supported on Linux arm64 because Google's SDK and NDK
host tools are x86_64-only. Use an x86_64 Linux host or container.

If your distribution ships Node.js older than 20, install a current LTS from
<https://nodejs.org> (or NodeSource); `cargo prepare --check` reports the
version it found. `dx` can still build with its bundled Tailwind even when the
manual `npx @tailwindcss/cli` workflow is unavailable.

### macOS

`xcode-select --install` starts the Command Line Tools installer; finish it and
re-run `cargo prepare`. Full Xcode is required for iOS and remains an App Store
installation. Homebrew LLVM is used for the web worker; the build script
detects the Homebrew prefix automatically.

If a new terminal cannot find `brew`, `node`, or `npx`, add Homebrew to the
shell environment, for example with
`eval "$(/opt/homebrew/bin/brew shellenv)"` (Intel Macs:
`/usr/local/bin/brew`).

### Windows

`cargo prepare` uses winget for Visual Studio Build Tools, the Windows SDK,
WebView2, LLVM, Node.js, and (with `--all`) JDK 17. UAC prompts are handled by
winget and the Visual Studio installer. Visual Studio Build Tools is a large
download and may take a while.

### Android

On a supported host, `cargo prepare --all`:

- downloads the Android command-line tools and verifies their SHA-1;
- accepts SDK licenses (only with the install command; use `--dry-run` to
  inspect first);
- installs `platform-tools`, platform 34, build-tools 34, NDK 25.2.9519653,
  and CMake 3.22.1;
- adds the Android Rust targets;
- sets `JAVA_HOME`, `ANDROID_HOME`, `ANDROID_SDK_ROOT`, `ANDROID_NDK_HOME`, and
  `NDK_HOME`, and adds the platform-tools and command-line tools to `PATH`.

Re-run `cargo prepare --all` to repair a partial setup. On Apple Silicon, NDK
25.2.9519653 may use x86_64 host tools and require Rosetta 2; install a newer
NDK manually if you prefer native arm64 host tools.

### iOS

iOS requires macOS and full Xcode with the iOS SDK. `cargo prepare --all`
verifies Xcode, `xcodebuild`, and `xcrun`, and adds the Rust targets:

- `aarch64-apple-ios`
- `aarch64-apple-ios-sim`
- `x86_64-apple-ios`

Signing and provisioning are not automated.

### Web worker failures

If a web build fails with `failed to build the web SharedWorker`:

1. Run `cargo prepare --check`.
2. Check that `clang --print-targets` lists `wasm32`.
3. Check that `llvm-ar` (or a versioned `llvm-ar-*` or `ar`) works.
4. Unset `CC`/`AR` if they point at a native compiler.
5. Re-run `dx check --platform web` or `dx build --web`.

If Local mode reports that `local-worker.js` is missing, rebuild the web app;
the generated worker assets are not committed.

## Version pins

- Dioxus CLI: **0.7.10**
- Tailwind CLI: **4.1.5**
- `wasm-bindgen` / `wasm-bindgen-cli-support`: **0.2.129** (locked by the app)
- `sqlite-wasm-rs`: **0.5.5**
- `sqlite-wasm-vfs`: **0.2.0** (vendored patch)
- Rust: stable
- Node.js: current LTS, minimum 20
- Android versions listed above

Updating a pin requires updating this page, the `cargo-prepare` constants, and
the corresponding lockfile or dependency entry.

## Manual and global installation

The repository alias is the supported entry point:

```toml
# .cargo/config.toml
[alias]
prepare = "run --quiet -p cargo-prepare --"
```

You can also install the tool globally:

```sh
cargo install --path tools/cargo-prepare
cargo prepare --check
```

Cargo currently warns when the repository alias shadows the global binary, and
plans to make that a hard error. Use the repository alias inside this
repository, or remove the alias if you prefer the global binary. Cargo passes
the `prepare` subcommand name to the external binary; the tool accepts it.

## Non-goals

- Cross-building native desktop bundles for another operating system.
- Code signing, notarization, and store submission.
- Installing Android emulator system images.
- Installing full Xcode from the App Store.
- Installing an external `wasm-bindgen-cli`.
- Upgrading application dependencies.
