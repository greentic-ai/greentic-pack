# Wizard Extension Catalog V1 (Capability Packs)

This document defines the baseline catalog format used by `greentic-pack wizard` for extension packs and standardizes extension type entries for provider capability packs.

## Goal

Provide one reusable catalog shape for:

- Messaging
- Events
- OAuth
- MCP
- State
- Telemetry
- Secrets
- Capability offers

Each extension type can be owned by a provider team while keeping one wizard contract and one setup/update flow.

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
- `templates` is optional; if empty, wizard injects a default scaffold template.
- `edit_questions` is optional; if empty, wizard injects default `entry_label`.

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

Supported plan steps: `ensure_dir`, `write_files`, `delegate`, `run_cli`.

Template variables:

- `{{extension_type_id}}`
- `{{extension_type_name}}`
- `{{template_id}}`
- `{{template_name}}`
- `{{canonical_extension_key}}`
- `{{qa.<question_id>}}`

## Canonical extension mapping for capability packs

- `messaging` -> `greentic.ext.capabilities.v1`
- `events` -> `greentic.ext.capabilities.v1`
- `oauth` -> `greentic.ext.capabilities.v1`
- `mcp` -> `greentic.ext.capabilities.v1`
- `state` -> `greentic.ext.capabilities.v1`
- `telemetry` -> `greentic.ext.capabilities.v1`
- `secrets` -> `greentic.ext.capabilities.v1`
- `capability-offer` -> `greentic.ext.capabilities.v1`

## Reference catalog

Use [`extensions_capability_packs.catalog.v1.json`](./extensions_capability_packs.catalog.v1.json) as the baseline file for owner alignment and OCI catalog publishing.
