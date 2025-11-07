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
cargo run -p packc -- build \
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

### Telemetry configuration

`packc` initialises Greentic's telemetry stack automatically. Configure the
following environment variables as needed:

- `OTEL_EXPORTER_OTLP_ENDPOINT` (defaults to `http://localhost:4317`)
- `RUST_LOG` (standard filtering for tracing; `PACKC_LOG` still overrides when set)
- `OTEL_RESOURCE_ATTRIBUTES` (recommend `deployment.environment=dev` for local work)

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
- `docs/publishing.md` – notes on publishing the crates to crates.io.
- `docs/pack-format.md` – on-disk `.gtpack` layout, hashing rules, and
  verification semantics.

## Releases & Publishing

Version numbers come from each crate’s `Cargo.toml`. When changes land on
`master`, the automation tags any crate whose manifest version changed with
`<crate-name>-v<semver>` (for single-crate repos this matches the repo name).
The publish workflow then runs `cargo fmt`, `cargo clippy`, `cargo build`, and
`cargo test --workspace --all-features` before invoking
`katyo/publish-crates@v2`. Publishing is idempotent—reruns succeed even when
all crates are already uploaded—while still requiring `CARGO_REGISTRY_TOKEN`
for new releases.

## Signing & Verification

Greentic packs can now embed developer signatures directly inside their
`pack.toml` manifest. Signatures allow downstream tooling (including the
runner) to verify that the pack contents have not been tampered with.

### Generating keys

Ed25519 keys are managed using industry standard PKCS#8 PEM files. You can
generate a developer keypair with OpenSSL:

```bash
openssl genpkey -algorithm ed25519 -out sk.pem
openssl pkey -in sk.pem -pubout -out pk.pem
```

The private key (`sk.pem`) is used for signing, while the public key (`pk.pem`)
is distributed to verifiers.

### Signing manifests

Use `packc sign` to produce a signature and embed it into the pack manifest:

```bash
packc sign \
  --pack examples/weather-demo \
  --key ./sk.pem
```

By default the manifest is updated in place. Provide `--out` to write the
signed manifest to a separate location and `--kid` to override the derived key
identifier. The command prints the key id, digest, and timestamp, and it can
emit JSON with the `--json` flag.

After signing, the manifest contains a new block:

```toml
[greentic.signature]
alg = "ed25519"
key_id = "1f2c3d4e5f6a7b8c9d0e1f2c3d4e5f6a"
created_at = "2025-01-01T12:34:56Z"
digest = "sha256:c0ffee..."
sig = "l4dbase64urlsig..."
```

The digest covers a canonical view of the pack directory that excludes build
artifacts, VCS metadata, `.packignore` entries, and the signature block itself.

### Verifying manifests

Verification is available from both the CLI and the library API:

```bash
packc verify --pack examples/weather-demo --pub ./pk.pem
```

The `--allow-unsigned` flag lets verification succeed when no signature is
present (returning a synthetic signature with `alg = "none"`). Library users
can call `packc::verify_pack_dir` with `VerifyOptions` to replicate the same
behaviour. The runner will use this API with `allow_unsigned = false` by
default, providing an `--allow-unsigned` escape hatch for development flows.

## Licensing

`greentic-pack` is licensed under the terms of the MIT license. See
[LICENSE](LICENSE) for details.
