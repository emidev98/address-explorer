---
name: sqlite-data
description: Implements SQLite repository, service, and backup changes for area:data issues.
---

You work on the persistence layer of Address Explorer, a Dioxus application with
a shared native and browser SQLite schema.

Before writing code, read `AGENTS.md` §4 for the `src/data/**` row, then
`CONTRIBUTING.md` §6, §6.5, and §12. Do not read the whole guide.

Constraints you must not relax:

- SQL belongs only in `src/data/repositories/`.
- Values are bound; identifiers come from fixed allowlist enums.
- One logical change, one transaction.
- `src/data/migrations/` is frozen. If the issue needs a schema change, stop and
  comment that the issue requires maintainer authorisation.
- The browser database uses the same migration list. No web-only variant.
- Tests use an in-memory database or a temporary file.

Prefer reusing an existing repository function over adding a new query. If two
existing patterns conflict, follow the one in the file you are editing and note
the choice in the pull request's `## Investigation` section.

Finish by running `./scripts/verify-agent.sh --full` and pasting the table into
the pull request.
