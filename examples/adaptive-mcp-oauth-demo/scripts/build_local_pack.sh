#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd -- "$PACK_DIR/../../.." && pwd)"
OUT="${1:-$ROOT_DIR/tmp/adaptive-mcp-oauth-demo.gtpack}"
ANSWERS_TMP="${TMPDIR:-/tmp}/adaptive-mcp-oauth-pack-wizard.answers.$$.$RANDOM.json"

"$SCRIPT_DIR/prepare_local_assets.sh"

cd "$ROOT_DIR/greentic-pack"
cat > "$ANSWERS_TMP" <<JSON
{
  "wizard_id": "greentic-pack.wizard.run",
  "schema_id": "greentic-pack.wizard.answers",
  "schema_version": "1.0.0",
  "locale": "en",
  "answers": {
    "pack_dir": "$PACK_DIR",
    "create_pack_scaffold": false,
    "run_delegate_flow": false,
    "run_delegate_component": false,
    "run_doctor": false,
    "run_build": true,
    "sign": false,
    "dry_run": false
  }
}
JSON

PACK_BIN="$ROOT_DIR/greentic-pack/target/debug/greentic-pack"
if [[ -x "$PACK_BIN" ]]; then
  "$PACK_BIN" wizard validate --answers "$ANSWERS_TMP"
  "$PACK_BIN" wizard apply --answers "$ANSWERS_TMP"
else
  cargo run -p greentic-pack -- wizard validate --answers "$ANSWERS_TMP"
  cargo run -p greentic-pack -- wizard apply --answers "$ANSWERS_TMP"
fi

BUILT_PACK="$(find "$PACK_DIR/dist" -maxdepth 1 -type f -name '*.gtpack' | head -n 1)"
if [[ -z "$BUILT_PACK" ]]; then
  echo "wizard apply completed but no gtpack found in $PACK_DIR/dist" >&2
  rm -f "$ANSWERS_TMP"
  exit 1
fi

mkdir -p "$(dirname "$OUT")"
cp -f "$BUILT_PACK" "$OUT"
rm -f "$ANSWERS_TMP"

echo "Built pack: $OUT"
