# PR-01 — make the extension model explicit, with a generic deployer definition

## Goal
Make `greentic-pack` explain the pack model clearly and consistently across:
- wizard UX
- docs/help
- scaffolded files
- validation messages

This PR must make deployer extensions explicit **without** baking target-specific knowledge into `greentic-pack`.

## Scope lock (2026-03-06)

This PR must build on the wizard/catalog system that already exists in this repo.

Already implemented and therefore out of scope here:
- `greentic-pack wizard` as the canonical interactive entrypoint
- application pack vs extension pack wizard flows
- AnswerDocument replay (`run`, `validate`, `apply`)
- i18n-keyed wizard UI
- capability-first extension catalog loading and rendering

This PR is specifically about the missing product model for **deployer extensions**
inside that existing system.

## Core model to document

### Application pack
Contains application/product behavior such as:
- flows
- assets
- schemas
- local indexes/routing metadata
- app-specific capability usage

### Extension pack
Contains a pack that **offers** a platform-adjacent capability, such as:
- deployer
- control-chain
- observer/audit hook
- telemetry provider
- session/state provider
- secrets provider
- messaging/events/oauth provider

This must be documented as an extension-pack subtype within the existing
catalog-driven extension-pack UX, not as a separate top-level wizard product.

### Deployer extension
A deployer extension is a specific kind of extension pack that offers:

`greentic.deployer.v1`

Its role is to translate Greentic deployment intent into generated deployment artifacts, deployment plans, and optional lifecycle operations.

## Deployer extension definition
Add a clear product-facing definition:

> A deployer extension is a Greentic extension pack that implements the `greentic.deployer.v1` contract and converts deployment intent into deployer-defined plans, generated artifacts, and optional lifecycle operations.

## What a deployer extension is not
Make the docs/wizard explicit that a deployer extension is **not**:
- the live operator runtime
- a request-path provider
- a business logic pack
- a hard-coded part of Greentic core
- a `greentic-pack` built-in target template system

## Important architectural rule
`greentic-pack` must **not** hard-code target-specific deployer fields.

That means `greentic-pack` should not need updates when someone creates a new deployer for:
- a new orchestrator
- a new cloud platform
- a proprietary internal platform
- a future deployment target not known today

Target-specific schemas, questions, examples, and docs belong to the deployer extension pack itself.

## Required implementation direction

Add deployer support by extending the existing extension catalog model:
- add a deployer extension type in the catalog
- add deployer-focused docs/help/i18n text
- add generic deployer metadata/validation where appropriate

Do **not** introduce:
- a second wizard system
- a deployer-only top-level menu branch
- target-specific deployer schema fields in `greentic-pack`

## Required metadata model
Ensure the scaffolded metadata can express at minimum:
- capability offered
- contract id
- supported ops
- compatibility info
- optional generic extension metadata

Example conceptual metadata:

```json
{
  "version": 1,
  "provides": [
    {
      "capability": "greentic.deployer.v1",
      "contract": "greentic.deployer.v1",
      "ops": ["generate", "plan", "apply", "remove", "status", "rollback"]
    }
  ]
}
```

This is intentionally generic. It does **not** encode deployer-target-specific input fields.

The exact file format may align with the existing
`extensions.greentic.ext.capabilities.v1` path or a new generic deployer
metadata file, but this PR must make the contract visible and deterministic for
follow-on scaffold work.

## I18n requirement
Any docs, help text, wizard labels, and scaffolded user-facing strings added or changed by this PR must be i18n-ready:
- all new wizard strings must come from i18n keys
- English strings must be added to `assets/i18n/en.json` or equivalent source
- locale registry files must be updated where required
- tests/snapshots must cover localized output where such coverage already exists

## Validation updates
`doctor` or equivalent validation should verify for deployer packs:
- capability id is present
- contract id is present
- at least one op is declared where required by the chosen scaffold
- generic scaffolded operation placeholders/flows exist when the scaffold says they should

It must **not** validate deployer-target-specific schema fields.

Current repo state to preserve:
- existing capability-offer validation for control/observer/provider-like packs
- existing legacy provider-extension compatibility paths remain separate
- deployer validation must be additive and must not repurpose the legacy
  `greentic.provider-extension.v1` schema-core path as the deployer model

## Tests
- extension-type docs/help tests
- metadata validation tests
- wizard text snapshot tests
- localized string coverage tests
- deployer definition presence in scaffolded README/docs

## Acceptance note

After this PR, a follow-up engineer should no longer need to guess:
- where deployer extensions fit in the current wizard/catalog model
- what generic metadata a deployer extension must declare
- what is intentionally left to deployer-specific extension packs
