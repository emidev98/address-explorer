# Address Explorer remote server

This archive contains the optional pure-server binary. It is not required by
the desktop or browser-local application.

## Run

```sh
IP=0.0.0.0 \
PORT=8080 \
DATABASE_PATH=/var/lib/address-explorer/app.db \
ADDRESS_EXPLORER_TLS_HOSTS=addresses.example.com,192.168.1.10 \
ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
ADDRESS_EXPLORER_PRICING_FILE=./assets/pricing/btc_usd.bin \
./address-explorer
```

The binary also finds `assets/pricing/btc_usd.bin` next to the executable when
`ADDRESS_EXPLORER_PRICING_FILE` is not set. The database directory must be
writable. Creating the first password runs the SQLite migrations and imports the
bundled pricing history into the new encrypted database.

Keep the empty `public/` directory next to the binary. The Dioxus server reads it
at startup even when the PWA is served separately.

## Security

The server is HTTPS-only for non-loopback binds. It generates a local CA and
server certificate in the database directory unless you provide
`ADDRESS_EXPLORER_TLS_CERT` and `ADDRESS_EXPLORER_TLS_KEY`. Install the CA on
clients that connect directly.

Every server requires a database password before it serves any data. The first
client creates it on the setup screen; do this before exposing the server to an
untrusted network. Unlock returns one active session token; every data request
must send it as a bearer token. A new unlock replaces the previous token.
Locking or the inactivity timeout (at most one hour on a network server) clears
the vault key and the token. While locked, remote reset is refused; run
`address-explorer reset-vault` on the host.

Remote backups are encrypted with a separate passphrase. See
`docs/deployment.md` in the repository for the full environment, CORS, backup,
and recovery notes.
