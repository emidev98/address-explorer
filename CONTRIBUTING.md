# Contributing Guide

This document is the single source of truth for repository development rules: Dioxus conventions, Rust architecture, code style, SQLite persistence, testing, platform compatibility, and review.

The repository configuration (`Cargo.toml`, `Cargo.lock`, `Dioxus.toml`, CI definitions, and established project conventions) is authoritative. If a guide and the actual configuration disagree, contributors **MUST** resolve the discrepancy deliberately and update this document or the project configuration rather than silently changing project-wide assumptions.

## 1. Normative language (RFC 2119 / RFC 8174)

The key words **MUST**, **MUST NOT**, **REQUIRED**, **SHALL**, **SHALL NOT**, **SHOULD**, **SHOULD NOT**, **RECOMMENDED**, **NOT RECOMMENDED**, **MAY**, and **OPTIONAL** are to be interpreted as described in [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) as clarified by [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174). Only these words written in all uppercase have their defined normative meaning.

- **MUST / MUST NOT** specify an absolute requirement or prohibition.
- **SHOULD / SHOULD NOT** specify a strong recommendation. A deviation requires a specific, understood reason; contributors **SHOULD** document that reason in the pull request or review discussion.
- **MAY / OPTIONAL** identify a permitted choice, not a requirement.

These meanings apply to this guide and its normative statements. Lowercase forms in ordinary prose do not carry the RFC-defined meaning.

## 2. Engineering principles and source of truth

Contributions **MUST** prioritize correctness, clarity, security, accessibility, and maintainability over cleverness or premature optimization. The detailed requirements below cover UI architecture, persistence, clean code, testing, and platform compatibility; contributors **SHOULD** use those sections as the implementation checklist rather than maintaining separate local rules that diverge from this guide.

When sources disagree, contributors **MUST** prioritize project configuration and code verified by tests, followed by official documentation for the exact dependency version, this guide, and then third-party tutorials or blog posts. An existing implementation is not proof that a behavior is intentional or safe.

## 3. Supported stack and repository layout

- **Language:** stable Rust unless the repository explicitly documents a toolchain exception.
- **UI:** Dioxus. The version pinned by `Cargo.toml`/`Cargo.lock` determines the usable API. Section 5 describes the Dioxus conventions; contributors **MUST** update that section deliberately when the dependency baseline changes rather than mixing APIs.
- **Charts:** Apache ECharts, vendored in `assets/vendor/echarts/`, is the default and only charting library. Section 5.10 describes the required chart component boundaries.
- **Persistence:** SQLite through `rusqlite`. Native builds open a platform database file. The web Local backend opens a virtual SQLite database through `sqlite-wasm-vfs` `relaxed-idb` inside the SharedWorker. Browser/WASM code **MUST NOT** use native filesystem paths, `dirs`, or `std::fs`, and **MUST NOT** assume the database is a file on disk; the browser database is exported as a normal SQLite file only through the backup path.
- **Targets:** Linux, macOS, Android, iOS, and the web PWA. Platform-specific APIs **SHOULD** be isolated behind narrow modules or target-specific compilation. The web target is a single deployable PWA directory, not one HTML file and not a native binary.
- **Web backends:** one web build supports two runtime modes. **Local** uses the browser SQLite database; **Remote** uses any Address Explorer server over HTTP(S) with the server's explicit CORS allowlist. The mode is selected in **Settings → General**; Local is the first-run default.
- **Browser support:** latest 2026+ Chrome (desktop/Android), Edge, Firefox, and Safari (macOS/iOS) only. SharedWorker and service-worker support are required. Older browsers are unsupported; Local mode shows a clear incompatibility message and Remote remains available. No legacy polyfills are in scope.
- **Currency:** Bitcoin is the base and accounting currency; fiat values are a derived display layer governed by the default-fiat rules in Section 3.1.
- **Bundled data:** the packed BTC/USD history lives in `assets/pricing/btc_usd.bin`; the researched exchange-wallet catalog lives in `src/data/migrations/0019_addresses_data.sql`. Native builds import both through the seed migrations. The web PWA precaches the same price-pack asset and the worker imports it on first initialization or when the seed migration is still pending; it **MUST NOT** be embedded in the main PWA WASM.

Contributors **MUST NOT** upgrade Dioxus, `rusqlite`, SQLite features, or other major dependencies as an incidental part of an unrelated change. Dependency changes **SHOULD** state the reason, compatibility impact, and tests performed. For an application, `Cargo.lock` **MUST** be committed so dependency resolution is reviewed and reproducible.

### 3.1 Currency model and default-fiat conversion

Bitcoin is the application's base and accounting currency. BTC balances, on-chain movements, ledger BTC amounts, and BTC fees **MUST** remain stored and handled in BTC or satoshis. Fiat values are derived presentation values; a fiat amount **MUST NOT** become the source of truth for a BTC balance. BTC-denominated columns, charts, and KPIs **MAY** continue to display BTC; any fiat equivalent shown alongside them **MUST** use the default currency described below.

The persisted `default_fiat_currency` setting (the SQLite `setting` table) is the single source of truth for every displayed or derived fiat value. It **MUST** fall back to USD when it has never been configured or is empty, and it **MUST** be normalized to an uppercase ISO code at the application boundary before it is displayed or passed to a price service. Contributors **MUST NOT** hardcode USD — or any other fiat code — into a display, label, conversion, or service when the default currency is available.

**Client rules**

- The root component loads the persisted currency once and provides it through the `FiatCurrency` context in `src/components/fiat_currency.rs`.
- Screens and components **MUST** read it with `use_fiat_currency()` and react to changes. They **MUST NOT** fetch the setting independently or keep a second independently mutable copy.
- Fiat values **MUST** be formatted with `format_fiat(amount, symbol)` from `src/components/format.rs`. USD-specific formatting is only an internal branch of that function, not a reason to call `format_usd` directly in a view.
- Labels that name a currency — KPI titles, table headers, inspector fields, chart titles, and status messages — **MUST** use the selected currency code.
- A BTC balance converted to fiat **MUST** use the latest BTC price resolved in the selected currency (`latest_price_api` / `PriceService`) together with `sats_to_fiat` from `src/domain/price.rs`. A balance **MUST NOT** be converted with a hardcoded USD price.

**Server and application rules**

- Exchange-ledger original fiat amounts and fees **MAY** be retained for provenance, but every default-currency display **MUST** use the derived read-time `converted_*` fields populated by `ExchangeFiatService` (`src/application/exchange_fiat.rs`). Converted values **MUST NOT** be written back as if they were the imported source amounts. BTC/XBT fees remain BTC.
- Historical fiat prices use the stored USD series as the canonical source. When the default currency changes, contributors **MUST** materialize the selected currency's history through `ensure_currency_history` / `ensure_currency_history_api` (`src/application/prices/`, `src/api/prices.rs`) or extend that workflow. That workflow:
  - reuses `sync_prices_bulk` rather than introducing a second conversion algorithm;
  - finds the missing target close times through the prices repository;
  - persists one target row per stored USD close time at full granularity;
  - never overwrites existing target rows, including manual edits;
  - is idempotent and retryable, and reports non-fatal failures as warnings.
- The "Sync Prices" action (`sync_prices_api`, `src/components/sync_prices_dialog.rs`) extends that workflow with one unified range sync: every run fills the BTC/USD daily closes of the range from Coin Metrics `PriceUSD`, USD optionally adds the finer recent closes the free tier still serves, and any other currency refreshes USD first and then converts every stored USD close time of the range through `sync_prices_bulk`. The daily stage is mandatory; a failing intraday or conversion stage is a warning, never a silent success.
- A new feature that needs historical prices in the default currency **MUST** use this preparation path, not ad-hoc conversion in a screen or repository.
- Saving the preference and preparing its history are separate concerns: the preference **MUST** be persisted first. If preparation fails, the saved preference **MUST** remain, the failure **MUST** be visible to the user, and a retry path **MUST** exist (opening Historical Prices or using Sync Prices). A conversion failure **MUST NOT** silently roll back the preference or be presented as success.

**Data and external sources**

- `historical_prices` is a multi-symbol table. USD remains the reference series imported by the data migrations; Coin Metrics supplies the BTC/USD `PriceUSD` daily closes and the recent `ReferenceRateUSD` observations behind finer closes, and Frankfurter supplies the daily USD → fiat rates.
- Fiat symbols **MUST** be derived from those sources through the existing clients. No SQL query, domain type, or view may assume that USD is the only display currency; the USD constant is reserved for the reference/source series and the documented fallback.

### 3.2 Suggested project layout

The layout example **SHOULD** be adapted to the repository as it exists. Contributors **MUST NOT** reorganize the project solely to match this example.

```text
src/
├── main.rs                 # Entry point and composition root
├── app.rs                  # Root component, Route enum, route URLs
├── navigation.rs           # Destination manifest: route, label, icon
├── api/                    # Thin native #[server] controllers + web wrappers
├── application/            # Services: use cases and orchestration
├── clients/                # External API clients, one module per provider
├── components/             # Reusable presentation components
│   ├── app_shell.rs        # Top bar, side panel, drawer, compact rail
│   ├── icon.rs             # Shared Material icons
│   ├── modal.rs            # Shared dialog shell
│   ├── page_header.rs      # Shared screen title row
│   ├── table_controls.rs   # Table state, pagination, sorting, selection
│   └── charts/             # One component per Apache ECharts chart
├── data/                   # Database, migrations, seeds, repositories
├── domain/                 # Types, validation, pure business rules
├── screens/                # Screen components
│   └── settings/           # Settings subsections
└── web/                    # Browser boot, SharedWorker client
crates/
├── core/                   # Worker protocol and version constants
└── web-worker/             # SharedWorker WASM entry (includes src modules)
tools/
└── cargo-prepare/          # Development environment installer (`cargo prepare`)
build.rs                    # Web worker + stable seed generation
web/worker-entry.js         # Classic SharedWorker JavaScript loader
public/                     # PWA manifest, service worker, icons
```

The `data`, `application`, `clients`, and `domain` modules stay in `src/` as the shared backend source. `crates/web-worker` includes those files for the WASM worker; the main PWA WASM **MUST NOT** link `rusqlite`, SQLite C, the application layer, the data layer, or the external API clients. `crates/core` owns the worker envelope and protocol version. The worker reuses the same schema, migrations, seeds, repositories, services, and external clients as native builds.

A small application **MAY** use fewer modules. Modules **SHOULD** be introduced when they establish a meaningful boundary, not to increase the number of files.

## 4. Development setup and quality commands

Contributors **SHOULD** install the stable Rust toolchain, platform prerequisites for the selected Dioxus renderer, and the Dioxus CLI version recommended for the pinned dependency. Linux desktop builds generally require the GTK/WebKitGTK development libraries for the selected renderer. Android development requires the applicable Android SDK/NDK and Java toolchain. Contributors **MUST** follow the official, version-matched platform guide for exact package names and setup steps.

Run `cargo prepare` once per machine to install the pinned toolchain, system
build dependencies, web worker compiler, Node/Tailwind, and (with
`cargo prepare --all`) the Android/iOS toolchains. See
[`docs/BUILDING.md`](docs/BUILDING.md) for the OS/bundle matrix, version pins,
and troubleshooting. `cargo prepare --check` verifies the environment without
changing it.

The web PWA and its SharedWorker add these prerequisites:

- the `wasm32-unknown-unknown` target;
- a wasm-capable `clang` and `lld`/`wasm-ld` (required by `sqlite-wasm-rs`);
- no external `wasm-bindgen-cli`: `build.rs` uses `wasm-bindgen-cli-support` as a build dependency;
- optional `wasm-pack` and a browser driver if you add worker browser tests;
- Node/npx for Tailwind, as today;
- HTTPS or `localhost` for service-worker and PWA testing.

```sh
rustup target add wasm32-unknown-unknown
# Linux example; use the platform's LLVM packages elsewhere.
sudo apt-get install clang lld
```

The Dioxus CLI is the default interface for formatting and compile checks. `dx check` applies the target, feature, and asset arguments the application actually builds with. Tests and Clippy have no `dx` subcommand and remain Cargo commands.

Typical checks from the repository root are:

```sh
# Format Rust code and RSX after every change (Section 4.1)
cargo fmt --all
dx fmt

# Check the web client, desktop target, and fullstack server build
dx check --platform web
dx check --platform desktop
dx check --fullstack

# Native tests and Clippy: dx has no test or Clippy subcommand
cargo test -p address-explorer --no-default-features --features server
cargo test -p address-explorer --no-default-features --features desktop
cargo clippy -p address-explorer --all-targets --no-default-features --features server -- -D warnings
cargo clippy -p address-explorer --all-targets --no-default-features --features desktop -- -D warnings

# Compile fallback when dx check reports a known static-analysis false positive
cargo check -p address-explorer --all-targets --no-default-features --features server
cargo check -p address-explorer --all-targets --no-default-features --features desktop

# Web main client (no rusqlite/server in this WASM)
cargo check -p address-explorer --target wasm32-unknown-unknown --no-default-features --features web
cargo clippy -p address-explorer --target wasm32-unknown-unknown --no-default-features --features web -- -D warnings

# SharedWorker WASM. Set CC/AR to a wasm-capable clang when it is not on PATH.
cargo check -p address-explorer-web-worker --target wasm32-unknown-unknown
cargo clippy -p address-explorer-web-worker --target wasm32-unknown-unknown -- -D warnings

# Inspect commands supported by the pinned Dioxus CLI
dx --help
```

The default `web` feature compiles only client-side code and is checked for the
wasm target with `dx check` or `cargo check`. Native tests require `server` (or
`desktop`, which includes it); the native test commands above use those
features. `desktop` and `mobile` expand to the all-in-one native stack
(`server` + `remote-backend`); use them for native compilation, tests, Clippy,
and bundling. `web` enables the browser client and `web-client`; it **MUST NOT**
enable `server`, `rusqlite`, or the native backend. The worker is a separate
workspace crate; `build.rs` generates its web assets.

### 4.0 Web PWA and native build commands

The root `build.rs` makes web builds self-contained. For a web target it:

- builds `address-explorer-web-worker` in release mode with a separate
  `CARGO_TARGET_DIR`;
- generates the classic worker with `wasm-bindgen-cli-support` in `no_modules`
  mode;
- writes `public/local-worker.js`, `public/worker/*`, and
  `public/btc_usd.bin`;
- leaves native desktop/mobile and pure-server builds untouched.

No external `wasm-bindgen-cli`, no `cargo xtask`, and no precache manifest are
required.

```sh
# Development: both commands build the SharedWorker and seed automatically
dx serve
dx serve --web
# pass --port 8090 if 8080 is already in use

# Deployable PWA
dx build --web --release
# or:
dx bundle --web --release --no-default-features --features web
# Output: target/dx/address-explorer/release/web/public
```

The deployable `public/` directory includes `index.html`, hashed JS/WASM,
`local-worker.js`, `worker/*`, the manifest, service worker, icons, fonts,
vendored ECharts, and stable `btc_usd.bin`. Static hosts **MUST** rewrite
unknown paths to `index.html` for the client-side router and **MUST** serve the
PWA over HTTPS (or `localhost`) for the service worker.

Native and server builds remain:

```sh
dx bundle --desktop --release --no-default-features
dx bundle --android --release --no-default-features
dx bundle --ios --release --no-default-features
cargo build --release --no-default-features --features server
```

`desktop` and `mobile` already include `server` and `remote-backend`; do not add
those features again. The pure server build selects only `server` and cannot
forward. These native commands **MUST NOT** invoke clang or build the worker.

Before opening a pull request, contributors **MUST** run `cargo fmt --all` and
`dx fmt`, the `dx check` commands for the affected targets, the Cargo test
commands, and Clippy for the affected workspace. If a command is unavailable for
the pinned toolchain, contributors **SHOULD** use the documented equivalent and
**MUST** record the limitation in the pull request. Platform-specific build or
bundle commands **SHOULD** be run for every affected target.

### 4.1 Formatting after every change

Formatting is part of every code update, not a pre-PR cleanup step.
Contributors — and coding agents updating the repository — **MUST** run
`cargo fmt --all` and then `dx fmt` after the last code edit and before running
checks or reporting the work as complete. `cargo fmt --all` formats the ordinary
Rust and `dx fmt` formats the `rsx!` blocks. A change **MUST NOT** be handed off
or opened for review while its files are unformatted.

**`dx fmt --all-code` MUST NOT be used on this repository.** The pinned Dioxus
0.7.10 CLI cannot format this codebase reliably in that mode: it re-emits the
ordinary Rust from a token stream and, while doing so, *corrupts* `rsx!` event
handlers. It breaks the closure signature across lines, writes a truncated copy
of the body after the parameter list, and leaves the real body in place:

```rust
// What `dx fmt --all-code` produced for a correct `onclick: move |_| { ... }`
onclick: move |_
        | { cancel_state.cancel(& value_for_scrim, mode, & id_for_scrim); }
    cancel_state.cancel(&value_for_scrim, mode, &id_for_scrim);
```

The result does not compile, and re-running the formatter does not repair it —
once a file is corrupted the CLI often refuses to parse it again. A full run of
`dx fmt --all-code` on a clean tree reproduced this in 12 files and 26 handler
closures. `cargo fmt --all` followed by `dx fmt` produces the same readable
result without corrupting anything, so that is the required pipeline.

If `dx fmt` reports that it cannot parse a file, the contributor **MUST** find
and repair the duplicated closure described above, then re-run `cargo fmt --all`
and `dx fmt`. Do not rewrite correct code to silence the formatter, and do not
leave a large unrelated formatting diff in the change — revert churn outside the
logical change and call out anything that had to stay in the pull request
description.

### 4.2 `dx check` and Cargo fallbacks

`dx check --platform web` checks the web client, `dx check --platform desktop`
checks the native desktop target, and `dx check --fullstack` checks the
fullstack client and server builds. Use `dx check --platform android` for
Android work once the SDK/NDK environment is configured.

`dx check` also performs Dioxus-specific static analysis. That analysis is
syntactic and can report hooks called from a helper function as "hook called
outside component or hook"; when a known false positive appears, the Cargo
compile check (`cargo check -p address-explorer --all-targets --no-default-features --features server`) is
the authoritative compile gate. Record the false positive rather than changing
correct code to satisfy it.

That analysis classifies a free function that returns `Element` as a component
and a free function whose name starts with `use_` as a hook; a hook call in any
other function is reported. A helper that calls hooks therefore **MUST** be
named with the `use_` prefix (Section 5.4), for example `use_toast_provider` in
`src/components/toast.rs`. Following that rule keeps the check honest, so a
remaining "hook called outside component or hook" error is worth investigating
instead of being recorded.

Tests and Clippy remain Cargo commands because the pinned Dioxus CLI does not
expose equivalents. They are not optional: a complete check run includes both
test commands and Clippy.

Dioxus bundling is platform-sensitive. A successful build on one operating system does not prove that a native bundle for another operating system can be produced from that host. Release automation **SHOULD** use a build matrix with native runners wherever cross-compilation is unsupported or unreliable. Signing, notarization, store submission, and release credentials **MUST** be handled outside ordinary development checks and **MUST NOT** be committed to the repository.

## 5. Dioxus application development

This section defines the Dioxus-specific conventions for the application. The example API baseline is **Dioxus 0.7.1**; the version pinned in `Cargo.toml` and `Cargo.lock` always takes precedence. Contributors **MUST NOT** mix APIs from different Dioxus versions. In Dioxus 0.7, the legacy `cx`, `Scope`, and `use_state` APIs **MUST NOT** be used.

### 5.1 Dependency features and application entry point

The feature set in `Cargo.toml` is authoritative:

```toml
default = ["web"]
web = ["dioxus/web", "web-client"]
desktop = ["dioxus/desktop", "server", "remote-backend"]
mobile = ["dioxus/mobile", "server", "remote-backend"]
server = ["dioxus/server", "dioxus/fullstack", "dep:rusqlite", "dep:bitcoin", "dep:tower-http"]
remote-backend = ["dep:tokio"]
web-client = ["dep:tokio", "dep:address-explorer-core"]
```

`desktop` and `mobile` are **all-in-one native features**: they include
`server` and `remote-backend`, so every native binary contains the local
backend and can optionally use a remote backend. `server` enables the Dioxus
server, SQLite, Bitcoin validation, and CORS and is required for the
application, data, and server-function tests. `remote-backend` is an internal
native feature; it gates the remote helper, runtime forwarding, native remote
version checks, and the Backend settings section. The server-side version
middleware is always compiled with `server`. `dirs` is a native-only target
dependency.

`web` enables the PWA client and `web-client`; it **MUST NOT** enable `server`,
`rusqlite`, or the native backend. The browser `web-client` uses the SharedWorker
for Local data and `reqwest`/fetch for Remote data. `crates/web-worker` is a
separate workspace crate and is not linked into the main PWA WASM. The root
`build.rs` generates its classic worker output.

There are no local-only or remote-only native variants and no separate web
Local/Remote bundles. Build native targets through `desktop` or `mobile`; build
one web artifact with `dx build --web` or `dx bundle --web`. Local and Remote
are runtime modes of that artifact.

A native or web application **MUST** initialize the Dioxus runtime through the API documented for its pinned version. The minimal Dioxus 0.7 shape is:

```rust
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! { "Hello, Dioxus!" }
}
```

The application root is `app::Root`, which provides the backend preference
context and owns the mobile hard-reload remount. The pure web server uses
`dioxus::serve` so it can install the API version middleware; native and web
clients use `dioxus::launch`.

### 5.2 Components, props, and RSX

- UI markup **MUST** use `rsx!`; components **MUST** use `#[component]` and a naming convention supported by the pinned Dioxus version, normally `UpperCamelCase`.
- Component props **SHOULD** be owned values. For ordinary props, prefer `String` and `Vec<T>` over borrowed `&str` and slices. Props and custom prop types **MUST** satisfy the trait bounds required by the pinned Dioxus component macro, including `Clone` and `PartialEq` where required.
- Components **SHOULD** render from props and reactive state. They **MUST NOT** perform blocking database, filesystem, network, or CPU-intensive work as an incidental rendering side effect.
- Straightforward repeated and conditional nodes **SHOULD** use RSX's native `for` and `if` forms. Rust expressions embedded in RSX **MUST** be enclosed in braces; iterator expressions used to produce nodes **MUST** also be enclosed in braces.
- Attributes, event handlers, and inline styles **SHOULD** remain readable. Complex expressions **SHOULD** be computed before the `rsx!` block when that makes the markup easier to understand.

### 5.3 Assets, styles, and accessibility

- Local project assets **SHOULD** be referenced with `asset!` rather than hard-coded runtime filesystem paths.
- Paths passed to `asset!` **MUST** start with `/` and be interpreted relative to the configured project asset root for the pinned Dioxus/Manganis version.
- A project stylesheet **SHOULD** use the documented `document::Stylesheet` component or the project's established stylesheet configuration.
- Images **SHOULD** have meaningful `alt` text unless they are purely decorative and receive an appropriate accessibility treatment.

#### Styling: Tailwind CSS v4 and Material 3

The interface uses a Material 3 theme implemented with Tailwind CSS v4 in
`tailwind.css`. `assets/tailwind.css` is the generated stylesheet that the
application loads, and `assets/main.css` holds hand-written layout and
component overrides that sit on top of the generated theme. Contributors
**MUST** rebuild the generated stylesheet after changing the theme or the
Tailwind classes in the Rust views; do not hand-edit the generated file:

```bash
npx @tailwindcss/cli@4.1.5 -i tailwind.css -o assets/tailwind.css
```

The theme provides:

- Material 3 color roles as utilities, such as `bg-primary`,
  `bg-surface-container`, and `text-on-surface-variant`.
- The full type scale, such as `text-headline-medium`, `text-body-medium`,
  and `text-label-large`.
- Shape tokens (`rounded-md3-md`), elevation (`shadow-md3-1`), and motion
  easings (`ease-md3-standard`).
- Component classes used by the views, including `md3-top-app-bar`,
  `md3-side-nav`, `md3-nav-drawer`, `md3-screen`, `md3-button-filled`,
  `md3-text-field`, `md3-checkbox`, `md3-date-picker`,
  `md3-date-picker-popup`, `md3-table`, and `md3-card`.

Every checkbox **MUST** use the shared `Checkbox` component in
`src/components/checkbox.rs` instead of styling a native input in a view. It
renders a native input with the Material 3 18 px container, 40 px state layer,
48 px interaction target, optional label, and selected/unselected/indeterminate
and disabled states. Table selection stays behind `TableSelectionHeader` and
`TableSelectionCell`, which use the same component.

Single-line form controls share one height token: `--md3-control-height`
(2.5rem / 40 px), declared in the shared dimensions block of `tailwind.css`.
`.md3-text-field`, `.md3-input`, `.md3-multiselect-trigger`, and
`.md3-date-picker` consume it, and a base-layer rule applies it to native
`input` and single-select elements, so a new control inherits the height
without extra markup. A new input or dropdown **MUST** use those shared
classes instead of hardcoding a height; a control that intentionally needs a
different size (for example a multi-line textarea) **MUST** document why.

The available palettes are Ocean (the default), Forest, Aurum, Sumi, and
Orange. `data-theme` selects the palette and `data-mode` selects `light` or
`dark`; both are applied to the document root. Dark is the default appearance
until a stored preference is applied.

The palette and appearance are changed in **Settings → Theme**. Preferences
are saved automatically to the SQLite `setting` table, created by
`src/data/migrations/0001_setting.sql`. The theme is loaded
during server rendering, applied in the document head before the app markup
is parsed, and serialized into the page so hydration starts with the same
values instead of flashing the default palette. Palette definitions live in
`assets/material3-tailwind-palettes.css`, which `tailwind.css` imports. The
theme aliases the `--md-*` palette variables to the `--md-sys-color-*` roles,
so palette changes affect both the component classes and the Tailwind color
utilities. Changes that alter palettes, color roles, or component classes
**MUST** be verified in both light and dark appearance modes before review.

Primary navigation is owned by the `AppShell` layout in
`src/components/app_shell.rs` and is configured by the persisted
`navigation_position` interface setting:

- **Auto** (the default) shows the side panel from 64 rem upward and the top
  app bar below that.
- **Top** always uses the top app bar on tablet and wider windows.
- **Side** shows the side panel from 48 rem upward.
- Below 48 rem, every option uses the compact top app bar, a 48 px menu
  button, and the modal slide-in drawer. A change to the shell **MUST** keep
  that phone behavior usable with keyboard, Escape, the scrim, and the close
  button.

The persistent side panel is compressible through the footer button at its
bottom. The `side_nav_collapsed` interface setting stores that state; it
defaults to expanded and requires no schema migration because it lives in the
existing `setting` key/value table. Expanded and compact presentations share
`FullNavPanel` / `CompactNavPanel` in `src/components/app_shell.rs`; new
destinations **MUST** be added to `Destination::PRIMARY` or
`Destination::SETTINGS` in `src/navigation.rs` so every presentation stays in
sync, including an icon for each top-level destination.

- The expanded panel shows icon + label links, pins the Settings group to the
  bottom with a flexible gap above it, and places the compress button below
  Settings in a dedicated footer.
- The compact panel is a 5 rem icon rail. Settings becomes a bottom gear whose
  flyout exposes the four subsections. The flyout closes on Escape, outside
  click, route change, or selection.
- The mobile drawer always stays expanded and never shows the compress button;
  it keeps the destination links followed by the Settings subsections.
- The top app bar keeps its hover/tap Settings dropdown. Contributors
  **SHOULD** verify expanded and compact side panels, Top mode, and the mobile
  drawer at phone, tablet, and wide widths in both appearance modes when
  changing the shell.

Charts draw on a canvas and cannot consume the component classes or Tailwind
utilities: when a new style is added it **MUST** also be available for the
charts. New color roles and visual treatments **MUST** be expressed through
the chart theme tokens in `src/components/charts/theme.rs`, which reference
the same `--md-sys-color-*` variables (Section 5.10); chart styling **MUST
NOT** fork the theme with hardcoded colors.

The bundled typefaces live in `assets/fonts/` and are injected through the
`FontFaces` component in `src/components/theme.rs`; the app **MUST NOT**
request a remote font. Themes may override `--font-sans`, `--font-heading`,
`--font-numeric`, and the radius scale in `tailwind.css`. Charts **MUST** use
the `FONT_FAMILY`, `FONT_HEADING`, and `FONT_NUMERIC` tokens from
`src/components/charts/theme.rs` so canvas text follows the active theme; the
ECharts host resolves those tokens and re-applies options when the fonts
finish loading or the theme changes.

### 5.4 Signals, derived state, context, and hooks

- Local reactive state **SHOULD** use `use_signal`. Values derivable from other reactive values **SHOULD** use direct expressions or `use_memo` rather than a second independently mutable source of truth.
- Hooks **MUST** be called in a consistent order during component evaluation; they **MUST NOT** be called conditionally inside branches or loops.
- A helper function that calls hooks **MUST** be named with the `use_` prefix, such as `use_toast_provider` or `use_table_state`. The prefix is the Dioxus convention for custom hooks and it is also how `dx check` recognises hook functions (Section 4.2); a hook call in a function without the prefix is reported as an error.
- Shared state **MAY** be passed through props or provided with `use_context_provider` and retrieved with `use_context`. The choice **SHOULD** make ownership and dependencies clear.

### 5.5 Rendering, event handlers, and persistence lifecycle

- Components **SHOULD NOT** contain substantial SQL, domain algorithms, complex parsing, or multi-step workflows. Those operations **SHOULD** live in application, domain, worker, or data modules.
- Event handlers **MAY** validate simple input, update transient UI state, and start an application operation. They **MUST NOT** synchronously wait for blocking I/O or a long calculation.
- Reactive state **SHOULD** represent transient interface state, such as selected tabs, form values, loading status, or displayed results. SQLite **MUST** remain the durable source of truth for persisted records; a signal showing a saved value **MUST NOT** be treated as proof that a database transaction committed successfully.
- Writes **MUST** occur in explicit user actions or application workflows, not repeatedly because a component re-rendered. Effects **SHOULD** be reserved for lifecycle-related or reactive side effects; user-initiated operations **SHOULD** start from their event handler or application service.
- Form values **MUST** be validated at the domain/application boundary, not only by visual constraints. User-facing errors **SHOULD** be understandable and actionable; SQL text, stack traces, filesystem internals, and sensitive data **MUST NOT** be exposed to users.

### 5.6 Resources, async I/O, CPU work, and cancellation

`use_resource` **SHOULD** be used for component data backed by an asynchronous operation when its dependency-tracking and lifecycle behavior fit the use case. A resource's initial/loading state **MUST** be handled explicitly. Where relevant, the UI **SHOULD** distinguish loading, success, empty, and failure states.

Async execution and parallel computation are different: an async task does not automatically move CPU-heavy synchronous work onto a dedicated thread. Therefore:

- SQLite connection ownership and worker constraints **MUST** follow Section 6.2.
- CPU-heavy algorithms **MUST** use a suitable blocking-task facility, dedicated worker, or thread pool such as Rayon when background or parallel execution is required.
- Network and other non-blocking I/O **SHOULD** use async APIs when suitable.
- Worker results **MUST** return through a mechanism supported by the pinned Dioxus runtime. Worker threads **MUST NOT** rely on undocumented cross-thread signal access.
- Long-running work **SHOULD** expose progress and cancellation when that materially improves usability. Async workflows **MUST** define relevant cancellation behavior; cancellation **MUST NOT** leave partially committed data or a permanently active loading indicator.
- Background work **MUST** have a clear lifetime and shutdown strategy. Detached infinite loops and unbounded thread creation are **NOT RECOMMENDED**. Locks **SHOULD NOT** be held across `.await` points; critical sections **SHOULD** remain short and release locks before yielding.

CPU-intensive algorithms **SHOULD** be ordinary Rust functions with clear inputs and outputs, testable without Dioxus. Rayon **MAY** be used for data-parallel work when the workload justifies the dependency and overhead. A dedicated `core` crate **MAY** be useful when logic needs independent testing or reuse, but it remains a library compiled into the application, not a separate process or service.

### 5.7 Routing

When the application needs in-app URL routing, contributors **MAY** use Dioxus Router. The `router` feature **MUST** be enabled before using the API. Routes **SHOULD** be declared in one `Routable` enum; shared page structure **SHOULD** use layouts and `Outlet<Route>` where appropriate. Routing **SHOULD NOT** be introduced for a simple single-screen interface unless it materially improves navigation or structure.

### 5.8 Fullstack and hydration boundaries

When fullstack/server functionality is intentionally enabled, server functions **MUST** be treated as client/server boundaries: server-only operations **MUST NOT** be assumed to execute locally in a native application without a configured server.

Native builds in this repository are all-in-one: `desktop` and `mobile` include
the `server` feature, so server-function bodies execute in-process when the
active backend is the bundled local database. When the user selects a remote
backend in **Settings → General**, the same functions forward to the remote
REST server instead. Contributors **MUST** treat that runtime choice as a real
transport boundary:

- Remote calls **MUST** handle network errors, timeouts, and API version
  mismatches through `ServerFnError`.
- A server function **MUST NOT** assume it is executing locally merely because
  the crate was compiled with `server`.
- The backend URL is a local preference, never backend data. It **MUST** be
  read before a backend is selected and **MUST NOT** be stored in the backend
  SQLite `setting` table.
- Switching backends **MUST** perform a hard application reload so no stale
  signal, resource, or cached row from the previous backend remains visible.
- The **pure server** build (`server` without `desktop`/`mobile`) **MUST**
  force `BackendMode::Local` and **MUST NOT** forward API requests.
- The **web PWA** dispatches at runtime. **Local** uses typed SharedWorker RPC
  and **MUST NOT** issue HTTP requests to `/api/*`. **Remote** uses a full-URL
  HTTP client, sends `x-address-explorer-api-version`, and reports CORS or
  network failures as connection problems without inventing an HTTP status.
- Web API wrappers **MUST** keep the same public `*_api` names and typed
  signatures as native so screens do not branch on transport or mode. The worker
  RPC and web remote client are transport adapters; they **MUST NOT** contain
  business rules, SQL, or UI state.
- A backend switch **MUST** hard-reload the page. Loading a second backend in
  the same page session is forbidden.

For a server-rendered web application, the initial client render **MUST** match the server-rendered UI for hydration to succeed. In a fullstack SSR flow, server-dependent initial data **SHOULD** use the documented `use_server_future` mechanism where appropriate. Browser-only APIs **MUST** run only after hydration, for example in `use_effect`, and **MUST NOT** be accessed during server rendering. These requirements apply only when the project intentionally enables and deploys the relevant fullstack/SSR architecture.

### 5.9 Dioxus development workflow

Contributors **SHOULD** use the Dioxus CLI version recommended for the dependency baseline and commands documented for the intended target. `dx serve` **MAY** be used for local development when configured for that target. The default `cargo fmt --all` + `dx fmt` and `dx check` commands are defined in Section 4; platform bundling, system dependencies, and release packaging are covered in Sections 4 and 10. Version-matched official Dioxus documentation takes precedence over outdated snippets, except that `dx fmt --all-code` **MUST NOT** be used (Section 4.1).

### 5.10 Charts (Apache ECharts)

Apache ECharts is the default charting library for this project. The browser bundle and its licenses are vendored under `assets/vendor/echarts/` so charts work without a CDN or network connection. Contributors **MUST NOT** add another charting library, load ECharts from a third-party CDN, or enable a second charting stack without updating this guide and documenting the reason.

- Every chart **MUST** be its own dedicated component, normally under `src/components/charts/`. The component **MUST** own the complete presentation: the ECharts option, layout, colors, labels, arrows, tooltips, animations, responsive behavior, and chart-specific interactions.
- Chart components **MUST** receive only their business data, plus optional presentation metadata such as an accessible label. A screen or page **MUST NOT** build an ECharts option, call the ECharts JavaScript API, embed chart JSON, or choose chart internals; it passes data to the chart component and lets the component decide how to display it.
- Chart props and data types **MUST** stay domain-oriented. They **MUST NOT** expose raw ECharts options, canvas elements, or other rendering details to the calling screen. The component converts its data into an option internally.
- Chart components **MUST NOT** fetch, persist, or own business records. Loading, empty, and error states belong to the screen or application layer that supplies the data. Pure helpers such as layout calculation and option construction **SHOULD** be separate functions with unit tests.
- The shared hosts in `src/components/charts/echarts.rs` — `EChartsHost`, plus `EChartsZoomHost` for charts that forward ECharts interaction events such as `datazoom` back to Rust — are the only place that may initialize, update, resize, or dispose ECharts. A chart component passes its serialized option to one of those hosts instead of manipulating the DOM or JavaScript directly.
- Options **MUST** be generated with typed serialization such as `serde_json`, never by concatenating untrusted values into JavaScript source. Any custom JavaScript **MUST** escape data with the same care.
- Chart elements that should follow the app theme **MUST** use the color tokens from `src/components/charts/theme.rs` instead of hardcoded colors. A token is a `var(--md-sys-color-*)` string referencing the same Material 3 color role the CSS uses, with an optional `~alpha` suffix (for example `var(--md-sys-color-primary~0.15)`) for translucent fills the canvas cannot express through CSS. The host resolves every token in the option against the computed page style when it applies the option, and re-applies all options when `data-theme` or `data-mode` changes, so charts track palette and appearance switches without a reload. When filling a shape that carries text, the fill and label colors **MUST** be a paired role and its `on-*` counterpart (for example `primary` with `on-primary`) so the palette keeps the contrast it guarantees.
- Charts **MUST** provide an accessible label and remain legible in the supported light and dark appearance modes. Contributors **SHOULD** verify the rendered chart, resizing, and both modes before requesting review.
- When updating ECharts, replace the vendored bundle and license files together, record the new version in this guide or the vendoring README, and run the checks in Section 4.

### 5.11 Data tables: pagination and sorting

Every screen that lists rows in a table **MUST** use the shared backend-driven table pattern defined here, so all tables paginate and sort the same way. The historical prices screen (`src/screens/historical_prices.rs`) is the reference implementation.

**Query contract.**

- A table listing endpoint **MUST** be a server function taking a single query struct that flattens the shared [`TableQuery`](src/domain/table.rs) (`#[serde(flatten)]`) and adds the table's specific filters as typed fields (for example `PriceTableQuery` adds `symbol`, `from_time`, `to_time`). It **MUST** return the shared `TablePage<T>` (`items` plus the filtered `total` across all pages).
- `TableQuery` carries the 0-based `page`, the `page_size`, and the optional `sort_by` key with its `sort_dir`. Page numbers are 0-based in the query and 1-based in the UI. Services **MUST** clamp `page_size` to a documented maximum.
- Every listing table **MUST** send the shared `DEFAULT_PAGE_SIZE` from `src/domain/table.rs` (10 rows) as its `page_size`, so all tables display at most 10 rows per page. Screens **MUST NOT** define their own page-size constant.
- Any change to filters or sorting **MUST** reset the page to 0.

**Pagination bar.**

- Screens **MUST** render the shared `TablePagination` component (from `src/components/table_controls.rs`) directly below the table, so the data stays the first thing the page shows: Previous and Next buttons around an editable 1-based page index and the totals (` of N · M entries`).
- The page index **MUST** be editable: entering a number navigates to that page, clamped to the valid range. The endpoint **MUST** therefore accept arbitrary page indexes, not only previous/next steps.

**Sorting.**

- Sorting **MUST** run in the backend, never in the UI over one loaded page. Column headers of sortable columns **MUST** use the shared `SortableHeader` component; clicking the active column toggles its direction, clicking another column selects it.
- Sortable columns **MUST** be a fixed allowlist enum owned by the table's repository (for example `PriceSortColumn`), parsed from the client's string key and mapped to a fixed `ORDER BY` expression. Client input **MUST NOT** reach SQL as an identifier (Section 6.3), and an unknown sort key **MUST** be rejected at the service boundary.
- The `ORDER BY` expression **MUST** sort by the column's typed storage class: dates and timestamps as epoch-integer (or ISO date) columns, numbers as `REAL`/`INTEGER` columns — never as formatted display strings. Dates must compare as dates and numbers as numbers ("2024-…" and `100.0 > 20.0`, never lexicographic `"100.0" < "20.0"`). A deterministic secondary key (for example the primary key or timestamp) **MUST** follow the sort column so paginating a sorted listing never overlaps or skips rows.

**Extending the pattern.**

- When a table's behavior changes, the change **MUST** extend the shared definitions rather than hand-rolling equivalents: a new sortable column goes to the repository's allowlist enum and gains a `SortableHeader`, and a new filter becomes a typed field of the endpoint's query struct. Screens **MUST NOT** implement their own pagination bar or sort headers.
- A new table **MUST** be built from these pieces: a `TableQuery`-flattening endpoint, a service that validates the sort key and clamps the page size, repository `WHERE` fragments shared between the list and count queries, and the shared controls from `src/components/table_controls.rs`.

**Multi-select and bulk actions.**

- Multi-selection is opt-in per table. A table with one or more bulk actions uses `use_table_selection` and renders `TableSelectionHeader` as its first header cells and `TableSelectionCell` as the first body cell of every row. Tables without a bulk action **MUST NOT** render selection cells. Screens **MUST NOT** hand-roll checkboxes, select-all state, or selected-row classes; the shared `Checkbox` component provides the control, including the indeterminate state when only some rows are selected.
- `TableSelectionHeader` owns the select-all checkbox and the `th` directly next to it. Bulk action buttons (for example deletion or transaction sync) **MUST** be passed as its children so they read as applying to the selection; row-level actions stay in the second body cell.
- The selector `th`/`td` use `.md3-table-select-cell` and the bulk-action header uses `.md3-table-select-actions`; these cells shrink to their content. A row whose checkbox is checked carries `.md3-table-row-selected` so CSS can mark the whole row.
- Selection **MUST** be cleared whenever the page, sort, or filter changes and after a successful bulk action. Bulk commands **MUST** call one batch endpoint that rejects an empty selection and deduplicates ids. A batch that only performs local writes **MUST** apply them in a single transaction and return the affected count. A batch that performs external I/O per item (for example syncing each address) **MUST NOT** hold a database transaction across those requests; each item changes atomically and the endpoint returns a per-item summary that the UI reports, including partial failures.

### 5.12 Catalog settings (Wallet Types and Exchange Types)

The Settings area has two provider-catalog sub-settings built on one generic pattern. Contributors **MUST** extend that pattern rather than hand-rolling a second catalog screen:

- **Two-level taxonomy.** `exchanges_types` and `wallets_types` are the provider rows; `exchange_categories` and `wallet_categories` are their categories. A provider has at most one optional category. The `CatalogKind` enum selects both tables through a fixed allowlist; client input **MUST NOT** reach an SQL identifier.
- **One generic API.** The endpoints live under `/api/settings/catalogs/:kind/...` and serve both kinds. The `kind` path segment is parsed into `CatalogKind` before any service call. Pickers use the generic `/api/settings/catalogs/:kind/options` endpoint.
- **Identity.** Provider names are unique case-insensitively. A slug is derived from the name at creation and **MUST NOT** change afterwards, so CSV imports and external references stay stable. The seed rows from `src/data/migrations/0007_exchanges_types_data.sql` and `src/data/migrations/0009_wallets_types_data.sql` are fully user-owned; future migrations **MUST NOT** re-seed providers by name.
- **Brand color.** Each seed row carries the provider's primary published brand color from its official brand guidelines or press kit. The color only colors the provider badge; a row may have none, in which case `src/components/provider_badge.rs` derives a stable fallback. A user-edited color **MUST NOT** be overwritten: new colors for existing rows are added as a backfill migration (for example `src/data/migrations/0020_provider_brand_colors.sql`) that updates only `brand_color IS NULL` rows, and the seed files are updated at the same time so fresh databases match (Section 6.5.2).
- **Listings.** Provider tables follow Section 5.11 through the shared `TableQuery`, `CatalogTypePage` with global KPI totals, an allowlisted sort enum, a clamped page size, and the shared `TablePagination` / `SortableHeader` controls.
- **Deletion is a cascade.** Deleting providers removes their addresses (and the addresses' stored transactions) and, for exchanges, their ledger accounts (and the accounts' ledger rows) in one transaction. The UI **MUST** preview the affected counts and require confirmation; a category delete that still has providers **MUST** require typing the category name. The "Confirm destructive actions" setting may skip a dialog only when no related rows exist.
- **CSV import is create-only and partial.** Valid rows are inserted, duplicate names/slugs and invalid rows are skipped and reported, and an unknown category falls back to Uncategorized. The preview and the apply path **MUST** re-run the same pure planner against fresh database state.

### 5.13 Date selection fields

Every date or date-range filter **MUST** use the shared `DatePicker` component
in `src/components/date_picker.rs`. Views **MUST NOT** add a second date picker,
style a native `<input type="date">`, or implement their own calendar markup.

- **One compact field.** A range is displayed in one input as
  `YYYY-MM-DD – YYYY-MM-DD`; an open side is shown as `…`, and `Single` mode
  displays one date. The popup is the only part that may grow on screen.
- **Controlled value.** `value: DateSelection` supplies the committed or
  default selection and is rendered directly while the field is idle, so a
  screen prefill appears as soon as its data arrives. `on_change` emits a
  validated replacement as soon as a complete range (or a single date in
  Single mode) is picked, after OK for open-ended ranges, after Enter or a
  valid blur for typed input, and on Clear. Incomplete calendar edits remain a
  draft; Cancel, Escape, or an outside click discards them.
- **Validated values.** `src/domain/date_selection.rs` owns `IsoDate`,
  `DateSelection`, `DateSelectionMode`, `DateBounds`, typed input parsing, and
  the range selection state machine. Dates are strict UTC `YYYY-MM-DD` civil
  dates; impossible dates such as `2026-02-30` are rejected before they can
  reach a query.
- **Bounds and ordering.** `min_date` and `max_date` are inclusive bounds for
  both endpoints. Out-of-bounds days are disabled, typed out-of-bounds values
  are rejected, and a second calendar click before the start date swaps into a
  valid range rather than producing a reversed one. Defaults and bounds
  **SHOULD** be derived from the same data span so a screen cannot open with an
  invalid default. Clearing the field resets to the configured `min..max`
  range when both bounds exist (or to the available open-ended side) instead
  of emptying a bounded filter.
- **Modes.** `DateSelectionMode::Range` permits both, one, or neither endpoint
  (open-ended ranges). `DateSelectionMode::Single` stores only `from` and is
  used when a filter needs one date rather than a range.
- **Quick presets.** Above the range summary, the popup offers `Last week`,
  `Last month`, `Last year`, `This week`, `This month`, and `This year` in
  Range mode, and `Start of week`, `Start of month`, `Start of year`, and
  `Today` in Single mode. Weeks are Monday-first. "This" presets run from the
  period start through today; "Last" presets cover the full previous period.
  Presets clip to `min_date`/`max_date`, are disabled when unavailable, apply
  immediately, and close the popup.
- **Material 3 behavior.** The popup follows the M3 docked date-picker anatomy:
  labeled input, calendar icon button, month/year menu buttons, previous/next
  month controls, weekday column headers, a six-week grid, adjacent-month days,
  today/selected/in-range/disabled states, and Cancel/OK text actions. Wide
  layouts show two months; narrow layouts and `compact` show one. Calendar day
  targets are 48 px even though the single-line input keeps the shared 40 px
  control height.
- **Accessibility.** The field has a visible label, supporting text for the
  format or a validation error, and an inline error state. Supporting text is
  hidden by default across the application and is opted into through General
  Settings; the persisted `InterfaceSettings::show_date_range_support` value is
  loaded once by the app root and shared through context. Hidden supporting
  text stays available to assistive technology, and the field keeps its
  `aria-invalid` state. The dialog exposes grid and columnheader roles, full
  `"Monday, January 5, 2026"` day labels, `aria-selected`, `aria-current`, and
  `aria-disabled`. Arrow keys, Home/End, Page Up/Down, Shift+Page Up/Down,
  Enter/Space, Escape, and Shift+M/Y are supported. Focus returns to the input
  after closing, and range changes are announced through a polite live region.
- **Tests.** Pure parsing, bounds, ordering, open-ended states, selection
  transitions, calendar math, and preset periods live in
  `src/domain/date_selection.rs`, `src/domain/calendar.rs`, and
  `src/domain/date_presets.rs`. A change to the
  picker **SHOULD** add or update tests there instead of relying only on manual
  screen checks.

### 5.14 Shared UI primitives

Screens compose the existing primitives instead of re-implementing their markup:

- `Icon` / `IconName` (`src/components/icon.rs`) for Material icons.
- `PageHeader` (`src/components/page_header.rs`) for the screen title row.
- `Modal` (`src/components/modal.rs`) for scrim, Escape handling, and dialog ARIA. Views **MUST NOT** hand-roll a dialog scrim.
- `TableState` / `use_table_state` and the table controls in
  `src/components/table_controls.rs`.
- `ConfirmDialog`, `Checkbox`, `DatePicker`, `ProviderBadge`, `CsvDropzone`,
  `MultiSelect`, and the chart components.
- The formatters in `src/components/format.rs` and the UTC helpers in
  `src/domain/civil_time.rs`.

Navigation labels, page titles, and icons are declared once in
`src/navigation.rs` (`Destination`); screen URLs are declared in `src/app.rs`.
A new screen **MUST** add its destination there and use the same label in the
page header. A URL change **MUST** be deliberate and documented.

UI preferences (`FiatCurrency`, `InterfaceSettings`, theme) are loaded once by
the app root and shared through context. Screens **MUST NOT** fetch the same
setting independently or keep a second mutable copy.

### 5.15 Backend selection, web modes, and API versioning

Native builds support two runtime backends:

- **Local:** the bundled SQLite database; server functions execute in-process.
- **Remote:** the same Address Explorer server build, called over HTTP.

The web PWA supports the same two modes in one bundle:

- **Local:** browser SQLite owned by a SharedWorker; the UI calls it through
  typed RPC and never through `/api/*`.
- **Remote:** an arbitrary Address Explorer base URL called with the web HTTP
  client and CORS.

**Native preference.** The active backend is a local preference in
`backend.json` (`dirs::data_dir()/address-explorer/backend.json`; override with
`BACKEND_CONFIG_PATH`). It **MUST NOT** be stored in the backend database.
`BackendMode`, `BackendConfig`, URL validation, the startup health check, and
the hard-reload helper live in `src/backend.rs`.

**Web preference.** The web preference is stored in `localStorage` under the
versioned key `address-explorer.backend`:

```json
{ "version": 1, "mode": "local", "base_url": "https://addresses.example.com" }
```

- `mode` is `"local"` or `"remote"`; the first-run default is `"local"`.
- `base_url` is retained when switching to Local.
- The key is per-origin and is not backend data. It **MUST** be read before the
  backend is initialized. A schema change **MUST** bump `version` and define a
  migration or reset path.
- If `localStorage` is unavailable, Local still works; warn that the mode will
  reset on reload.

**Remote URL validation.** A remote URL **MUST** be an absolute `http://` or
`https://` base URL with a host and optional port/base path. It **MUST NOT**
include `/api`, a query, or a fragment. Trailing slashes are normalized. HTTPS is
required except on loopback; an HTTP page cannot call an HTTP remote because
browsers block mixed content.

**Connection test.** Settings “Test connection” calls `GET <base>/api/health`
and compares `API_VERSION`. Saving a remote URL is allowed only after a
successful matching test. A mismatch **MUST** block saving and show both
versions in a toast.

**Remote transport.** The native forwarding macros in `src/api/macros.rs` and
`src/api/remote.rs` mirror the Dioxus server-function wire contract. The web
remote client mirrors the same contract with a full base URL: path parameters in
the URL, `serde_qs`/URL-encoded query parameters, a JSON body keyed by argument
name, plain JSON on success, and the `{ message, code, data }` error envelope.
The absolute endpoint path **MUST** match the server function’s explicit
`#[get]`/`#[post]` route. Do not use anonymous `#[server]` paths for
remote-capable endpoints. CORS/network failures **MUST** be reported as a
connection problem, not as an API error with an invented status.

**API versioning.** `src/domain/health.rs` owns `API_VERSION` and
`API_VERSION_HEADER` (`x-address-explorer-api-version`). The client installs the
header before the first request via `src/api/version.rs`. Every web remote
`/api/*` request except `/api/health` **MUST** carry it. The pure server applies
`api::version::guard` and returns HTTP 426 with a structured error when the
header is missing or mismatched. The client checks `/api/health` at native
startup, in web Settings “Test connection”, and periodically while a remote
backend is active. A mismatch **MUST** block the remote backend and show a toast
with both versions; it **MUST NOT** be silently accepted.

**Server CORS.** The server **MUST** read `ADDRESS_EXPLORER_CORS_ORIGINS` as a
comma-separated allowlist of absolute origins (scheme + host + optional port, no
path or trailing slash). There is **no** permissive `*` default. When the
variable is unset or empty, the server sends no CORS headers and only
same-origin requests are accepted. The CORS layer **MUST** allow `GET`, `POST`,
`PUT`, `DELETE`, `OPTIONS`, the `content-type` and
`x-address-explorer-api-version` headers, and **MUST** answer `OPTIONS`
preflight before the API-version guard (a valid preflight **MUST NOT** receive
426). CORS **MUST** be the outermost layer so error responses also carry the
headers the browser needs. Example:

```sh
ADDRESS_EXPLORER_CORS_ORIGINS="https://addresses.example.com,https://staging.example.com"
```

**Worker protocol.** `crates/core` owns `WORKER_PROTOCOL_VERSION`, separate from
`API_VERSION`. The PWA and worker **MUST** compare it on connection. A mismatch
**MUST** block Local mode with an update/reload message; it **MUST NOT** silently
run mismatched RPC payloads.

**Switching.** Saving a backend change **MUST** hard-reload the application:
release desktop restarts the process, web reloads the page, and mobile uses a
webview reload or a full-root remount with a blocking overlay. Debug desktop
builds use the full-root remount so the reload can coexist with `dx serve`
without spawning a second managed process. Local and remote data are separate;
switching **MUST NOT** attempt to merge or migrate them.

**Bump policy.** Increment `API_VERSION` whenever an endpoint path or a
request/response body shape changes incompatibly. Increment
`WORKER_PROTOCOL_VERSION` whenever an RPC request/response variant or payload
shape changes. Update the native remote helper, web remote wrapper, worker RPC,
and endpoint checklist in the same change.

## 6. SQLite and `rusqlite` conventions

### 6.1 Connection lifecycle and file location

- Debug builds use `<workspace>/target/db/app.db`; release builds use the platform application-data directory, falling back to the existing legacy `~/address-explorer/app.db` when that file exists. The `DATABASE_PATH` environment variable overrides both. The database **MUST NOT** be stored in read-only packaged resources.
- The database directory **MUST** be created before opening a new database file. Failure to create or open it **MUST** be reported as an application error.
- Database paths **MUST NOT** be hard-coded to a specific user's home directory. Paths **MUST** be built with `Path`/`PathBuf` or a platform-aware directory API.
- The database filename and application-data directory **SHOULD** remain stable across releases so upgrades can find existing user data.
- Tests **SHOULD** use an in-memory database for isolated repository tests and a temporary on-disk database for reopening, persistence, or file-system behavior.
- A `rusqlite::Connection` **SHOULD** have a clear owner and lifecycle. A connection **MUST NOT** be stored in a Dioxus UI signal.
- The active backend preference is **not** database data. Native builds store it in the local `backend.json` described in Section 5.15; the web PWA stores it in `localStorage`. Both can be read before a backend is selected. The SQLite database is used only when the active mode is `Local`.
- The browser database is not a filesystem path. It uses the fixed virtual filename `address-explorer.db`, an IndexedDB database behind `relaxed-idb`, and no `std::fs`, `dirs`, `Path`, or `DATABASE_PATH`.
- The virtual filename **MUST** remain stable so upgrades find the same browser data. Export produces an ordinary SQLite file; import accepts one. Neither operation makes the live browser database a filesystem file.

Contributors **SHOULD** choose deliberately whether to use `rusqlite`'s `bundled` feature. Bundling SQLite can make the SQLite version and native dependency more consistent and reduce reliance on a system SQLite development package, but it can increase compile time and **MUST** be verified for every target. When using it, contributors **MUST** keep the dependency range compatible with the repository's supported version and **MUST** commit the lockfile. If linking system SQLite instead, CI and packaging documentation **MUST** describe the required system library and minimum version.

### 6.2 Threading and responsiveness

A `rusqlite::Connection` implements `Send`, but not `Sync`; it is not a general-purpose shared connection for simultaneous use from arbitrary threads. Contributors **MUST NOT** bypass the type system or use `unsafe` to share a connection concurrently.

**RECOMMENDED** default: serialize database access through a dedicated database worker or clearly owned worker context. That worker owns its connection, accepts typed requests, and returns owned result values. A smaller application **MAY** open a connection for an operation or use a mutex-protected connection where appropriate, provided access is serialized and blocking work remains off the UI path. A mutex **MUST NOT** be held while waiting for unrelated UI input, making network calls, or running unrelated CPU-heavy work.

SQLite supports concurrent readers but only one active writer per database at a time. Therefore:

- Contributors **SHOULD NOT** create a new operating-system thread for every query.
- Write operations that form one logical change **MUST** use a transaction.
- Lock waits **SHOULD** be bounded with an intentional busy-timeout policy if concurrent access is expected.
- WAL mode **MAY** help workloads with concurrent readers and a writer, but it is not a default requirement. It **SHOULD** be tested on all supported platforms and **MUST NOT** be assumed to remove SQLite's single-writer constraint.
- Database operations **MUST NOT** hold a global application lock while performing unrelated computation or waiting for user input.
- In the web build, the SharedWorker is the only owner of the `rusqlite::Connection`. The main thread **MUST NOT** open a connection, and multiple tabs **MUST NOT** open the IndexedDB VFS directly.
- Each tab owns one `MessagePort`. Requests are typed, correlated by request ID, and serialized by the worker; no connection or `rusqlite::Statement` crosses the port.
- Write endpoints run one at a time. The worker **MUST** await the patched commit barrier before returning success.
- Long syncs and seed imports **MUST** send progress and support cancellation without leaving a partial transaction or a stuck loading state.

### 6.3 SQL safety and repository boundaries

- External or user-provided SQL values **MUST** be bound with positional or named parameters (`?1`, `:name`, `params!`, or `named_params!`). They **MUST NOT** be inserted into SQL through `format!`, concatenation, or interpolation.
- SQL identifiers such as table names, column names, and sort directions cannot generally be bound as values. If identifiers are dynamic, they **MUST** be selected from a fixed allowlist.
- Queries **SHOULD** select explicit columns rather than `SELECT *`, so schema dependencies remain visible in review.
- Repository functions **SHOULD** return domain types rather than expose `rusqlite::Row`, `Statement`, or connection details to UI components.
- Prepared statements **MAY** be cached when queries repeat and caching improves clarity or measured performance.
- Database errors **SHOULD** retain their source for diagnostics while being translated to a safe user-facing message at the UI boundary.
- Dynamic query builders **SHOULD NOT** be introduced unless they solve a real query requirement more clearly than fixed queries.

### 6.4 Transactions and consistency

- A group of operations representing one logical change **MUST** be atomic and **MUST** run in a transaction.
- Transactions **MUST** be committed explicitly after all operations succeed. An uncommitted `rusqlite::Transaction` rolls back when dropped by default; code **SHOULD** rely on this as the failure path, not as an obscure control-flow technique.
- Transactions **SHOULD** be short. Code **SHOULD NOT** perform network requests, wait on channels, or run CPU-heavy analysis while a transaction is open.
- Constraints such as `PRIMARY KEY`, `UNIQUE`, `NOT NULL`, `CHECK`, and foreign keys **SHOULD** encode invariants in the schema when feasible.
- Foreign-key enforcement **MUST** be enabled on every connection where the schema relies on it, for example using `PRAGMA foreign_keys = ON`.
- Code **MUST** distinguish “not found” from other query failures when the domain semantics require that distinction.

Code **SHOULD** use the application's error type around `rusqlite::Result` when a workflow combines database errors with domain validation or other operations.

### 6.5 Schema migrations and initialization

Schema changes **MUST** be versioned and repeatable. A contributor changing the schema **MUST** include a migration and tests for a fresh database and the relevant supported previous schema version.

- Migrations **SHOULD** use ordered, checked-in SQL migration files or a small explicit migration module; the repository **SHOULD** use one approach consistently.
- SQLite's `PRAGMA user_version` **MAY** be used for simple local schema versioning, provided each migration updates it in the same transaction as its schema changes.
- Each migration **MUST** run transactionally where SQLite permits it and **MUST** fail loudly when an unexpected schema version is encountered.
- Migrations **MUST NOT** silently discard user data. Destructive changes **MUST** have an explicit migration strategy, review, and tests.
- A migration that may already have shipped **MUST NOT** be edited in place; contributors **MUST** add a new migration instead.
- Migrations **SHOULD** be deterministic. The migration runner **MUST** track applied versions so an already-applied migration is not run again.

Database initialization **MUST** follow this order: resolve the platform-approved application-data directory; create it if needed; open and configure the connection; run pending migrations; then permit normal repository operations. The exact path-resolution mechanism is platform-specific and **MUST NOT** assume the current working directory is writable.

Each connection **MUST** apply the settings the schema relies on, such as foreign-key enforcement and a deliberate busy timeout. Connection pragmas **SHOULD** be chosen intentionally and **MUST** be tested on every supported target.

#### 6.5.1 Migration files and version numbering

This repository keeps one ordered migration list in `src/data/migrations.rs`
([`MIGRATIONS`](src/data/migrations.rs)) and one checked-in SQL file per
entry under `src/data/migrations/`.

- **File name.** Every migration file **MUST** be named
  `00NN_snake_case_description.sql`, counting from `0001`. `NN` is the
  migration's 1-based position in [`MIGRATIONS`](src/data/migrations.rs) and is
  also the `PRAGMA user_version` it stamps. The highest number is always the
  latest migration, so the newest change is the last file in the directory and
  the last entry of the list.
- **One source of truth for the order.** The array order and the numeric
  prefix **MUST** agree: entry *i* (0-based) is version `i + 1`. A test
  (`migration_names_are_numbered_from_one`) fails when a prefix and its
  position disagree, so a new migration **MUST** be appended with the next
  free number and **MUST NOT** be renumbered later.
- **One schema file and at most one data file per table.**
  `00NN_<table>.sql` creates one table with its final columns, indexes, and
  triggers; `00NN_<table>_data.sql` inserts the reference rows owned by that
  table. A fresh database therefore never replays an `ALTER TABLE` sequence.
  Files use `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`,
  `CREATE TRIGGER IF NOT EXISTS`, `INSERT OR IGNORE`, or `WHERE NOT EXISTS`, so
  re-running a file is a no-op and never overwrites user edits.
- **Rust seed migrations.** Migrations that import a large bundled dataset
  (`0015_historical_prices_data`, `0019_addresses_data`) live in
  `src/data/seeds.rs` as `Migration::Seed` entries and consume their ordinal.
  `0019_addresses_data` reads the checked-in
  `src/data/migrations/0019_addresses_data.sql` catalog. Seeds run for
  persistent databases (native on-disk and browser-local); in-memory databases
  advance past the version without
  importing, so both kinds follow the same linear `user_version` sequence.
- **Transactions.** The runner opens one `IMMEDIATE` transaction per entry,
  executes the SQL or seed, updates `user_version`, and commits. A failing
  migration rolls back its own step and aborts startup; a migration **MUST NOT**
  swallow errors to appear successful.

#### 6.5.2 Upgrading existing databases

- **Resuming an older database.** The runner applies every entry above the
  stored `user_version` and leaves the rows already present untouched. Every
  schema and data file is idempotent, so a step that runs again never
  duplicates rows or overwrites user edits.
- **Newer databases** (`user_version > CURRENT_SCHEMA_VERSION`), for example a
  development database left over from a newer checkout, **MUST** fail loudly
  instead of being downgraded.
- **`hello_world` leftovers.** The smoke-test table was removed from the
  migrations. A database created before its removal may still contain it; it is
  unused and **MUST NOT** be dropped by a migration.
- When a new backfill is needed for rows that already exist (seed data that was
  added after a table shipped), add a **new numbered SQL migration** that
  updates only the unset values (`WHERE brand_color IS NULL OR
  trim(brand_color) = ''`). Never edit the seed file alone and assume existing
  databases will pick the change up.

#### 6.5.3 Adding a migration

1. Take the next free number (`CURRENT_SCHEMA_VERSION + 1`) and add
   `src/data/migrations/00NN_<name>.sql`.
2. Append the matching `("00NN_<name>", Migration::Sql(include_str!(...)))`
   entry to `MIGRATIONS`; never insert or reorder an entry.
3. Keep the SQL idempotent and transactional, and never overwrite user data
   that the migration does not own.
4. For a new table, add the schema file and, when the table owns reference
   rows, its `_data` file immediately after it.
5. Add or update tests: fresh database, upgrade from an older version, and
   assertions that existing rows survive.
6. Update Section 12 of this guide with the schema change.

#### 6.5.4 File and version summary

| Version | File | Kind |
| --- | --- | --- |
| 1 | `0001_setting.sql` | schema |
| 2–3 | `0002_exchange_categories.sql`, `0003_exchange_categories_data.sql` | schema + data |
| 4–5 | `0004_wallet_categories.sql`, `0005_wallet_categories_data.sql` | schema + data |
| 6–7 | `0006_exchanges_types.sql`, `0007_exchanges_types_data.sql` | schema + data |
| 8–9 | `0008_wallets_types.sql`, `0009_wallets_types_data.sql` | schema + data |
| 10 | `0010_addresses.sql` | schema |
| 11–13 | `0011_transactions.sql`, `0012_transaction_inputs.sql`, `0013_transaction_outputs.sql` | schema |
| 14–15 | `0014_historical_prices.sql` (file), `0015_historical_prices_data` (Rust seed) | schema + data |
| 16–18 | `0016_exchange_accounts.sql`, `0017_exchange_transactions.sql`, `0018_fiat_exchange_rates.sql` | schema |
| 19 | `0019_addresses_data` (Rust seed + SQL catalog) | data |
| 20 | `0020_provider_brand_colors.sql` | data backfill |
| 21 | `0021_flow_indexes.sql` | read-path indexes |
| 22 | `0022_exchange_transactions_btc_fee_sats.sql` | data backfill + triggers |

The current `CURRENT_SCHEMA_VERSION` is `22`.

#### 6.6 Browser-local SQLite (web Local mode)

- **Engine.** `rusqlite` compiles to `wasm32-unknown-unknown` through
  `sqlite-wasm-rs`. The worker installs `sqlite-wasm-vfs` `relaxed-idb`, opens
  the virtual database, enables `PRAGMA foreign_keys = ON`, and uses the
  `synchronous = OFF` setting the VFS requires.
- **Commit barrier.** The repository vendors `sqlite-wasm-vfs` under
  `vendor/sqlite-wasm-vfs` and exposes `wait_committed(filename)` for normal
  writes. The worker **MUST** await it after every write endpoint, before
  export, and before the last port closes. Reporting a save as successful
  without waiting for the IndexedDB commit is forbidden. Reapply and test the
  patch when the crate version changes.
- **One schema.** The browser database uses the exact same `MIGRATIONS` list and
  `CURRENT_SCHEMA_VERSION` as native. A web-only schema or migration list is
  forbidden. A migration change **MUST** be tested against a native database and
  a browser database.
- **Seeds.** On WASM, `0015_historical_prices_data` reads the stable
  `public/btc_usd.bin` asset generated by `build.rs` and fetched by the worker; it **MUST NOT** use `std::fs` or
  `include_bytes!` for the 6.2 MB pack. `0019_addresses_data` remains
  `include_str!`. A new browser database runs migrations and the full seed; a
  failed seed leaves Local unavailable but keeps Remote usable.
- **Storage and backup.** Request `navigator.storage.persist()` automatically
  on every Local-mode start; a denial or unsupported browser must not block
  Local mode. Settings shows `navigator.storage.estimate()` usage only. Export
  downloads the virtual database as `.sqlite`; import validates the
  header, `PRAGMA integrity_check`, `user_version`, and expected tables, then
  restores the previous database if validation fails. The file is unencrypted;
  warn users. Browser SQLite is unencrypted in v1; do not store secrets.
- **External sync.** Local mode may call Coin Metrics Community, Mempool, and
  Frankfurter directly; all three allow browser CORS. Handle offline, timeout,
  rate-limit, and CORS failures without corrupting local data.
- **Browser floor.** Local mode requires SharedWorker and IndexedDB. On an
  unsupported browser, show a blocking compatibility message and keep Remote
  available; do not attempt a best-effort multi-tab database.
- **Migrations across targets.** Exported SQLite files are portable across
  native and web builds. Import validates the file, runs the same migration
  list on older backups, and rejects backups whose schema is newer than the
  running build. Backend preferences (`backend.json` / `localStorage`) are not
  part of the database and are never imported.

#### 6.7 Database backup, import, and reset

Settings → General → **Data** manages the **active backend**. The same
`export_data_api`, `import_data_api`, and `clear_data_api` names select worker
RPC in web Local mode, an in-process server function in native Local mode, and
the remote HTTP transport in Remote mode. Screens and components **MUST NOT**
branch on the transport.

- **Export.** Produce a consistent SQLite snapshot with the SQLite online
  backup API (native/server) or the worker VFS export (web Local). The worker
  **MUST** await the commit barrier before exporting browser data.
- **Import.** Check the SQLite header, size limit, `PRAGMA integrity_check`,
  `user_version`, and required tables. Older backups run the normal migration
  list; newer backups are rejected with a clear message. The live database is
  backed up before replacement; any restore or commit failure restores it.
- **Reset.** Create a fresh database through the normal migration and seed
  path, back up the live database, restore the fresh copy, and restore the
  safety copy on failure. The result matches the browser's existing clear
  behavior: user data is removed and bundled reference rows return.
- **Browser worker.** Keep the previous database in memory during import and
  clear. A failed replacement **MUST** restore it and reopen Local mode.
- **Remote endpoints.** `POST /api/data/export`, `POST /api/data/import`, and
  `POST /api/data/clear` act on the server database. These endpoints inherit
  the CORS origin allowlist and API version guard; there is no authentication,
  so a server **MUST NOT** be exposed to untrusted origins.
- **Remote clients.** The client that performed the import or clear reloads.
  Other clients connected to the same remote server are not notified and must
  refresh before they see the replaced data.
- **Files.** Exports use the normalized filename chosen in the in-app modal.
  Web downloads use the browser's download location; desktop writes to the
  user's Downloads folder; mobile writes to the app documents folder and shows
  the absolute path. Exported files are ordinary, unencrypted SQLite files.
- **Versions.** Adding these endpoints bumps `API_VERSION`; renaming the worker
  RPC endpoints bumps `WORKER_PROTOCOL_VERSION`. No schema migration is
  introduced by the data section itself.

## 7. Single Responsibility Principle and module design

The Single Responsibility Principle (SRP) states that a module, struct, or function **SHOULD** have exactly one responsibility — one reason to change. In this repository, a "reason to change" is a single actor or concern that can request a change: the UI design, a business rule, the database schema, an external API contract, or a platform API. A unit of code that changes for two different reasons **MUST** be split.

### 7.1 Applying SRP at every level

SRP applies recursively — to the crate, each module, each struct, and each function:

- Every module **MUST** have a single responsibility that can be stated in one sentence without "and" (for example: "`frankfurter::http` performs HTTP requests to the Frankfurter API"). A module whose responsibility needs an "and" ("parses responses *and* decides which assets to display") **SHOULD** be split.
- A unit that mixes pure computation with I/O (SQL, HTTP, filesystem, clocks) **MUST** be split so the pure part can be tested without the side effect.
- A unit that talks to two different external systems (for example, SQLite and a network API) **SHOULD** be split into two focused units plus a third that orchestrates them.
- A file that both wires the application together (composition) and implements behavior **SHOULD** be split: wiring lives in the entry point, behavior in modules.
- A useful extraction test: a module **SHOULD** be movable into its own crate without dragging in unrelated code. If extraction would pull in UI, persistence, and networking at once, the boundary is wrong.

### 7.2 Layer responsibilities

The repository follows a layered architecture in which each layer has exactly one job. The classic *controller → service → repository* layering maps onto this Dioxus application as follows:

| Layer | Location | Responsibility |
| --- | --- | --- |
| Controllers (entry points) | `src/api/` (server functions), `src/screens/`, event handlers in `src/components/` | Translate requests and user input into service calls; render results |
| Services (business logic) | `src/application/` (one module per domain) | Encapsulate business rules; orchestrate repositories and clients |
| Repositories | `src/data/repositories/` | The only modules that execute SQL (Section 6.3) |
| API clients | `src/clients/<provider>/` | The only modules that speak to an external API |
| Models (structs) | `src/domain/`, `src/clients/<provider>/types.rs` | Plain data types shared between layers |

Rules for each layer:

- **API controllers** (`src/api/`) are the `#[server]` entry points of the application. They **MUST** be thin: open the database, call exactly one service operation, and translate errors into `ServerFnError`. They **MUST NOT** contain business rules or SQL.
- **Controllers (screens and event handlers)** are the entry points for user actions. They **MUST** be thin: parse input, call exactly one service operation (directly or through an API controller), and render the outcome. They **MUST NOT** contain business rules, SQL, or direct HTTP calls. Section 5.5 applies.
- **Services** encapsulate the business logic of one domain each. A service **MUST** own the rules and workflow for its domain and **SHOULD** expose one public function per use case, orchestrating repositories and clients to fulfill it. Services **MUST NOT** contain SQL text, HTTP details, RSX, or Dioxus signal code.
- **Repositories** are the only place where SQL appears (Section 6.3). One repository **SHOULD** cover one domain entity or aggregate. Repositories translate between rows and domain types and **MUST NOT** make business decisions (validation rules, pricing, formatting) — those belong to services or the domain.
- **API clients** are the entry points for external APIs. There **SHOULD** be one client module per provider, and transport (`http.rs`), wire types (`types.rs`), configuration (`config.rs`), and errors (`error.rs`) **SHOULD** each live in their own file inside the provider folder, as the existing `src/clients/` modules do.
- **Models (structs)** are plain data. A model **MUST NOT** contain SQL, HTTP, another layer's transport details, or Dioxus code. Small behavior that belongs to the value itself, such as validation or formatting, **MAY** live on the type.

Where the current repository does not yet contain a listed folder, contributors **SHOULD** create it when the first real use case appears and **MUST NOT** instead place that logic in a screen or component.

### 7.3 Dependency rule

Dependencies **MUST** point inward: controllers → services → repositories/clients → models. An inner layer **MUST NOT** import an outer layer:

- Models **MUST NOT** import services, repositories, clients, screens, or components.
- Services **MUST NOT** import screens, components, or RSX code.
- Repositories and clients **MUST NOT** import services; a service composes them, never the reverse.
- Circular dependencies between modules are **NOT RECOMMENDED** and usually indicate a missing module or a misplaced type.

When a service needs a side effect that tests must replace (clock, network, randomness), define a narrow trait — a *port* — owned by the service layer, and implement it in the outer *adapter* (ports-and-adapters / hexagonal architecture). A port **SHOULD NOT** be introduced for a single trivial use without a concrete testing or substitution need (Section 8.3). Passing the dependency as a parameter (dependency injection) **SHOULD** be preferred over constructing it inside the function; construction and wiring **SHOULD** happen once, at the entry point (the *composition root*).

### 7.4 Where structs and models live

All shared data types **MUST** have a designated home; contributors **MUST NOT** redefine the same concept ad hoc in whichever file happens to need it:

- **Domain types** (the application's own concepts, such as an address entry or a balance) live in the domain module for their domain (`src/domain/` in the Section 3 layout).
- **Wire types** for an external API live in that provider's `types.rs` (for example `src/clients/mempool/bitcoin/types.rs`) and **MUST** be mapped to domain types at the client boundary — external API schemas **MUST NOT** leak into services or UI.
- **Persistence types**, when they differ from domain types, live next to their repository and **MUST NOT** cross the repository boundary.
- **View types** used only for rendering (for example `BalancePoint` in Section 5.10) live next to the component that consumes them.

When data crosses a layer boundary, it **SHOULD** be mapped explicitly (a DTO/mapping step) rather than reusing another layer's type for convenience, unless the type is a genuine shared domain type.

### 7.5 Additional structural patterns

- **Use-case entry points:** each user-visible operation **SHOULD** correspond to one public service function named from the domain vocabulary (`rename_address`, `refresh_exchange_rates`). An event handler **SHOULD** call exactly one such function.
- **Unit of work:** when one logical change spans multiple repository calls, the coordinating service **MUST** run them in one transaction (Section 6.4); a repository **SHOULD NOT** silently open cross-entity transactions on its own.
- **Anti-patterns** that **MUST NOT** be introduced: god modules that accumulate unrelated functions; `Manager`/`Util`/`Helpers` grab-bag modules; business logic inside event handlers or wiring code; SQL or HTTP outside repositories/clients; one struct serving as the model for every layer.

### 7.6 Module-building checklist

Before adding or extending a module, contributors **SHOULD** be able to answer "yes" to each of the following:

1. The module's responsibility fits in one sentence without "and".
2. The module belongs to exactly one layer in Section 7.2 and imports only from inner layers.
3. Its public surface is minimal — items are `pub` only when another layer needs them; otherwise `pub(crate)` or private.
4. Its types come from the locations in Section 7.4 rather than ad-hoc redefinition.
5. Its pure logic can be unit-tested without Dioxus, a database file, or the network (Sections 5.6 and 9).

## 8. Clean-code standards for Rust

### 8.1 Naming and function design

- Names **MUST** describe intent. Contributors **SHOULD** prefer `load_recent_notes` over `get_data`, and `DatabaseWorker` over `Manager` when that is what the type does.
- Functions **SHOULD** do one coherent job. Split a function when it mixes input validation, SQL, transformation, UI updates, and error presentation.
- Functions **SHOULD** use early returns to handle invalid states and keep the main path easy to read.
- Booleans **SHOULD** read naturally at the call site. Contributors **SHOULD** prefer a named enum when a boolean's meaning is unclear or more than two states are likely.
- Unnecessary abbreviations, unexplained numeric literals, deeply nested conditionals, and excessively long parameter lists **SHOULD** be avoided.
- Typed domain structures and enums **SHOULD** be preferred over loosely related tuples or stringly typed state when the structure has meaning.

### 8.2 Errors, invariants, and panic policy

- Fallible functions **SHOULD** return `Result` and use `Option` for a legitimate absence rather than panic.
- `unwrap()` and `expect()` **MAY** be used in tests or for a truly impossible invariant explained by a specific message. They **MUST NOT** be routine production error handling.
- A shared error enum or a library such as `thiserror` **SHOULD** be used when it makes error sources and variants clearer. Code **SHOULD NOT** convert every failure to an uninformative string too early.
- Input **MUST** be validated at boundaries. Once validation succeeds, internal functions **SHOULD** rely on explicit invariants.
- Errors **MUST NOT** be silently swallowed with `let _ = ...` or empty `Err` branches. Ignoring an error **MAY** be acceptable only when intentional and documented.

### 8.3 Abstractions, comments, and unsafe code

- A trait, generic framework, macro, or dependency **SHOULD NOT** be introduced for one trivial use without a concrete benefit.
- Duplicated business rules **SHOULD** be extracted when duplication could allow behavior to diverge.
- Comments **SHOULD** explain why a non-obvious decision exists, especially around concurrency, migrations, platform limitations, security, and user-visible behavior. Comments **SHOULD NOT** restate the code, narrate obvious control flow, or reference section numbers in this guide. Keep contracts and invariants; delete essays and stale examples.
- Public modules and non-obvious public APIs **SHOULD** have documentation comments describing contracts, errors, and important edge cases.
- `unsafe` **MUST** be avoided unless necessary. Every unsafe block **MUST** include a nearby `// SAFETY:` comment explaining why its requirements are upheld.
- Clippy warnings **SHOULD** be resolved rather than broadly allowed. A targeted lint allowance **MAY** be used with a comment explaining its rationale.

### 8.4 Dependencies, side effects, and privacy

- A dependency **SHOULD** be added only when it provides meaningful value over the standard library or existing dependencies.
- Filesystem access, database access, clocks, environment variables, and OS APIs **SHOULD** be kept at clear boundaries so domain logic remains testable.
- SQL, serialization details, or platform-specific paths **SHOULD NOT** be embedded in domain types unless the type genuinely represents that detail.
- Secrets, private user data, authentication tokens, and sensitive database contents **MUST NOT** be logged.
- Analytics, telemetry, or network behavior **MUST NOT** be added without documenting its purpose and privacy implications.

## 9. Tests and quality gates

A change **MUST** include tests when it changes business rules, persistence behavior, data validation, migrations, concurrency behavior, or error handling. UI-only changes **SHOULD** include component tests or documented manual verification appropriate to the feature.

Coverage expectations by change type:

- **Domain logic:** unit tests for ordinary cases, edge cases, and invalid inputs.
- **Repository/SQL:** SQLite tests using an isolated in-memory database or temporary file.
- **Migrations:** fresh-database migration, migration from supported prior schema versions, and assertions that existing data remains correct.
- **Concurrency:** tests or a documented verification plan for cancellation, worker shutdown, ordering, and race-prone behavior where relevant.
- **UI behavior:** verification of loading, success, empty, and failure states when applicable.
- **Currency and fiat conversion:** tests for default-currency propagation, BTC-to-fiat calculations, conversion idempotency, preservation of existing/manual prices, and non-fatal failure warnings where applicable.
- **Platform-specific code:** build or execute checks on the affected platform; mock the OS boundary when practical.
- **Browser worker and SQLite:** worker browser tests for VFS install/open, migrations, seed import, write plus commit barrier, export/import round-trip, invalid import rollback, and worker protocol mismatch.
- **Data backup and restore:** export/import round-trip, older-schema upgrade, newer-schema rejection, missing-table rejection, non-SQLite input, reset-to-defaults, and rollback that preserves the previous database.
- **Worker protocol:** request IDs, sequential writes, progress/cancel, `DataChanged` broadcast, and a two-tab RPC check.
- **Endpoint parity:** every endpoint exposed by native server functions is also exposed by the worker RPC and the web remote wrapper; a test fails when one is missing.
- **Remote HTTP and CORS:** JSON, query, path, error envelope, 426, missing version header, allowlisted origin success, unlisted origin denial, and OPTIONS preflight.
- **PWA and offline:** first-run local seed, persistence across reload, offline reload after installation, multi-tab writes, unsupported-browser message, and export/import.

Tests **MUST NOT** depend on a developer's real database, home directory, network connection, or manually prepared application state. Tests **SHOULD** be deterministic and clean up temporary files they create.

The baseline pre-PR commands are listed in Section 4. Release builds and target-specific bundle commands **SHOULD** be tested for platform-sensitive changes. If a check cannot be run, the pull request **MUST** identify which check was skipped and why.

## 10. Cross-platform compatibility

- Platform-specific imports and APIs **SHOULD** be kept behind narrow modules or `cfg` gates.
- Code **MUST NOT** assume that the current working directory is writable or points to the application directory.
- UI layout, fonts, keyboard behavior, touch targets, scrolling, and accessibility **SHOULD** be checked on each supported renderer/platform. CSS and WebView behavior can differ.
- Android lifecycle transitions, background restrictions, permission prompts, and low-memory behavior **SHOULD** be considered for long-running work.
- A feature unavailable on a platform **MUST** fail gracefully or be explicitly marked unsupported; it **MUST NOT** silently report success.
- CI **SHOULD** include Linux and macOS builds and Android compilation/package checks. Platform packaging, signing, and notarization requirements **MUST** be documented separately from ordinary development builds.
- The web PWA is supported only on latest 2026+ Chrome (desktop/Android), Edge, Firefox, and Safari (macOS/iOS). Do not add legacy polyfills or a dedicated-worker fallback for older browsers.
- On an unsupported browser, Local mode **MUST** fail gracefully with a clear message; Remote remains available. It **MUST NOT** silently create a second database or report Local as ready.
- Service workers, `navigator.storage.persist()`, and persistent browser storage require HTTPS or `localhost`. A plain-HTTP deployment can run the app but not the offline shell.
- iOS/Safari storage eviction and private browsing **MUST** be considered; SQLite export is the user-facing recovery path.
- A static host **MUST** rewrite unknown paths to `index.html` for the client-side router.

## 11. Pull requests and review

Every pull request **SHOULD** be small enough to review as one logical change. A pull request **MUST** explain:

- what changed and why;
- relevant user-visible behavior or API/schema impact;
- how it was tested and on which platforms;
- migration or compatibility implications, if any;
- known limitations or follow-up work.

Before requesting review, contributors **MUST**:

- [ ] Format Rust and RSX code with `cargo fmt --all` and `dx fmt` after the last change.
- [ ] Run `dx check` for the affected targets, the Cargo test commands, and Clippy for the affected workspace.
- [ ] Add or update tests and documentation where needed.
- [ ] Include a migration for database schema changes, named with the next `00NN_` version (Section 6.5), and update the schema reference in Section 12.
- [ ] Verify every new or changed fiat display and conversion uses the `FiatCurrency` context and the default currency, without hardcoded USD assumptions (Section 3.1).
- [ ] Check that no secrets, local database files, build artifacts, or unintended changes are included; describe formatting churn in the pull request.
- [ ] Web changes build with plain `dx serve`/`dx build`; `build.rs` generated `local-worker.js`, `worker/*`, and `btc_usd.bin`. No xtask or precache manifest is required.
- [ ] A new or changed endpoint is implemented in the native controller, exposed through worker RPC, exposed through the web remote wrapper, and covered by the endpoint parity test.
- [ ] API request/response shape changes bump `API_VERSION`; worker RPC changes bump `WORKER_PROTOCOL_VERSION`; both remote transports and the docs are updated.
- [ ] CORS changes update `ADDRESS_EXPLORER_CORS_ORIGINS` documentation and the allowlist/preflight tests. No `*` default was introduced.
- [ ] Web Local writes await the patched commit barrier before reporting success; export/import and migration behavior were tested.
- [ ] Data export/import/reset was exercised against the active local and remote backends, including cross-target import and rollback on a rejected file.
- [ ] Browser localStorage keys and JSON shapes are versioned; an incompatible change has a reset/migration path.
- [ ] Local mode was tested offline after an installed PWA load; Remote mode was tested against an allowlisted cross-origin server and an API-version mismatch.
- [ ] No native↔web migration or encryption guarantee is implied beyond what is implemented.
- [ ] The supported browser floor was not weakened, and unsupported browsers still get the Local message plus Remote.
- [ ] Describe any validation that could not be performed.

Reviewers **SHOULD** evaluate correctness, readability, error handling, concurrency, SQL safety, migration safety, privacy, respect for the layer boundaries in Section 7, currency handling (Section 3.1), and platform compatibility—not only whether the code compiles.

## 12. Database tables and reference data

`src/data/migrations/` is the only schema source of truth; do not duplicate
the SQL here. The browser-local database uses the same tables, migrations,
invariants, and `CURRENT_SCHEMA_VERSION`; web-only tables or seed variants are
forbidden. `historical_prices` is seeded in the browser from the generated stable
`public/btc_usd.bin` (source `assets/pricing/btc_usd.bin`); `addresses` is seeded from the checked-in
`0019_addresses_data.sql`. Export produces a normal SQLite snapshot; import
validates the header, `PRAGMA integrity_check`, schema version, and required
tables, upgrades older schemas through the same migration list, and rejects
newer schemas. The operation is a portable cross-target backup, not a backend
preference migration. A database at `CURRENT_SCHEMA_VERSION = 22` has these tables:

| Table | Purpose | Reference rows |
| --- | --- | --- |
| `setting` | Key/value user preferences | none; documented defaults |
| `exchange_categories` | Exchange provider categories | 8, user-owned |
| `wallet_categories` | Wallet provider categories | 9, user-owned |
| `exchanges_types` | Exchange providers | 16, user-owned |
| `wallets_types` | Wallet providers | 18, user-owned |
| `addresses` | Tracked addresses and cached statistics | researched catalog from `0019_addresses_data` |
| `transactions` | Cached on-chain transactions | none |
| `transaction_inputs` | Cached transaction inputs | none |
| `transaction_outputs` | Cached transaction outputs | none |
| `historical_prices` | Daily/intraday price closes by symbol | packed BTC/USD series from `0015_historical_prices_data` |
| `exchange_accounts` | Imported ledger accounts | none |
| `exchange_transactions` | Imported ledger rows | none |
| `fiat_exchange_rates` | Cached Frankfurter rates | none |

Important invariants live in the migrations and are tested there:

- `addresses.exchange_id` and `addresses.wallet_id` are mutually exclusive;
  triggers enforce the rule on insert and owner updates.
- `exchange_transactions.amount_sats` is always satoshis. A BTC/XBT fee is
  stored in satoshis; `0022_exchange_transactions_btc_fee_sats.sql` rejects
  fractional BTC/XBT fees.
- `historical_prices` is `WITHOUT ROWID` and keyed by `(symbol, close_time)`.
- Foreign-key enforcement is a connection pragma (`PRAGMA foreign_keys = ON`),
  not a schema property.
- `hello_world` is a removed legacy smoke-test table. If it exists in an old
  database it is unused and **MUST NOT** be dropped by a migration.

Inspect a development database with:

```sh
sqlite3 target/db/app.db ".schema"
sqlite3 target/db/app.db "PRAGMA user_version;"
```

When a migration ships, update the version and table list above only when the
newest migration changes them. Add or update the fresh-database and upgrade
tests in `src/data/migrations.rs`.

## 13. References

Contributors **MUST** use documentation matching the versions pinned by the repository.

- [Dioxus 0.7 documentation](https://dioxuslabs.com/learn/0.7/)
- [Dioxus hooks](https://dioxuslabs.com/learn/0.7/essentials/basics/hooks/)
- [Dioxus state and signals](https://dioxuslabs.com/learn/0.7/tutorial/state/)
- [Dioxus resources](https://dioxuslabs.com/learn/0.7/essentials/basics/resources/)
- [Dioxus Router](https://dioxuslabs.com/learn/0.7/essentials/router/)
- [Dioxus platform support](https://dioxuslabs.com/learn/0.7/guides/platforms/)
- [Dioxus desktop/mobile bundling](https://dioxuslabs.com/learn/0.7/tutorial/bundle/)
- [Apache ECharts documentation](https://echarts.apache.org/en/index.html)
- [Apache ECharts option reference](https://echarts.apache.org/en/option.html)
- [Frankfurter API documentation](https://frankfurter.dev/)
- [Coin Metrics API documentation](https://docs.coinmetrics.io/)
- [`rusqlite` documentation](https://docs.rs/rusqlite/latest/rusqlite/)
- [`rusqlite::Connection`](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html)
- [`rusqlite::Transaction`](https://docs.rs/rusqlite/latest/rusqlite/struct.Transaction.html)
- [SQLite documentation](https://www.sqlite.org/docs.html)
- [`sqlite-wasm-rs`](https://docs.rs/sqlite-wasm-rs/)
- [`sqlite-wasm-vfs`](https://docs.rs/sqlite-wasm-vfs/)
- [MDN SharedWorker](https://developer.mozilla.org/en-US/docs/Web/API/SharedWorker)
- [MDN Service Worker API](https://developer.mozilla.org/en-US/docs/Web/API/Service_Worker_API)
- [MDN IndexedDB API](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API)
- [MDN Storage API](https://developer.mozilla.org/en-US/docs/Web/API/Storage_API)
- [`tower-http` CORS](https://docs.rs/tower-http/latest/tower_http/cors/)
- [Rust Design Patterns](https://rust-unofficial.github.io/patterns/)
- [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119)
- [RFC 8174](https://www.rfc-editor.org/rfc/rfc8174)

When an example conflicts with official documentation for the pinned dependency version, the official documentation and a focused reproducible test **MUST** take precedence over an outdated snippet.
