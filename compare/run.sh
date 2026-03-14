#!/bin/bash
# Run full layout comparison for a fixture.
# Usage: ./compare/run.sh fixtures/foo.html
#
# Steps:
#   1. Extract Chrome layout (JSON + screenshot)
#   2. Render pipeline (PNG + JSON dump)
#   3. Element-level comparison
#   4. Pixel-level diff

set -e

if [ -z "$1" ]; then
  echo "Usage: ./compare/run.sh <fixture.html>"
  exit 1
fi

HTML="$1"
BASE="${HTML%.html}"
DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Chrome extraction ==="
node "$DIR/extract-chrome.js" "$HTML"

echo ""
echo "=== Pipeline render + dump ==="
brokkr run -- --fixture "$HTML"
brokkr run -- --fixture --dump "$HTML"

echo ""
echo "=== Element comparison ==="
node "$DIR/compare.js" "${BASE}_chrome.json" "${BASE}_pipeline.json" || true

echo ""
echo "=== Pixel diff ==="
node "$DIR/pixeldiff.js" "${BASE}_chrome.png" "${BASE}_pipeline.png"
