# Address Explorer remote server

This archive contains the optional pure-server binary. It is not required by
the desktop or browser-local application.

## Run

```sh
IP=0.0.0.0 \
PORT=8080 \
DATABASE_PATH=/var/lib/address-explorer/app.db \
ADDRESS_EXPLORER_CORS_ORIGINS=https://your-web-origin \
ADDRESS_EXPLORER_PRICING_FILE=./assets/pricing/btc_usd.bin \
./address-explorer
```

The binary also finds `assets/pricing/btc_usd.bin` next to the executable when
`ADDRESS_EXPLORER_PRICING_FILE` is not set. The database directory must be
writable. The first start runs the SQLite migrations and imports the bundled
pricing history.

Keep the empty `public/` directory next to the binary. The Dioxus server reads it
at startup even when the PWA is served separately.

## Security

The server has no authentication. `ADDRESS_EXPLORER_CORS_ORIGINS` is a browser
CORS allowlist, not access control. Do not expose the server to untrusted
networks or origins. See `docs/deployment.md` in the repository for the
container image and backup notes.
