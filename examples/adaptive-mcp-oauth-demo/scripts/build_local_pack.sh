#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd -- "$PACK_DIR/../../.." && pwd)"
OUT="${1:-$ROOT_DIR/my/tmp/adaptive-mcp-oauth-demo.gtpack}"

"$SCRIPT_DIR/prepare_local_assets.sh"

cd "$ROOT_DIR/greentic-pack"
cargo run -p greentic-pack -- build --in "$PACK_DIR" --gtpack-out "$OUT" --offline

echo "Built pack: $OUT"
