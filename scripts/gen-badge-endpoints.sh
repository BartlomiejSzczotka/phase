#!/usr/bin/env bash
# Generate shields.io endpoint JSON files from coverage-stats.json
# Usage: ./scripts/gen-badge-endpoints.sh [path-to-stats-json] [output-dir]
set -euo pipefail

STATS_FILE="${1:-data/coverage-stats.json}"
OUT_DIR="${2:-data/badges}"

if [ ! -f "$STATS_FILE" ]; then
  echo "Error: $STATS_FILE not found" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

badge_color() {
  local pct=$1
  if [ "$pct" -ge 90 ]; then echo "brightgreen"
  elif [ "$pct" -ge 80 ]; then echo "green"
  elif [ "$pct" -ge 70 ]; then echo "yellowgreen"
  else echo "yellow"
  fi
}

# Overall coverage
coverage_pct=$(jq -r '.coverage_pct' "$STATS_FILE")
coverage_int=${coverage_pct%.*}
supported=$(jq -r '.supported_cards' "$STATS_FILE")
total=$(jq -r '.total_cards' "$STATS_FILE")
keywords=$(jq -r '.keyword_count' "$STATS_FILE")
overall_color=$(badge_color "$coverage_int")

jq -n --arg msg "${coverage_int}%" --arg color "$overall_color" \
  '{schemaVersion: 1, label: "card coverage", message: $msg, color: $color}' \
  > "$OUT_DIR/coverage.json"

jq -n --arg msg "${keywords}/${keywords}" \
  '{schemaVersion: 1, label: "keywords", message: $msg, color: "brightgreen"}' \
  > "$OUT_DIR/keywords.json"

jq -n --arg msg "${supported}/${total}" --arg color "$overall_color" \
  '{schemaVersion: 1, label: "cards", message: $msg, color: $color}' \
  > "$OUT_DIR/cards.json"

# Format badges
jq -r '.formats | to_entries[]
  | select(.key | test("^(pauper|standard|pioneer|modern|legacy|commander|vintage)$"))
  | "\(.key):\(.value.pct)"' "$STATS_FILE" |
while IFS=: read -r fmt pct; do
  color=$(badge_color "$pct")
  label="$(echo "${fmt:0:1}" | tr '[:lower:]' '[:upper:]')${fmt:1}"
  jq -n --arg label "$label" --arg msg "${pct}%" --arg color "$color" \
    '{schemaVersion: 1, label: $label, message: $msg, color: $color}' \
    > "$OUT_DIR/format-${fmt}.json"
done

echo "Generated badge endpoints in $OUT_DIR"
