# Server deployment

Address Explorer's web PWA is Local-first and does not require a remote server.
The optional pure-server build provides the REST API used by **Settings →
General → Remote**.

The release workflow publishes:

- `address-explorer-server_<version>_linux_x86_64.tar.gz`
- `address-explorer-server_<version>_linux_arm64.tar.gz`
- `ghcr.io/emidev98/address-explorer:<version>` and `:latest`

It does **not** deploy a live backend.

## Why Supabase is not used

Supabase provides Postgres, Auth, Storage, and Edge Functions. The Address
Explorer server is a long-running Rust/Axum process using SQLite. Supabase
cannot run that binary or provide the persistent SQLite volume it needs.
Porting the data layer to Postgres would be a separate application change.

Use a host that can run a container and attach persistent storage. On free
tiers, be aware that Render and Koyeb free instances have ephemeral filesystems
and scale to zero; a SQLite database on them is not durable. Oracle Cloud Always
Free or an existing VPS/home server is the realistic persistent option.

## Run the tarball

```sh
tar -xzf address-explorer-server_<version>_linux_x86_64.tar.gz
cd address-explorer-server_<version>_linux_x86_64

IP=0.0.0.0 \
PORT=8080 \
DATABASE_PATH=/var/lib/address-explorer/app.db \
ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
./address-explorer
```

The binary finds `assets/pricing/btc_usd.bin` next to itself. The database
directory must be writable. The first start applies migrations and imports the
bundled price history.

## Run the container

```sh
docker run --rm \
  -p 8080:8080 \
  -v address-explorer-data:/data \
  -e ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
  ghcr.io/emidev98/address-explorer:latest
```

The image listens on `0.0.0.0:8080`, stores SQLite at `/data/app.db`, and
exposes `/api/health`.

## Environment

| Variable | Purpose |
| --- | --- |
| `IP` | Bind address; use `0.0.0.0` in a container |
| `PORT` | HTTP port, default `8080` |
| `DATABASE_PATH` | Writable SQLite file path |
| `ADDRESS_EXPLORER_CORS_ORIGINS` | Comma-separated browser origins; no wildcard |
| `ADDRESS_EXPLORER_PRICING_FILE` | Optional path to `btc_usd.bin` |

## Security

The server has **no authentication**. `ADDRESS_EXPLORER_CORS_ORIGINS` is a
browser CORS allowlist, not access control: direct HTTP clients can still read
and modify the data. Do not expose it to untrusted networks. Keep the URL
private or add authentication before making it public.

## Backups

The database and exported files are unencrypted. Before upgrading, copy the
SQLite file with the SQLite backup API or:

```sh
sqlite3 /data/app.db ".backup '/data/backups/app-$(date +%Y%m%d%H%M%S).db'"
```

Settings → General → Data can also export the active database as a normal
SQLite file. Older schemas are upgraded on import; newer schemas are rejected.

## Future automatic deployment

A host-specific deployment job can consume the tarball or GHCR image. Any
host with a persistent volume can be added without changing the application.
