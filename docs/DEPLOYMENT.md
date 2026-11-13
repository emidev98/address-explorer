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
ADDRESS_EXPLORER_TLS_HOSTS=addresses.example.com,192.168.1.10 \
ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
./address-explorer
```

The server is HTTPS-only for non-loopback binds. On first start it creates a
local CA and server certificate under `ADDRESS_EXPLORER_TLS_DIR` (default
`<database directory>/tls`). Install `ca.crt` on every client and browser, or
provide `ADDRESS_EXPLORER_TLS_CERT` and `ADDRESS_EXPLORER_TLS_KEY` from a
trusted CA. The CA certificate can be copied from the TLS directory or retrieved as a
JSON-encoded PEM string from `GET /api/tls/ca`.

The database directory must be writable. Creating the first database password
applies migrations and imports the bundled price history. The binary looks for
`assets/pricing/btc_usd.bin` beside itself; set `ADDRESS_EXPLORER_PRICING_FILE`
to use a different path. Keep the empty `public/` directory next to the binary:
the Dioxus server reads it at startup even when the PWA is served separately.

### Container

```sh
docker run --rm \
  -p 8080:8080 \
  -v address-explorer-data:/data \
  -e ADDRESS_EXPLORER_TLS_HOSTS=addresses.example.com \
  -e ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
  ghcr.io/emidev98/address-explorer:latest
```

The image healthcheck uses `curl -fk https://127.0.0.1:8080/api/health`
because the generated certificate is not in the container trust store. The CA
is stored in `/data/tls/ca.crt`; copy it out with
`docker cp <container>:/data/tls/ca.crt .` before installing it on clients.

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
| `PORT` | HTTPS port, default `8080` |
| `DATABASE_PATH` | Writable SQLite file path |
| `ADDRESS_EXPLORER_TLS_DIR` | Directory for the local CA, leaf, and key; default `<database directory>/tls` |
| `ADDRESS_EXPLORER_TLS_HOSTS` | Comma-separated DNS names/IPs for the certificate; required with a wildcard bind |
| `ADDRESS_EXPLORER_TLS_CERT` / `_KEY` | Optional operator-provided certificate and key |
| `ADDRESS_EXPLORER_TLS_CA` | Optional CA chain for operator-provided certificates; native clients can read the same variable as a PEM file to trust a local CA |
| `ADDRESS_EXPLORER_HSTS` | Set to `1` only when the certificate is trusted by clients |
| `ADDRESS_EXPLORER_CORS_ORIGINS` | Comma-separated browser origins; no wildcard |
| `ADDRESS_EXPLORER_PRICING_FILE` | Optional path to `btc_usd.bin` |

## API contract, CORS, and health

`GET /api/health` returns the application version, API version, TLS
fingerprint, and whether the server requires database encryption. It is public
and exempt from the API version guard.

Every other remote `/api/*` request **MUST** carry
`x-address-explorer-api-version` equal to the server's `API_VERSION` (currently
`3`). A missing or mismatched header receives HTTP 426 with a structured error
body. Settings “Test connection” checks `/api/health` before a remote URL can be
saved; a mismatch blocks the backend and shows both versions.

`POST /api/encryption/unlock` verifies the vault password and returns a random
session token. Every data or encryption-control request must send
`Authorization: Bearer <token>`. A new unlock replaces the previous token.
Health, encryption status, setup, unlock, and the CA download are public.

`ADDRESS_EXPLORER_CORS_ORIGINS` is a comma-separated allowlist of absolute
origins. There is no permissive `*` default; when unset or empty, the server
sends no CORS headers and only same-origin requests are accepted. The CORS
layer allows `GET`, `POST`, `PUT`, `DELETE`, `OPTIONS`, the `content-type` and
`x-address-explorer-api-version` headers, and answers `OPTIONS` preflight
before the version guard.

## Security

The pure server is HTTPS-only for non-loopback binds. TLS protects every
request, including the vault password and session token. The server generates a
local CA so it can run without a public domain; install that CA on each client,
or configure a trusted certificate. Plain HTTP is accepted only on loopback.

Every Address Explorer build requires a database password before serving any
data. On the pure server, the first client creates it on the setup screen; do
this before exposing the server to an untrusted network. Unlock verifies the
password even when the vault is already open and returns one active session
token. Every data request must send that token. A new unlock replaces the
token; locking or the inactivity timeout clears the vault key and the token.

The server database and the browser database are always encrypted at rest. The
password is never stored, and there is no recovery path. Remote reset while
locked is refused; run `address-explorer reset-vault` on the host and type
`ERASE`. While unlocked, reset keeps encryption and rotates the session.

Remote backups are encrypted with a separate backup passphrase and are
authenticated by SQLCipher. Import validates the canonical schema and rebuilds
application data into a fresh database; triggers, views, extra tables, and
altered schema objects are rejected.

`ADDRESS_EXPLORER_CORS_ORIGINS` remains a browser CORS allowlist, not access
control. Authorization is enforced by the session token.

The PWA and server send a Content Security Policy, `X-Content-Type-Options`,
`X-Frame-Options`, `Referrer-Policy`, `Permissions-Policy`, and API
`Cache-Control: no-store`. The CSP still needs `'unsafe-eval'` because Dioxus
`document::eval` and ECharts use `new Function`; this is a documented residual
XSS risk, not a reason to omit the other headers.

## Backups and upgrades

Back up before upgrading. For an unencrypted database, use the SQLite backup
API or:

```sh
sqlite3 /data/app.db ".backup '/data/backups/app-$(date +%Y%m%d%H%M%S).db'"
```

When encryption is enabled, `app.db` is a SQLCipher database and the plain
`sqlite3` command cannot read it. Use **Settings → General → Data → Export**
for an encrypted backup with a separate passphrase, or copy the encrypted file
together with the knowledge of its password. Settings can also import or reset
the active database. Imports are sanitized before they replace live data;
backups created by a newer schema are rejected.

See [`CONTRIBUTING.md`](../CONTRIBUTING.md) for the full validation and rollback
rules, and [`server/README.md`](server/README.md) for the tarball quickstart.

## Host choice

Use a host that can run a container or a native binary and attach persistent
storage. Free tiers with ephemeral filesystems and scale-to-zero behavior are
not durable for SQLite. A small VPS, a home server, or a persistent free-tier
host is the realistic option. Supabase cannot run the Rust server binary or
provide the persistent SQLite volume; porting the data layer to Postgres would
be a separate application change.

## Security policy

**Settings → Security** stores these limits in the active
database:

- password minimum length (12–64 characters);
- normal API body limit and maximum imported backup size;
- session token length, concurrent Argon2 operations, failed-unlock threshold,
  unlock backoff, and general API rate limits;
- external request timeout and response cap;
- exported-file permission mode (0600 or stricter 0400).

The **Lock after inactivity** control lives in the same screen. It bounds the
database key and the remote session; a network server applies at most one hour.
The compiled defaults are active before the vault is unlocked. Unlocking loads
the database values and replaces them. Backups cannot change the running policy:
imports preserve the current values. Changing a limit never alters an existing
password; the new minimum applies to the next setup or password change.
