# PR-02 — scaffold deployer extensions generically, with replay and i18n as first-class workflow

## Goal
Make `greentic-pack wizard` generate a useful, deterministic **generic deployer extension** scaffold.

This PR must avoid target-specific deployer field knowledge in `greentic-pack`.

## Scope lock (2026-03-06)

This PR must extend the existing extension catalog/template system.

Already implemented and therefore not to be redesigned here:
- AnswerDocument replay and apply flow
- `--dry-run`, `--emit-answers`, `wizard validate`, `wizard apply`
- i18n-keyed wizard UI
- extension catalog loading (`fixture://`, `file://`, `http(s)://`, `oci://`)
- template plans with `ensure_dir`, `write_files`, `write_binary_files`,
  `delegate`, and `run_cli`
- base extension-pack scaffold creation

This PR should add a **deployer extension type/template** to that existing
system, not invent a parallel replay or wizard layer.

## Required wizard behavior
When a user chooses:
- `Create extension pack`
- then a deployer-related extension type/template

the wizard should gather enough information to scaffold a valid generic deployer pack.

## Questions the wizard should ask
At minimum, gather:

### Identity / metadata
- pack id / name
- title/description
- version
- locale defaults where applicable

### Capability declaration
- confirm capability: `greentic.deployer.v1`
- contract id
- supported ops:
  - generate
  - plan
  - apply
  - remove
  - status
  - rollback

### Scaffold style
- generic shell only
- generic shell + placeholder op flows
- generic shell + placeholder schemas/docs

### Replay/document behavior
- emit answers
- schema version
- migration policy if supported
- dry-run / scaffold preview if supported

These requirements are already satisfied by the current wizard framework.
This PR should reuse that framework and only add deployer-specific questions
that are still generic.

## What the wizard must NOT ask
The wizard must **not** ask for deployer-target-specific fields that would require `greentic-pack` to know deployer internals.

Examples of questions that do **not** belong in `greentic-pack`:
- Helm chart values fields
- Juju relation field details
- Terraform variable shapes
- K8s ingress details
- Snap confinement-specific config fields
- provider-specific serverless deployment fields

Those belong to the deployer extension’s own schemas, flows, setup/update questions, and docs.

## Expected scaffold shape
The generated pack should remain generic, for example:

```text
pack/
  pack.cbor or pack.json template
  extension.meta.json
  README.md
  assets/
    schemas/
      deployer-input.schema.json
      deployer-plan.schema.json
      deployer-status.schema.json
    examples/
      sample-input.json
  flows/
    generate.flow.*
    plan.flow.*
    apply.flow.*
    remove.flow.*
    rollback.flow.*
    status.flow.*
  components/
    ...
  i18n/
    en.json
```

The exact files may vary by scaffold style, but the scaffold must make it obvious where a deployer author can later define:
- deployer-specific schemas
- deployer-specific questions
- deployer-specific examples
- deployer-specific operation logic

In this repo, prefer expressing this through the current catalog template plan
and existing base scaffold:
- `flows/`
- `components/`
- `i18n/`
- `assets/`
- `qa/`
- `extensions/`

Recommended generic additions for the deployer template:
- `README.md`
- placeholder deployer metadata file(s)
- placeholder schemas under `assets/` or `assets/schemas/`
- placeholder examples
- placeholder op flows or op-specific README/docs stubs

## Replay-first workflow
This PR must explicitly support:
- `emit answers`
- `replay answers`
- checked-in answer fixtures for CI
- migration/version support where applicable

Do not add new replay envelope formats unless strictly necessary. Use the
current `greentic-pack.wizard.answers` AnswerDocument path.

## I18n requirement
Any wizard updates must preserve and/or improve i18n support:
- all newly added prompts, menu labels, descriptions, and validation messages must be i18n keyed
- English strings must be added to the canonical English locale file
- locale catalogs/indexes must be updated as required by the repo
- replay fixtures used in tests should include locale coverage where practical
- there must be no new hard-coded English-only wizard text in implementation paths

## Guidance on using other wizards
The generated README/docs should instruct contributors to use:
- `greentic-flow wizard` to create/update deployer operation flows
- `greentic-component wizard` for renderers/validators/helpers
- replayed answers in all of those tools for deterministic automation

This guidance should reflect the current product wording:
- `Create extension pack`
- `Update extension pack`
- `Add extension to existing pack`

## Required repo assets
For each wizard template, check in:
- example replay answer documents
- expected scaffold snapshot(s)
- validation snapshot(s)
- localized string snapshot updates where that pattern already exists

## Tests
- create-deployer replay fixture tests
- scaffold snapshot tests
- migration/version replay tests
- localized wizard snapshot tests
- README/template guidance tests

## Acceptance note

This PR is complete when deployer scaffolding exists as a generic catalog
template inside the current extension-pack system, and no target-specific
deployer assumptions are embedded in `greentic-pack`.
