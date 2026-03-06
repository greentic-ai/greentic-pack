# greentic-pack: Stage 2 — Surgical Update (apply audit inputs)

    **Pre-req:** Stage 1 audit completed and its findings copied into the “Audit Inputs” section below.

    ## Objective
    Implement the minimal set of changes to support:
    - Stable AnswerDocument import/export (envelope)
    - Schema identity + version (`schema_id`, `schema_version`) for this wizard
    - Non-interactive execution via `--answers <file>`
    - Optional migration via `--migrate`
    - i18n keys (schema uses keys; labels resolved by locale)
    - Preserve/alias existing CLI paths where required

    ## Repo description
    Pack-level orchestrator wizard; composes sub-wizards; emits nested AnswerDocuments

    ## Audit Inputs (paste from Stage 1)
    Fill these **before coding**:

    - Wizard command path(s): `crates/packc/src/cli/mod.rs:102`, `crates/packc/src/cli/mod.rs:303`, `crates/packc/src/cli/wizard.rs:55`
    - Current flags (locale/answers): Global `--locale` is supported and forwarded to wizard (`crates/packc/src/cli/mod.rs:56`, `crates/packc/src/cli/mod.rs:262`). `WizardArgs` is currently empty (no `--answers`, `--emit-answers`, `--schema-version`, `--migrate`) (`crates/packc/src/cli/wizard.rs:26`).
    - Schema location/model: Dynamic per-prompt QA spec JSON generated in `wizard.rs` and executed via `greentic_qa_lib::WizardDriver`; catalog question model in `wizard_catalog.rs` (`crates/packc/src/cli/wizard.rs:1321`, `crates/packc/src/cli/wizard.rs:1516`, `crates/packc/src/cli/wizard_catalog.rs:108`).
    - Execution model (plan/apply): Side-effecting direct execution (filesystem writes, delegate/process runs), with update/validate pipeline calling `doctor` then `build` and optional `sign` (`crates/packc/src/cli/wizard.rs:378`, `crates/packc/src/cli/wizard.rs:1180`, `crates/packc/src/cli/wizard.rs:1256`).
    - Tests to update/add: `crates/packc/tests/wizard.rs`, `crates/packc/tests/wizard_cli_smoke.rs`, `crates/packc/tests/wizard_guardrails.rs`

    ## Proposed changes (minimal)
    ### 1) Add/standardize AnswerDocument envelope
    - Implement (or adapt) a small struct matching:
      - `wizard_id`, `schema_id`, `schema_version`, `locale`, `answers`, `locks`
    - Ensure read/write JSON is stable and documented.
    - Do **not** centralize in a shared repo unless later desired.

    ### 2) CLI flags + semantics (surgical)
    Implement/alias:
    - `--answers <FILE>`: load AnswerDocument; run non-interactive validate/apply
    - `--emit-answers <FILE>`: write AnswerDocument produced (interactive or merged)
    - `--schema-version <VER>`: pin version for interactive mode
    - `--migrate`: if AnswerDocument version older, migrate (and optionally re-emit)

    **Compatibility rule:** if existing flags already exist, keep them and add aliases.

    ### 3) Schema identity + versioning
    - Define stable identifiers:
      - `wizard_id`: e.g. `greentic-pack.wizard.<purpose>`
      - `schema_id`: e.g. `greentic-pack.<purpose>`
      - `schema_version`: start at `1.0.0` unless audit shows existing versioning
    - Ensure interactive renders and validators emit these IDs/versions into AnswerDocument.

    ### 4) Validate vs apply split
    Prefer separate subcommands:
    - `wizard validate --answers ...`
    - `wizard apply --answers ...`
    If existing model differs, adapt with minimal surface changes but keep semantics.

    ### 5) Migration
    - If breaking changes are present or anticipated, add a migration function:
      - input: old AnswerDocument
      - output: new AnswerDocument
    - If no breaking change yet, implement a stub that returns identity but wires the mechanism.

    ### 6) i18n wiring
    - Ensure schema/question definitions use i18n keys
    - Ensure `--locale` controls resolution only (answers stay stable)

    ## Acceptance criteria
    - [x] `wizard run` interactive still works
    - [x] `wizard validate --answers answers.json` works (no side effects)
    - [x] `wizard apply --answers answers.json` works (side effects)
    - [x] `wizard run --emit-answers out.json` produces AnswerDocument with correct ids/versions
    - [x] `wizard validate --answers old.json --migrate` succeeds (if old version) and can re-emit migrated doc
    - [x] Tests updated/added per audit notes, minimal and focused

    Evidence:
    - Interactive wizard compatibility: `crates/packc/tests/wizard.rs`, `crates/packc/tests/wizard_cli_smoke.rs`
    - Validate/apply/emit/migrate paths: `crates/packc/tests/wizard_answer_document.rs`
    - i18n/guardrails unchanged and passing: `crates/packc/tests/wizard_guardrails.rs`
    - Help routing for new surface:
      - `greentic-pack help wizard run`
      - `greentic-pack help wizard validate`
      - `greentic-pack help wizard apply`

    ## Implementation notes (apply from audit)
    - **Files to touch (from audit):**
      - `crates/packc/src/cli/wizard.rs`
      - `crates/packc/src/cli/mod.rs` (if adding wizard subcommands to CLI surface)
      - `crates/packc/src/cli/wizard_catalog.rs` (if catalog answer schema wiring changes)
      - `crates/packc/src/cli/wizard_i18n.rs` (if locale/resolution wiring needs small adjustments)
      - `docs/cli.md`
      - `crates/packc/i18n/en.json` (and locale bundles only if new help/i18n keys are added)
    - **Tests to touch/add (from audit):**
      - Update `crates/packc/tests/wizard.rs` for new validate/apply and answers paths
      - Update/add smoke coverage in `crates/packc/tests/wizard_cli_smoke.rs`
      - Update guardrails in `crates/packc/tests/wizard_guardrails.rs` if new wizard keys are introduced
      - Add focused tests for:
        - `--answers` non-interactive load path
        - `--emit-answers` envelope output
        - `--migrate` old->current document handling
        - validate vs apply side-effect split

    ## Work list (no implementation yet)
    1. Define wizard AnswerDocument contract for pack wizard:
       - pick stable `wizard_id` and `schema_id`
       - set initial `schema_version` and migration policy baseline
       - document fields and sort/serialization expectations
    2. Decide CLI shape with minimal churn:
       - either add `wizard run|validate|apply` subcommands
       - or preserve `wizard` default entrypoint and add compatibility aliases
       - ensure current interactive flow remains default-compatible
    3. Map current side-effecting flows into explicit validate/apply boundaries:
       - identify which actions are read-only validation
       - identify which actions are mutating/apply
    4. Design answers lifecycle:
       - import path: `--answers <FILE>`
       - export path: `--emit-answers <FILE>`
       - merge precedence between interactive input and imported answers
    5. Design migration hook:
       - implement version gate behavior for older docs
       - define `--migrate` semantics for validate/apply modes
    6. Finalize i18n invariants:
       - answers remain stable IDs/values
       - locale only affects rendered labels/prompts/help text
    7. Test plan and acceptance mapping:
       - map each acceptance criterion to concrete test(s)
       - identify which existing tests to adjust vs keep unchanged
    8. Docs/update plan:
       - update `docs/cli.md` with new wizard semantics
       - update help/i18n keys required by new CLI surface

    ## Risk controls
    - No large refactors; keep changes localized
    - Preserve existing UX defaults
    - Avoid schema mega-merges; keep nested docs for composition

    ## Common target behavior (all repos)

**Goal:** Standardize wizard execution and portability via a stable AnswerDocument envelope and consistent CLI semantics, while keeping schema ownership local to each wizard.

### AnswerDocument envelope (portable JSON)
```json
{
  "wizard_id": "greentic.pack.wizard.new",
  "schema_id": "greentic.pack.new",
  "schema_version": "1.1.0",
  "locale": "en-GB",
  "answers": { "...": "..." },
  "locks": { "...": "..." }
}
```

### Required CLI semantics
All wizards should converge on these flags and semantics (names can vary only if you provide compatibility aliases):
- `--locale <LOCALE>`: affects i18n rendering only; **answers must remain stable IDs/values**
- `--answers <FILE>`: non-interactive input (AnswerDocument)
- `--emit-answers <FILE>`: write AnswerDocument produced (interactive or merged)
- `--schema-version <VER>`: pin schema version used for interactive rendering/validation
- `--migrate`: allow automatic migration of older AnswerDocuments (including nested ones where applicable)
- Separate `validate` vs `apply` paths (subcommands or flags), recommended:
  - `wizard validate --answers ...`
  - `wizard apply --answers ...`

### Versioning rules
- Patch/minor: backwards compatible additions (defaults) only
- Major: breaking changes require migration logic
- Avoid flattening composed schemas into one mega-schema; prefer nested AnswerDocuments for composed flows.

### i18n rules
- Schema uses i18n keys; runtime resolves by locale
- Answers never depend on localized labels; only stable values/IDs

Date: 2026-03-02
