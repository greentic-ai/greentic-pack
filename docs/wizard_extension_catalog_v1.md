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
  "canonical_extension_key": "greentic.provider-extension.v1",
  "name_key": "wizard.catalog.type.messaging.name",
  "description_key": "wizard.catalog.type.messaging.description",
  "edit_questions": [ ... ],
  "templates": [ ... ]
}
```

Notes:

- `canonical_extension_key` is optional; fallback rules in the CLI are:
  - `capability-offer` -> `greentic.ext.capabilities.v1`
  - everything else -> `greentic.provider-extension.v1`
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

- `messaging` -> `greentic.provider-extension.v1`
- `events` -> `greentic.provider-extension.v1`
- `oauth` -> `greentic.provider-extension.v1`
- `mcp` -> `greentic.provider-extension.v1`
- `state` -> `greentic.provider-extension.v1`
- `telemetry` -> `greentic.provider-extension.v1`
- `secrets` -> `greentic.provider-extension.v1`
- `capability-offer` -> `greentic.ext.capabilities.v1`

## Reference catalog

Use [`extensions_capability_packs.catalog.v1.json`](./extensions_capability_packs.catalog.v1.json) as the baseline file for owner alignment and OCI catalog publishing.
