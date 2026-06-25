# WS-D Phase 3 — design: resolver network acquisition + http/webhook flip

**Status:** 3a (resolver `store://`/`oci://` acquisition) — ready to implement. 3b (flip
http/webhook capabilities to `ext://`) — **gated on team + Maarten sign-off** per the RFC; this
doc locks the 4 open questions with recommended defaults so that decision can be made.
**Builds on:** Phase 1 (`2026-06-25-ext-component-resolver-design.md`, shipped packc `.1`) and
Phase 2 (`2026-06-25-ext-component-phase2-producer-sidecar.md`, shipped packc `.2` + greentic-component #90).
**RFC:** `greentic-designer/docs/superpowers/specs/2026-06-22-component-extension-distribution-unification-proposal.md`.

## The 4 RFC open questions — recommended decisions

The RFC §7 flags four questions as "need team + Maarten decision before any work." The
standardization-program roadmap already recommends defaults; this doc adopts them as the
proposed decision (subject to sign-off before 3b is executed):

1. **Ref scheme** → `ext://<id>#component` (dedicated scheme). *Already built and shipped in
   Phase 1/2; do not overload `store://`.* Keeps the embedded-component concept explicit and
   lets the resolver distinguish "the component embedded in extension X" from "a standalone
   store component."
2. **Store indexing** → design-time `describe` is sufficient; the Store does **not** need to
   separately index the embedded component for discoverability in this phase. The extension is
   the discoverable unit; its embedded component is an implementation detail surfaced via the
   `component.json` sidecar at resolve time.
3. **Versioning** → **one SemVer** for the combined `.gtxpack`. Extension and embedded
   component share a release line (they already share the core lib). The inner component
   version is surfaced in `describe`/the sidecar but is not independently published.
4. **OCI deprecation** → **keep** the standalone component-OCI publish during the transition as
   a **derived** artifact (built from the same source/version), **not** the source of truth.
   Do not deprecate until all first-party consumers have moved to `ext://`. Bare-ref/external
   consumers keep working.

## 3a — resolver `store://` / `oci://` acquisition (implement now)

### Problem
Phase 1's `resolve_extension_source` (`crates/packc/src/cli/ext_resolver.rs:132-144`) bails on
any non-`file://` scheme. Extensions published by the Phase-2 producer live in the **Store**
(`greentic-component store publish` → `POST /api/v1/extensions`), so `store://` extension refs
cannot yet be resolved. Phase 3a wires acquisition so `ext://<id>#component` works when the
extension is declared in `pack.extensions.json` with a `store://` (and, best-effort, `oci://`)
source.

### Acquisition design

**`store://` (primary, real, testable).** Acquire the extension `.gtxpack` from the
store-server's dedicated, public endpoint:

```
GET {store_base}/api/v1/extensions/{name}/{version}/artifact
→ 200 application/octet-stream  (the .gtxpack bytes)
  header x-artifact-sha256: <hex>   (whole-archive digest)
```

(Handler: `greentic-store-server/.../handlers/extensions/artifact.rs`; no auth required;
404 if version missing or `approval_state != approved`.) `store_base` comes from
`GREENTIC_STORE_URL` (same env the Phase-2 producer uses); if unset, error with a clear message.

- Parse the source `reference` (`store://<name>@<version>`) into `(name, version)`. Confirm the
  exact ref shape against `extension_refs.rs` fixtures; **require an explicit version** in 3a
  (latest/tag resolution via the store index is deferred — `allow_tags` handling noted but not
  implemented here).
- Optionally cross-check the response `x-artifact-sha256` against the bytes (whole-archive
  integrity), distinct from the **inner component** digest the sidecar verifies in
  `extract_and_verify`.
- Cache the downloaded `.gtxpack` under the runtime cache dir keyed by `x-artifact-sha256` to
  avoid re-downloading; in **offline** mode, use the cache and error on miss (mirror the
  non-ext path's offline behavior at `resolve.rs:366-369`).

**`oci://` (secondary, no producer yet → best-effort/deferred).** The Phase-2 producer does not
publish extensions to OCI, so this path is currently **untestable end-to-end**. Implement a thin
branch that reuses `DistClient` only if it cleanly returns the `.gtxpack` bytes for an extension
artifact (verify media-type acceptance — `DistClient` is wasm/pack-centric; an extension
`.gtxpack` may be `MediaTypeRejected`). If it does not fit cleanly, **bail with a clear
"oci:// extension acquisition not yet supported (no producer)"** message rather than forcing a
fragile path. Document the gap; revisit when an OCI extension producer exists. Do **not** block
3a on this.

### Code changes (`crates/packc`)

- `src/cli/ext_resolver.rs`:
  - Refactor `extract_and_verify(extension_id, &Path)` → `extract_and_verify_bytes(extension_id, &[u8])`
    (the ZIP reader already wraps a `Cursor`); keep a `&Path` wrapper that reads then delegates,
    so the `file://` path is unchanged.
  - Add an acquisition entry point that has access to the `DistClient`, the offline flag, and a
    tokio `Handle` (for the store HTTP GET / any async). E.g.
    `resolve_ext_component_with_dist(pack_dir, raw_ref, dist, offline, handle) -> Result<(Vec<u8>, String)>`,
    with the existing `resolve_ext_component` kept for the file://-only/test path or made to
    delegate.
  - `resolve_extension_source` gains `store://` (HTTP GET → bytes) and the guarded `oci://`
    branch; `file://`/bare unchanged.
- `src/cli/resolve.rs`: the `ext://` branch at `resolve.rs:349-361` calls the new
  dist-aware entry point, passing `&self.dist`, the offline flag
  (`self.runtime.network_policy() == NetworkPolicy::Offline`), and the current `Handle`
  (`Handle::try_current()` as the non-ext path does at `resolve.rs:363`).
- HTTP client: reuse the workspace `reqwest` (already a dep; blocking is available — the non-ext
  path uses `block_on` over a `Handle`, so use either blocking reqwest off-thread or async +
  `block_on`, matching the file's existing pattern).

### TDD
- `extract_and_verify_bytes`: in-memory ZIP fixtures (happy, missing `component.json`,
  missing asset, digest mismatch) — port the existing Phase-2 tests to the bytes variant.
- `store://` ref parsing: `store://greentic.http@1.2.0` → `(name, version)`; malformed → error.
- store acquisition: use a mock HTTP server (`wiremock` or a tiny `axum`/`tiny_http` stub) that
  serves a fixture `.gtxpack` at `/api/v1/extensions/{name}/{version}/artifact` with an
  `x-artifact-sha256` header; assert the resolver downloads, extracts, and digest-verifies. If a
  mock server is too heavy for the existing test harness, factor the URL-building + response
  handling into a pure function and unit-test that, plus one `#[ignore]` integration test.
- offline: `store://` in offline mode with empty cache → clear error; with cache hit → success.

### Release
- packc: bump `1.2.0-research.2` → `.3`, republish (`crates-publish.yml`). Lockstep: workspace
  version + intra path-pins (`greentic-pack`, `pack_component_template`) `.2`→`.3`; external
  greentic-* pins unchanged. No greentic-types change.

## 3b — flip http/webhook to `ext://` (GATED — not executed in this phase)

Recorded so the sign-off decision has the full plan. **Do not execute without sign-off**, and
note `http_call` is a stable, priority-100 capability — flipping its `component_ref` before all
plumbing exists breaks every `http_call` flow build.

Prerequisites (must all land before the catalog flip):
1. **`greentic.http` extension exists + is published** embedding `component-http`
   (`components-public/crates/component-http`) via the Phase-2 producer path. Today `component-http`
   is a standalone OCI component with no extension wrapper. One combined `.gtxpack`, one SemVer
   (decision Q3).
2. **Designer compiler `Ext` arm.** `greentic-designer/src/flow_generator/compiler/resolve.rs`
   (`build_resolve_sidecar_with_extensions`, ~L104-163) currently emits only `Oci`/`Local` from a
   capability `component_ref`. Add handling so an `ext://<id>#component` `component_ref` emits an
   `Ext` resolve-source entry (the sidecar `kind:"ext"` shape).
3. **`pack.extensions.json` declares `greentic.http`** (with a `store://` source) so packc's
   resolver can locate it — now resolvable thanks to 3a.
4. **Catalog flip (the final, one-line, gated change):** in
   `greentic-designer/src/flow_generator/catalog.baseline.yaml`, `http_call.component_ref`
   `oci://ghcr.io/greenticai/components/component-http:latest` → `ext://greentic.http#component`.
5. **OCI as derived (Q4):** keep `components-public` publishing `component-http` to GHCR during
   transition; reclassify as derived/optional, not the catalog source.

`webhook` is not yet a live capability (still `reserved_capability`); no flip applies until it is
promoted.

## Out of scope
- Latest/tag version resolution for `store://` extension refs (requires the store index endpoint).
- An OCI extension producer (and therefore robust `oci://` extension acquisition).
- Deprecating the standalone component-OCI publish.
