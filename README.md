# Greentic Pack

`greentic-pack` provides two core capabilities for building Greentic packages:

- `packc`: a developer-facing CLI that validates pack manifests, normalises
  metadata, generates simple SBOM reports, and emits the `data.rs` payload that
  powers the Greentic pack component.
- `pack_component`: a reusable Wasm component crate that exposes the
  `greentic:pack-export` interface using the artefacts produced by `packc`.

## Repository layout

```
greentic-pack/
├── Cargo.toml                # Cargo workspace manifest
├── crates/
│   ├── packc/                # Builder CLI
│   └── pack_component/       # Wasm component library
├── docs/                     # Additional guides
├── examples/                 # Sample packs
└── .github/workflows/        # CI automation
```

### packc

The CLI expects a pack directory that contains `pack.yaml` alongside its flow
files and templates. Example:

```bash
cargo run -p packc -- \
  --in examples/weather-demo \
  --out dist/pack.wasm \
  --manifest dist/manifest.cbor \
  --sbom dist/sbom.cdx.json
```

Running the command performs validation, emits the CBOR manifest, generates a
CycloneDX SBOM, regenerates `crates/pack_component/src/data.rs`, and compiles
`pack_component` to the requested Wasm artifact. Use `--dry-run` to skip writes
while still validating the pack inputs.

> ℹ️ The build step expects the `wasm32-unknown-unknown` Rust target. Install it
> once with `rustup target add wasm32-unknown-unknown`.

Greentic packs only transport flows and templates. Execution-time tools are
resolved by the host through the MCP runtime, so flows should target
`mcp.exec` nodes rather than embedding tool adapters. The `tools` field remains
in `PackSpec` for compatibility but new packs should rely on MCP.

### pack_component

`pack_component` is a thin wrapper around the generated `data.rs`. It exposes
helpers for inspecting the embedded manifest and flow assets. The component
depends on the shared bindings from `greentic-interfaces`; no WIT files are
vendored in this repository. Re-run `packc build` whenever the manifest or flow
assets change to ensure `data.rs` stays in sync.

## Examples

- `examples/weather-demo` – a toy conversational pack demonstrating the expected
  directory structure. Use this sample to smoke test `packc` or bootstrap new
  packs.

## Further documentation

- `docs/usage.md` – CLI flags, build workflow, and guidance for designing MCP
  aware flows.

## Licensing

`greentic-pack` is licensed under the terms of the MIT license. See
[LICENSE](LICENSE) for details.
