#!/usr/bin/env bash
# Append or refresh the deployed web URL in a GitHub release body.
#
# Usage: scripts/release-announce.sh <tag> <url>
# Environment: GH_TOKEN (or an authenticated gh CLI).

set -euo pipefail

TAG="${1:?usage: release-announce.sh <tag> <url>}"
URL="${2:?usage: release-announce.sh <tag> <url>}"

START_MARKER="<!-- address-explorer-deployment:start -->"
END_MARKER="<!-- address-explorer-deployment:end -->"

if [[ ! "$URL" =~ ^https://[^[:space:]]+$ ]]; then
  echo "Refusing to write '$URL' into the release notes." >&2
  exit 2
fi

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

BODY="$WORK_DIR/body.md"
gh release view "$TAG" --json body --jq '.body // ""' > "$BODY"

CLEAN_BODY="$WORK_DIR/body-clean.md"
awk -v start="$START_MARKER" -v end="$END_MARKER" '
  $0 == start { skipping = 1; next }
  $0 == end { skipping = 0; next }
  !skipping { print }
' "$BODY" > "$CLEAN_BODY"

TRIMMED_BODY="$WORK_DIR/body-trimmed.md"
awk '
  NF { for (i = 0; i < blanks; i++) print ""; blanks = 0; print; next }
  { blanks++ }
' "$CLEAN_BODY" > "$TRIMMED_BODY"

SECTION="$WORK_DIR/deployment.md"
{
  echo "$START_MARKER"
  echo "## Web app"
  echo
  echo "Production deployment: <${URL}>"
  echo
  echo "$END_MARKER"
} > "$SECTION"

MERGED="$WORK_DIR/merged.md"
{
  if [ -s "$TRIMMED_BODY" ]; then
    cat "$TRIMMED_BODY"
    printf '\n'
  fi
  cat "$SECTION"
} > "$MERGED"

gh release edit "$TAG" --notes-file "$MERGED"
echo "Linked $TAG to $URL."
