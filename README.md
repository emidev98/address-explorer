# Address Explorer

Multi-Platform application for exploring Bitcoin addresses, on-chain funds flow,
historical prices, and exchange ledgers built with Dioxus

## Features

- Track Bitcoin addresses, balances, transaction history, and on-chain funds
  flow.
- Import exchange CSV ledgers, match the data automatically with on-chain data,
  link transactions manually, and review provider balances and KPIs.
- Manage exchange and wallet provider catalogs, and categories.
- Explore historical BTC prices, sync price ranges, edit stored closes, and
  convert the history into the default fiat currency.
- Run Local-first in the browser with SQLite in a SharedWorker, or point the
  same PWA at a remote Address Explorer server.
- Export, import, and reset the active local or remote database from
  **Settings → General → Data**. Exported SQLite files are unencrypted.
- Use the installed PWA offline after the first load, on current browsers with
  SharedWorker and service-worker support.

## Quick start

Rust must already be available. Install it from <https://rustup.rs>, then from
the repository root:

```sh
cargo prepare
```

`cargo prepare` installs the pinned Dioxus CLI, Rust targets, system build
libraries, the web SharedWorker compiler toolchain, and Node.js. With `--all`
it also installs the Android toolchain and, on macOS, verifies the iOS
toolchain. `cargo prepare --check` verifies without changing anything.

```sh
# Web PWA development: Local browser SQLite or Remote server
dx serve
# or: dx serve --web

# Desktop all-in-one: bundled local backend plus optional remote backend
dx serve --desktop

# Single-process desktop alternative
cargo run --no-default-features --features desktop
```

The web commands build the SharedWorker and copy the stable pricing seed
automatically through `build.rs`. See [`docs/BUILDING.md`](docs/BUILDING.md)
for the full environment setup, bundle matrix, and troubleshooting.

## Build and deploy

```sh
# Web PWA
dx build --web --release --locked
# Output: target/dx/address-explorer/release/web/public

# Native desktop
dx bundle --desktop --release --locked --no-default-features

# Optional remote server
cargo build --release --locked --no-default-features --features server
```

The deployable web directory includes the app shell, SharedWorker WASM,
`local-worker.js`, the PWA manifest and service worker, icons, and the stable
`btc_usd.bin` seed. Serve it over HTTPS (or `localhost`) and rewrite unknown
paths to `index.html` for the client-side router. See
[`docs/deployment.md`](docs/deployment.md) for static-host and server
deployment details.

## Runtime modes

| Mode | Where it runs | Data |
| --- | --- | --- |
| Local | Desktop, mobile, or web PWA | Local SQLite (native) or browser SQLite through a SharedWorker (web) |
| Remote | Desktop, mobile, or web PWA | A separate Address Explorer server over HTTP(S) |

The web PWA supports both modes in one bundle; Local is the first-run default.
The backend preference is stored outside the backend database (`backend.json`
on native, `localStorage` on web). Switching backends reloads the application
and does not migrate data. Remote clients require the server's explicit CORS
allowlist and a matching API version.

For diagrams of the build pipeline, deployment topology, and client-server
transports, see [`docs/architecture.md`](docs/architecture.md).

## Documentation

- [Architecture](docs/architecture.md) — build, deployment, and transport
  diagrams.
- [Building](docs/BUILDING.md) — toolchains, commands, bundle matrix, and
  troubleshooting.
- [Deployment](docs/deployment.md) — PWA hosting and the optional remote
  server.
- [Releases](docs/release.md) — release workflow, artifacts, Vercel, and
  changelog.
- [Installation](docs/installation.md) — installing published builds.
- [Contributing](CONTRIBUTING.md) — code rules, testing, SQLite, and review.
- [Agent contributions](docs/agents/GOVERNANCE.md) — frozen paths, the review
  gate, and how a contribution earns trust. Start at [`AGENTS.md`](AGENTS.md).

## License

MIT
