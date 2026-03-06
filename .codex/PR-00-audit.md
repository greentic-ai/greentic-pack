# PR-00 Audit — greentic-pack Greentic-X Readiness Audit

## Goal

Audit the current `greentic-pack` codebase before making any Greentic-X-related changes so that all follow-up PRs reuse the **existing** wizard, extension catalog, scaffold, doctor, build, signing, i18n, and template systems.

This PR is mandatory and must be completed before any implementation PR in this repo.

## Why this audit exists

We already know from the product direction that:

- `greentic-pack` is the correct repo for new pack creation patterns
- the deterministic AnswerDocument workflow is canonical
- extension/application creation already exists
- we do **not** want Codex to invent a second scaffolding system
- we do **not** want repo-local guesses to diverge from the actual code

Therefore this PR should inspect the real code and then update the implementation PRs in this folder with exact file paths, real commands, real abstractions, and the minimum invasive change set.

## Audit scope

### 1. Wizard entrypoints and command shapes
Audit the real CLI and wizard code for:

- main command layout
- `wizard run`, `wizard validate`, `wizard apply`
- main-menu and submenu structure
- replay/AnswerDocument normalization behavior
- any existing create/update/apply/finalize pipelines
- direct non-wizard commands such as `add-extension capability`

Document:

- exact source files
- exact structs/enums
- actual command names and flags
- where new menu items or templates are registered

### 2. Application pack vs extension pack scaffolding
Identify how the repo currently distinguishes:

- application pack creation
- extension pack creation
- add-extension-to-existing-pack flows
- template selection and catalog resolution
- baseline scaffold generation

Document:

- real pack scaffold generation flow
- how directories/files are materialized
- how baseline directories are created
- how template write plans are applied
- where replay-complete answer docs are emitted

### 3. Extension catalog mechanism
Audit the default catalog and template model:

- catalog schema
- where bundled/default/local/remote catalogs are resolved
- interpolation model
- QA answers vs edit answers
- `write_files` and `write_binary_files`
- where extension type IDs / template IDs are defined and matched

Document:

- real catalog format
- whether Greentic-X templates should be added as:
  - new template under an existing extension type
  - new extension type
  - application-pack template path
- exact extension type best suited for Greentic-X runtime packs and Greentic-X ops/contract packs

### 4. Pack manifest / extension persistence model
Audit how `pack.yaml`, extension JSON files, capabilities extensions, and any lock/build artifacts are currently managed.

Document:

- canonical persistence points
- manifest update code paths
- capability-first wiring path
- how extension metadata becomes build/runtime material

### 5. Doctor/build/finalize/signing pipeline
Inspect:

- doctor flow
- resolve/build behavior
- signing flow
- how scaffolded templates participate in doctor/build
- how warnings/errors are surfaced

Document:

- exact pipeline order
- real code paths
- tests already covering these stages

### 6. i18n requirements
Audit:

- where CLI strings live
- how translations are generated/validated
- how wizard text should be updated without violating existing i18n practices

Document:

- exact files and scripts
- whether there are golden tests or generated snapshots

### 7. Existing template patterns worth reusing
Find the closest existing template(s) to:

- capability runtime-like packs
- packs exposing a capability offer
- component-backed scaffold templates
- scaffold-only extension packs

Document the best reuse candidates.

## Deliverables

This audit PR must produce:

1. `docs/audits/greentic-x-pack-audit.md` with findings
2. updated versions of the follow-up PRs in this folder, rewritten with:
   - exact file paths
   - exact command/menu names
   - real catalog/template insertion points
   - actual manifest and schema terms used in the repo
3. a concise recommendation:
   - whether Greentic-X contract/ops/runtime packs should be modeled as:
     - application pack templates,
     - extension pack templates,
     - or a mix

## Constraints

- Do not implement Greentic-X behavior yet
- Do not add speculative new CLI trees
- Do not create a second scaffold engine
- Prefer catalog/template-driven changes over hard-coded product logic
- Keep repo behavior generic

## Exit criteria

This PR is complete only when a human can look at the audit and say:

- exactly where the implementation belongs
- exactly what existing machinery must be reused
- exactly how to avoid parallel implementations
