#!/usr/bin/env bash
# Append or refresh the generated changelog section of a GitHub release.
#
# Usage: scripts/release-changelog.sh <tag>
# Environment: GH_TOKEN (or an authenticated gh CLI), GITHUB_REPOSITORY.

set -euo pipefail

TAG="${1:?usage: release-changelog.sh <tag>}"
REPOSITORY="${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"

START_MARKER="<!-- address-explorer-changelog:start -->"
END_MARKER="<!-- address-explorer-changelog:end -->"

WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

PREVIOUS_TAG=""
if PREVIOUS_TAG="$(git describe --tags --abbrev=0 --exclude="$TAG" "$TAG^" 2>/dev/null)"; then
  RANGE="$PREVIOUS_TAG..$TAG"
else
  PREVIOUS_TAG=""
  RANGE="$TAG"
fi

declare -a features=()
declare -a fixes=()
declare -a improvements=()
declare -a maintenance=()
declare -a other=()

while IFS=$'\t' read -r subject hash; do
  [ -n "$subject" ] || continue

  type="${subject%%:*}"
  if [ "$type" = "$subject" ]; then
    group="other"
  else
    type="${type%%(*}"
    case "$type" in
      feat) group="features" ;;
      fix) group="fixes" ;;
      perf|refactor) group="improvements" ;;
      chore|docs|test|ci|build|style) group="maintenance" ;;
      *) group="other" ;;
    esac
  fi

  line="- ${subject} (${hash})"
  case "$group" in
    features) features+=("$line") ;;
    fixes) fixes+=("$line") ;;
    improvements) improvements+=("$line") ;;
    maintenance) maintenance+=("$line") ;;
    *) other+=("$line") ;;
  esac
done < <(git --no-pager log --no-merges --pretty=format:'%s%x09%h%n' "$RANGE")

CHANGELOG="$WORK_DIR/changelog.md"
{
  echo "$START_MARKER"
  echo "## Changelog"
  echo
  if [ -n "$PREVIOUS_TAG" ]; then
    echo "**Full Changelog**: https://github.com/${REPOSITORY}/compare/${PREVIOUS_TAG}...${TAG}"
  else
    echo "Initial release."
  fi
  echo

  emit_section() {
    local title="$1"
    shift
    local -a lines=("$@")
    if [ "${#lines[@]}" -eq 0 ]; then
      return
    fi
    echo "### ${title}"
    echo
    printf '%s\n' "${lines[@]}"
    echo
  }

  emit_section "Features" "${features[@]}"
  emit_section "Fixes" "${fixes[@]}"
  emit_section "Improvements" "${improvements[@]}"
  emit_section "Maintenance" "${maintenance[@]}"
  emit_section "Other Changes" "${other[@]}"

  echo "$END_MARKER"
} > "$CHANGELOG"

BODY="$WORK_DIR/body.md"
gh release view "$TAG" --json body --jq -r '.body // ""' > "$BODY"

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

MERGED="$WORK_DIR/merged.md"
{
  if [ -s "$TRIMMED_BODY" ]; then
    cat "$TRIMMED_BODY"
    printf '\n\n'
  fi
  cat "$CHANGELOG"
} > "$MERGED"

gh release edit "$TAG" --notes-file "$MERGED"
echo "Updated release notes for $TAG."
