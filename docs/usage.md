# Greentic Pack Usage Guide

This guide expands on the README with end-to-end instructions for building
Greentic packs and integrating them with the MCP runtime.

## Workflow overview

1. **Author a pack manifest** – create `pack.yaml` with metadata, `flow_files`,
   optional `template_dirs`, and (legacy) `imports_required` entries.
2. **Write flows** – author `.ygtc` files that orchestrate conversation
   behaviour. Flows should reference MCP tools using `mcp.exec` nodes so the
   host can negotiate tool execution at runtime.
3. **Add templates** – drop supplementary assets (markdown, prompts, UI
   fragments) under directories listed in `template_dirs`.
4. **Run `packc`** – build the pack artifacts locally. The CLI validates the
   manifest, fingerprints flows/templates, writes a CBOR manifest, and emits a
   Wasm component backed by the generated `data.rs` payload.
5. **Ship the artifacts** – publish the resulting Wasm module (`pack.wasm`) and
   manifest/SBOM outputs to the desired distribution channel.

## CLI reference

`packc` exposes a single command with structured flags:

```text
Usage: packc --in <DIR> [--out <FILE>] [--manifest <FILE>] [--sbom <FILE>]
             [--component-data <FILE>] [--dry-run] [--log <LEVEL>]
```

- `--in` – path to the pack directory containing `pack.yaml`.
- `--out` – location for the compiled Wasm component (default `dist/pack.wasm`).
- `--manifest` – CBOR manifest output (default `dist/manifest.cbor`).
- `--sbom` – CycloneDX JSON report capturing flow/template hashes (default
  `dist/sbom.cdx.json`).
- `--component-data` – override the generated `data.rs` location if you need to
  export the payload somewhere other than `crates/pack_component/src/data.rs`.
- `--dry-run` – validate inputs without writing artifacts or compiling Wasm.
- `--log` – customise the tracing filter (defaults to `info`).

`packc` writes structured progress logs to stderr. When invoking inside CI, pass
`--dry-run` to skip Wasm compilation if the target toolchain is unavailable.

## Example build

```bash
rustup target add wasm32-unknown-unknown   # run once
cargo run -p packc -- \
  --in examples/weather-demo \
  --out dist/pack.wasm \
  --manifest dist/manifest.cbor \
  --sbom dist/sbom.cdx.json
```

Outputs:

- `dist/pack.wasm` – a Wasm component exporting `greentic:pack-export` stub
  methods backed by the embedded data bundle.
- `dist/manifest.cbor` – canonical pack manifest suitable for transmission.
- `dist/sbom.cdx.json` – CycloneDX summary documenting flows/templates.
- `crates/pack_component/src/data.rs` – regenerated Rust source containing raw
  bytes for the manifest, flow sources, and templates.

## Authoring MCP-aware flows

- Use `mcp.exec` nodes to describe remote actions. Specify the MCP component and
  action identifiers so the runtime can resolve them at execution time.
- Pipe user input into node arguments through the `in` variables and reference
  pack parameters for defaults (e.g. `parameters.days_default`).
- Avoid hard-coding tool implementations in the pack. The host negotiates MCP
  sessions and provides the necessary connectors.

Example snippet from the bundled weather demo:

```yaml
forecast_weather:
  mcp.exec:
    component: "weather_api"
    action: "forecast_weather"
    args:
      q: in.q_location
      days: parameters.days_default
routing:
  - to: weather_text
```

## Component integration

The generated `pack_component` crate exposes helper functions for host runtimes:

- `manifest_cbor()` – raw CBOR manifest bytes.
- `manifest_value()` / `manifest_as<T>()` – JSON/typed views of the manifest.
- `flows()` / `templates()` – iterate embedded resources.
- `Component` – an implementation of the `greentic:pack-export` interface with
  stubbed execution hooks ready for future expansion.

Hosts are expected to load `pack.wasm`, instantiate the component, call
`list_flows`, and use MCP to execute the declared `mcp.exec` nodes.

## CI tips

- Run `cargo fmt --all` and `cargo clippy --workspace` locally before pushing.
- Add `--dry-run` to CI invocations of `packc build` if the Wasm toolchain is
  not provisioned.
- Keep example packs up to date; tests use `examples/weather-demo` as a contract
  to ensure generated artifacts capture MCP nodes correctly.

## Troubleshooting

| Issue | Resolution |
| ----- | ---------- |
| `Rust target 'wasm32-unknown-unknown' is not installed` | Run `rustup target add wasm32-unknown-unknown` once before building without `--dry-run`. |
| CLI fails with duplicate flow/template IDs | Ensure each entry in `flow_files` and `template_dirs` maps to unique logical paths. |
| Missing MCP tool at runtime | Confirm the host has loaded the proper MCP component; packs should never embed the tool implementation. |

