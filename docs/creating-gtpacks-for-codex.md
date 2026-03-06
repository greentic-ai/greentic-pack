# Creating Different Types of GTPacks for Codex

This guide defines the Codex/replay workflow for `greentic-pack`.

## Prerequisites

Before running wizard flows that delegate to component editing, ensure
`greentic-component` is available on `PATH`.

Preferred install path:

```bash
if ! command -v greentic-component >/dev/null 2>&1; then
  cargo install cargo-binstall || true
  cargo binstall greentic-component
fi
```

If `cargo binstall` is rate-limited by the GitHub API or binstall fallback is
disabled in the environment, use:

```bash
cargo install --locked greentic-component
```

## Deterministic contract

Use AnswerDocument flow:

1. Record answers (`wizard run --dry-run --emit-answers`)
2. Validate answers (`wizard validate`)
3. Apply answers (`wizard apply`)

## Commands

Record:

```bash
greentic-pack wizard run --dry-run --emit-answers .codex/pack-wizard.answers.json
```

Validate:

```bash
greentic-pack wizard validate \
  --answers .codex/pack-wizard.answers.json \
  --emit-answers .codex/pack-wizard.answers.normalized.json
```

Apply:

```bash
greentic-pack wizard apply --answers .codex/pack-wizard.answers.normalized.json
```

## Required AnswerDocument envelope

```json
{
  "wizard_id": "greentic-pack.wizard.run",
  "schema_id": "greentic-pack.wizard.answers",
  "schema_version": "1.0.0",
  "locale": "en",
  "answers": {},
  "locks": {}
}
```

## Minimal replayable `answers` keys

- `pack_dir` (string, required)
- `create_pack_scaffold` (bool)
- `create_pack_id` (string, required when `create_pack_scaffold=true`)
- `run_delegate_flow` (bool)
- `run_delegate_component` (bool)
- `run_doctor` (bool)
- `run_build` (bool)
- `sign` (bool)
- `sign_key_path` (string, required when `sign=true`)
- optional extension replay fields:
  - `extension_operation`
  - `extension_catalog_ref`
  - `extension_type_id`
  - `extension_template_id`
  - `extension_template_qa_answers`
  - `extension_edit_answers`
- optional passthrough:
  - `flow_wizard_answers`
  - `component_wizard_answers`

## Example: create application pack deterministically

```json
{
  "wizard_id": "greentic-pack.wizard.run",
  "schema_id": "greentic-pack.wizard.answers",
  "schema_version": "1.0.0",
  "locale": "en",
  "answers": {
    "pack_dir": "./acme-weather",
    "create_pack_scaffold": true,
    "create_pack_id": "acme.weather",
    "run_delegate_flow": false,
    "run_delegate_component": false,
    "run_doctor": true,
    "run_build": true,
    "sign": false
  },
  "locks": {}
}
```

## Extension replay note

Extension create/update/add flows now emit replay-complete AnswerDocuments.

Canonical persistence stays capability-first:

- `extensions/<type>.json` stores the catalog answers and derived capability payload
- `pack.yaml` is updated through `extensions.greentic.ext.capabilities.v1`

Default catalog behavior:

- the wizard now asks `Check for a new version [Y/n]` for the default extension catalog path only
- `Enter` / `Y` uses an editable default GitHub docs URL:
  `https://github.com/greenticai/greentic-pack/blob/master/docs/extensions_capability_packs.catalog.v1.json`
- `n` uses the bundled/local default catalog ref `file://docs/extensions_capability_packs.catalog.v1.json`
- if the default GitHub URL cannot be fetched, the wizard falls back to an embedded copy bundled with the binary
- disconnected/offline environments therefore do not fail just because the docs file is absent

Extension pack scaffold baseline:

- all extension pack create/apply paths now create the same base pack structure before extension-specific template content is written
- baseline directories: `flows/`, `components/`, `i18n/`, `assets/`, `qa/`, `extensions/`
- baseline seed files: `assets/README.md`, `qa/README.md`
- catalog templates can derive scaffold file names and contents from edit answers such as
  `{{edit.component_ref}}`, in addition to template QA answers such as `{{qa.pack_id}}`
- catalog templates can now include scaffold code and other checked-in assets directly in the template plan:
  text files via `write_files` and binary artifacts via `write_binary_files`
- both `write_files` and `write_binary_files` support variable interpolation in relative paths as well as file contents, so a provider template can generate named code/assets like
  `components/{{edit.component_ref}}/component.manifest.json` and
  `components/{{edit.component_ref}}/component.wasm`
- the default catalog now also includes a scaffold-first `deployer` type with canonical key
  `greentic.deployer.v1`
- the deployer template writes placeholder flows (`generate`, `plan`, `apply`, `remove`, `status`, `rollback`),
  JSON schemas under `assets/schemas/`, a sample input under `assets/examples/`,
  and a component bundle under `components/{{edit.component_ref}}/`
- deployer persistence now writes both `extensions/deployer.json` and
  `pack.yaml -> extensions.greentic.deployer.v1.inline`
- deployer validation is generic: it checks `version`, `provides[].capability`,
  `provides[].contract`, declared ops, and any declared `flow_refs`

For CI or low-level scripted edits, `greentic-pack add-extension capability` remains the canonical direct command.

For generic deployer metadata, the direct CLI path is:

```bash
greentic-pack add-extension deployer --pack-dir <DIR> \
  --contract-id greentic.deployer.v1 \
  --op generate \
  --op plan
```

For external extension dependencies, use the editable source file flow:

```bash
greentic-pack add-extension dependency --pack-dir <DIR> \
  --id greentic.deployer.v1 \
  --role deployer \
  --ref oci://ghcr.io/greenticai/packs/deployer:0.6.0 \
  --allow-tags

greentic-pack extensions-lock --in <DIR>
```

This writes:
- `pack.extensions.json` as the human-edited logical dependency file
- `pack.extensions.lock.json` as the machine-generated pinned lock file

When both files exist, `greentic-pack lint` and `greentic-pack build` now verify
that ids, roles, and source refs still match. If you edit `pack.extensions.json`,
rerun `greentic-pack extensions-lock --in <DIR>`.

`greentic-pack doctor --in <DIR> --json` also reports stale extension lock state
as validation diagnostics (for example `PACK_EXTENSION_DEPENDENCY_LOCK_STALE`)
instead of failing before producing structured output.

In the interactive main menu this path is labeled `Add extension to existing pack`
to distinguish it from `Create extension pack`.

## i18n compliance note

When updating wizard CLI text, follow:

- [cli-i18n-codex-playbook.md](/home/vgrishkyan/greentic/greentic-i18n/docs/cli-i18n-codex-playbook.md)

Batch translation updates and run `tools/i18n.sh` to avoid high token/credit churn.
