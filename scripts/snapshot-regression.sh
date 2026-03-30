#!/usr/bin/env bash
set -euo pipefail

# Snapshot regression: compare card-data.json before/after parser changes.
# Usage:
#   1. Before making changes: cp client/public/card-data.json /tmp/card-data-before.json
#   2. After making changes: ./scripts/snapshot-regression.sh
#   3. Or with custom path: ./scripts/snapshot-regression.sh /path/to/before.json

BEFORE="${1:-/tmp/card-data-before.json}"
AFTER="client/public/card-data.json"

if [[ ! -f "$BEFORE" ]]; then
  echo "No snapshot found at $BEFORE"
  echo "Create one first: cp client/public/card-data.json $BEFORE"
  exit 1
fi

if [[ ! -f "$AFTER" ]]; then
  echo "No current card-data.json found at $AFTER"
  echo "Generate it first: ./scripts/gen-card-data.sh"
  exit 1
fi

DIFF=$(diff <(jq -S . "$BEFORE") <(jq -S . "$AFTER") | head -100 || true)

if [[ -z "$DIFF" ]]; then
  echo "PASS: No regressions detected"
  exit 0
else
  echo "FAIL: Regressions detected:"
  echo "$DIFF"
  exit 1
fi
