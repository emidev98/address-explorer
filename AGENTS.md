# Agent Instructions

This file is the operating manual for any coding agent — Copilot, Claude, Codex,
Jules, Cursor, OpenCode, or a human using one.

[`CONTRIBUTING.md`](CONTRIBUTING.md) is the binding specification and the single
source of truth. This file does not replace it; it tells you **which sections
apply to the files you are about to touch**, so you do not have to hold 1,455
lines in context to stay compliant.

## 0. Prime directive

`CONTRIBUTING.md` is **frozen**. So are the files that enforce it. You **MUST
NOT** modify any path listed in [`.github/frozen-paths.txt`](.github/frozen-paths.txt),
including `CONTRIBUTING.md`, `AGENTS.md`, `.github/`, `Cargo.toml`, `Cargo.lock`,
`build.rs`, `src/data/migrations/`, and `vendor/`.

CI enforces this. If you believe a frozen file must change, **open an issue**
with the label `needs-spec` and explain why. Do not open a pull request for it.

## 1. Set up once

```sh
cargo prepare --yes --non-interactive   # Rust, dx, Node, wasm clang, system libs
cargo prepare --check                   # verify without changing anything
```

Do not invent setup steps. If `cargo prepare` fails, report the failure instead
of working around it.

## 2. Verify before opening a pull request

```sh
./scripts/verify-agent.sh          # machine-checkable CONTRIBUTING subset (~30 s)
./scripts/verify-agent.sh --full   # adds dx check, tests, clippy, mutation tests
```

A pull request **MUST** paste the `verify-agent` result table. The individual
gates are the ones in `CONTRIBUTING.md` §4:

```sh
cargo fmt --all --check
dx check --platform web
dx check --platform desktop
cargo test -p address-explorer --no-default-features --features server
cargo test -p address-explorer --no-default-features --features desktop
cargo clippy -p address-explorer --all-targets --no-default-features --features server -- -D warnings
cargo clippy -p address-explorer --target wasm32-unknown-unknown --no-default-features --features web -- -D warnings
dx build --web --release --locked
```

## 3. Scope budget

One pull request = one issue = one area.

| Limit | Value |
| --- | --- |
| Changed lines (excluding `Cargo.lock`, `assets/tailwind.css`, snapshots) | **≤ 400** |
| Files changed | **≤ 8** |
| Workspace crates touched | **≤ 1** |
| New dependencies | **0** |
| New public APIs | 0 unless the issue asks for them |

Over 800 lines the pull request is closed rather than reviewed. Split at a real
seam — a separate module, a separate test file, a separate screen — not at an
arbitrary line number.

If you cannot describe the change in one sentence without the word "and", it is
too large.

## 4. Routing table — read the section that governs your files

| You are touching | Read first | Hard rules that bite |
| --- | --- | --- |
| `src/data/**` | §6, §6.5, §12 | SQL only in `src/data/repositories/`. Bound parameters only. Never edit a shipped migration; append `00NN_`. |
| `src/data/migrations/**` | §6.5.1–§6.5.3 | **Frozen.** Open an issue instead. |
| `src/data/backup.rs` | §6.6, §6.7 | Await the commit barrier before reporting success. |
| `crates/web-worker/**`, `src/web/**` | §6.1, §6.2, §6.6 | No `std::fs`, `dirs`, `Path`, or `DATABASE_PATH`. One worker owns the connection. |
| `src/components/**`, `src/screens/**` | §5.2–§5.5, §5.14 | Use the shared primitives. No SQL, no blocking I/O, no direct ECharts. |
| `src/components/charts/**` | §5.10 | One component per chart. Theme tokens only, never hardcoded colors. |
| `src/components/format.rs`, fiat displays | §3.1 | BTC is the unit of account. `format_fiat` + `use_fiat_currency()`. |
| Table screens | §5.11 | `TableQuery`/`TablePage`, allowlisted sort enum, shared pagination. |
| Date filters | §5.13 | Shared `DatePicker` only. |
| `src/domain/**`, `src/application/**` | §7, §8 | Pure logic, no RSX, no SQL, no HTTP. Dependencies point inward. |
| `src/clients/**` | §7.2, §7.4 | One provider per module; map wire types to domain types at the boundary. |
| `src/api/**`, `crates/web-worker/src/rpc.rs` | §5.15 | Endpoint parity across native, worker RPC, and web wrapper. Bump `API_VERSION`/`WORKER_PROTOCOL_VERSION` on shape changes. |
| Tests anywhere | §9 | Deterministic, hermetic, no real database or network. |
| Any Rust | §8 | Clean code, error handling, comment discipline. |

## 5. The ten rules agents break most

1. **Format first, last, always.** `cargo fmt --all`, then `dx fmt` on the files
   you changed. Formatting is part of the edit, not cleanup.
2. **Never run `dx fmt --all-code`.** The pinned Dioxus CLI corrupts `rsx!`
   event handlers with it (CONTRIBUTING §4.1). CI rejects it.
3. **Never edit a shipped migration.** Append the next `00NN_` and register it.
4. **No `unwrap()` / `expect()` in production code.** Use `Result`/`Option`.
5. **No interpolated SQL values.** Bind them. Identifiers come from allowlists.
6. **No hardcoded fiat.** Read the default currency; do not assume USD.
7. **No new dependencies.** Not even a small one.
8. **Comments explain why, not what.** Never reference CONTRIBUTING section
   numbers in code comments. Do not restate a function signature in a doc comment.
9. **Do not restructure.** No renames, reordering, import churn, or formatting
   sweeps outside the logical change.
10. **Stop when blocked.** If the issue is ambiguous, add a comment asking for a
    decision instead of guessing.

## 6. Pull request shape

Use the repository template exactly. The description is validated by CI.

- `## What` — one sentence, ≤ 140 characters, verb-first.
- `## Why` — at most three bullets, with `Refs #N` or `Fixes #N`.
- `## Change` — at most five bullets, no code excerpts.
- `## Verification` — the commands you ran and their results.
- `## Investigation` — files read, pattern reused, alternatives rejected,
  prior art, paths deliberately untouched.
- `## Scope` — area, line/file count, out of scope.
- `## Compliance` — frozen files untouched, no new dependencies, attestations.

Write for a reviewer who has 60 minutes. Do not narrate the diff.

## 7. When you are unsure

- Missing acceptance criteria → label the issue `needs-spec`, ask, stop.
- The fix needs a frozen file → open an issue, do not guess.
- Two existing patterns conflict → follow the one in the file you are editing and
  say so in `## Investigation`.
- The change is expanding → stop and split it.

Humans and agents follow the same rules. The only difference is that an agent
gets one attempt per issue, so a narrow, boring, well-verified pull request is
always the better choice.
