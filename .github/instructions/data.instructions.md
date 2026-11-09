---
applyTo: "src/data/**"
---

# Data layer rules

Full specification: `CONTRIBUTING.md` §6, §6.5, §12.

- SQL lives only in `src/data/repositories/`. Services, screens, components, and
  clients never contain SQL text.
- Bind every value. Identifiers may be dynamic only from a fixed allowlist enum.
- No `SELECT *`; list explicit columns.
- One logical change = one transaction, committed explicitly. No network or
  heavy CPU work inside a transaction.
- `PRAGMA foreign_keys = ON` on every connection that depends on it.
- `src/data/migrations/` is frozen for contributions. Append a new `00NN_` file
  only when the maintainer has explicitly authorised a schema change, register it
  in `MIGRATIONS`, never reorder or renumber, and add fresh-database, upgrade, and
  data-survival tests.
- The browser database uses the same migration list. Never add a web-only schema
  or seed variant.
- Tests use an in-memory database, or a temporary file for reopen/persistence
  behaviour. Never a real database, home directory, or network.
