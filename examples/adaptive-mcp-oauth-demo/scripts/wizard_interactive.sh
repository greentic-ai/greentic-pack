#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
PACK_DIR="$(cd -- "$SCRIPT_DIR/.." && pwd)"
ROOT_DIR="${GREENTIC_ROOT:-$(cd -- "$PACK_DIR/../../.." && pwd)}"

OPERATOR_BIN="$ROOT_DIR/greentic-operator/target/debug/greentic-operator"
PROVIDER_REGISTRY="$ROOT_DIR/my/tmp/providers-empty.json"

if [[ ! -x "$OPERATOR_BIN" ]]; then
  echo "greentic-operator binary not found: $OPERATOR_BIN" >&2
  echo "Build it first:" >&2
  echo "  cd $ROOT_DIR/greentic-operator && cargo build --bin greentic-operator" >&2
  exit 1
fi

mkdir -p "$(dirname "$PROVIDER_REGISTRY")"
if [[ ! -f "$PROVIDER_REGISTRY" ]]; then
  echo '{"providers":[]}' > "$PROVIDER_REGISTRY"
fi

exec "$OPERATOR_BIN" wizard --mode create --provider-registry "$PROVIDER_REGISTRY" --offline
