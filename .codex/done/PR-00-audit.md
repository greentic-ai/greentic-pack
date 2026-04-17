# PR-00-AUDIT — audit greentic-pack extension/deployer UX and legacy structure

## Goal
Audit `greentic-pack` so the follow-up PRs can make the extension model, and especially the **deployer extension** model, explicit and easy to scaffold correctly.

The objective is not to redesign the whole wizard system. It is to identify where the current implementation is:
- too vague
- too legacy
- too implicit
- too inconsistent between help text, wizard UX, and scaffold outputs

## Key design assumption
The rest of the platform capabilities already exist or are being handled elsewhere. This repo’s job is to:
- scaffold packs
- describe extension metadata
- wire deterministic replay fixtures
- lock external extension references
- validate pack structure

## Audit questions

### 1. Extension model clarity
- Where does `greentic-pack` currently define the difference between:
  - application packs
  - extension packs
  - deployer extensions
  - observer/control/provider extensions?
- Is that difference visible in:
  - wizard screens
  - docs/help
  - scaffolded metadata
  - validation code?

### 2. Deployer extension model
- Does the current wizard explain what a deployer extension actually is?
- Does the scaffold capture:
  - capability id
  - supported targets
  - supported ops
  - input/output schema placeholders
  - target-specific notes?
- Are deployer extensions currently confused with runtime/provider features?

### 3. Wizard product surface
- Is `greentic-pack wizard` clearly the canonical UX?
- Are old/legacy subcommands or code paths still present and confusing?
- Do docs and help mirror the actual wizard-first behavior?

### 4. Replay support
- Where are `AnswerDocument` fixtures emitted and replayed today?
- Are they stable enough for CI?
- Does the current scaffold encourage replay-based deterministic tests?

### 5. Locking / extension refs
- How are extension references added today?
- Are tag refs and digest refs clearly separated?
- Is there a stable lockfile model?
- Are media type and signature hooks already anticipated?

### 6. Deletion / cleanup opportunities
Identify old code or UX that should be removed because it causes confusion:
- stale command paths
- placeholder extension types/tests that should not leak into product UX
- ambiguous metadata fields
- docs that don’t explain deployer extensions properly

## Required outputs
Produce:
1. A module/command audit of wizard, extension add/lock/doctor, and scaffold generation.
2. A list of exact places where deployer extensions are under-specified.
3. A delete/refactor list for old wizard and extension UX.
4. A recommended scaffold shape for deployer extensions.
5. A recommended lock/ref-validation flow.

## Acceptance criteria
The audit is complete only when a follow-up engineer can implement the next PRs without guessing:
- what a deployer extension is
- what metadata it must declare
- what `greentic-pack` should scaffold
- how replay fixtures and lockfiles should be used
