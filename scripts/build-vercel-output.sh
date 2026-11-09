#!/usr/bin/env bash
# Turn the deployable Dioxus web directory into a Vercel Build Output API tree.
#
# Usage: scripts/build-vercel-output.sh <public-dir> <output-dir>

set -euo pipefail

PUBLIC_DIR="${1:?usage: build-vercel-output.sh <public-dir> <output-dir>}"
OUTPUT_DIR="${2:?usage: build-vercel-output.sh <public-dir> <output-dir>}"

if [ ! -f "$PUBLIC_DIR/index.html" ]; then
  echo "missing $PUBLIC_DIR/index.html; build the web PWA first" >&2
  exit 1
fi

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/static"
cp -a "$PUBLIC_DIR/." "$OUTPUT_DIR/static/"

cat > "$OUTPUT_DIR/config.json" <<'JSON'
{
  "version": 3,
  "routes": [
    {
      "src": "/index.html",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/sw.js",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/sw-register.js",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/local-worker.js",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/worker/(.*)",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/btc_usd.bin",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/manifest.webmanifest",
      "headers": { "Cache-Control": "public, max-age=0, must-revalidate" },
      "continue": true
    },
    {
      "src": "/assets/(.*)",
      "headers": { "Cache-Control": "public, max-age=31536000, immutable" },
      "continue": true
    },
    { "src": "/api/(.*)", "status": 404 },
    { "handle": "filesystem" },
    { "src": "/(.*)", "dest": "/index.html" }
  ]
}
JSON

echo "Created Vercel output at $OUTPUT_DIR"
