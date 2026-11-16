#!/usr/bin/env bash
# Build the PWA and deploy a Vercel Build Output tree without letting Vercel
# compile the repository.
#
# Usage: scripts/deploy-vercel.sh [--prod]
# Environment: VERCEL_TOKEN (or an authenticated `vercel` CLI), VERCEL_ORG_ID,
# VERCEL_PROJECT_ID.

set -euo pipefail

MODE="${1:-}"
case "$MODE" in
  "" | --prod) ;;
  *)
    echo "usage: scripts/deploy-vercel.sh [--prod]" >&2
    exit 2
    ;;
esac

if ! command -v dx >/dev/null 2>&1; then
  echo "dx was not found; run \`cargo prepare\` first." >&2
  exit 1
fi

PUBLIC_DIR="target/dx/address-explorer/release/web/public"

dx build --web --release --locked
bash scripts/build-vercel-output.sh "$PUBLIC_DIR" .vercel/output

if [ "$MODE" = "--prod" ]; then
  npx --yes vercel@latest deploy --prebuilt --yes --prod
else
  npx --yes vercel@latest deploy --prebuilt --yes
fi
