# WS-D Phase 2 — `ext://` component producer + `component.json` sidecar

**Status:** Ready to implement (design decisions locked 2026-06-25).
**Builds on:** [`2026-06-25-ext-component-resolver-design.md`](./2026-06-25-ext-component-resolver-design.md) (Phase 1 resolver, shipped in packc `1.2.0-research.1`).
**Spans two repos:** `greentic-component` (producer) + `greentic-pack` / packc (resolver).

## Problem

Phase 1 shipped a packc resolver for `ext://<id>#component` that reads a top-level
`{ "component": { "id", "asset", "digest" } }` block from a file named `describe.json`
inside the extension's `.gtxpack`. Investigation while starting Phase 2 found that
contract is **unsatisfiable as written**:

1. The canonical, store-validated extension manifest **is** `describe.json` (describe-v2,
   owned upstream by `greentic-extension-sdk-contract` in `greentic-designer-sdk`). Its
   JSON Schema root is **`additionalProperties: false`** — a top-level `component` block is
   **rejected at store publish**. Components there live only under
   `runtime.components.<name>.{oci_ref?, gtpack?, sha256, world}`, which has **no field for
   a bare wasm asset embedded inside the same `.gtxpack`**.
2. No producer emits the Phase-1 shape today. The component-bearing producer is
   `greentic-component store publish` (`ComponentExtension` kind), whose `.gtxpack`
   currently contains exactly `describe.json` + `component.wasm` and points runtime
   components at OCI, not an embedded asset.

So Phase 1's resolver read a **synthetic** schema that collides with the real manifest.
Because packc `.1` is a prerelease and **nothing consumes the `ext://` path yet**, this is
the zero-breakage moment to correct the contract.

## Decisions (locked)

- **Carry the metadata in a packc-owned sidecar, not in `describe.json`.** The producer
  writes a separate `component.json` file into the `.gtxpack`; the canonical describe-v2
  `describe.json` stays untouched and store-valid. Resolver reads `component.json`.
  - Rejected alternative: extend describe-v2 upstream with a `runtime.components.<name>.asset`
    field — architecturally cleaner but triggers another exact-pin release-train cascade
    (`greentic-extension-sdk-contract` crate + `greentic-store-server` schema + packc).
    Deferred; revisit during Phase 3 canonicalization if desired.
- **Producer is `greentic-component store publish` (`ComponentExtension`).** For a
  ComponentExtension the published `--wasm` **is** the runtime component; it is already
  packed as `component.wasm` at the gtxpack root. The sidecar therefore points at the
  **existing** entry — no separate asset, no new CLI flag, no `DescribeInputs` change.

## The contract — `component.json`

Written at the **root** of the `.gtxpack` ZIP, alongside `describe.json` and `component.wasm`:

```json
{
  "component": {
    "id": "greentic.component-http",
    "asset": "component.wasm",
    "digest": "sha256:<hex-of-component.wasm>"
  }
}
```

- `id` — the store id (`store_id`). Informational; the resolver does **not** enforce
  `id == extension_id` (Phase-1 code validates only `asset` non-empty + `digest` match).
- `asset` — ZIP entry name of the runtime wasm. For ComponentExtension this is the existing
  `component.wasm` at root. (Resolver reads whatever path this names — arbitrary paths like
  `assets/component-foo.wasm` remain valid for other producers.)
- `digest` — `sha256:<lowercase-hex>` of the asset bytes; already computed as
  `wasm_sha256_hex` in `store_publish::run` (prefix with `sha256:`).

## Repo A — `greentic-component` (producer)

File: `crates/greentic-component/src/cmd/store_publish.rs`.

- `build_gtxpack(describe_json, wasm)` → add a `component.json` ZIP entry. Keep it a dumb
  packer: pass the sidecar bytes in (build them in `run()`), or pass `(store_id, sha256_hex)`
  and build inside. Use a single `const COMPONENT_WASM_ENTRY: &str = "component.wasm"` for both
  the ZIP entry name and the sidecar `asset` so they cannot drift.
- In `run()`: build the sidecar from the existing `store_id` + `wasm_sha256_hex`
  (`digest = format!("sha256:{wasm_sha256_hex}")`). No new inputs, no new CLI arg.
- The sidecar is **embedded in the artifact only** — it is not sent as a separate multipart
  part and does not change the upload metadata.

**TDD (extend the existing ZIP round-trip test ~lines 337–423):**
- `gtxpack_embeds_component_json_sidecar`: build → unzip → `component.json` present;
  `component.id == store_id`, `component.asset == "component.wasm"`,
  `component.digest == "sha256:" + sha256(component.wasm bytes)`.
- digest in the sidecar equals the hash of the actual `component.wasm` entry (no drift).

## Repo B — `greentic-pack` / packc (resolver)

File: `crates/packc/src/cli/ext_resolver.rs`.

- `extract_and_verify`: read **`component.json`** instead of `describe.json`
  (the `archive.by_name("describe.json")` at line 157 → `"component.json"`).
- Update the module doc block (lines ~8, ~13–32) and every error string that names
  `describe.json` (lines ~159, ~167, ~169, ~176, ~209) to say `component.json`.
- Keep struct names (`GtxpackDescribe` / `GtxpackComponentEntry`) and the `{component:{…}}`
  shape unchanged. Optionally rename `GtxpackDescribe` → `GtxpackComponentSidecar` for clarity
  (cosmetic; only if it stays within the file).

**TDD:** `crates/packc/tests/ext_component_resolver.rs` — the `build_gtxpack` fixture helper
writes `describe.json`; change it to write `component.json`. The existing 8 tests
(happy-path, unknown-ext, no-embedded-component, digest-mismatch, malformed-ref, …) then
exercise the new filename. Add one fixture mirroring the **real** producer output
(`asset == "component.wasm"` at root) so the cross-repo contract is locked by a test.

## Release

- packc: bump `1.2.0-research.1` → `.2`, republish (`crates-publish.yml`). Resolver contract
  change only; no dependency re-pin needed.
- greentic-component: ship on `research`; normal publish cadence. No version coupling to packc.

## Out of scope (Phase 3)

- Flipping http/webhook capabilities to source via `ext://`; deriving standalone component-OCI.
- `oci://` / `store://` extension acquisition in the resolver (Phase-1 bails; still bails).
- Canonicalizing the embedded-asset representation into describe-v2 upstream.
