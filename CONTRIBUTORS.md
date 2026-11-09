# Contributors

Address Explorer accepts contributions from humans and from coding agents on the
same terms. This file records what has been contributed, so trust is evidence-based
rather than assumed.

The rules behind it are in [`docs/agents/GOVERNANCE.md`](docs/agents/GOVERNANCE.md).

## Humans

| Contributor | Role | Areas |
| --- | --- | --- |
| [@emidev98](https://github.com/emidev98) | Maintainer, code owner, approver | all |

## Agents

Agent contributions are attributed from an `Agent:` trailer in the pull request
description. The squash commit message is configured to *Pull request title and
description* so the trailer survives the merge. Without that, this table silently
undercounts.

```
Agent: claude-code
Agent-Model: claude-opus-4.7
Agent-Task: #123
Human-Reviewer: @emidev98
```

The table below is generated. Run `node scripts/update-contributors.mjs` and commit
the result, or trigger the **Contributors ledger** workflow.

<!-- BEGIN GENERATED: contributors -->

_No merged agent contributions recorded yet._

<!-- END GENERATED: contributors -->

## How to read this

- **Merged** counts pull requests merged to `main`, not attempts. A closed pull
  request is not a contribution.
- **Areas** are the `area:*` labels the pull request touched. Trust is granted per
  area, never globally.
- **Last merged** shows whether the contributor is active. Stale counts do not
  accumulate credit forever.

## Trust tiers

Tiers are described in `docs/agents/GOVERNANCE.md` §5 and are assigned by the
maintainer from this evidence. No tier ever covers a frozen path.

The threshold that grants a tier is deliberately public, so an agent can reason
about it: ten merged pull requests in an area with a clean record makes a
contributor eligible for T1; twenty-five with clean follow-ups makes them eligible
for T2, which is expressed as an area-scoped review bypass and a `CODEOWNERS`
entry for the human sponsor.
