#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="$(cd -- "$PACK_DIR/../../.." && pwd)"

DEFAULT_WASM_WASIP2="$ROOT_DIR/component-adaptive-card/target/wasm32-wasip2/release/component_adaptive_card.wasm"
DEFAULT_WASM_WASIP1="$ROOT_DIR/component-adaptive-card/target/wasm32-wasip1/release/component_adaptive_card.wasm"
SRC_WASM="${ADAPTIVE_CARD_WASM:-$DEFAULT_WASM_WASIP2}"
SRC_MANIFEST="${ADAPTIVE_CARD_MANIFEST:-$ROOT_DIR/component-adaptive-card/component.manifest.json}"

if [[ ! -f "$SRC_WASM" && -f "$DEFAULT_WASM_WASIP1" ]]; then
  SRC_WASM="$DEFAULT_WASM_WASIP1"
fi

if [[ ! -f "$SRC_WASM" ]]; then
  echo "Adaptive card wasm not found: $SRC_WASM" >&2
  echo "Build it first:" >&2
  echo "  cd $ROOT_DIR/component-adaptive-card && cargo component build --release" >&2
  exit 1
fi

if [[ ! -f "$SRC_MANIFEST" ]]; then
  echo "Adaptive card manifest not found: $SRC_MANIFEST" >&2
  exit 1
fi

cp -f "$SRC_WASM" "$PACK_DIR/components/component_adaptive_card.wasm"
cp -f "$SRC_MANIFEST" "$PACK_DIR/components/component.manifest.json"

echo "Prepared local assets in: $PACK_DIR/components"
