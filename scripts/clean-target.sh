#!/usr/bin/env bash
# Reclaim disk from target/ without nuking the active incremental cache.
#
# Runs cargo-sweep to prune stale fingerprints (keyed on cargo's own build
# metadata, not dir mtime), then removes non-cargo-managed subtrees that
# accumulate but aren't pruned by sweep.
#
# Usage: scripts/clean-target.sh [--dry-run]
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="$ROOT/target"
DRY_RUN=0
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=1

if ! command -v cargo-sweep >/dev/null 2>&1; then
  echo "installing cargo-sweep..."
  cargo install cargo-sweep
fi

before=$(du -sh "$TARGET" 2>/dev/null | cut -f1 || echo "?")
echo "target/ before: $before"

sweep_args=(--time 3 --recursive "$TARGET")
if [[ $DRY_RUN -eq 1 ]]; then
  cargo sweep --dry-run "${sweep_args[@]}"
  echo "(dry run — skipping rm of target/tool and target/wasm-dev)"
else
  cargo sweep "${sweep_args[@]}"
  rm -rf "$TARGET/tool" \
         "$TARGET/wasm32-unknown-unknown/wasm-dev" \
         "$TARGET/codex-copy" \
         "$TARGET/x86_64-unknown-linux-gnu"
fi

after=$(du -sh "$TARGET" 2>/dev/null | cut -f1 || echo "?")
echo "target/ after:  $after"
