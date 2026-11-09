# Claude Instructions

Read [`AGENTS.md`](AGENTS.md) first. It is the operating manual and applies to
this agent unchanged.

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) only for the sections named in the
routing table in `AGENTS.md` §4, based on the files you are about to touch.

`CONTRIBUTING.md`, `AGENTS.md`, and every path in
[`.github/frozen-paths.txt`](.github/frozen-paths.txt) are frozen. A
`PreToolUse` hook blocks edits to them locally, and CI blocks the pull request
regardless.
