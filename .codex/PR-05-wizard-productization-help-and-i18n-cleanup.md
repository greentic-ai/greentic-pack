# PR-05 — productize `greentic-pack wizard` with help cleanup and i18n-safe updates

## Goal
Make the wizard-first model the clear product surface for pack authoring, and eliminate legacy command/help ambiguity.

All wizard/help changes in this PR must be implemented in an i18n-safe way.

## Scope lock (2026-03-06)

Much of the generic wizard-first cleanup is already done in this repo:
- `greentic-pack wizard` is the main interactive entrypoint
- AnswerDocument replay is documented and implemented
- create/update application pack flows exist
- create/update extension pack flows exist
- `Add extension to existing pack` is now explicitly labeled
- wizard docs and Codex/human guides already describe the current product flow

This PR should therefore focus on the remaining **deployer-specific** product
surface and any residual help/i18n ambiguity that appears once deployer support
is added.

## Why
The current UX can become confusing if:
- `wizard` opens the main menu
- help still describes old command shapes
- deployer scaffolding appears more specialized than it really is
- new strings bypass localization

Current repo-specific confusion still remaining:
- deployer extensions are not yet represented at all in the product surface
- capability extension packs are documented, but deployer extension meaning is
  still implicit/absent
- legacy `add-extension provider` compatibility paths still exist and can be
  confused with future deployer work if not documented carefully

This PR should ensure contributors understand:
- `greentic-pack wizard` is the main interactive entrypoint
- replay/emit are first-class options
- extension/deployer authoring is a supported, documented workflow
- localization remains first-class as wizard UX evolves

## Deliverables

### 1. Help cleanup
The help output should clearly document:
- `greentic-pack wizard [OPTIONS]`
- replay/emit/dry-run options
- locale/migration/schema options if supported
- no stale or confusing subcommand descriptions unless intentionally retained and justified

Do not rewrite already-correct wizard docs/help just to restate shipped behavior.
Prefer targeted updates for deployer support and any legacy/deployer ambiguity.

### 2. Wizard docs cleanup
The wizard docs/README should include a short but concrete section:
- create application pack
- create extension pack
- create generic deployer extension
- update pack
- add extension
- replay answers in CI
- locale/i18n behavior for wizard output

The first, second, fourth, fifth, and sixth bullets are largely already present.
This PR should primarily add the missing deployer-specific material and align
terminology with the current menu/flow names.

### 3. Wizard menu clarity
Menu labels and descriptions should clearly distinguish:
- extension pack
- generic deployer capability scaffolding
- add extension ref vs create extension pack

Because the current product already distinguishes:
- `Create extension pack`
- `Update extension pack`
- `Add extension to existing pack`

this PR should only add deployer-specific menu/catalog wording if deployer is
introduced as a new extension type/template.

### 4. Product-facing deployer docs
The scaffolded README/help for a deployer extension should explain:
- what deployer extensions do
- that target-specific questions belong inside the deployer extension, not `greentic-pack`
- how to use `greentic-flow wizard` and `greentic-component wizard` to flesh them out
- how to use replay fixtures as the canonical reproducible path

### 5. I18n cleanup
Review all touched wizard/help paths and:
- replace hard-coded strings with i18n keys
- update English locale entries
- update locale registries if required
- ensure fallback behavior is explicit and tested
- keep menu/help text consistent across locales where coverage exists

Only update locales for strings actually touched by deployer/productization
changes. Do not churn unrelated locale bundles.

## Tests
- help output snapshot tests
- wizard menu snapshot tests
- locale-aware wizard snapshot tests
- replay docs/tests
- generated deployer README snapshot tests

## Acceptance note

This PR is complete when the deployer extension path feels like a first-class
part of the current wizard-first product surface, without reopening already
completed generic wizard cleanup work.
