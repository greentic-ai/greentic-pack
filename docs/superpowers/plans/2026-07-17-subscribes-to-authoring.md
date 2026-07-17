# `subscribes_to` Authoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a hand-written pack declare, per flow in `pack.yaml`, the event topics it subscribes to, so greentic-start's (already-working, live-verified) event router routes matching NATS business events to that flow instead of the default.

**Architecture:** `subscribes_to` becomes a per-flow `pack.yaml` config field, sibling to `tags`/`entrypoints`. It threads: `FlowConfig` (greentic-pack) → `PackFlowEntry` (greentic-types) → the canonical CBOR codec's `EncodedFlowEntry` (greentic-types `src/cbor`) → `manifest.cbor` → runtime read (greentic-start, unchanged). Two repos, sequenced by a crates.io publish gate: greentic-types must publish the new field before greentic-pack can pin+use it.

**Tech Stack:** Rust 1.95.0, serde, ciborium (canonical CBOR codec in greentic-types `src/cbor`), `assert_cmd` CLI tests. greentic-pack additionally targets `wasm32-wasip2`.

## Global Constraints

- Two repos, TWO worktrees, both off `origin/research`:
  - **greentic-types**: `/home/bima-pangestu/.cache/wt/types-subscribes-to` (detached at `68e37f1` = published `1.3.0-research.1`; the local `research` branch is STALE at `1.2.0-research.1` — do NOT use it). Create a branch here in Task 1.
  - **greentic-pack**: `/home/bima-pangestu/.cache/wt/pack-subscribes-to`, branch `feat/subscribes-to-authoring`.
- **PUBLISH GATE between Slice 1 and Slice 2**: greentic-pack pins greentic-types with an exact `=` version. Slice 2 CANNOT compile until Slice 1's new version (`1.3.0-research.2`) is published to crates.io. The publish wait is Task 1 Step 12 + the PUBLISH GATE section.
- Version bump: greentic-types `1.3.0-research.1` → `1.3.0-research.2`. Publishing is automated (`.github/workflows/{crates-publish,tag-on-version-bump}.yml` on the research lane).
- The CBOR key must be exactly `subscribes_to` (snake_case, no `#[serde(rename)]`) — the runtime reads `extract_string_array_from_map(map, "subscribes_to")`.
- `PackFlowEntry` is NOT `#[non_exhaustive]`: adding the field is source-breaking for every struct-literal site; the compiler enumerates them.
- No AI/Claude co-author attribution on commits or PRs (both repos).
- greentic-pack: `#![forbid(unsafe_code)]`; use `greentic_interfaces::canonical`, never `bindings::*`; validate WASM with `cargo check --target wasm32-wasip2`; update `.codex/repo_overview.md` before+after PR.
- Gate per repo: `bash ci/local_check.sh` green before the PR is declared done.

---

## SLICE 1 — greentic-types (worktree `/home/bima-pangestu/.cache/wt/types-subscribes-to`)

### Task 1: Add `subscribes_to` to `PackFlowEntry` and thread it through the canonical CBOR codec

The public type and the canonical codec's compact intermediate must BOTH carry the field, and the encode+decode threads must copy it — or the field is silently dropped on the wire. One task, because these edits are meaningless apart and share one test.

**Files:**
- Branch: create `feat/subscribes-to-types` off the current detached HEAD (`68e37f1`).
- Modify: `src/pack_manifest.rs:136-149` (`PackFlowEntry`)
- Modify: `src/cbor/mod.rs:186-193` (`EncodedFlowEntry`), `:304-309` (encode), `:576-582` (decode)
- Modify: `Cargo.toml` (version bump)
- Modify: `tests/pack_manifest_roundtrip.rs:147`, `tests/pack_validation_components.rs:20` and `:53` (fixtures — add the field)
- Test: `tests/pack_manifest_roundtrip.rs` (extend)

**Interfaces:**
- Produces: `PackFlowEntry.subscribes_to: Vec<String>` (public field). Slice 2's greentic-pack `build.rs:909` will set it. The published crate version becomes `1.3.0-research.2`.

- [ ] **Step 1: Create the branch**

```bash
cd /home/bima-pangestu/.cache/wt/types-subscribes-to
git switch -c feat/subscribes-to-types
git status -sb | head -1   # expect: feat/subscribes-to-types
```

- [ ] **Step 2: Write the failing round-trip test**

Add to `tests/pack_manifest_roundtrip.rs`. This test builds a manifest whose flow declares `subscribes_to`, runs it through the CANONICAL codec (`encode_pack_manifest` → `decode_pack_manifest`), and asserts the field survives — the discriminating check that catches a missed codec thread. Model it on the existing manifest construction at line 147 (reuse the same helpers the file already imports):

```rust
#[test]
fn subscribes_to_survives_canonical_roundtrip() {
    use greentic_types::{decode_pack_manifest, encode_pack_manifest};

    let mut manifest = sample_pack_manifest(); // the file's existing helper that builds a PackManifest
    manifest.flows[0].subscribes_to = vec!["orders.*".to_string()];

    let bytes = encode_pack_manifest(&manifest).expect("encode");
    let decoded = decode_pack_manifest(&bytes).expect("decode");

    assert_eq!(
        decoded.flows[0].subscribes_to,
        vec!["orders.*".to_string()],
        "subscribes_to must survive the canonical CBOR encode->decode; a missed \
         EncodedFlowEntry thread silently drops it"
    );
}
```

If the file has no reusable `sample_pack_manifest()` helper, build the `PackManifest` inline copying the construction at `tests/pack_manifest_roundtrip.rs:140-165` and set `subscribes_to` on its one flow. Do NOT invent field names — copy the exact existing construction and add `subscribes_to: vec!["orders.*".into()]` to the `PackFlowEntry { .. }`.

- [ ] **Step 3: Run it — expect a COMPILE failure**

```bash
cargo test --all-features subscribes_to_survives_canonical_roundtrip 2>&1 | tail -20
```
Expected: FAIL to compile — `no field subscribes_to on type PackFlowEntry` (and the fixtures at lines 147/20/53 also don't have it yet). This proves the field is genuinely absent.

- [ ] **Step 4: Add the field to `PackFlowEntry`**

In `src/pack_manifest.rs`, inside `PackFlowEntry` (after `entrypoints`, ~line 148):

```rust
    /// Event topics this flow subscribes to (glob patterns, e.g. `orders.*`).
    /// Matched against an incoming business event's topic by the runtime's
    /// `select_target_flows`; empty means the flow only runs as the default.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    #[cfg_attr(feature = "schemars", schemars(default))]
    pub subscribes_to: Vec<String>,
```

- [ ] **Step 5: Add the field to the canonical codec's `EncodedFlowEntry` and thread encode+decode**

In `src/cbor/mod.rs`, in `struct EncodedFlowEntry` (after `entrypoints`, ~line 192):

```rust
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    subscribes_to: Vec<String>,
```

In the encode map (~line 304-309, building `EncodedFlowEntry { .. }` from `flow_entry`), add after `entrypoints`:

```rust
                    subscribes_to: flow_entry.subscribes_to.clone(),
```

In the decode map (~line 576-582, building `PackFlowEntry { .. }` from `flow_entry`), add after `entrypoints`:

```rust
                    subscribes_to: flow_entry.subscribes_to,
```

- [ ] **Step 6: Fix the other struct-literal construction sites the compiler flags**

Add `subscribes_to: vec![]` (or a value where the test wants one) to:
- `tests/pack_manifest_roundtrip.rs:147` — the `PackFlowEntry { .. }` fixture (if your new test reuses this manifest, set it there; otherwise `vec![]`).
- `tests/pack_validation_components.rs:20` and `:53` — the `flow_with_component` fixtures: `subscribes_to: vec![]`.

Run `cargo build --all-features` and add the field anywhere else the compiler reports `missing field subscribes_to`.

- [ ] **Step 7: Run the test — expect PASS**

```bash
cargo test --all-features subscribes_to_survives_canonical_roundtrip -- --nocapture
```
Expected: PASS. The field survives the canonical round-trip.

- [ ] **Step 8: Add the backward-compat test**

An old-shape encoded manifest (no `subscribes_to` key) must decode to an empty vec. The cleanest way to produce an "old" manifest is to encode a manifest whose flow has empty `subscribes_to` (which `skip_serializing_if` omits from the bytes) and assert it decodes to empty:

```rust
#[test]
fn manifest_without_subscribes_to_decodes_to_empty() {
    use greentic_types::{decode_pack_manifest, encode_pack_manifest};
    let manifest = sample_pack_manifest(); // flow[0].subscribes_to defaults to empty
    let bytes = encode_pack_manifest(&manifest).expect("encode");
    let decoded = decode_pack_manifest(&bytes).expect("decode");
    assert!(
        decoded.flows[0].subscribes_to.is_empty(),
        "a manifest without the key must decode to an empty vec (backward-compat)"
    );
}
```

Run: `cargo test --all-features manifest_without_subscribes_to_decodes_to_empty` → PASS.

- [ ] **Step 9: Bump the version**

In `Cargo.toml`, change `version = "1.3.0-research.1"` → `version = "1.3.0-research.2"`.

- [ ] **Step 10: Full gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
bash ci/local_check.sh   # if present; if it fails outside this change, note it in the PR
```
All green.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat(manifest): add subscribes_to to PackFlowEntry + canonical CBOR codec

greentic-start's event router already routes to a flow whose subscribes_to
matches an incoming business-event topic (live-verified), but no pack could
declare the field. Add subscribes_to to PackFlowEntry and thread it through the
canonical CBOR codec (EncodedFlowEntry encode+decode) so it reaches manifest.cbor
as the top-level snake_case key the runtime reads. Optional + skip-if-empty, so
old manifests decode unchanged and existing golden bytes are byte-identical when
empty. Bumps the research lane to 1.3.0-research.2."
```

- [ ] **Step 12: Open the PR (base research), merge, and WAIT for publish**

```bash
git push -u origin feat/subscribes-to-types
gh pr create --base research --title "feat(manifest): add subscribes_to to PackFlowEntry" --body "<summary; no AI attribution>"
```
After merge, the research-lane version bump triggers `tag-on-version-bump.yml` → `crates-publish.yml`. **Do not start Slice 2 until `greentic-types@=1.3.0-research.2` is live on crates.io.** Verify:
```bash
cargo search greentic-types 2>/dev/null | grep "1.3.0-research.2" || echo "NOT YET PUBLISHED — wait"
```

---

## PUBLISH GATE

Slice 2 pins `greentic-types = "=1.3.0-research.2"`. It will not resolve until Step 12's publish lands. This is a hard, human-in-the-loop wait — do not proceed on a red `cargo search`.

---

## SLICE 2 — greentic-pack (worktree `/home/bima-pangestu/.cache/wt/pack-subscribes-to`, branch `feat/subscribes-to-authoring`)

### Task 2: Confirm the two latent manifest writers are dead

Before adding the field, confirm no OTHER live path writes a flow entry that would silently omit `subscribes_to`. Pure investigation; no code change. If a live writer is found, STOP and report — the plan assumed there is none.

**Files:** none (read-only).

- [ ] **Step 1: Confirm `packc/src/manifest.rs build_manifest` is test-only**

```bash
cd /home/bima-pangestu/.cache/wt/pack-subscribes-to
grep -rn "build_manifest" crates/packc/src crates/packc/tests | grep -v "fn build_manifest"
```
Expected: all call sites are inside `#[cfg(test)]` modules or `tests/`. If any production (non-test) caller exists, record it and treat it as a real second writer needing the field.

- [ ] **Step 2: Confirm `greentic-pack-lib builder.rs PackBuilder` has no live shipper**

```bash
grep -rn "PackBuilder" crates/ --include=*.rs | grep -v "test" | grep -v "builder.rs:"
```
Read the hits. `PackBuilder` is labelled legacy (used by `pack_lock_doctor` + tests). Confirm the CLI `build` path (`crates/packc/src/cli/`) does NOT use `PackBuilder` to emit `manifest.cbor` (it uses `encode_pack_manifest` on a greentic-types `PackManifest`). Record the finding.

- [ ] **Step 3: Confirm no read→re-emit path via `reader.rs convert_gpack_flow`**

```bash
grep -rn "convert_gpack_flow" crates/ --include=*.rs
```
It drops all but id/kind/entry/hash. Confirm it feeds only inspection/lint output, not a manifest re-write. Record.

Write the three findings into the Task 2 report. If all three are dead/inspection-only, proceed. If any is a live writer, escalate (the plan's scope was wrong).

### Task 3: Add `subscribes_to` to `FlowConfig` and populate it in the build

**Files:**
- Modify: `crates/packc/src/config.rs:119-126` (`FlowConfig`)
- Modify: `crates/packc/schemas/pack.schema.v1.json` (near the flow `tags` at line 715)
- Modify: `crates/packc/src/build.rs:909-915` (`PackFlowEntry` construction)
- Modify: `Cargo.toml` (greentic-types pin), `Cargo.lock`
- Test: `crates/packc/tests/subscribes_to_build.rs` (new)

**Interfaces:**
- Consumes: `greentic_types::PackFlowEntry.subscribes_to` (from Slice 1, published `1.3.0-research.2`).
- Produces: a `manifest.cbor` whose flow entries carry `subscribes_to` from `pack.yaml`.

- [ ] **Step 1: Bump the greentic-types pin**

In `Cargo.toml`, change the greentic-types dependency `version = "=1.3.0-research.1"` → `version = "=1.3.0-research.2"` (the workspace dep, ~line 119-120). Then:
```bash
cargo update -p greentic-types --precise 1.3.0-research.2
```
Expected: `Cargo.lock` updates to `1.3.0-research.2`. If cargo reports it is not available, the publish gate has not cleared — STOP.

- [ ] **Step 2: Write the failing build-pipeline test**

Create `crates/packc/tests/subscribes_to_build.rs`. Model it on `tests/build_component_manifests.rs` (the `build_pack` + `decode_pack_manifest` pattern at lines 430-463). It writes a minimal pack whose `pack.yaml` declares `subscribes_to` on a flow, runs `greentic-pack build`, decodes the produced `manifest.cbor`, and asserts the flow entry carries the topic. Reuse the smallest valid pack fixture the repo's other build tests use (copy an existing `write_*_fixture` helper into this test file or a shared test-util; do NOT hand-roll an invalid pack):

```rust
use std::fs;
use assert_cmd::cargo::cargo_bin;
use std::process::Command;
use greentic_types::decode_pack_manifest;
use tempfile::TempDir;

// Reuse an existing minimal-pack fixture writer from the repo's test suite.
// It must produce a pack.yaml whose flow entry can carry `subscribes_to`.
// See tests/build_component_manifests.rs / tests/components_extension.rs for the
// established `write_pack_fixture` shape; the ONLY addition here is the
// `subscribes_to: ["orders.*"]` line under the flow in pack.yaml.

#[test]
fn build_writes_subscribes_to_into_manifest() {
    let temp = TempDir::new().expect("temp");
    let pack_dir = write_minimal_pack_with_subscription(&temp); // flow declares subscribes_to: ["orders.*"]

    let output = Command::new(cargo_bin!("greentic-pack"))
        .current_dir(&pack_dir)
        .arg("build")
        .output()
        .expect("run build");
    assert!(
        output.status.success(),
        "build failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let bytes = fs::read(pack_dir.join("dist/manifest.cbor")).expect("manifest");
    let manifest = decode_pack_manifest(&bytes).expect("decode manifest");
    let flow = manifest
        .flows
        .iter()
        .find(|f| f.id.as_str() == "on_order")
        .expect("the on_order flow must be in the manifest");
    assert_eq!(
        flow.subscribes_to,
        vec!["orders.*".to_string()],
        "the pack.yaml subscribes_to must reach manifest.cbor"
    );
}
```

Implement `write_minimal_pack_with_subscription` by copying the repo's smallest existing pack-fixture writer and adding, under the flow in the generated `pack.yaml`:
```yaml
    subscribes_to:
      - "orders.*"
```
The flow id must be `on_order` to match the assertion (or adjust the assertion to the fixture's id).

- [ ] **Step 3: Run it — expect FAIL**

```bash
cargo test -p greentic-pack --test subscribes_to_build 2>&1 | tail -25
```
Expected: FAIL — either `no field subscribes_to on FlowConfig` (compile) once you reference it, or (if the fixture builds) the assertion fails because `flow.subscribes_to` is empty (the compiler doesn't yet copy it). Both are the correct pre-implementation failure.

- [ ] **Step 4: Add the field to `FlowConfig`**

In `crates/packc/src/config.rs`, inside `FlowConfig` (after `entrypoints`, ~line 125):
```rust
    #[serde(default)]
    pub subscribes_to: Vec<String>,
```

- [ ] **Step 5: Populate it at the build seam**

In `crates/packc/src/build.rs`, in the `PackFlowEntry { .. }` construction (~line 909-915), add after `entrypoints`:
```rust
            subscribes_to: cfg.subscribes_to.clone(),
```

- [ ] **Step 6: Document it in the pack schema**

In `crates/packc/schemas/pack.schema.v1.json`, in the flow object's `properties` (as a sibling of `tags`, near line 715), add:
```json
"subscribes_to": {
  "type": "array",
  "items": { "type": "string" },
  "description": "Event topic glob patterns this flow subscribes to (e.g. \"orders.*\"). Matched against a business event's topic at runtime; empty means the flow only runs as the pack default."
}
```
Match the surrounding JSON indentation and add the trailing comma placement correctly (validate with `python3 -m json.tool crates/packc/schemas/pack.schema.v1.json > /dev/null`).

- [ ] **Step 7: Run the test — expect PASS**

```bash
cargo test -p greentic-pack --test subscribes_to_build -- --nocapture
```
Expected: PASS. `flow.subscribes_to == ["orders.*"]`.

- [ ] **Step 8: MUTATION CHECK — prove the test discriminates**

Mandatory. Revert only the copy at `build.rs`:
```bash
# temporarily drop the field-copy so subscribes_to is not written
sed -i 's/            subscribes_to: cfg.subscribes_to.clone(),//' crates/packc/src/build.rs
git diff --stat crates/packc/src/build.rs   # MUST be non-empty; if empty the edit didn't land — stop
cargo test -p greentic-pack --test subscribes_to_build 2>&1 | grep -E "test result|FAILED"
```
Expected: the test FAILS (manifest flow has empty `subscribes_to`). If it PASSES, the test is not discriminating — fix the test, not the mutation. Then restore:
```bash
git checkout crates/packc/src/build.rs
```
Re-apply Step 5's one line (the checkout discarded it), then re-run Step 7 → PASS.

- [ ] **Step 9: Full gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --target wasm32-wasip2
cargo test --workspace --locked
bash ci/local_check.sh
```
All green. Update `.codex/repo_overview.md` if it tracks the schema/config surface.

- [ ] **Step 10: Commit**

```bash
git add -A
git commit -m "feat(build): author subscribes_to per flow in pack.yaml

FlowConfig gains subscribes_to (sibling to tags/entrypoints); the build copies
it onto PackFlowEntry at build.rs so it lands in manifest.cbor, where the
already-working runtime router reads it. Documented in pack.schema.v1.json.
Bumps the greentic-types pin to =1.3.0-research.2 (the publish that added the
field). No runtime change is needed."
```

### Task 4: End-to-end live verification (the payoff)

Proves the whole authored chain — `pack.yaml` → build → `manifest.cbor` → real NATS → runtime routes to the subscriber — with a NATURALLY compiled pack, replacing this session's hand-injected B2 proof.

**Files:** none (verification). If it fails, fix the cause and note it; do not adjust tests to match.

- [ ] **Step 1: Build a real pack that declares a subscription**

Author a pack whose `pack.yaml` has a flow (`on_order`) with `subscribes_to: ["orders.*"]` and a sibling default flow. Build it with `greentic-pack build`. Decode `dist/manifest.cbor` and confirm `on_order.subscribes_to == ["orders.*"]` (this reuses Task 3's assertion but on a real, bootable pack).

- [ ] **Step 2: Boot greentic-start with real NATS against that pack**

Reuse this session's harness (`~/.local/bin/nats-server`; the boot recipe under the scratchpad `re-verify/`). Build the pack into a bootable bundle, start `nats-server`, boot greentic-start with `GREENTIC_EVENTS_NATS_URL=nats://127.0.0.1:4222`, and confirm from the log the listener subscribed (`business event listener subscribed subject=greentic.events.>`). Capture greentic-start's log to a file.

- [ ] **Step 3: Publish a matching event and observe the subscriber fire**

Publish an `EventEnvelope` to `greentic.events.<tenant>.orders.created`. The envelope's `tenant` object needs the required `attempt: u32` field (`{"env":"dev","tenant":"demo","tenant_id":"demo","team":"general","attempt":0}`), or `convert()` drops it as "unroutable" — this bit the first publish last time. Grep the greentic-start log:
```bash
grep -E "delivered [0-9]+ flow invocation|route event .* -> on_order|no subscriber" <log>
```
Expected: the router selects `on_order` (the subscriber), NOT the default flow. Because this pack's `subscribes_to` was AUTHORED and COMPILED (not hand-injected), this closes the loop end to end.

- [ ] **Step 4: Clean up and record**

Kill `nats-server` and greentic-start by PID. Write the observed log lines (verbatim) into the Task 4 report. If the subscriber did NOT fire, the authored field did not reach the runtime — investigate (likely a codec or key-name mismatch) and report; do not paper over it.

---

## Rollout

- Slice 1 PR (greentic-types, base `research`) merges and publishes `1.3.0-research.2` FIRST.
- Slice 2 PR (greentic-pack, base `research`, branch `feat/subscribes-to-authoring`) follows, pinning the new version.
- Each repo's `bash ci/local_check.sh` must pass. No AI attribution. Update `.codex/repo_overview.md` in greentic-pack.
- Out of scope, unchanged: designer authoring UX, and a NATS publisher/deployment. This ships the authoring contract; those turn it into traffic.
