# Adaptive Card + MCP + OAuth Demo (Team Setup)

This example is prepared for reproducible local runs in terminal via `greentic-pack wizard` + `greentic-operator wizard`.

## What it gives

- A valid offline `.gtpack` build (no private registry fetch required).
- Interactive wizard path for creating a demo bundle.
- Demo flow with Adaptive Cards and MCP/OAuth context in payload.

## Prerequisites

- Repos checked out under one root (default expected layout):
  - `<root>/greentic-pack`
  - `<root>/greentic-operator`
  - `<root>/component-adaptive-card`
- Built binaries:
  - `greentic-pack`
  - `greentic-operator`
- Built adaptive-card component:

```bash
cd <root>/component-adaptive-card
cargo component build --release
```

## 1) Build demo pack (via `greentic-pack wizard apply`)

```bash
cd <root>/greentic-pack/examples/adaptive-mcp-oauth-demo
./scripts/build_local_pack.sh <root>/tmp/adaptive-mcp-oauth-demo.gtpack
```

This script:

- prepares local adaptive-card component assets,
- generates a temporary `AnswerDocument`,
- runs `greentic-pack wizard validate`,
- runs `greentic-pack wizard apply` (which builds the pack),
- copies resulting `.gtpack` to the output path.

Optional interactive pack wizard menu:

```bash
cd <root>/greentic-pack
cargo run -p greentic-pack -- wizard
```

## 2) Create bundle with wizard (interactive)

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

## 3) Smoke run

```bash
GREENTIC_ENV=dev <root>/greentic-operator/target/debug/greentic-operator demo run \
  --packs-dir <root>/tmp/adaptive-mcp-oauth-interactive-bundle/packs \
  --pack adaptive-mcp-oauth-demo.gtpack \
  --flow adaptive_mcp_oauth_demo \
  --tenant demo \
  --input '{"trigger":"start"}'
```
