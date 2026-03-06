# Creating Different Types of GTPacks for Codex

This guide defines the Codex/replay workflow for `greentic-pack`.

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

## Coverage note (important)

Current `greentic-pack wizard` AnswerDocument apply path is strongest for application-pack scaffold + validate/build/sign replay.

For extension-entry creation/update, keep deterministic automation on CLI commands (especially `add-extension capability`) until extension-specific wizard answer schema is expanded.

## i18n compliance note

When updating wizard CLI text, follow:

- [cli-i18n-codex-playbook.md](/home/vgrishkyan/greentic/greentic-i18n/docs/cli-i18n-codex-playbook.md)

Batch translation updates and run `tools/i18n.sh` to avoid high token/credit churn.
