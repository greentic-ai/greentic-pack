# Wizard Extension Catalog V1 (Capability Packs)

This document defines the baseline catalog format used by
`greentic-pack wizard` for extension packs and standardizes extension type
entries for capability-first extension packs.

## Goal

Provide one reusable catalog shape for:

- Admin
- Messaging
- Events
- OAuth
- MCP
- State
- Telemetry
- Secrets
- Control
- Observer
- Deployer
- Capability offers
- Custom scaffold

Each extension type can be owned by a team while keeping one wizard contract
and one setup/update/apply flow.

## Catalog JSON shape

Top-level:

```json
{
  "extension_types": [ ... ]
}
```

Each extension type:

```json
{
  "id": "messaging",
  "canonical_extension_key": "greentic.ext.capabilities.v1",
  "name_key": "wizard.catalog.type.messaging.name",
  "description_key": "wizard.catalog.type.messaging.description",
  "edit_questions": [ ... ],
  "templates": [ ... ]
}
```

Notes:

- `canonical_extension_key` is optional; CLI fallback is `greentic.ext.capabilities.v1`.
- Production catalogs should define meaningful `edit_questions`.
- Production catalogs should define at least one concrete template with
  `qa_questions` and a usable `plan`.
- `fixture://extensions.json` remains useful for tests/dev.
- For the default production catalog path only, the wizard now asks
  `Check for a new version [Y/n]`.
- `Enter` / `Y` offers the GitHub docs URL for
  `extensions_capability_packs.catalog.v1.json` as the default, but the user
  can overwrite it with any supported catalog ref or URL.
- `n` uses the bundled/local default catalog ref
  `file://docs/extensions_capability_packs.catalog.v1.json`.
- If the default GitHub URL cannot be fetched, the wizard falls back to the
  embedded bundled default catalog instead of failing.

## Question format

```json
{
  "id": "owner",
  "title_key": "wizard.qa.owner",
  "description_key": "wizard.qa.owner.help",
  "kind": "string",
  "default": "bima"
}
```

`kind` supports: `string`, `enum`, `boolean`, `integer`.

## Template format

```json
{
  "id": "messaging-provider-v1",
  "name_key": "wizard.catalog.template.messaging_provider_v1.name",
  "description_key": "wizard.catalog.template.messaging_provider_v1.description",
  "qa_questions": [ ... ],
  "plan": [
    { "type": "ensure_dir", "paths": ["flows", "components", "i18n", "schemas"] },
    { "type": "write_files", "files": { "README.md": "# {{qa.display_name}}\n" } }
  ]
}
```

Supported plan steps: `ensure_dir`, `write_files`, `write_binary_files`, `delegate`, `run_cli`.

Template variables:

- `{{extension_type_id}}`
- `{{extension_type_name}}`
- `{{template_id}}`
- `{{template_name}}`
- `{{canonical_extension_key}}`
- `{{qa.<question_id>}}`
- `{{edit.<question_id>}}`

`write_files` and `write_binary_files` interpolate variables in both relative
paths and file contents. That lets provider templates scaffold named files like
`components/{{edit.component_ref}}/component.manifest.json` without hardcoding
`provider`, `controller`, or similar ids in Rust.

Wizard replay/apply persists:

- selected extension type
- selected template
- template QA answers
- edit answers
- pack dir

Catalog-driven persistence remains capability-first:

- `extensions/<type>.json` stores catalog answers and derived capability data
- `pack.yaml` is updated through `extensions.greentic.ext.capabilities.v1`

Wizard create/apply also guarantees a common base extension-pack scaffold
before template-specific files are written:

- directories: `flows/`, `components/`, `i18n/`, `assets/`, `qa/`, `extensions/`
- seed files: `assets/README.md`, `qa/README.md`

## Canonical extension mapping for capability packs

- `messaging` -> `greentic.ext.capabilities.v1`
- `events` -> `greentic.ext.capabilities.v1`
- `oauth` -> `greentic.ext.capabilities.v1`
- `mcp` -> `greentic.ext.capabilities.v1`
- `state` -> `greentic.ext.capabilities.v1`
- `telemetry` -> `greentic.ext.capabilities.v1`
- `secrets` -> `greentic.ext.capabilities.v1`
- `admin` -> `greentic.ext.capabilities.v1`
- `control` -> `greentic.ext.capabilities.v1`
- `observer` -> `greentic.ext.capabilities.v1`
- `deployer` -> `greentic.deployer.v1`
- `runtime-capability` -> `greentic.ext.capabilities.v1`
- `contract` -> `greentic.ext.capabilities.v1`
- `ops` -> `greentic.ext.capabilities.v1`
- `capability-offer` -> `greentic.ext.capabilities.v1`
- `custom-scaffold` -> `greentic.ext.capabilities.v1`

## Deployer scope

The current deployer slice is scaffold-first plus generic metadata persistence:

- the default catalog now includes a `deployer` extension type
- the baseline template writes placeholder flows, schemas, examples, and a
  component bundle using `{{edit.component_ref}}`
- wizard persistence records `extensions/deployer.json` with
  `canonical_extension_key = greentic.deployer.v1`
- wizard persistence also writes `extensions.greentic.deployer.v1.inline` in
  `pack.yaml`
- lint/build validation checks generic deployer metadata and any declared flow
  refs

Extension dependency lock/pinning work remains separate. This catalog entry is
about making deployer packs explicit, replayable, and generically validated.

## Additional generic scaffold patterns

The default catalog also now includes three generic capability-first template
families intended for higher-level solution scaffolding without product-specific
Rust logic:

- `runtime-capability`
  - component-backed runtime capability scaffold
  - writes runtime schemas, example payloads, a component bundle, and can
    create a capability offer through the normal extension path
- `contract`
  - schema/rules/policy-oriented scaffold
  - writes contract assets, examples, and an optional hook component scaffold
- `ops`
  - ops metadata and execution-oriented scaffold
  - writes ops metadata, schemas, examples, a component bundle, and can create
    a capability offer through the normal extension path

## Reference catalog

Use [extensions_capability_packs.catalog.v1.json](/projects/ai/greentic-ng/greentic-pack/docs/extensions_capability_packs.catalog.v1.json)
as the baseline file for owner alignment and OCI catalog publishing.
