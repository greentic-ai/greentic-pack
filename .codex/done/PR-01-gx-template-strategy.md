# PR-01 Post-Audit Implementation — Greentic-X Template Strategy in greentic-pack

## Depends on

- `PR-00-audit.md`

## Goal

Add the minimum generic template/catalog support in `greentic-pack` needed to
scaffold Greentic-X-related packs by extending the **existing extension
catalog/wizard system**.

This PR must not introduce a second scaffold engine, a separate Greentic-X
wizard tree, or a parallel replay model.

## Scope lock (2026-03-06)

The current repo already has the correct machinery:

- `greentic-pack wizard`
- AnswerDocument replay via `wizard run`, `wizard validate`, `wizard apply`
- extension create/update/add flows
- bundled/local/remote extension catalog loading
- template plans with:
  - `ensure_dir`
  - `write_files`
  - `write_binary_files`
  - `{{qa.*}}` and `{{edit.*}}` interpolation
- normal finalize flow:
  - `doctor`
  - `build`
  - optional sign

Therefore this PR must be implemented by extending:

- `docs/extensions_capability_packs.catalog.v1.json`
- `crates/packc/assets/extensions_capability_packs.catalog.v1.json`
- wizard/i18n/tests/docs already used by extension templates

It must not add a new top-level scaffold framework.

## Intent

Greentic-X should be modeled as a set of reusable scaffold templates and
extension patterns, not as hard-coded repo business logic.

The repo should stay generic:

- templates are examples/patterns
- persistence uses current manifest/extension machinery
- wizard behavior remains generic
- no Greentic-X runtime engine or policy logic is added here

## Required outcome

Add generic scaffold coverage for three Greentic-X-oriented pack patterns using
the existing extension catalog path:

1. runtime capability-oriented pack scaffold
2. contract-oriented scaffold
3. ops-oriented scaffold

## Modeling rules

### A. Prefer templates over new code paths

If the desired pack shape can be expressed as:

- baseline scaffold
- generated files/assets
- existing extension persistence
- existing finalize flow

then it should be implemented as catalog templates, not new Rust control flow.

### B. Prefer existing canonical extension keys when possible

If a new scaffold only needs capability-offer style persistence, reuse:

- `greentic.ext.capabilities.v1`

Only introduce a new canonical extension key if the scaffold truly needs:

- distinct manifest persistence
- distinct validation logic
- distinct CLI/add-extension behavior

This is the same threshold used for `greentic.deployer.v1`.

### C. Do not assume application-pack templates are the primary insertion point

The rich scaffold engine currently lives in the extension catalog flow. This PR
should treat that as the default insertion point unless the audit proves a
specific scaffold belongs elsewhere.

## Expected template shapes

### 1. Runtime capability-oriented scaffold

This should likely reuse the current capability-first pattern and scaffold a
pack that can:

- carry a component reference or component bundle
- persist canonical metadata through `greentic.ext.capabilities.v1`
- optionally create a capability offer
- pass through normal `doctor` / `build` / `sign`

Preferred reuse candidates:

- existing capability-offer templates
- control-style component-backed scaffold patterns

### 2. Contract-oriented scaffold

This should start as a scaffold-first template unless the audit reveals a
distinct runtime extension schema is required.

Expected contents:

- schemas
- examples
- policy/rule/transition placeholders
- docs/readme placeholders

This should not introduce a contract engine in this PR.

### 3. Ops-oriented scaffold

This should also start as a scaffold/template problem.

Expected contents:

- op metadata placeholders
- input/output schema placeholders
- optional component bundle or delegated component path
- capability or extension wiring only if supported by current repo patterns

This should not introduce op execution logic in this PR.

## Work items

### 1. Add catalog templates

Update the real extension catalog files with the new templates and defaults.

For each scaffold define:

- extension type choice
- template id
- template QA questions
- edit questions
- generated file tree
- interpolation points
- any binary/text scaffold artifacts

### 2. Preserve deterministic replay

All new template paths must work with the existing replay-complete
AnswerDocument flow.

This PR should document and test the real answer keys added by the templates,
using the existing extension replay fields rather than inventing a new schema.

### 3. Reuse baseline scaffold behavior

Generated packs must use the current extension scaffold baseline:

- `flows/`
- `components/`
- `i18n/`
- `assets/`
- `qa/`
- `extensions/`

plus any existing seed files.

### 4. Keep finalize compatibility

Generated output must work with the existing finalize pipeline without any
special-case Greentic-X code.

### 5. Update i18n correctly

Any new wizard strings must follow the real repo i18n workflow and update the
relevant source locale and tests only as needed.

### 6. Add focused tests

At minimum:

- template selection / scaffold output tests
- replay normalization/apply tests where new answer fields matter
- finalize compatibility for baseline scaffold output
- regression tests for any new template-specific constraints

## Non-goals

- no Greentic-X runtime implementation
- no contract validation engine
- no op execution engine
- no parallel wizard tree
- no separate replay model
- no hard-coded Greentic-X business semantics in Rust

## Deliverables

- updated extension catalog assets
- tests for the new templates
- doc updates referencing the real wizard/catalog flow
- example AnswerDocument snippets only if they reflect the actual generated
  fields and shipped templates

## Success criteria

A developer can use the existing `greentic-pack wizard` extension flow to
deterministically scaffold the new pack patterns without any bespoke side
channel or secondary scaffold engine.
