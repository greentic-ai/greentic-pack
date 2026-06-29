# packc `ext://<id>#component` Resolver — Design (WS-D Phase 1)

Date: 2026-06-25
Status: **UNBLOCKED — implementation-ready.** The greentic-types release-train cascade is DONE:
`greentic-types 1.2.0-research.2` (carrying the `Ext` variant) is published, and 8 consumers
(interfaces/host/wasmtime/guest, config/config-types, qa, session, state, secrets, distributor-client)
are republished at `.1` consuming it. greentic-flow + greentic-pack are the remaining repos to bump
to `.2` AND implement the resolver (see §"Flow↔packc boundary" below — the one design decision, now DECIDED).
Repo: greentic-pack (branch `research`)
Part of: Component/Extension Distribution Unification RFC
(`greentic-designer/docs/superpowers/specs/2026-06-22-component-extension-distribution-unification-proposal.md`),
sequenced as WS-D Phase 1 in
`greentic-designer/docs/superpowers/plans/2026-06-23-full-standardization-program.md`.

## Problem

A capability such as HTTP ships as two artifacts (runtime `component-http` + design
`greentic.http` extension) on two version lines, which drift. The RFC's end state is
**one published `.gtxpack` per capability that embeds its runtime component as an asset**,
with the build extracting that component instead of pulling a separate OCI artifact.

This spec covers **only WS-D Phase 1**: teach `packc` to resolve a new flow component-ref
scheme `ext://<id>#component` by extracting the component wasm embedded in the referenced
extension's `.gtxpack` and embedding it as a `Local` source — exactly like the existing
`oci://` path. Phase 2 (extensions actually embed their component) and Phase 3 (flip
http / webhook-trigger to `ext://`) are out of scope.

## Release-train coupling — corrected (deep trace, 2026-06-25)

An earlier read claimed this was packc-local and unblocked. A deep trace of the resolve
path **contradicts that** — the clean design IS coupled to the greentic-types release-train:

- The **resolved sidecar uses TYPED greentic-types enums**, not just a flow-ref string:
  `ComponentSourceRefV1` (`greentic-types/src/flow_resolve.rs`, variants `Local|Oci|Repo|Store`,
  `serde(tag="kind")`) and `FlowResolveSummarySourceRefV1`
  (`greentic-types/src/flow_resolve_summary.rs`, same shape). A clean `ext://` adds an
  `Ext { ref, digest }` variant to **both** enums → a **greentic-types change** → the SAME
  `<1.2.0-0` consumer-graph cap gate as WS-B (see memory `greentic-types-release-train-gate`).
- **packc does not acquire `.gtxpack` bytes.** Extensions are only *validated*
  (`extension_refs.rs` `validate_extensions_*`, format/digest); there is no download/fetch.
  So "reuse existing extension-acquisition" has nothing to reuse — Phase 1 must ADD at least
  a `file://` local-extension read (`build.rs:178` only checks lock alignment).
- **No `describe.json` schema exists** for an in-`.gtxpack` embedded component. It must be
  defined (closest analogue: the `ComponentDescribe` CBOR sidecar
  `greentic_types::schemas::component::v0_6_0`).

**Conclusion:** WS-D Phase 1 done cleanly is **also release-train-gated**, exactly like WS-B.
The release-train is the universal gate for the §4.x / RFC migration.

### Two implementation options (decision deferred to when the gate clears)

- **Option Ext-variant (clean, recommended once unblocked):** add `Ext { ref, digest }` to
  `ComponentSourceRefV1` + `FlowResolveSummarySourceRefV1` in greentic-types; dispatch it in
  `cli/resolve.rs::collect_from_summary`; resolve it in `PackResolver::resolve`. Requires the
  greentic-types release-train.
- **Option coercion-hack (unblocked, NOT recommended):** carry `ext://…` as a string through
  an existing summary variant and special-case `req.reference.starts_with("ext://")` in
  `PackResolver::resolve` before the `DistClient` call. Avoids the greentic-types change but
  leaves `ext://` without a clean typed home in the sidecar; rejected to avoid tech debt on a
  foundational path.

## Goals / non-goals

**Goals**
- `packc` resolves `ext://<extension-id>#component` to a `Local` embed of the component
  wasm extracted from that extension's `.gtxpack`.
- `oci://` / `repo://` / `store://` / local-path component refs keep working unchanged.
- Runner unchanged (extraction is build-time; it still loads a bare `Local` component).

**Non-goals**
- Extensions embedding their component asset (Phase 2).
- Flipping real capabilities to `ext://` (Phase 3).
- Runner change (extraction is build-time; runner still loads a bare `Local` component).
  NOTE: contrary to the original draft, the clean design DOES require a greentic-types change
  (the `Ext` sidecar variant) — see §"Release-train coupling — corrected".

## Design

### Reference shape

`ext://<extension-id>#component` — e.g. `ext://greentic.http#component`. The `<extension-id>`
matches an entry the pack already declares in `pack.extensions.json`
(`PackExtensionsFile.extensions[].id`, `extension_refs.rs`). The `#component` fragment names
the embedded runtime component (Phase 1 supports the single canonical component per
extension; a future multi-component extension could use `#component=<name>`).

### Resolution flow (where the extension `.gtxpack` comes from — decided: pack.extensions.json)

1. packc's component-source resolution recognizes the `ext://` scheme (new case alongside
   the existing oci/repo/store/local handling).
2. Look up `<extension-id>` in the pack's declared dependencies
   (`read_extensions_file` → `PackExtensionsFile`). Unknown id → error (see Errors).
3. Acquire that extension's `.gtxpack` **reusing packc's existing extension-acquisition
   path** (the same mechanism that fetches a declared extension dependency from its
   `oci://` / `store://` / local source; the exact function is pinned in the
   implementation plan). Phase 1 is exercised against a local/fixture extension source so
   no network is required for tests.
4. Extract the embedded runtime component from the `.gtxpack` (a ZIP):
   - read `describe.json`, which advertises the embedded component id + **digest**;
   - read the component wasm asset (RFC layout: `assets/component-<name>.wasm`);
   - verify the wasm bytes hash to the advertised digest.
5. Write the extracted wasm into the resolve work dir and emit
   `ComponentSourceRefV1::Local { path, digest }` — from here the existing `Local` embed
   path is identical to the OCI case (`packc` embeds → runner resolves `Local`).

### Expected extension `.gtxpack` layout (RFC §4.3)

```
greentic.http.gtxpack            (ZIP)
├── extension.wasm               # design-time authoring (ignored by this resolver)
├── assets/component-<name>.wasm # runtime component — extracted
└── describe.json                # advertises { embedded component id, asset path, digest }
```

The resolver depends only on `describe.json` (for the asset path + digest) and the named
asset. It does not parse `extension.wasm`.

## Flow↔packc boundary for `ext://` (resolve_summary layer) — DECIDED

A subtlety not in the original draft: greentic-flow's `src/resolve_summary.rs` IS on packc's
path — packc calls `greentic_flow::resolve_summary::write_flow_resolve_summary_for_flow`, which
calls `resolve_source(source) -> (FlowResolveSummarySourceRefV1, wasm_path, digest)`. flow's
`resolve_source` matches exhaustively on `ComponentSourceRefV1`, so adding `Ext` forces 3 arms
(`component_id_from_source` ~L148, `resolve_source` ~L165, `summary_source_ref` ~L190). The
non-mechanical one is `resolve_source`: it returns a wasm_path + digest, but flow has NO
extension-acquisition and must NOT extract the `.gtxpack` (that is packc's job).

**Decision (cleanest, no duplication): flow DECLARES, packc RESOLVES.**
- flow `resolve_source` for `Ext { r#ref, digest }`:
  - `summary_ref` → `FlowResolveSummarySourceRefV1::Ext { r#ref }` (declaration only).
  - `digest` → pass through the ref's optional pinned `digest` if present, else **deferred**
    (empty string sentinel `""` — meaning "computed by packc at embed"). flow does NOT open the
    extension.
  - `wasm_path` → not meaningful for `ext://`; return a sentinel/unused path (it is only consumed
    for the `Local` byte-embed, which `ext://` does not take at the flow layer). Prefer refactoring
    so the `ext://` arm doesn't need a real path — e.g. flow's resolve treats `ext://` like the
    remote arms (which also don't produce a local wasm at flow time): mirror how `Oci/Repo/Store`
    return via `resolve_remote` WITHOUT a local file, OR return the deferred tuple. Match whatever
    the remote arms already do for "not-locally-materialised" (inspect `resolve_remote`'s return).
  - `component_id_from_source` (~L148): `Ext { r#ref, .. } => r#ref` (name extracted from
    `ext://<id>#component` by the existing split logic; `<id>` is the component name source).
- packc `PackResolver::resolve` (the §Design resolver) OWNS `ext://` fully: locate the extension
  via `pack.extensions.json`, extract `assets/component-<name>.wasm`, compute the real digest,
  VERIFY against the extension `describe.json`'s advertised digest (and against the summary's
  pinned digest if non-deferred), embed as `Local`. The lock's digest for an `ext://` node is
  written HERE.

Rationale: flow stays a pure declaration layer (no new acquisition dependency); packc is the
single place that touches `.gtxpack` bytes (already true for the Local-embed path). Mirror
whatever flow's remote arms (`Oci/Repo/Store`) do for the "resolved-later, no local file" shape so
the `Ext` arm is consistent — confirm by reading `resolve_remote`'s exact return before coding.

## Error handling

All fail the build with a clear message before any embed; oci/store/local paths are
untouched:
- **Unknown extension id** — `ext://<id>#component` where `<id>` is not in
  `pack.extensions.json`: `ext:// component ref names extension '<id>' not declared in pack.extensions.json`.
- **Extension does not embed a component** — `.gtxpack` has no embedded-component entry in
  `describe.json` / the named asset is absent: `extension '<id>' does not embed a runtime component`.
- **Digest mismatch** — extracted wasm hash ≠ advertised digest:
  `embedded component digest mismatch for extension '<id>'`.
- **Malformed `ext://` ref** (missing `#component`, empty id): rejected at parse with the
  expected-form message.

## Testing (TDD)

A synthetic fixture exercises the resolver end-to-end without network or a real extension,
mirroring the existing component-fixture helpers (`crates/packc/tests/components_extension.rs`
`write_stub_wasm` / `write_describe_sidecar`):

1. **Happy path** — build a minimal extension `.gtxpack` (ZIP) containing
   `assets/component-foo.wasm` (stub wasm) + `describe.json` advertising it with a correct
   digest; a `pack.extensions.json` declaring the extension with a local/fixture source.
   Assert `ext://<id>#component` resolves to a `ComponentSourceRefV1::Local` with the
   extracted path and the advertised digest, and that the embedded bytes match.
2. **Unknown extension id** → the declared error.
3. **Extension without an embedded component** → the declared error.
4. **Digest mismatch** (tamper the asset) → the declared error.
5. **Malformed ref** (`ext://greentic.http`, no `#component`) → parse error.
6. **Regression** — existing `oci://` / `store://` / local resolve tests stay green
   (`crates/packc/tests/resolve.rs`, `flow_resolve_sidecar.rs`, `components_extension.rs`).

## Files (anticipated; pinned in the plan)

- `crates/packc/src/extension_refs.rs` or the component-source resolution module — add the
  `ext://` scheme case + the extension-`.gtxpack` component extraction.
- A small helper for ZIP extraction + digest verification of the embedded component.
- `crates/packc/tests/` — a new `ext_component_resolver.rs` test (+ fixture builder).

## Integration points (deep trace 2026-06-25 — for when the gate clears)

- **Sidecar enums (greentic-types):** `ComponentSourceRefV1` in `src/flow_resolve.rs`
  (~L83) and `FlowResolveSummarySourceRefV1` in `src/flow_resolve_summary.rs` (~L96). Add
  `Ext { r#ref: String, digest: Option<String> }` to both (serialises `"kind":"ext"`).
- **Scheme dispatch:** `crates/packc/src/cli/resolve.rs::collect_from_summary` (~L131-199) —
  add the `Ext` arm next to `Local|Oci|Repo|Store`; pass the ref into `LockedComponent.r#ref`.
- **Resolution to bytes:** `crates/packc/src/cli/resolve.rs::PackResolver::resolve` (~L327-381)
  — add an `ext://` branch BEFORE the `DistClient` call: locate the extension via
  `read_extensions_file` (`extension_refs.rs`), read the `file://` extension `.gtxpack`, ZIP-
  extract the component wasm + verify digest, return `ResolvedComponent { bytes, .. }`.
- **ZIP read reuse:** `crates/greentic-pack/src/reader.rs::read_archive_entries` (~L780) or
  `ZipArchive::by_name("assets/component-<name>.wasm")`; path-safety via `normalize_entry_path`.
- **Embed path (unchanged):** resolved `Local`/bytes → `build.rs::local_component_artifact`
  (~L1700) → `LockComponentBinary { logical_path: "components/<id>.wasm" }` → `package_gtpack`.
- **Digest:** `format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))` (matches
  `flow_resolve.rs::compute_sha256` ~L479).
- **Fixtures:** mirror `crates/packc/tests/components_extension.rs::write_stub_wasm` (8-byte
  wasm magic) + `write_describe_sidecar`; resolve tests run with
  `GREENTIC_PACK_USE_DESCRIBE_CACHE=1` (see `tests/resolve.rs`).
- **`describe.json` schema:** none exists — define an in-`.gtxpack` component descriptor
  (id / asset-path / digest); closest analogue `ComponentDescribe` (v0_6_0 CBOR sidecar).
- **Conventions:** `#![forbid(unsafe_code)]`; Rust 1.95 / edition 2024;
  `cargo fmt --all -- --check` + `cargo clippy --workspace --all-targets -- -D warnings`;
  `ci/local_check.sh`; never import `greentic_interfaces::bindings::*`.

## Risks

- **Exact extension-acquisition reuse point** is not yet pinned (the plan traces the
  function that fetches a declared extension dependency to its bytes). If no such reusable
  path exists for the local/fixture case, the plan adds a minimal local-source acquire used
  by both the resolver and the fixture test.
- **`#component` fragment grammar** — Phase 1 fixes it as the literal `#component` (single
  canonical component). Multi-component extensions are a future extension of the grammar,
  not Phase 1.
- **describe.json embedded-component schema** — Phase 1 defines the minimal fields the
  resolver reads (component id, asset path, digest). Phase 2 (extension build) must emit
  exactly these; the plan records the shape so Phase 2 matches.
