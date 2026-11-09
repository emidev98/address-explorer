# Agent Instructions — `crates/web-worker`

Inherits [`../../AGENTS.md`](../../AGENTS.md).

Read `CONTRIBUTING.md` §6.1, §6.2, §6.6, and §5.15 before editing anything here.

- This crate is the **only** owner of the browser `rusqlite::Connection`. The
  main thread never opens one; multiple tabs never open the VFS directly.
- Browser code MUST NOT use `std::fs`, `dirs`, `std::path::Path`, or
  `DATABASE_PATH`. The database is `address-explorer.db` behind `relaxed-idb`.
- Every write endpoint awaits the vendored commit barrier before returning
  success. Reporting a save as successful without it is forbidden.
- `rpc.rs` is a transport adapter: decode arguments, call one application
  service, encode JSON. No business rules, no SQL.
- Endpoint names in `rpc.rs` and `is_write()` must match the native controllers
  and the web wrappers. Adding or renaming one bumps `WORKER_PROTOCOL_VERSION`.
- WASM target only: `cargo check -p address-explorer-web-worker --target wasm32-unknown-unknown`.
  Set `CC`/`AR` to a wasm-capable clang when it is not on `PATH`.
