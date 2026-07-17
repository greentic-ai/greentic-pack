# Authoring `subscribes_to`: close the event-routing contract gap

Date: 2026-07-17
Status: design approved, not implemented
Repos: `greentic-types` (Slice 1), `greentic-pack` (Slice 2). `greentic-flow` and `greentic-start` untouched.

## Problem

`greentic-start`'s business-event router already routes an incoming NATS event to a flow that
declares a matching `subscribes_to` topic — **live-verified** with a real nats-server: with the
field present in the manifest, `select_target_flows` selects the subscriber; absent, it falls to
the pack's default flow. The routing code is sound.

But **no pack can declare `subscribes_to` today**. The field is absent from every write-side type:

- `greentic_types::PackFlowEntry` (`pack_manifest.rs:132-149`) = `id`/`kind`/`flow`/`tags`/`entrypoints`.
- `greentic-pack`'s `FlowConfig` (`crates/packc/src/config.rs:119-126`) = `id`/`file`/`tags`/`entrypoints`.

So `select_target_flows` is always empty in production and every business event fires the pack's
default flow, regardless of the event topic. This makes the shipped topic-routing feature inert —
declared as follow-up debt in the PR that introduced `subscribes_to` (greentic-start #287).

### What this spec does NOT solve (deliberate, out of scope)

- **Designer authoring UX.** Designer has no trigger-node concept; it cannot yet emit a per-flow
  `subscribes_to`. This slice makes hand-written `pack.yaml` packs able to declare it; a designer
  surface is a separate, later slice.
- **A NATS publisher / a deployment with the listener enabled.** No deployment sets
  `GREENTIC_EVENTS_NATS_URL` and no service publishes business events to NATS today. This slice is
  the foundational enabler; the road works, traffic arrives later. This is an accepted, known
  precondition — building the authoring contract now unblocks the rest.

## Goal

A hand-written pack can declare, per flow, the event topics it subscribes to, in `pack.yaml`, and
the compiler writes those into `manifest.cbor` so the (already-working) runtime routes matching
events to that flow instead of the default.

## Design

`subscribes_to` is a **pack-level per-flow config field** in `pack.yaml`, sibling to the existing
`tags` and `entrypoints`. This is not a new pattern — it is a carbon copy of how `tags`/`entrypoints`
already flow from `pack.yaml` → `FlowConfig` (`cfg`) → `PackFlowEntry` at `build.rs:909`, and it
aligns with the runtime, which reads `subscribes_to` from the **top-level** manifest flow-entry map
(`greentic-start/src/messaging_app.rs:633`), exactly where `PackFlowEntry` serializes it.

Authoring surface chosen over the alternative (declaring it inside the `.ygtc` FlowDoc): pack.yaml
matches the existing convention, aligns with the top-level manifest read, needs no `greentic-flow`
change, and is the surface a future designer wizard already targets (wizard answers → FlowConfig).
The `.ygtc` alternative was rejected: the compiled `Flow` drops unknown FlowDoc fields, so it would
require threading through `greentic-flow` (a third repo, trunk `develop`) for no semantic gain.

### Data flow

```
pack.yaml  flows[].subscribes_to: ["orders.*"]
   → FlowConfig.subscribes_to (cfg)                      [greentic-pack config.rs]
   → PackFlowEntry.subscribes_to                         [greentic-types type + greentic-pack build.rs:909]
   → EncodedFlowEntry.subscribes_to (canonical codec)    [greentic-types src/cbor/mod.rs encode+decode]
   → manifest.cbor  (string key "subscribes_to")         [encode_pack_manifest]
   → parse_flow_entry reads top-level "subscribes_to"    [greentic-start messaging_app.rs:633 — EXISTS]
   → select_target_flows                                 [greentic-start event_router.rs — EXISTS, verified]
```

Every link right of `build.rs:909` is already present and live-verified. The change adds only the
two left-most write-side links.

### Slice 1 — `greentic-types` (must publish before Slice 2 can adopt)

**Correction found while planning:** `manifest.cbor` is NOT produced by serde-deriving `PackManifest`.
It is written by greentic-types' own **canonical CBOR codec** (`greentic_types::encode_pack_manifest`,
`src/cbor/mod.rs`), which serializes a compact intermediate `EncodedFlowEntry` (`cbor/mod.rs:186-193`)
with plain string field names (`id`/`kind`/`flow`/`tags`/`entrypoints`). That is the string-keyed map
the runtime reads (`extract_string_array_from_map(map,"subscribes_to")` matched the string key when I
hand-injected it in the live B2 test). So the field must be threaded through the CANONICAL CODEC, not
just added to the public type. Slice 1 is therefore four edits, not one:

1. **`PackFlowEntry`** (`src/pack_manifest.rs:136-149`), alongside `tags`/`entrypoints`:
   ```rust
   #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
   pub subscribes_to: Vec<String>,
   ```
2. **`EncodedFlowEntry`** (`src/cbor/mod.rs:186-193`) — the compact codec struct that actually hits
   `manifest.cbor`. `#[derive(Serialize,Deserialize)]`, plain names, so:
   ```rust
   #[serde(default, skip_serializing_if = "Vec::is_empty")]
   subscribes_to: Vec<String>,
   ```
   The plain field name serializes as the CBOR string key `subscribes_to`, matching the runtime read.
3. **Encode thread** (`cbor/mod.rs:304-309`, building `EncodedFlowEntry` from `PackFlowEntry`):
   `subscribes_to: flow_entry.subscribes_to.clone(),`
4. **Decode thread** (`cbor/mod.rs:576-582`, building `PackFlowEntry` from `EncodedFlowEntry`):
   `subscribes_to: flow_entry.subscribes_to,`

- Wire-compatible: `#[serde(default)]` lets old `manifest.cbor` (no key) decode to an empty vec;
  `skip_serializing_if` keeps existing golden manifests byte-identical when empty. No
  `deny_unknown_fields` anywhere in the crate. `PackFlowEntry.subscribes_to` follows the in-crate
  `agents` precedent (`pack_manifest.rs:121-129`).
- `PackFlowEntry` is **not** `#[non_exhaustive]`, so every struct-literal site must add the field. In
  greentic-types those are: the decode at `cbor/mod.rs:576`, and test fixtures
  `tests/pack_manifest_roundtrip.rs:147`, `tests/pack_validation_components.rs:20` and `:53`. The
  compiler enumerates them; greentic-pack `build.rs:909` is fixed in Slice 2.
- **Discriminating test (catches the exact codec trap):** a round-trip through the CANONICAL codec —
  build a `PackManifest` whose flow carries `subscribes_to: ["orders.*"]`, `encode_pack_manifest` →
  decode → assert the field survives. If the field is added to `PackFlowEntry` but NOT threaded
  through `EncodedFlowEntry`, this test FAILS (the canonical encode silently drops it). Extend the
  existing `tests/pack_manifest_roundtrip.rs`. Plus a backward-compat test: an old-shape encoded
  manifest without the key decodes to an empty vec.
- **Branch/version:** the local `research` checkout is STALE (at `1.2.0-research.1`; behind published).
  The real publish lane is `origin/research` @ `1.3.0-research.1` (tag `v1.3.0-research.1`, commit
  `68e37f1`). Base the change on `origin/research`, bump `1.3.0-research.1` → `1.3.0-research.2`.
  Publishing is automated (`.github/workflows/{crates-publish,tag-on-version-bump}.yml`), so a version
  bump on the research lane publishes the new crate — no manual token needed.

### Slice 2 — `greentic-pack` (after Slice 1 is published)

1. Add to `FlowConfig` (`crates/packc/src/config.rs:119-126`), sibling to `tags`:
   ```rust
   #[serde(default)]
   pub subscribes_to: Vec<String>,
   ```
2. Document it in the pack schema `crates/packc/schemas/pack.schema.v1.json` (array of strings, a
   sibling of `tags` in the flow object; describe as event-topic glob patterns, e.g. `orders.*`).
3. Populate it at `build.rs:909` when constructing `PackFlowEntry`:
   ```rust
   subscribes_to: cfg.subscribes_to.clone(),
   ```
4. Bump the greentic-types pin `=1.3.0-research.1` → `=1.3.0-research.2` (workspace dep at
   `Cargo.toml:119-120`) and update `Cargo.lock`.

### Two latent writers — confirm dead before shipping

Neither feeds the live `manifest.cbor`, but both are structurally "manifest writers" with their own
flat `FlowEntry` that would silently omit `subscribes_to`:

- `crates/packc/src/manifest.rs` `build_manifest` (`FlowEntry` at `manifest.rs:157`) — investigation
  found it is called ONLY from `#[cfg(test)]` code. Confirm still test-only; if so, leave it.
- `greentic-pack-lib` `builder.rs` `PackBuilder` (`FlowEntry` at `builder.rs:317`) — labelled legacy,
  used by `pack_lock_doctor`/tests, not the CLI `build` path. Confirm no live consumer ships packs
  via `PackBuilder`; if one does, it is a genuine second writer needing the field.

Also `crates/greentic-pack/src/reader.rs:1009` `convert_gpack_flow` drops all but id/kind/entry/hash
when reading `PackFlowEntry` → legacy `FlowEntry`; if any read→re-emit path exists it would lose the
field. Verify no such round-trip is on a live path.

## Error handling

Empty `subscribes_to` = today's behaviour (default-flow fallback); a flow that omits it is unchanged.
No pattern validation is added (YAGNI): the runtime's `topic_matches` is lenient, a malformed glob
simply never matches, and there is no crash surface. If validation is wanted later it belongs at the
runtime match layer, not the pack compiler.

## Testing

- **greentic-types:** a `PackFlowEntry` with `subscribes_to = ["orders.*"]` round-trips through CBOR
  (serialize → deserialize preserves it); an old-shape manifest map without the key deserializes with
  an empty vec (backward-compat).
- **greentic-pack:** a build-pipeline test — a `pack.yaml` whose flow declares
  `subscribes_to: ["orders.*"]` → `build` → decode `manifest.cbor` → assert the flow entry carries
  `["orders.*"]`. Mutation-checked: dropping `cfg.subscribes_to` at `build.rs:909` (or reverting to
  the empty default) must fail this test. Assert the mutation landed (`git diff` non-empty) before
  trusting the run.
- **End-to-end live verify (the payoff):** author a real pack with `subscribes_to` in `pack.yaml`,
  build it, boot `greentic-start` with `GREENTIC_EVENTS_NATS_URL` against a real nats-server, publish
  an `EventEnvelope` to `greentic.events.<tenant>.orders.created`, and observe the subscriber flow
  selected — this time NATURALLY compiled from the authored pack, replacing the earlier hand-injected
  `subscribes_to` proof (session B2). This closes the authoring → manifest → runtime loop end to end.
  Reuse the verified harness; the `EventEnvelope` needs `tenant.attempt: u32` (a required field that
  dropped the first test publish last time).

## Rollout / sequencing

Two PRs, strictly ordered:

1. **greentic-types** on `origin/research`: add the field, bump to `1.3.0-research.2`, merge → CI
   publishes the crate. Wait for the publish to land on crates.io.
2. **greentic-pack** on `origin/research` (this branch `feat/subscribes-to-authoring`): add
   `FlowConfig` field + schema + `build.rs:909` + bump the pin to `=1.3.0-research.2`. Cannot compile
   until Slice 1 is published.

`bash ci/local_check.sh` must pass in each repo. No AI attribution on commits or PRs.

## Related

- greentic-start #287 — introduced `subscribes_to` on the read side, declared this authoring gap as
  follow-up debt.
- Live verification (this session) — proved the routing code honors `subscribes_to` when present
  (B2, hand-injected) and falls to default when absent (B1). Runtime needs no change.
- Designer authoring UX + a NATS publisher/deployment — the two out-of-scope pieces that turn this
  enabler into production traffic.
