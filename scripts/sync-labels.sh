#!/usr/bin/env bash
#
# sync-labels.sh — create or update the repository labels from .github/labels.yml.
#
# Requires the `gh` CLI, authenticated with write access to the repository.
# Safe to re-run: existing labels are updated in place, descriptions and colors
# included.

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

source_file=".github/labels.yml"

if [[ ! -f "$source_file" ]]; then
  echo "sync-labels: $source_file not found" >&2
  exit 2
fi

if ! command -v gh >/dev/null 2>&1; then
  echo "sync-labels: the gh CLI is required" >&2
  exit 2
fi

if ! gh auth status >/dev/null 2>&1; then
  echo "sync-labels: run 'gh auth login' first" >&2
  exit 2
fi

created=0
updated=0

while IFS=$'\t' read -r name color description; do
  [[ -z "${name:-}" ]] && continue
  if gh label create "$name" --color "$color" --description "$description" --force >/dev/null 2>&1; then
    updated=$((updated + 1))
  else
    echo "sync-labels: could not apply '$name'" >&2
  fi
done < <(
  awk '
    /^- name:/ {
      name = $3
      color = ""
      description = ""
    }
    /^  color:/ {
      color = $2
      gsub(/"/, "", color)
    }
    /^  description:/ {
      description = $0
      sub(/^  description:[[:space:]]*/, "", description)
      if (name != "") print name "\t" color "\t" description
    }
  ' "$source_file"
)

echo "sync-labels: applied labels from $source_file"
