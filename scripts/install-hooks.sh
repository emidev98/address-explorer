#!/usr/bin/env bash
#
# install-hooks.sh — install a local git pre-commit hook that runs the fast
# CONTRIBUTING checks.
#
# This is convenience, not a boundary: `git commit --no-verify` bypasses it and
# CI runs the same checks regardless. Install it once per clone.

set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

hooks_dir="$(git rev-parse --git-path hooks)"
mkdir -p "$hooks_dir"

hook="$hooks_dir/pre-commit"

cat >"$hook" <<'EOF'
#!/usr/bin/env sh
# Installed by scripts/install-hooks.sh. Bypass with `git commit --no-verify`.
root="$(git rev-parse --show-toplevel)"
exec "$root/scripts/verify-agent.sh"
EOF

chmod +x "$hook"

echo "install-hooks: wrote $hook"
echo "install-hooks: the fast lane runs on every commit; CI runs the full set."
