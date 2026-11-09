#!/usr/bin/env bash
#
# protect-files.sh — PreToolUse hook that refuses edits to frozen paths.
#
# This is convenience, not a boundary. PreToolUse hooks can be bypassed by
# writing through the Bash tool, so the authoritative gates are
# .github/workflows/pr-guard.yml (server-side) and scripts/verify-agent.sh
# (local). This hook just stops the obvious accident before it happens.
#
# Exit 2 blocks the tool call and returns stderr to the agent.

set -uo pipefail

input=$(cat)

file=""
if command -v jq >/dev/null 2>&1; then
  file=$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input" 2>/dev/null)
fi
if [[ -z "$file" ]]; then
  file=$(printf '%s' "$input" \
    | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | head -1)
fi

[[ -z "$file" ]] && exit 0

repo="${CLAUDE_PROJECT_DIR:-}"
if [[ -z "$repo" ]]; then
  repo=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
fi

rel="${file#"$repo"/}"
list="$repo/.github/frozen-paths.txt"
[[ -f "$list" ]] || exit 0

while IFS= read -r raw; do
  pattern="${raw%%#*}"
  pattern="$(printf '%s' "$pattern" | tr -d '[:space:]')"
  [[ -z "$pattern" ]] && continue
  # shellcheck disable=SC2254 # intentional glob matching
  case "$rel" in
    $pattern)
      echo "Blocked: $rel is a frozen path ($pattern)." >&2
      echo "CONTRIBUTING.md and the files that enforce it are maintainer-owned." >&2
      echo "Open an issue with the 'needs-spec' label instead of editing it." >&2
      exit 2
      ;;
  esac
done <"$list"

exit 0
