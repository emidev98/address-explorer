---
applyTo: "crates/web-worker/**,src/web/**,crates/core/**"
---

# Browser and worker rules

Full specification: `CONTRIBUTING.md` §5.15, §5.18, §6.1, §6.2, §6.6.

- The SharedWorker is the only owner of the browser `rusqlite::Connection`.
  The main thread never opens one; multiple tabs never open the VFS directly.
- Browser code MUST NOT use `std::fs`, `dirs`, `std::path::Path`, or
  `DATABASE_PATH`. The database is the fixed virtual filename
  `address-explorer.db` behind `relaxed-idb`.
- Each tab owns one `MessagePort`; requests are typed, correlated by id, and
  serialised by the worker. No connection or statement crosses the port.
- After every write endpoint, await the vendored `wait_committed(filename)`
  commit barrier before reporting success. Never report a save as successful
  without it.
- The worker reuses the same migrations, seeds, repositories, services, and
  clients as native. No web-only business rules, SQL, or UI state.
- `rpc.rs` is a transport adapter only: decode, call one service, encode.
- `/api/*` is never intercepted or cached by the service worker.
- Endpoint parity is mandatory across native controllers, worker RPC, and web
  wrappers. Renaming or reshaping one bumps `WORKER_PROTOCOL_VERSION`.
- Build with `--target wasm32-unknown-unknown`; set `CC`/`AR` to a wasm-capable
  clang when it is not on `PATH`.
