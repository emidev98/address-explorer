# Agent Instructions — `tools/cargo-prepare`

Inherits [`../../AGENTS.md`](../../AGENTS.md). This path is **frozen** for
contributions: it configures the environment every other gate depends on.

Read `CONTRIBUTING.md` §4 and `docs/BUILDING.md` before touching it.

If you believe the installer is wrong:

- Do not patch it in a contribution pull request.
- Open an issue describing the failing `cargo prepare` output, the platform, and
  the pinned versions involved.

If the maintainer explicitly asks for a change here:

- `cargo prepare` must stay idempotent and safe to re-run.
- `--yes --non-interactive` must never prompt.
- `--check` must remain read-only: it verifies and reports, never installs.
- `--github-env` / `--github-env-web-tools` output is consumed by
  `.github/actions/prepare`; keep the contract stable.
- Adding a tool means adding it to `--check` in the same change.
