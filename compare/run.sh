#!/bin/bash
# Run layout comparison for a test email.
# Usage: ./compare/run.sh test-emails/foo.html
#
# Steps:
#   1. Extract Chrome layout (JSON + screenshot)
#   2. Render pipeline (PNG + JSON dump)
#   3. Compare the two JSON files

set -e

if [ -z "$1" ]; then
  echo "Usage: ./compare/run.sh <html-file>"
  exit 1
fi

HTML="$1"
BASE="${HTML%.html}"
DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== Chrome extraction ==="
node "$DIR/extract-chrome.js" "$HTML"

echo ""
echo "=== Pipeline render + dump ==="
brokkr run -- "$HTML"
brokkr run -- --dump "$HTML"

echo ""
echo "=== Comparison ==="
node "$DIR/compare.js" "${BASE}_chrome.json" "${BASE}_pipeline.json"
