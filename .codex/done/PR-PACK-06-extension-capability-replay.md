# PR-PACK-06 — Capability-first extension wizard replay, deterministic persistence, and control-offer prompt gating

Repo: `greentic-pack`

## Why this PR exists
The current draft overstates what already works. `greentic-pack wizard` has a stable AnswerDocument path for application-pack flows, but extension-pack flows are still partially interactive and wizard-local:

- `wizard apply --answers ...` only replays application-pack scaffold/delegate/build/sign behavior in [`crates/packc/src/cli/wizard.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/src/cli/wizard.rs).
- Extension create/update flows collect catalog answers interactively, but those answers are not emitted into the AnswerDocument envelope and therefore cannot be replayed deterministically.
- Extension persistence currently writes `extensions/<type>.json` plus a synthetic inline block keyed as `greentic.wizard.<type>.v1`, which does not align with the canonical capability-first `add-extension capability` path.
- The owner-facing catalog [`docs/extensions_capability_packs.catalog.v1.json`](/projects/ai/greentic-ng/greentic-pack/docs/extensions_capability_packs.catalog.v1.json) is incomplete for several extension types, while the wizard still defaults to `fixture://extensions.json`.

## Audit summary of current code
- Catalog loading and default backfills live in [`crates/packc/src/cli/wizard_catalog.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/src/cli/wizard_catalog.rs). Empty templates/edit questions are silently synthesized by `parse_catalog_bytes`, so several catalog entries currently work only because of generic defaults.
- Interactive extension creation/update lives in [`crates/packc/src/cli/wizard.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/src/cli/wizard.rs):
  - create flow: `run_create_extension_pack`
  - update flow: `run_update_extension_pack`
  - add-extension shortcut: `run_add_extension`
  - persistence: `persist_extension_edit_answers` and `merge_extension_answers_into_pack_yaml`
- AnswerDocument replay currently captures only app-pack level keys such as `pack_dir`, `create_pack_scaffold`, delegate flags, `run_doctor`, `run_build`, and `sign`. It does not capture extension-specific catalog state.
- Existing docs already state the current limitation: [`docs/creating-gtpacks-for-codex.md`](/projects/ai/greentic-ng/greentic-pack/docs/creating-gtpacks-for-codex.md) says extension-entry determinism should stay on `add-extension capability` until the extension wizard answer schema expands.

## Objective
Upgrade extension-pack wizard flows so they are fully reproducible from emitted answers while keeping capability-first storage canonical and preserving the current user-facing menu flow unless determinism requires a change.

## Follow-up update for this PR
This PR also needs to tighten the control-extension offer path so wizard-driven
replay/build flows remain deterministic and buildable:

- In extension edit questions, when `create_offer=false`, the wizard must not
  prompt for offer-only fields:
  - `offer_id`
  - `cap_id`
  - `component_ref`
  - `op`
  - `version`
  - `priority`
  - `requires_setup`
  - `qa_ref`
  - `hook_op_names`
- When `requires_setup=false`, the wizard must skip `qa_ref`.
- `qa_ref` must only be emitted into `setup` when `requires_setup=true`.
- `create_offer=false` must remain the safe default path for `control` and
  other capability-first scaffolds until a real `components[].id` exists for
  `provider.component_ref`.

## Decisions proposed for this PR
- Default catalog: use [`docs/extensions_capability_packs.catalog.v1.json`](/projects/ai/greentic-ng/greentic-pack/docs/extensions_capability_packs.catalog.v1.json) as the wizard default.
- Keep `fixture://extensions.json` as an explicit test/dev fallback only, not the primary default.
- Control payload: generate only canonical `greentic.ext.capabilities.v1` data.
- Control scaffold baseline: default to `offers: []` so generated packs are valid and safe for `doctor` / `build`.
- Control prompt baseline: `create_offer=false` should avoid all offer-specific
  prompts and produce a clean `offers: []` scaffold.
- When the user opts into creating a control offer, generate the same offer shape used by `add-extension capability`, including:
  - `offer_id`
  - `cap_id`
  - `version`
  - `provider.component_ref`
  - `provider.op`
  - `priority`
  - `requires_setup`
  - optional `setup.qa_ref`
  - optional `applies_to.op_names`
- `observer` remains supported for now because it is already present in wizard UX; fixture and docs catalogs must be aligned immediately instead of drifting silently.
- Wizard-local inline blobs such as `greentic.wizard.<type>.v1` are transitional only and should be removed once replay-complete canonical persistence lands.
- Capability payload normalization must never emit `setup.qa_ref` unless
  `requires_setup=true`.

## Scope
### 1. Catalog completeness and alignment
- Make [`docs/extensions_capability_packs.catalog.v1.json`](/projects/ai/greentic-ng/greentic-pack/docs/extensions_capability_packs.catalog.v1.json) complete for every supported extension type:
  - meaningful `edit_questions`
  - at least one non-stub template
  - non-empty `qa_questions`
  - usable `plan`
- Add a real `control` entry with meaningful QA/edit fields and a scaffold that produces a valid pack for `doctor` and `build`.
- Make the docs catalog the default wizard catalog source.
- Keep `fixture://extensions.json` available as an explicit fallback for tests and local development, and align it with the docs catalog so type lists and semantics match.
- Keep stable type/template IDs where already present.

### 2. Extension AnswerDocument support
- Extend the wizard answer schema in [`crates/packc/src/cli/wizard.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/src/cli/wizard.rs) so extension flows emit and consume enough state for deterministic replay:
  - catalog ref
  - operation kind (`create_extension_pack`, `update_extension_pack`, `add_extension`)
  - selected extension type
  - selected template
  - template QA answers
  - edit answers
  - pack dir
  - finalize/sign toggles where applicable
- `wizard run --answers ...`, `wizard validate --answers ...`, and `wizard apply --answers ...` must all understand this extension-specific state instead of relying on interactive-only session memory.

### 3. Deterministic capability-first persistence
- Replace the current wizard-local merge behavior in `persist_extension_edit_answers` / `merge_extension_answers_into_pack_yaml` with canonical capability-first persistence.
- Generated output must remain deterministic and idempotent:
  - write/update `extensions/<type>.json`
  - merge into `pack.yaml` without clobbering unrelated extension data
  - stable key ordering
  - no duplicate offers/entries on repeated apply
- Do not introduce a second competing storage model for capability offers. Wizard-generated control/capability data must align with `greentic-pack add-extension capability`.
- Wizard output must be canonical `greentic.ext.capabilities.v1` payload plus deterministic merge behavior.
- Normalize emitted capability offers so:
  - `setup` is omitted when `requires_setup=false`
  - `qa_ref` is never persisted without `requires_setup=true`
  - `create_offer=false` yields no offer object at all and keeps `offers: []`
    valid

### 4. Replay/apply execution
- Ensure extension create/update/apply can be reconstructed from the AnswerDocument alone.
- Preserve current UX/menu shape where possible:
  - `Create extension pack`
  - `Update extension pack`
  - `Add extension`
- Keep `add-extension capability` as the canonical low-level path for CI and migration semantics. The wizard may call into shared capability persistence logic, but should not diverge from it.

### 5. Tests
- Extend coverage in:
  - [`crates/packc/tests/wizard.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/tests/wizard.rs)
  - [`crates/packc/tests/wizard_answer_document.rs`](/projects/ai/greentic-ng/greentic-pack/crates/packc/tests/wizard_answer_document.rs)
  - other focused integration tests only if needed
- Add coverage for:
  - create/update/add-extension replay for extension types, including `control`
  - emitted answers for extension flows
  - `validate` and `apply` using extension AnswerDocuments
  - deterministic writes to `extensions/<type>.json`
  - deterministic/idempotent `pack.yaml` merge behavior
  - running apply twice with no second-run diff
  - `doctor` and `build` success for generated control extension packs
  - prompt gating when `create_offer=false` so scripted wizard runs do not
    drift by asking offer-only questions
  - omission of `setup` when `requires_setup=false`
  - migration behavior for older/incomplete extension answer docs where relevant

### 6. Docs
- Update:
  - [`docs/cli.md`](/projects/ai/greentic-ng/greentic-pack/docs/cli.md)
  - [`docs/creating-gtpacks-for-codex.md`](/projects/ai/greentic-ng/greentic-pack/docs/creating-gtpacks-for-codex.md)
  - [`docs/creating-gtpacks-for-humans.md`](/projects/ai/greentic-ng/greentic-pack/docs/creating-gtpacks-for-humans.md)
- Remove or revise the current limitation note once extension replay is actually supported.
- Clarify that capability offers require an existing `components[].id` matching
  `provider.component_ref`.
- Clarify that `create_offer=false` yields a valid scaffold with `offers: []`
  and is the safe default until component wiring exists.

## Non-goals
- No large wizard UI refactor.
- No parallel storage format for capability offers.
- No new catalog transport mechanism beyond the existing `fixture://`, `file://`, and `oci://` support.
- No schema changes to `greentic.ext.capabilities.v1`.

## Implementation notes
- Prefer extracting shared capability persistence helpers instead of teaching wizard code to hand-edit `pack.yaml` independently.
- Keep fixture/test catalogs readable; avoid generating giant opaque blobs.
- Use minimal schema expansion on the AnswerDocument, but make replay complete.
- For control scaffolds, do not invent placeholder provider references that would fail schema or CI checks. The default scaffold should stay valid with empty `offers`, and any generated offer should use the exact canonical capability-offer field layout.

## Acceptance criteria
- `wizard run --emit-answers` for extension flows emits replay-complete data.
- `wizard validate --answers ...` accepts normalized extension answers and remains side-effect free.
- `wizard apply --answers ...` can recreate/update extension packs deterministically without interactive state.
- Control extension scaffolds produced by the wizard pass `doctor` and `build`.
- Control scaffolds with `create_offer=false` are buildable out of the box and
  do not prompt for offer-only fields.
- Control offer payloads do not emit `setup.qa_ref` unless `requires_setup=true`.
- Reapplying the same extension AnswerDocument is idempotent: no duplicate capability entries and no diff after the second apply.
- Docs reflect the real behavior after the implementation lands.

## Work list
1. Switch wizard defaults to the docs catalog and align fixture/docs/test catalog content around one contract.
2. Expand the wizard AnswerDocument schema for extension operations.
3. Teach validate/apply to reconstruct extension operations from stored answers.
4. Replace wizard-local extension merge logic with canonical capability-first persistence shared with `add-extension capability`.
5. Gate offer-only prompts in extension edit flow when `create_offer=false`, and
   gate `qa_ref` on `requires_setup=true`.
6. Normalize capability payload emission so `setup` is only present when valid.
7. Complete catalog entries, especially `control`, with real QA/edit questions and valid scaffold plans.
8. Add deterministic/idempotency/replay coverage for extension flows, including
   prompt gating and `requires_setup=false` emission behavior.
9. Update docs to describe the new replay contract, component-ref expectations,
   and the safe `offers: []` scaffold path.

## Maintainer note
Current `ci/local_check.sh` failure should be treated as a pre-existing branch blocker, not as part of this PR's implementation scope. In this checkout Cargo fails before normal validation because the vendored dependency path `crates/vendor/greentic-interfaces-0.4.107/Cargo.toml` is missing. Maintainers should either restore the vendor path or adjust dependency resolution on the branch before requiring green `ci/local_check.sh`.
