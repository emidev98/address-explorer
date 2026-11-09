# Agent contribution governance

This document describes how a contribution — from a human or an agent — moves
from an idea to `main`. It is the implementation of the rules in
[`CONTRIBUTING.md`](../../CONTRIBUTING.md) and the operating manual in
[`AGENTS.md`](../../AGENTS.md).

Anyone can contribute. Nothing here is a permission system. It is a set of
gates that make "this follows the specification" a checkable statement rather
than a promise.

## 1. Why the specification is frozen

`CONTRIBUTING.md` is 1,455 lines of normative rules. Asking an agent to load it
all before editing does not produce compliance. Long-context performance degrades
well before the window is full: published measurements show long-context agent
runs retaining most of the *coverage* of a requirement set while collapsing on
strict success, and retrieval accuracy peaks at the start and end of a context
and drops in the middle.

So the specification is treated the way a constitution is treated: binding,
stable, and read selectively. Compliance is enforced by executable gates, not by
asking the model to remember paragraph 6.5.3.

### Frozen paths

[`.github/frozen-paths.txt`](../../.github/frozen-paths.txt) lists the paths a
contribution must not modify. It includes the specification, this document, the
operating manual, the entire enforcement layer, the dependency graph, the schema
history, and the deployment configuration.

The reason the enforcement layer is frozen is simple: an agent that can edit its
own guardrails has no guardrails.

If a change genuinely requires a frozen path, open an issue with `needs-spec`. A
maintainer either performs the change or labels a pull request with
`maintainer-override`.

## 2. Three classes of rule, three mechanisms

| Class | Example | Mechanism | Blocks merge |
| --- | --- | --- | --- |
| Machine-checkable | no interpolated SQL, no `SELECT *`, no shipped-migration edit, no new dependency | `scripts/verify-agent.sh`, run locally and in CI | Yes |
| Attestable | charts verified in light and dark, offline PWA reload, CORS mismatch handling | `## Compliance` checkboxes plus the maintainer's spot check | No, but a false attestation is treated as a broken contribution |
| Judgment | single responsibility, layer boundaries, naming, comment quality | Human review against `CONTRIBUTING.md` §7 and §8 | Yes |

A rule that cannot be classified this way is not a rule; it is advice. Advice
belongs in `AGENTS.md`, not in a gate.

### The current machine checks

`scripts/verify-agent.sh` reports one row per rule:

| Check | Rule | Scope |
| --- | --- | --- |
| formatting | §4 | absolute |
| frozen paths | §0 | absolute |
| migration immutability | §6.5 | absolute |
| no `unwrap()`/`expect()` | §8.2 | ratchet on added lines |
| no interpolated SQL | §6.3 | ratchet on added lines |
| no `SELECT *` | §6.3 | absolute |
| no native fs in browser code | §3, §6.1 | absolute |
| `format_usd` confined to `format.rs` | §3.1 | absolute |
| `dx fmt --all-code` never used | §4.1 | commit messages and PR body |
| no dependency change | §3 | absolute |
| comment discipline | §8.3 | ratchet on added lines |
| API surface version | §5.15 | advisory |
| native/web route parity | §9 | advisory |
| full lane | §4, §9 | `--full` only |

"Ratchet" means the rule is evaluated only on lines the contribution adds, so an
existing exception does not block unrelated work and a new one always does.

## 3. Scope budget

One pull request is one issue, one area, one logical change.

| Limit | Value | Enforced by |
| --- | --- | --- |
| Reviewable changed lines | ≤ 400 target, > 800 rejected | `pr-guard.yml` |
| Files changed | ≤ 8 | `pr-guard.yml` |
| Workspace crates touched | ≤ 1 | `pr-guard.yml` |
| New dependencies | 0 | `verify-agent.sh` |
| Description length | ≤ 2,500 characters | `pr-guard.yml` |
| `## What` | one sentence, ≤ 140 characters | `pr-guard.yml` |

The 400-line figure is the long-standing review-effectiveness ceiling: defect
detection falls sharply past a few hundred lines in a single review session. The
800-line hard stop exists because a pull request that large is not reviewed, it is
skimmed, and a skimmed merge is worse than a rejection.

Generated files (`Cargo.lock`, `assets/tailwind.css`, snapshots, the vendored
ECharts bundle, generated worker output) are excluded from the line count, because
they inflate a diff without adding review burden.

When a pull request exceeds the hard stop, close it and ask for a narrower
attempt. Iterating on a 2,000-line diff costs more maintainer time than a restart.

## 4. The pull request contract

The description is validated, not just requested. The template in
[`.github/pull_request_template.md`](../../.github/pull_request_template.md)
defines seven sections:

| Section | Purpose | Constraint |
| --- | --- | --- |
| `## What` | the change | one sentence, ≤ 140 characters |
| `## Why` | the reason | ≤ 3 bullets, with an issue reference |
| `## Change` | what moved | ≤ 5 bullets, no code blocks |
| `## Verification` | evidence it works | commands and results |
| `## Investigation` | evidence the work was researched | five fixed fields |
| `## Scope` | area, size, non-goals | declared size must match the diff |
| `## Compliance` | attestations | three checked boxes |

`## Investigation` is the part that distinguishes a contribution from a
speculation:

- **Files read** — where the existing behaviour lives.
- **Pattern reused** — the file and line the change follows.
- **Alternatives rejected** — at least one, with the reason.
- **Prior art** — related issues, pull requests, or commits.
- **Deliberately untouched** — what a reviewer should not expect to see changed.

This matters because the two most common causes of an unmerged agent pull request
are a failing test and *the issue already having been fixed by another pull
request*. `scripts/guard-pr.mjs` searches merged pull requests for the referenced
issue and reports candidates, so duplicate work is caught before review.

## 5. Trust tiers

Trust is earned per area, measured from merged history, and never granted to a
frozen path.

| Tier | Threshold | Effect |
| --- | --- | --- |
| T0 probation | default | full CI, maintainer approval, every path gated |
| T1 trusted | ≥ 10 merged pull requests in the area, ≥ 80% merge rate, no reverts in 30 days, ≥ 70% first-pass CI success | eligible for the merge queue after required checks, still one human approval |
| T2 co-owner | ≥ 25 merged, ≥ 90% merge rate, no reverts in 60 days, ≥ 3 follow-ups closed by the same contributor | area-scoped bypass of the review requirement, `CODEOWNERS` entry for the human sponsor |

Two constraints are structural:

- **A bot account cannot be a CODEOWNER.** GitHub resolves code ownership to
  accounts that can approve, and a bot approval does not reliably satisfy a
  required code-owner review. `CODEOWNERS` therefore lists the human sponsor, and
  an agent's graduation is implemented as a ruleset bypass scoped to an area path.
- **No tier ever covers a frozen path.** `.github/`, `Cargo.toml`, `Cargo.lock`,
  `build.rs`, `src/data/migrations/`, and `vendor/` stay with the maintainer at
  every tier.

Tier changes are recorded in the contribution ledger, described in §6.

## 6. The contribution ledger

`CONTRIBUTORS.md` records what has been contributed and by whom. Agents are listed
alongside humans, with the evidence needed to assess a tier change.

Attribution depends on one convention: every agent-assisted pull request carries a
trailer in its description, and the squash commit message is configured to *Pull
request title and description* so the trailer survives the merge. Without that,
the ledger silently undercounts.

```
Agent: claude-code
Agent-Model: claude-opus-4.7
Agent-Task: #123
Human-Reviewer: @emidev98
```

## 7. What is a gate and what is a courtesy

| Control | Strength |
| --- | --- |
| `CODEOWNERS` plus "require review from Code Owners" | server-side, cannot be bypassed by a contributor |
| `pr-guard.yml` → **Frozen files** | server-side, runs trusted tooling from the base branch |
| `pr-guard.yml` → **Shape and description** | server-side |
| `verify-agent.sh` in CI | server-side |
| `verify-agent.sh` locally | advisory; an agent can skip it, CI cannot |
| `.claude/hooks/protect-files.sh` | convenience only; `PreToolUse` hooks are bypassable through shell commands |
| `scripts/install-hooks.sh` (git `pre-commit`) | convenience only; `--no-verify` exists |

The last two exist to prevent accidents. They are not a security boundary and are
documented as such so nobody relies on them.

## 8. Changing a gate

1. Open an issue with `needs-spec`. Include the rule from `CONTRIBUTING.md` the
   gate would enforce, and a failing case the current gates miss.
2. The maintainer edits the frozen files, because that is what frozen means.
3. A new check is added to `scripts/verify-agent.sh` with a `CONTRIBUTING.md`
   section reference in its label, and a line in the table above.
4. The check must pass on `main` before it is merged. A gate that is red on a
   clean tree gets disabled by the next person who needs to ship, which makes it
   worse than no gate.

## 9. References

- [`CONTRIBUTING.md`](../../CONTRIBUTING.md) — the specification.
- [`AGENTS.md`](../../AGENTS.md) — the operating manual and routing table.
- [`.github/frozen-paths.txt`](../../.github/frozen-paths.txt) — the frozen list.
- [`scripts/verify-agent.sh`](../../scripts/verify-agent.sh) — the machine checks.
- [`scripts/guard-pr.mjs`](../../scripts/guard-pr.mjs) — the pull request gate.
- [`.github/CODEOWNERS`](../../.github/CODEOWNERS) — ownership.
