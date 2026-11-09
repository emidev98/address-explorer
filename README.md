# Address Explorer

A Dioxus application for exploring Bitcoin addresses, historical prices, and exchange ledgers.

## Features

- Track Bitcoin addresses, balances, and transaction history
- Explore historical BTC prices and fiat conversions
- Import exchange CSV ledgers, reconcile them with cached on-chain data, and review provider balances
- Display fiat prices and balances in the default currency chosen in Settings
- Export, import, and reset the active local or remote database from Settings → General → Data
- Switch between Material 3 color palettes and appearance modes in Settings
- Choose automatic, top, or side primary navigation; phones use a slide-in drawer
- Compress the side panel into an icon rail, with Settings and the toggle pinned to the bottom

## Installation

1. **Prerequisites**
   - Rust (latest stable) and Cargo. Install them from
     <https://rustup.rs>; the setup command uses this Cargo to bootstrap the
     rest of the toolchain.

2. **Clone the repository**
   ```bash
   git clone https://github.com/emidev98/address-explorer.git
   cd address-explorer
   ```

3. **Prepare the development environment**
   ```bash
   # Desktop + web toolchain for this operating system
   cargo prepare

   # Also Android everywhere supported, and iOS on macOS
   cargo prepare --all
   ```

   The command installs the pinned Dioxus CLI, Rust targets, system build
   libraries, the web SharedWorker compiler toolchain, and Node.js; with
   `--all` it also installs the Android and (on macOS) iOS toolchains. Use
   `cargo prepare --check` to verify without changing anything.
   See [docs/BUILDING.md](docs/BUILDING.md) for the OS/bundle matrix, manual
   fallbacks, and troubleshooting.

4. **Run the application**

   ```bash
   # Web PWA development (Local browser SQLite or Remote server)
   dx serve
   dx serve --web

   # Desktop all-in-one: bundled local backend + optional remote backend
   dx serve --desktop
   ```

   The root `build.rs` builds the SharedWorker and copies the stable seed
   automatically, so both web commands are self-contained.

   `dx serve --desktop` still starts DX's separate server process, but the
   desktop client is all-in-one and does not use it. For a single-process
   development run:

   ```bash
   cargo run --no-default-features --features desktop
   ```

5. **Build for production**

   Native builds are always all-in-one. The default backend is the bundled
   SQLite database; Settings → General can point the app at a remote Address
   Explorer server instead. Switching backends reloads the application.

   ```bash
   # Desktop
   dx bundle --desktop --release --no-default-features

   # Android
   dx bundle --android --release --no-default-features

   # iOS
   dx bundle --ios --release --no-default-features
   ```

   Web is one deployable PWA that supports both Local and Remote backends:

   ```bash
   dx build --web --release
   # or: dx bundle --web --release --no-default-features --features web
   ```

   Deploy the whole `target/dx/address-explorer/release/web/public` directory.
   The build includes the app shell, SharedWorker WASM, `local-worker.js`,
   manifest, service worker, icons, and the stable `btc_usd.bin` seed. No
   precache manifest or second command is required. Serve it over HTTPS (or
   `localhost`) and rewrite unknown paths to `index.html` for the router.

   The direct Cargo build for desktop is:

   ```bash
   cargo build --release --no-default-features --features desktop
   ```

## Backend selection

- Native desktop/mobile binaries contain the local SQLite backend and can
  optionally use a remote server configured in **Settings → General**.
- The web PWA supports the same two modes in one bundle. **Local** is the
  first-run default and stores the database in the browser through a
  SharedWorker. **Remote** accepts any Address Explorer base URL.
- The backend preference is stored locally (`backend.json` on native,
  `localStorage` on web), not in the backend database.
- Remote connections are checked against the app's API version. A mismatch
  blocks the server from serving API requests and shows a toast in the client.
- Remote web clients require the server to allow their origin in
  `ADDRESS_EXPLORER_CORS_ORIGINS` (comma-separated, no `*` default). A server
  that serves the PWA itself needs no CORS entry.
- Switching between local and remote performs a hard application reload so no
  stale data from the previous backend remains.
- The local and remote databases are separate; switching does not migrate data.
  Settings → General → Data exports, imports, or resets the active local or
  remote database. Exported SQLite files are portable between web, desktop,
  mobile, and remote backends; older schemas are upgraded on import and newer
  schemas are rejected. The database and exported files are not encrypted.

## Releases

Stable GitHub releases are built by the release workflow. It attaches desktop,
Android, web, and server artifacts, appends a generated changelog, and deploys
the PWA to Vercel. See [docs/release.md](docs/release.md) for the release
checklist and [docs/installation.md](docs/installation.md) for install notes.

The optional server image and tarball are documented in
[docs/deployment.md](docs/deployment.md); no live backend is deployed by the
workflow.

## Development

For development guidelines, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Styles

The interface uses a Material 3 theme implemented with Tailwind CSS v4. For
styling conventions, theme structure, and rebuild instructions, see
[CONTRIBUTING.md](CONTRIBUTING.md#styling-tailwind-css-v4-and-material-3).

## License

MIT
