# PR-02 Post-Audit Implementation — Greentic-X Docs and Replay Examples

## Depends on

- `PR-00-audit.md`
- `PR-01-gx-template-strategy.md`

## Goal

Document exactly how Codex and humans should create the new Greentic-X-related
pack patterns using the **existing** `greentic-pack` deterministic workflow and
the actual catalog/templates shipped by PR-01.

This PR is documentation and replay-example work only. It must not invent new
commands, flags, or alternate workflows.

## Scope lock (2026-03-06)

The current repo already has the canonical docs surfaces:

- `docs/creating-gtpacks-for-codex.md`
- `docs/creating-gtpacks-for-humans.md`
- `docs/cli.md`
- `docs/wizard_extension_catalog_v1.md`

The current canonical workflow is:

1. `greentic-pack wizard run --dry-run --emit-answers`
2. `greentic-pack wizard validate`
3. `greentic-pack wizard apply`

Therefore this PR should update the existing docs above unless PR-01 introduces
a narrow need for one additional doc file.

## Required content

Document, using the real shipped menu/template names from PR-01:

1. how to scaffold the runtime capability-oriented pack pattern
2. how to scaffold the contract-oriented scaffold
3. how to scaffold the ops-oriented scaffold

## Required doc topics

### 1. Prerequisites

Reuse the real prerequisites already documented for the wizard/finalize path,
including:

- `greentic-component` availability when relevant
- any other tool prerequisite actually required by the new templates

### 2. Deterministic workflow

For each shipped template, document the real command sequence:

- `wizard run --dry-run --emit-answers`
- `wizard validate`
- `wizard apply`

Do not document speculative shortcuts or unpublished flags.

### 3. Real AnswerDocument examples

Provide replay-safe examples only for the templates that actually ship in
PR-01.

These examples must use the real extension replay fields already supported by
the wizard, for example:

- `extension_operation`
- `extension_catalog_ref`
- `extension_type_id`
- `extension_template_id`
- `extension_template_qa_answers`
- `extension_edit_answers`

Do not invent a Greentic-X-specific replay schema.

### 4. Resulting scaffold layout

Show the actual generated directories/files for each template family, including:

- baseline scaffold directories
- template-specific files/assets
- extension sidecar files if applicable
- any canonical `pack.yaml` extension persistence that the template creates

### 5. Finalize path

Explain how the generated scaffold goes through the existing finalize flow:

- `doctor`
- `build`
- optional sign

If a template intentionally leaves placeholders that still compile but require
manual completion, say that explicitly.

### 6. Catalog/template selection guidance

Explain which wizard menu item / extension type / template to pick and why,
using the real interactive names.

## Preferred files to update

Default target files:

- `docs/creating-gtpacks-for-codex.md`
- `docs/creating-gtpacks-for-humans.md`
- `docs/cli.md`
- `docs/wizard_extension_catalog_v1.md`

Optional:

- example answer JSON snippets inline in docs
- checked-in example JSON only if it matches existing repo conventions and is
  referenced by docs/tests

## Constraints

- use only real command names already present in the repo
- document only templates that actually ship
- keep examples aligned with actual generated output
- do not create a parallel “Greentic-X workflow” if it is just the normal
  extension wizard path

## Non-goals

- no new CLI behavior
- no speculative docs for future template ideas that are not implemented
- no separate Codex-only scaffold engine documentation

## Success criteria

Both Codex and a human developer can follow the updated docs and deterministically
produce the same scaffold outputs for the newly added template families.
