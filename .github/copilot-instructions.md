# Copilot Instructions

Read [`AGENTS.md`](../AGENTS.md) first. It is the operating manual for this
repository and applies to Copilot coding agent, Copilot Chat, and Copilot code
review unchanged.

Summary of the binding rules:

- `CONTRIBUTING.md` is the specification. Read only the sections named in the
  routing table in `AGENTS.md` §4 that match the files you touch.
- `CONTRIBUTING.md`, `AGENTS.md`, and every path in
  [`.github/frozen-paths.txt`](frozen-paths.txt) are frozen. Do not modify them.
  Open an issue with `needs-spec` instead.
- Run `./scripts/verify-agent.sh` before opening a pull request and paste the
  result table into the description.
- One pull request = one issue = one area, ≤ 400 changed lines, ≤ 8 files.
- Use the pull request template exactly; the description is validated by CI.
