# Agent Instructions — `crates/core`

Inherits [`../../AGENTS.md`](../../AGENTS.md).

This crate owns the SharedWorker envelope and the protocol version, nothing else.

- `WORKER_PROTOCOL_VERSION` is bumped whenever an RPC request/response variant or
  payload shape changes. Do not bump it for a version bump elsewhere.
- No dependencies beyond what the protocol needs. No `rusqlite`, no HTTP, no
  Dioxus UI code.
- Changing this crate changes the browser/native contract. Bump the version, and
  update `crates/web-worker` and the web wrappers in the same pull request.
