#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="${GREENTIC_ROOT:-$(cd -- "$PACK_DIR/../../.." && pwd)}"

PACK_OUT="${PACK_OUT:-$ROOT_DIR/tmp/adaptive-mcp-oauth-demo.gtpack}"
BUNDLE_OUT="${BUNDLE_OUT:-$ROOT_DIR/tmp/adaptive-mcp-oauth-bundle-e2e}"
RAW_ANSWERS="${RAW_ANSWERS:-$ROOT_DIR/tmp/adaptive-mcp-oauth-e2e.answers.json}"
NORM_ANSWERS="${NORM_ANSWERS:-$ROOT_DIR/tmp/adaptive-mcp-oauth-e2e.answers.normalized.json}"
PROVIDER_REGISTRY="${PROVIDER_REGISTRY:-$ROOT_DIR/tmp/providers-empty.json}"
WIPE_BUNDLE_OUT="${WIPE_BUNDLE_OUT:-1}"

OPERATOR_BIN="$ROOT_DIR/greentic-operator/target/debug/greentic-operator"

if [[ ! -x "$OPERATOR_BIN" ]]; then
  echo "greentic-operator binary not found: $OPERATOR_BIN" >&2
  echo "Build it first:" >&2
  echo "  cd $ROOT_DIR/greentic-operator && cargo build --bin greentic-operator" >&2
  exit 1
fi

mkdir -p "$(dirname "$PACK_OUT")"
mkdir -p "$(dirname "$RAW_ANSWERS")"
mkdir -p "$(dirname "$PROVIDER_REGISTRY")"
if [[ ! -f "$PROVIDER_REGISTRY" ]]; then
  echo '{"providers":[]}' > "$PROVIDER_REGISTRY"
fi

if [[ "$WIPE_BUNDLE_OUT" == "1" && -d "$BUNDLE_OUT" ]]; then
  rm -rf "$BUNDLE_OUT"
fi

"$SCRIPT_DIR/build_local_pack.sh" "$PACK_OUT"

cat > "$RAW_ANSWERS" <<JSON
{
  "wizard_id": "greentic-operator.wizard.demo",
  "schema_id": "greentic-operator.demo.wizard",
  "schema_version": "1.0.0",
  "locale": "en",
  "answers": {
    "bundle": "$BUNDLE_OUT",
    "pack_refs": ["file://$PACK_OUT"],
    "targets": ["demo:default"],
    "allow_paths": ["greentic.adaptive-mcp-oauth.demo/adaptive_mcp_oauth_demo"],
    "execution_mode": "execute"
  },
  "locks": {}
}
JSON

"$OPERATOR_BIN" wizard \
  --mode create \
  --answers "$RAW_ANSWERS" \
  --migrate \
  --validate \
  --emit-answers "$NORM_ANSWERS" \
  --provider-registry "file://$PROVIDER_REGISTRY" \
  --offline

"$OPERATOR_BIN" wizard \
  --mode create \
  --answers "$NORM_ANSWERS" \
  --execute \
  --provider-registry "file://$PROVIDER_REGISTRY" \
  --offline

"$OPERATOR_BIN" demo run \
  --bundle "$BUNDLE_OUT" \
  --packs-dir "$BUNDLE_OUT/packs" \
  --pack "$(basename "$PACK_OUT")" \
  --flow adaptive_mcp_oauth_demo \
  --tenant demo \
  --input '{"trigger":"start"}'
