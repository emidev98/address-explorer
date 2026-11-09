# Agent Instructions — `src/data`

Inherits [`../../AGENTS.md`](../../AGENTS.md). This file covers only what differs.

Read `CONTRIBUTING.md` §6, §6.5, §6.7, and §12 before editing anything here.

## Hard rules

- Repositories in `src/data/repositories/` are the **only** place SQL appears.
  Services, screens, components, and clients MUST NOT contain SQL text.
- Values are bound (`?1`, `:name`, `params!`, `named_params!`). Never build a
  value into SQL with `format!`. Identifiers may be dynamic only when chosen
  from a fixed allowlist enum.
- `SELECT *` is not used. Select explicit columns.
- Every multi-step logical change runs in one transaction, committed explicitly.
  No network or long CPU work while a transaction is open.
- `PRAGMA foreign_keys = ON` is required on every connection that relies on it.

## Migrations

- `src/data/migrations/` is **frozen** for agents. Do not create, edit, or delete
  a migration file in a contribution pull request.
- If a schema change is genuinely required, open an issue with `needs-spec`.
- If you are deliberately given permission by the maintainer, follow §6.5.3
  exactly: append `00NN_<name>.sql`, append to `MIGRATIONS`, never reorder,
  never renumber, never edit a shipped file, and add fresh-database +
  upgrade + data-survival tests.

## Browser parity

- The browser database uses the same `MIGRATIONS` list and
  `CURRENT_SCHEMA_VERSION`. A web-only schema or seed variant is forbidden.
- On WASM, `0015_historical_prices_data` reads `public/btc_usd.bin`; it must not
  use `std::fs` or `include_bytes!` for the pack.
- A write endpoint reports success only after `wait_committed(filename)`.

## Tests

Use an in-memory database for isolated repository tests and a temporary on-disk
file for reopening, persistence, or migration behavior. Never touch a real
database, the developer home directory, or the network.
