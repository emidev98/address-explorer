# PWA and server deployment

Address Explorer's web PWA is Local-first: it runs without a remote server and
stores its database in the browser through a SharedWorker. The optional
pure-server build provides the REST API used by **Settings → General →
Remote**. The release workflow deploys the PWA but does not deploy a live
server backend.

For diagrams of the build/deployment topology and client-server transports,
see [`architecture.md`](architecture.md). For the release artifact list and
workflow gates, see [`release.md`](release.md).

## Deployment topology

- **PWA host** — static files over HTTPS, with an `index.html` fallback for
  application routes.
- **Optional remote server** — the `server` build on a host with a persistent
  writable volume for SQLite.
- **No shared database** — Local and Remote stores are separate. Switching
  backends reloads the app and never migrates data.

A server that also serves the PWA needs no CORS entry for that PWA. A PWA on a
different origin needs its origin in `ADDRESS_EXPLORER_CORS_ORIGINS`.

## Deploy the PWA

Build the deployable directory:

```sh
dx build --web --release --locked
```

Output: `target/dx/address-explorer/release/web/public`.

The static host **MUST**:

- serve the directory over HTTPS (or `localhost`) for the service worker;
- rewrite unknown application paths to `index.html` for the client-side
  router;
- return 404 for `/api/*` rather than the SPA shell;
- revalidate `index.html`, `sw.js`, `sw-register.js`, `local-worker.js`,
  `/worker/*`, `btc_usd.bin`, and the manifest;
- cache hashed `/assets/*` immutably.

The release workflow uses `scripts/build-vercel-output.sh` to build a Vercel
Build Output tree with those rules and runs `vercel deploy --prebuilt --prod`.
`vercel.json` disables the Vercel Git integration to avoid duplicate deploys.

## Run the remote server

### Tarball

The release tarball contains:

```text
address-explorer-server_<version>_linux_<arch>/
├── address-explorer
├── assets/pricing/btc_usd.bin
├── public/                 # required empty directory
├── LICENSE
└── README.md
```

```sh
tar -xzf address-explorer-server_<version>_linux_x86_64.tar.gz
cd address-explorer-server_<version>_linux_x86_64

IP=0.0.0.0 \
PORT=8080 \
DATABASE_PATH=/var/lib/address-explorer/app.db \
ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
./address-explorer
```

The database directory must be writable. The first start applies migrations
and imports the bundled price history. The binary looks for
`assets/pricing/btc_usd.bin` beside itself; set `ADDRESS_EXPLORER_PRICING_FILE`
to use a different path. Keep the empty `public/` directory next to the binary:
the Dioxus server reads it at startup even when the PWA is served separately.

### Container

```sh
docker run --rm \
  -p 8080:8080 \
  -v address-explorer-data:/data \
  -e ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
  ghcr.io/emidev98/address-explorer:latest
```

The release image is a multi-arch runtime image built from the prebuilt
binary. It listens on `0.0.0.0:8080`, runs as UID 10001, stores SQLite at
`/data/app.db`, and exposes `/api/health` as its healthcheck. The `/data`
volume must be writable by that UID; when using a host bind mount, adjust its
ownership before starting the container.

The repository root `Dockerfile` is a separate source-build alternative for
users who want to build the server inside Docker. The release workflow does
not use it.

## Environment

| Variable | Purpose |
| --- | --- |
| `IP` | Bind address; use `0.0.0.0` in a container |
| `PORT` | HTTP port, default `8080` |
| `DATABASE_PATH` | Writable SQLite file path |
| `ADDRESS_EXPLORER_CORS_ORIGINS` | Comma-separated browser origins; no wildcard |
| `ADDRESS_EXPLORER_PRICING_FILE` | Optional path to `btc_usd.bin` |

## API contract, CORS, and health

`GET /api/health` returns the application version and the server API version.
It is exempt from the API version guard.

Every other remote `/api/*` request **MUST** carry
`x-address-explorer-api-version` equal to the server's `API_VERSION` (currently
`2`). A missing or mismatched header receives HTTP 426 with a structured error
body. Settings “Test connection” checks `/api/health` before a remote URL can be
saved; a mismatch blocks the backend and shows both versions.

`ADDRESS_EXPLORER_CORS_ORIGINS` is a comma-separated allowlist of absolute
origins. There is no permissive `*` default; when unset or empty, the server
sends no CORS headers and only same-origin requests are accepted. The CORS
layer allows `GET`, `POST`, `PUT`, `DELETE`, `OPTIONS`, the `content-type` and
`x-address-explorer-api-version` headers, and answers `OPTIONS` preflight
before the version guard.

## Security

The server has **no authentication**. `ADDRESS_EXPLORER_CORS_ORIGINS` is a
browser CORS allowlist, not access control: direct HTTP clients can still read
and modify the data. Do not expose the server to untrusted networks or origins.
Keep the URL private or add authentication before making it public.

The server database, browser database, and exported files are unencrypted. Do
not store secrets in them.

## Backups and upgrades

Back up the SQLite file before upgrading, using the SQLite backup API or:

```sh
sqlite3 /data/app.db ".backup '/data/backups/app-$(date +%Y%m%d%H%M%S).db'"
```

Settings → General → Data can also export, import, or reset the active
database. Exports are ordinary unencrypted SQLite files. Import validates the
file, upgrades older schemas through the normal migration list, and rejects
backups created by a newer schema.

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for the full validation and rollback
rules, and [`server/README.md`](server/README.md) for the tarball quickstart.

## Host choice

Use a host that can run a container or a native binary and attach persistent
storage. Free tiers with ephemeral filesystems and scale-to-zero behavior are
not durable for SQLite. A small VPS, a home server, or a persistent free-tier
host is the realistic option. Supabase cannot run the Rust server binary or
provide the persistent SQLite volume; porting the data layer to Postgres would
be a separate application change.
