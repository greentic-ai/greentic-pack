#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd -- "$PACK_DIR/../../.." && pwd)"

DEFAULT_WASM_WASIP2="$ROOT_DIR/component-adaptive-card/target/wasm32-wasip2/release/component_adaptive_card.wasm"
DEFAULT_WASM_WASIP1="$ROOT_DIR/component-adaptive-card/target/wasm32-wasip1/release/component_adaptive_card.wasm"
SRC_WASM="${ADAPTIVE_CARD_WASM:-$DEFAULT_WASM_WASIP2}"
SRC_MANIFEST="${ADAPTIVE_CARD_MANIFEST:-$ROOT_DIR/component-adaptive-card/component.manifest.json}"
ADAPTIVE_CARD_SOURCE="${ADAPTIVE_CARD_SOURCE:-oci://ghcr.io/greenticai/components/component-adaptive-card:stable}"
FETCH_ROOT="${ADAPTIVE_CARD_FETCH_DIR:-$PACK_DIR/.cache/component-adaptive-card}"
FETCH_CACHE_DIR="${ADAPTIVE_CARD_CACHE_DIR:-$PACK_DIR/.cache/component-store}"
FETCHED_WASM="$FETCH_ROOT/component_adaptive_card.wasm"
FETCHED_MANIFEST="$FETCH_ROOT/component.manifest.json"
MCP_COMPOSED="${MCP_COMPOSED_WASM:-$PACK_DIR/components/mcp.exec/component.wasm}"
MCP_ROUTER="${MCP_ROUTER_WASM:-$ROOT_DIR/greentic-pack/crates/packc/tests/fixtures/router-echo-component.wasm}"
MCP_ADAPTER="${MCP_ADAPTER_WASM:-$ROOT_DIR/greentic-pack/crates/packc/assets/mcp_adapter_25_06_18.component.wasm}"
MCP_MANIFEST_OUT="$PACK_DIR/components/mcp.exec/component.manifest.json"

if [[ ! -f "$SRC_WASM" && -f "$DEFAULT_WASM_WASIP1" ]]; then
  SRC_WASM="$DEFAULT_WASM_WASIP1"
fi

if [[ ! -f "$SRC_WASM" || ! -f "$SRC_MANIFEST" ]]; then
  if ! command -v greentic-component >/dev/null 2>&1; then
    if [[ ! -f "$SRC_WASM" ]]; then
      echo "Adaptive card wasm not found: $SRC_WASM" >&2
    fi
    if [[ ! -f "$SRC_MANIFEST" ]]; then
      echo "Adaptive card manifest not found: $SRC_MANIFEST" >&2
    fi
    echo "greentic-component is required to fetch the published adaptive card artifact." >&2
    echo "Either install greentic-component or build it locally:" >&2
    echo "  cd $ROOT_DIR/component-adaptive-card && cargo component build --release" >&2
    exit 1
  fi

  mkdir -p "$FETCH_ROOT" "$FETCH_CACHE_DIR"
  greentic-component store fetch \
    --out "$FETCHED_WASM" \
    --cache-dir "$FETCH_CACHE_DIR" \
    "$ADAPTIVE_CARD_SOURCE"
  SRC_WASM="$FETCHED_WASM"
  SRC_MANIFEST="$FETCHED_MANIFEST"
fi

if [[ ! -f "$SRC_MANIFEST" ]]; then
  echo "Adaptive card manifest not found: $SRC_MANIFEST" >&2
  exit 1
fi

cp -f "$SRC_WASM" "$PACK_DIR/components/component_adaptive_card.wasm"
cp -f "$SRC_MANIFEST" "$PACK_DIR/components/component.manifest.json"

if [[ ! -f "$MCP_ROUTER" ]]; then
  echo "MCP router fixture not found: $MCP_ROUTER" >&2
  exit 1
fi

if [[ ! -f "$MCP_ADAPTER" ]]; then
  echo "MCP adapter not found: $MCP_ADAPTER" >&2
  exit 1
fi

if ! command -v wac >/dev/null 2>&1; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "wac is required for MCP composition, and cargo was not found to install it." >&2
    exit 1
  fi
  echo "wac not found; installing with cargo install wac-cli" >&2
  cargo install wac-cli
fi

if ! command -v wac >/dev/null 2>&1; then
  echo "wac is required for MCP composition" >&2
  exit 1
fi

mkdir -p "$(dirname "$MCP_COMPOSED")"
wac plug "$MCP_ADAPTER" --plug "$MCP_ROUTER" --output "$MCP_COMPOSED" >/dev/null

cat > "$MCP_MANIFEST_OUT" <<'JSON'
{
  "id": "mcp.exec",
  "version": "0.1.0",
  "world": "root:component/root",
  "supports": ["messaging"],
  "profiles": {
    "default": "default",
    "supported": ["default"]
  },
  "capabilities": {
    "wasi": {
      "random": false,
      "clocks": false
    },
    "host": {}
  },
  "artifacts": {
    "component_wasm": "component.wasm"
  }
}
JSON

echo "Prepared local assets in: $PACK_DIR/components"
