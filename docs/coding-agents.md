# Coding Agents Guide

This document is for coding agents and scripted tooling.

If you are a human operating the CLI yourself, start with the repository
`README.md` instead.

## Core Rule

Do not guess wizard payloads.

Always prefer one of these sources of truth:

1. `greentic-pack wizard --schema`
2. `greentic-flow wizard --schema`
3. `greentic-component wizard --schema` or the tool's own live schema output
4. an existing checked-in AnswerDocument that already matches the current tool

This matters because wizard schemas evolve over time. Hardcoded fields copied
from old examples can silently drift.

## Recommended Agent Workflow

Use deterministic replay instead of interactive prompting whenever possible.

The normal sequence is:

1. fetch schema
2. create or update an AnswerDocument
3. validate it
4. apply it
5. run source validation and build commands

## The Three Wizard Modes

### `wizard --schema`

Use this to fetch the live schema that describes the current AnswerDocument
contract.

Example:

```bash
greentic-pack wizard --schema
```

When you already have an answers file and want the schema resolved in the
context of those answers, use:

```bash
greentic-pack wizard --schema --answers .codex/pack.answers.json
```

That is especially useful for nested delegated schemas such as
`flow_wizard_answers` and `component_wizard_answers`.

### `wizard validate`

Use this to normalize and validate an AnswerDocument without side effects.

```bash
greentic-pack wizard validate \
  --answers .codex/pack.answers.json \
  --emit-answers .codex/pack.answers.normalized.json
```

### `wizard apply`

Use this to perform the real work described in the AnswerDocument.

```bash
greentic-pack wizard apply --answers .codex/pack.answers.normalized.json
```

## AnswerDocument Envelope

The outer document must follow this shape:

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

Important:

- `answers` must be an object
- `locks` should normally be an object, even if empty
- `schema_version` must match the tool's expected version unless you are
  explicitly using `--migrate`

## Minimal Replay Keys

At minimum, most answer documents need:

- `pack_dir`

Common additional keys:

- `create_pack_scaffold`
- `create_pack_id`
- `run_delegate_flow`
- `run_delegate_component`
- `run_doctor`
- `run_build`
- `sign`
- `sign_key_path`
- `asset_staging`
- `flow_wizard_answers`
- `component_wizard_answers`
- extension replay fields such as:
  - `extension_operation`
  - `extension_catalog_ref`
  - `extension_type_id`
  - `extension_template_id`
  - `extension_template_qa_answers`
  - `extension_edit_answers`

Do not assume that this list is complete for every version. Fetch the schema.

## How To Use `wizard --schema` Correctly

### For a new pack

Fetch the pack wizard schema:

```bash
greentic-pack wizard --schema > /tmp/pack-wizard.schema.json
```

Build the AnswerDocument from that schema, then validate:

```bash
greentic-pack wizard validate \
  --answers .codex/pack.answers.json \
  --emit-answers .codex/pack.answers.normalized.json
```

### For an existing pack update

When nested wizard answers depend on an existing pack, always fetch the schema
with the pack context available:

```bash
greentic-pack wizard --schema --answers .codex/pack.answers.json
```

This lets the pack wizard embed the current delegated runtime schemas instead of
forcing you to rely on stale generic assumptions.

### For nested flow edits

If you are generating or editing `answers.flow_wizard_answers`, fetch the flow
wizard schema from `greentic-flow` directly as well:

```bash
greentic-flow wizard --schema ./my-pack > /tmp/flow-wizard.schema.json
```

If your plan already exists:

```bash
greentic-flow wizard ./my-pack \
  --answers .codex/flow.answers.json \
  --schema > /tmp/flow-wizard.schema.json
```

Use the live flow schema for:

- `routing`
- `operation`
- `in_map`
- `out_map`
- `err_map`
- step-specific `answers`

Do not reduce route objects to simple `to` hops unless the flow schema says that
is all that is allowed.

### For nested component edits

Likewise, do not invent `component_wizard_answers`. Fetch the current
component-wizard schema or the tool-specific QA schema before constructing
payloads.

## Record / Validate / Apply Workflow

If you want a safe baseline file to start from, record one first:

```bash
greentic-pack wizard run --dry-run --emit-answers .codex/pack.answers.json
```

Then:

1. edit the emitted answers file
2. validate it
3. apply it

Example:

```bash
greentic-pack wizard run --dry-run --emit-answers .codex/pack.answers.json

greentic-pack wizard validate \
  --answers .codex/pack.answers.json \
  --emit-answers .codex/pack.answers.normalized.json

greentic-pack wizard apply \
  --answers .codex/pack.answers.normalized.json
```

This is often safer than creating the first AnswerDocument completely by hand.

## Creating A New Application Pack

Minimal example:

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

Recommended command sequence:

```bash
greentic-pack wizard validate \
  --answers .codex/create-pack.json \
  --emit-answers .codex/create-pack.normalized.json

greentic-pack wizard apply \
  --answers .codex/create-pack.normalized.json
```

## Adding Or Updating Flows

There are two main approaches.

### Approach A: let the pack wizard delegate

Set:

- `run_delegate_flow: true`
- `flow_wizard_answers: { ... }`

Then apply with:

```bash
greentic-pack wizard apply --answers .codex/pack.answers.json
```

Use this when the flow work is part of a larger pack-creation or pack-update
operation.

### Approach B: call `greentic-flow` directly

Use this when you only need to edit flows and do not need the outer pack wizard.

```bash
greentic-flow wizard ./my-pack --answers .codex/flow.answers.json
```

This is often the simpler choice for flow-only changes.

### Important routing note

When building `flow_wizard_answers`, preserve route objects exactly.

Valid route arrays may include entries like:

```json
[
  {
    "condition": "response.action == \"go\"",
    "to": "next_step"
  },
  {
    "out": true
  }
]
```

Do not normalize these into a single unconditional `to` route.

## Adding Or Updating Components

If the change is pack-wide, use `component_wizard_answers` inside the outer pack
wizard AnswerDocument and set `run_delegate_component: true`.

If the work is only about components, prefer calling `greentic-component`
directly.

After component changes, run:

```bash
greentic-pack update --in ./my-pack
greentic-pack lint --in ./my-pack
```

## Asset Staging

Use `asset_staging` when the answers file should declaratively copy external
files or directories into the pack before delegated wizard steps or builds run.

Example:

```json
{
  "asset_staging": [
    {
      "source": "./assets/cards",
      "destination": "assets/cards",
      "kind": "directory",
      "recursive": true
    },
    {
      "source": "./README-snippet.md",
      "destination": "assets/docs/readme-snippet.md",
      "kind": "file",
      "overwrite": true
    }
  ]
}
```

Rules:

- relative `source` paths are resolved relative to the AnswerDocument file
- `destination` must stay inside `pack_dir`
- staging runs before delegated flow/component work and before build steps

## Building And Validating Packs

After wizard apply, use the source commands explicitly.

Recommended sequence:

```bash
greentic-pack update --in ./my-pack
greentic-pack lint --in ./my-pack
greentic-pack resolve --in ./my-pack
greentic-pack build --in ./my-pack --gtpack-out ./dist/my-pack.gtpack
greentic-pack doctor ./dist/my-pack.gtpack
```

Use:

- `update` after structural edits
- `lint` for source validation
- `resolve` for `pack.lock.cbor`
- `build` for archive creation
- `doctor` for final inspection

## Extensions

For extension pack creation and edits, the same deterministic pattern applies.

The AnswerDocument may include:

- `extension_operation`
- `extension_catalog_ref`
- `extension_type_id`
- `extension_template_id`
- `extension_template_qa_answers`
- `extension_edit_answers`

Typical sequence:

1. fetch the current pack wizard schema
2. build the outer AnswerDocument
3. validate it
4. apply it
5. run `update`, `lint`, `resolve`, `build`, and `doctor`

For direct CLI extension mutations, the canonical low-level command remains:

```bash
greentic-pack add-extension capability --pack-dir <DIR> ...
```

## What Agents Should Avoid

Avoid these common mistakes:

- inventing nested wizard payloads without fetching schema first
- copying stale examples between repos or versions
- assuming `wizard apply` is only for source validation
- assuming nested `flow_wizard_answers` can be safely “simplified”
- skipping `wizard validate` when migrating or normalizing answer documents
- skipping `update` / `resolve` / `lint` after wizard apply

## Short Reference

Fetch schema:

```bash
greentic-pack wizard --schema
```

Fetch schema in context:

```bash
greentic-pack wizard --schema --answers .codex/pack.answers.json
```

Record a baseline answers file:

```bash
greentic-pack wizard run --dry-run --emit-answers .codex/pack.answers.json
```

Validate answers:

```bash
greentic-pack wizard validate --answers .codex/pack.answers.json
```

Apply answers:

```bash
greentic-pack wizard apply --answers .codex/pack.answers.json
```

Flow-only schema:

```bash
greentic-flow wizard --schema ./my-pack
```

Build after changes:

```bash
greentic-pack update --in ./my-pack
greentic-pack lint --in ./my-pack
greentic-pack resolve --in ./my-pack
greentic-pack build --in ./my-pack --gtpack-out ./dist/my-pack.gtpack
greentic-pack doctor ./dist/my-pack.gtpack
```
