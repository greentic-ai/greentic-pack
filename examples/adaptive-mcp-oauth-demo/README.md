# Adaptive Card + MCP + OAuth Demo (Team Setup)

This example is prepared for reproducible local runs in terminal via `greentic-pack wizard` + `greentic-operator wizard`.

## What it gives

- A valid offline `.gtpack` build (no private registry fetch required).
- Interactive wizard path for creating a demo bundle.
- Demo flow with Adaptive Cards, real MCP adapter call, and OAuth context payload.

## Prerequisites

- Repos checked out under one root (default expected layout):
  - `<root>/greentic-pack`
  - `<root>/greentic-operator`
  - `<root>/component-adaptive-card`
- Built binaries:
  - `greentic-pack`
  - `greentic-operator`
- Installed tooling:
  - `wasm-tools`
- Built adaptive-card component:

```bash
cd <root>/component-adaptive-card
cargo component build --release
```

## 1) End-to-end wizard run (recommended)

```bash
cd <root>/greentic-pack/examples/adaptive-mcp-oauth-demo
./scripts/e2e_wizard_run.sh
```

What this does:

- prepares local adaptive-card + composed `mcp.exec` assets,
- builds the `.gtpack` via `greentic-pack wizard validate/apply`,
- generates normalized AnswerDocument (non-legacy format),
- runs `greentic-operator wizard --mode create` validate + execute in offline mode,
- runs `greentic-operator demo run` for flow `adaptive_mcp_oauth_demo`.

By default it writes to:

- pack: `<root>/tmp/adaptive-mcp-oauth-demo.gtpack`
- bundle: `<root>/tmp/adaptive-mcp-oauth-bundle-e2e`
- answers: `<root>/tmp/adaptive-mcp-oauth-e2e.answers*.json`

Override paths with env vars: `PACK_OUT`, `BUNDLE_OUT`, `RAW_ANSWERS`, `NORM_ANSWERS`, `PROVIDER_REGISTRY`, `GREENTIC_ROOT`.
Set `WIPE_BUNDLE_OUT=0` to keep an existing bundle (default is `1`, overwrite path by deleting old bundle dir first).

## 2) Build demo pack only (via `greentic-pack wizard apply`)

```bash
cd <root>/greentic-pack/examples/adaptive-mcp-oauth-demo
./scripts/build_local_pack.sh <root>/tmp/adaptive-mcp-oauth-demo.gtpack
```

This script:

- prepares local adaptive-card component assets,
- composes a real `mcp.exec` component (`wasm-tools compose` + router fixture),
- generates a temporary `AnswerDocument`,
- runs `greentic-pack wizard validate`,
- runs `greentic-pack wizard apply` (which builds the pack),
- copies resulting `.gtpack` to the output path.

Optional interactive pack wizard menu:

```bash
cd <root>/greentic-pack
cargo run -p greentic-pack -- wizard
```

## 3) Create bundle with wizard (interactive)

```bash
cd <root>/greentic-pack/examples/adaptive-mcp-oauth-demo
./scripts/wizard_interactive.sh
```

If your repos live in a different root, set `GREENTIC_ROOT`:

```bash
GREENTIC_ROOT=<root> ./scripts/wizard_interactive.sh
```

Recommended wizard answers:

- Bundle output path: `<root>/tmp/adaptive-mcp-oauth-interactive-bundle`
- Add application pack: `y`
- Pack reference: `file://<root>/tmp/adaptive-mcp-oauth-demo.gtpack`
- Default pack: `y`
- Add providers: `n`
- Add non-well-known provider: `n`
- Execution mode: `execute`

## 4) Smoke run

```bash
GREENTIC_ENV=dev <root>/greentic-operator/target/debug/greentic-operator demo run \
  --packs-dir <root>/tmp/adaptive-mcp-oauth-interactive-bundle/packs \
  --pack adaptive-mcp-oauth-demo.gtpack \
  --flow adaptive_mcp_oauth_demo \
  --tenant demo \
  --input '{"trigger":"start"}'
```
