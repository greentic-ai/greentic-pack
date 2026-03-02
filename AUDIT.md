# greentic-pack Wizard Audit (PR-PACK-02 Stage 1)

Date: 2026-03-02  
Repo: `greentic-pack`

## Scope outcome
Audited current wizard implementation in `crates/packc` only. No code changes were made.

## Exact CLI help output
Command run:

```bash
cargo run -q -p greentic-pack --bin greentic-pack -- wizard --help
```

Output:

```text
Interactive pack wizard

Usage: greentic-pack wizard [OPTIONS]

Starts the interactive wizard main menu.
```

## A) CLI surface

### Findings
| Item | Current behavior | Files/lines |
|---|---|---|
| Wizard command path(s) | Single command `greentic-pack wizard` mapped to interactive flow; no clap subcommands under `wizard`. | `crates/packc/src/cli/mod.rs:102`, `crates/packc/src/cli/mod.rs:303`, `crates/packc/src/cli/wizard.rs:55` |
| Flags for locale | No wizard-specific locale flag; wizard receives global `--locale` from CLI and also auto-detects locale from env (`LC_ALL`, `LC_MESSAGES`, `LANG`) if unspecified. | `crates/packc/src/cli/mod.rs:56`, `crates/packc/src/cli/mod.rs:262`, `crates/packc/src/cli/wizard_i18n.rs:64`, `crates/packc/src/cli/wizard_i18n.rs:70` |
| Flags for answers import/export | None (`WizardArgs` is empty). No `--answers`, `--emit-answers`, `--schema-version`, `--migrate`. | `crates/packc/src/cli/wizard.rs:26`, `crates/packc/src/cli/wizard.rs:27`, `crates/packc/src/cli/wizard.rs:56` |
| Validate/apply split | No explicit split. Wizard performs side-effecting actions directly: scaffold/template apply/delegate, then `doctor` + `build` and optional `sign`. | `crates/packc/src/cli/wizard.rs:378`, `crates/packc/src/cli/wizard.rs:819`, `crates/packc/src/cli/wizard.rs:1180`, `crates/packc/src/cli/wizard.rs:1256` |
| Non-zero exit handling | External command failures are mostly handled in-menu (localized error + back/main-menu navigation). Spawn errors in `run_process` can bubble as fatal where `?` is used; delegate path converts failures to `false` and continues UX flow. | `crates/packc/src/cli/wizard.rs:1630`, `crates/packc/src/cli/wizard.rs:1645`, `crates/packc/src/cli/wizard.rs:1197`, `crates/packc/src/cli/wizard.rs:1284` |

### Notes
- Main menu options are interactive only: create/update app pack, create/update extension pack, exit.  
  Source: `crates/packc/src/cli/wizard.rs:746`.
- `greentic_pack` help-path parsing still recognizes `wizard new-app|new-extension|add-component` as known help path children, but these are not actual clap subcommands in current CLI surface.  
  Source: `crates/packc/bin/greentic_pack.rs:167`, `crates/packc/src/cli/mod.rs:103`.

## B) Schema + questions

### Findings
| Item | Current approach | Files/lines |
|---|---|---|
| Schema identity | No stable top-level `wizard_id`/`schema_id` envelope for wizard answers. |
| Schema versioning | Per-prompt ephemeral QA spec JSON uses `"version": "1.0.0"` only; not exposed as import/export schema contract. | `crates/packc/src/cli/wizard.rs:1337`, `crates/packc/src/cli/wizard.rs:1435`, `crates/packc/src/cli/wizard.rs:1534` |
| Question model | Catalog-driven model (`CatalogQuestion`) with kinds `string|enum|boolean|integer`; interactive prompts generated as QA spec JSON and run via `greentic_qa_lib::WizardDriver`. | `crates/packc/src/cli/wizard_catalog.rs:108`, `crates/packc/src/cli/wizard_catalog.rs:122`, `crates/packc/src/cli/wizard.rs:594`, `crates/packc/src/cli/wizard.rs:1346` |
| Validation rules | Enum must match listed choices; boolean maps menu selection to `true/false`; integer loops until `i64` parse succeeds; string accepts text/default. | `crates/packc/src/cli/wizard.rs:595`, `crates/packc/src/cli/wizard.rs:636`, `crates/packc/src/cli/wizard.rs:665`, `crates/packc/src/cli/wizard.rs:1507` |
| Defaults | Defaults are defined per question (`default` / `default_value`) and applied when input is empty or user selects back in enum flows. | `crates/packc/src/cli/wizard_catalog.rs:115`, `crates/packc/src/cli/wizard.rs:607`, `crates/packc/src/cli/wizard.rs:1527`, `crates/packc/src/cli/wizard.rs:1573` |
| i18n keys | Wizard UI text and catalog labels/questions use key-based i18n with locale fallback to `en-GB`; resolved map injected into QA driver. | `crates/packc/src/cli/wizard_i18n.rs:28`, `crates/packc/src/cli/wizard_i18n.rs:121`, `crates/packc/src/cli/wizard_catalog.rs:25`, `crates/packc/tests/fixtures/wizard/extensions.json:5` |

## C) Plan / execute / migrate

### Findings
| Item | Current approach | Files/lines |
|---|---|---|
| Plan representation | Extension scaffolding plan is data-driven via `WizardStep` sequence (`EnsureDir`, `WriteFiles`, `RunCli`, `Delegate`) in catalog templates. | `crates/packc/src/cli/wizard_catalog.rs:48`, `crates/packc/src/cli/wizard.rs:387` |
| Apply/execution | Plan is applied directly with filesystem/process side effects (`create_dir_all`, `write`, subprocess/delegate runs). | `crates/packc/src/cli/wizard.rs:385`, `crates/packc/src/cli/wizard.rs:411`, `crates/packc/src/cli/wizard.rs:425`, `crates/packc/src/cli/wizard.rs:438` |
| Validation-only path | None in wizard. “Update & validate” menu path actually executes `doctor` then `build` (side-effecting pipeline). | `crates/packc/src/cli/wizard.rs:1180`, `crates/packc/src/cli/wizard.rs:1193`, `crates/packc/src/cli/wizard.rs:1205` |
| Migration | No answer-document migration mechanism currently. |
| Locks/reproducibility | Wizard itself does not emit/consume an answer envelope or lock artifact. It relies on downstream pack commands (`doctor`, `build`) that use pack artifacts/lock behavior. | `crates/packc/src/cli/wizard.rs:1192`, `crates/packc/src/cli/wizard.rs:1203` |

### Existing non-interactive support
- No non-interactive answers import/export in wizard CLI.
- Scripted stdin is supported (used by tests/harness).
- Runtime/delegation override env vars exist:
  - `GREENTIC_PACK_WIZARD_SELF_EXE`
  - `GREENTIC_FLOW_BIN`
  - `GREENTIC_COMPONENT_BIN`
  Source: `crates/packc/src/cli/wizard.rs:73`, `crates/packc/src/cli/wizard.rs:1679`, `crates/packc/src/cli/wizard.rs:1726`.

## D) Tests

| Test | What it covers | Command |
|---|---|---|
| `tests/wizard.rs` | Main wizard behavior: navigation, locale rendering, extension catalog/template flows, delegate success/failure behavior, update/build pipeline ordering, edit-entry persistence into `extensions/*.json` + `pack.yaml`, run_cli interpolation and guard behavior. | `cargo test -p greentic-pack --test wizard` |
| `tests/wizard_cli_smoke.rs` | Spawn actual `greentic-pack wizard`, send stdin, verify exit code and menu rendering. | `cargo test -p greentic-pack --test wizard_cli_smoke` |
| `tests/wizard_guardrails.rs` | Guardrails for wizard source output/i18n key usage and fixture i18n label key presence. | `cargo test -p greentic-pack --test wizard_guardrails` |

## Constraints and compatibility requirements (for PR-PACK-03 Stage 2)
- Keep `greentic-pack wizard` interactive UX and menu contract intact unless explicitly changing CLI surface.
- Preserve locale behavior: global `--locale` plus env fallback and `en-GB` default.
- Preserve delegation resolution order and env overrides for local dev/test behavior.
- Preserve failure navigation semantics (`0) Back`, `M) Main Menu`) used throughout tests.
- Keep existing extension entry persistence behavior (`extensions/<type>.json` and inline extension merge in `pack.yaml`) unless intentionally migrated.
