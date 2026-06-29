# greentic-pack i18n build step — design

- **Date:** 2026-06-20
- **Status:** Approved (design)
- **Primary repo:** `greentic-pack` (`crates/packc`)
- **Secondary repo:** `greentic-designer` (forward selected languages only)

## Problem

When an operator exports a worker from greentic-designer and selects additional
languages, the exported `.gtpack` never contains the extra locale files. At
runtime greentic-start synthesises the webchat-gui locale-picker manifest
(`/v1/web/webchat/{tenant}/i18n/_manifest.json`) by listing
`assets/i18n/<code>.json` entries **inside the app `.gtpack` archive**
(`enumerate_pack_locale_codes` in `greentic-start/src/http_ingress/static_handler.rs`).
Because the archive has no extra locale files, the picker falls back to
English only and the operator's language selection appears to be ignored.

Root cause (verified): the designer's packc pipeline
(`greentic-designer/src/orchestrate/pack_via_packc/mod.rs::render_via_packc`)
receives the selected `langs` but only **logs** `inputs.langs.len()` — it never
materialises `assets/i18n/*.json` into the pack. The pack-build comments
themselves call this an unimplemented "Stage 4 i18n".

A parallel, duplicated implementation already exists in
`greentic-cards2pack/src/translate.rs` (extract → invoke
`greentic-i18n-translator` → write `assets/i18n/` + `_manifest.json`). The
designer deliberately dropped its `greentic-cards2pack` dependency, so copying
that logic into the designer would re-introduce duplication.

## Decision

Put the i18n materialisation in **`greentic-pack`** — the canonical pack
builder that the designer's packc pipeline, `greentic-cards2pack`, and direct
`gtc wizard` / `greentic-pack` CLI users all drive. The designer (and any other
caller) only forwards the selected language codes through the wizard-apply
answers. One source of truth, no per-caller duplication.

### Why not a WASM bundle extension

The chosen translation engine is the external `greentic-i18n-translator`
binary (it shells to codex-cli / an LLM). The bundle-standard extension runs
in a `wasm32-wasip2` sandbox and cannot spawn external processes, so the
translate step must run in host code. greentic-pack is a native binary and is
the shared host both pipelines already invoke.

### Translation engine & failure behaviour (agreed)

- **Engine:** call the `greentic-i18n-translator` binary, mirroring the proven
  contract in `greentic-cards2pack/src/translate.rs`.
- **Failure policy:** **non-fatal with explicit reporting.** If the translator
  is absent, no strings are found, or a language fails, the build still
  succeeds; every skip/failure is surfaced to the caller (build stderr / the
  designer wizard job progress). No silent drops. **No auto-install** (no
  `cargo binstall` at build time).

## Architecture

### greentic-pack (`crates/packc`)

All line numbers below are indicative, taken from the explored checkout
(`fix/inline-component-digest`); the implementation re-locates them against the
`develop` base.

1. **Answers schema** — `src/cli/wizard.rs::pack_wizard_answers_schema()`
   (~L879–951): add an optional
   `"langs": { "type": "array", "items": { "type": "string" } }` field.

2. **Execution plan** — `WizardExecutionPlan` (~L192–207) gains
   `i18n_langs: Vec<String>`; `execution_plan_from_answers()` (~L1347–1397)
   parses it (default empty).

3. **Injection point** — `apply_answer_document()` (~L1226–1345): after the
   delegate steps complete (~L1274) and **before** the `update` / `doctor` /
   `resolve` / `build` block (~L1275), insert:

   ```rust
   if !plan.i18n_langs.is_empty() {
       crate::i18n_build::materialize_i18n(&plan.pack_root, &plan.i18n_langs);
       // non-fatal: never returns Err to the caller
   }
   ```

   Running before `update`/`resolve`/`build` guarantees the manifest sync and
   archive assembly see the freshly written `assets/i18n/` files.

4. **New module** — `src/i18n_build.rs` (< 500 lines). Single entry point:

   ```rust
   pub fn materialize_i18n(pack_root: &Path, langs: &[String]);
   ```

   Progress and every skip/failure are reported via `eprintln!` to stderr,
   matching the `greentic-cards2pack` pattern. The designer captures
   greentic-pack's stderr through the `stderr_sink` it already passes to
   `run_pack_wizard`, so the messages surface in the wizard job progress with
   no extra plumbing.

   Internal flow:

   ```
   langs empty            -> no-op
   translator unavailable -> report("i18n: translator not found; skipped N langs: …"), return
   extract assets/cards/*.json -> assets/i18n/en.json
   no translatable strings -> report("i18n: no translatable strings"), return
   for lang in langs (minus "en", minus langs that already have assets/i18n/<lang>.json):
       run greentic-i18n-translator (per-lang temp cwd, bounded concurrency)
   write assets/i18n/_manifest.json  = ["en", ...successfully translated/carried-over]
   report("i18n: translated id, ja; failed de: <error>")
   ```

   - **Extraction** — port the focused Adaptive-Card field extractor from
     `greentic-cards2pack/src/i18n_extract` (fields: `text`, `title`,
     `placeholder`, `label`, `altText`, `value`, `errorMessage`; key
     `card.<card_id>.<json_path>.<field>`; skip values already using
     `$t(...)` / `{{i18n:…}}`). Output `en.json` is a sorted, pretty
     `BTreeMap<String,String>`.
   - **Translator contract** — exactly as cards2pack:
     `greentic-i18n-translator translate --langs <lang> --en <abs en.json> --auth-mode auto`,
     run in a per-language temp dir (`git init --quiet` so codex-cli trusts it);
     the translator writes `<lang>.json` next to `--en`, landing directly in
     `assets/i18n/`. Best-effort temp cleanup.
   - **Concurrency** — translate languages concurrently with a bound
     (`min(available_parallelism, langs.len())`), mirroring cards2pack.
   - **Carry-over wins** — a language that already ships an
     `assets/i18n/<lang>.json` is not re-translated but is still listed in the
     manifest.

5. **Binary resolution** — `src/external_tools.rs` (~L27): register
   `"greentic-i18n-translator" => &["GREENTIC_I18N_TRANSLATOR_BIN", "GREENTIC_I18N_TRANSLATOR_DEV_BIN"]`
   and resolve via the existing `resolve()` (env override → PATH → exe dir).
   Absent binary → non-fatal skip (see failure policy).

### greentic-designer (small change)

6. `src/orchestrate/pack_via_packc/mod.rs::build_pack_answers()` (the
   pack-build answers builder) includes `langs: inputs.langs` in the emitted
   answers JSON. The current log-only use of `inputs.langs` in
   `render_via_packc` is replaced by this real forwarding. No i18n module is
   added to the designer.

## Asset archiving (verified)

`greentic-pack`'s build does **not** require `pack.yaml` registration for
`assets/i18n/`:

- `build.rs::collect_extra_dir_files()` (~L969) walks every pack-root
  subdirectory except a fixed exclude list
  (`components`, `flows`, `dist`, `target`, `.git`, `.github`, `.idea`,
  `.vscode`, `node_modules`). `assets/` is **not** excluded, so
  `assets/i18n/*.json` is auto-discovered as an `ExtraFile`.
- `build.rs::map_extra_files()` (~L1044) includes any entry whose logical path
  starts with `assets/`.

**Open risk to confirm in the plan:** `is_reserved_extra_file()` must not skip
`assets/i18n/en.json` or `assets/i18n/_manifest.json`. A prior test in
`crates/packc/src/cli/inspect.rs` already asserts
`assets/i18n/_manifest.json` is *not* forbidden, which is a positive signal;
the archive test below makes it explicit.

## Existing manual flow

`docs/internationalise-pack-howto.md` documents adding `assets/i18n/<locale>.json`
**by hand**. This feature automates exactly that for the languages a caller
requests. The howto is updated to mention the automated `langs` path.

## Error handling

`materialize_i18n` never propagates an error to `apply_answer_document`. Every
failure mode becomes a reporter line. This satisfies both the global
"never ignore errors silently" rule (failures are reported) and the agreed
"success + clear warning" policy (the build is not blocked).

## Testing (TDD)

In `greentic-pack`:

1. **Unit — extraction:** sample card JSON → `en.json` has the expected keys
   and values; `$t(...)` / `{{i18n:…}}` values are skipped.
2. **Unit — manifest:** `_manifest.json` equals `["en", …translated]`; a
   carried-over language file is included; ordering is deterministic.
3. **Unit — empty langs:** `materialize_i18n(_, &[], _)` creates no
   `assets/i18n/` directory (byte-identical to today's behaviour).
4. **Unit — translator missing:** with `GREENTIC_I18N_TRANSLATOR_BIN` pointing
   at a non-existent path, the call writes no `<lang>.json` and does not panic
   (the stderr warning is verified via the integration/manual path, not by
   capturing stderr in a unit test).
5. **Integration — archive contents (crux):** stub the translator via
   `GREENTIC_I18N_TRANSLATOR_BIN` (a shell script that copies `en.json` →
   `<lang>.json`), run `wizard apply` with `langs: ["id","ja"]`, and assert the
   final `.gtpack` contains `assets/i18n/en.json`, `assets/i18n/id.json`,
   `assets/i18n/ja.json`, and `assets/i18n/_manifest.json`. This simultaneously
   proves the archiving rule and the `is_reserved_extra_file` risk.

In `greentic-designer`:

6. **Unit:** `build_pack_answers()` emits `langs` matching `inputs.langs`.

## Out of scope (follow-ups)

- Migrating `greentic-cards2pack` to delegate its translation to greentic-pack
  (removing the duplicated `translate.rs`). Tracked as a later cleanup; not
  required for this fix.
- The legacy `use_packc() == false` WASM bundle path in the designer
  (`session_adapter`) — already inactive by default; untouched here.
- Per-tenant or runtime-overlay locale merging in greentic-start (a separate,
  already-discussed overlay gap).

## Documentation updates

- `greentic-pack`: CLAUDE.md (build pipeline gains an i18n step), the
  wizard-answers reference, and `docs/internationalise-pack-howto.md`
  (automated `langs` path).
- `greentic-designer`: CLAUDE.md (packc pipeline now forwards `langs` to
  greentic-pack instead of dropping them).
- Parent workspace CLAUDE.md and greentic-docs multilanguage-i18n page: export
  now translates the selected languages into the `.gtpack`.
