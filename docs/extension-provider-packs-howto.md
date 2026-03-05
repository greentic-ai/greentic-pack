# Extension Capability Packs Howto

## Scope

This guide covers the canonical v0.6 extension path for new packs:

- author capability offers in `extensions.greentic.ext.capabilities.v1`
- use `greentic-pack add-extension capability` for deterministic edits
- validate with `lint`, `resolve`, `build`, and `doctor`

The provider-extension/schema-core track is legacy-only. If you maintain old
deployments, see `docs/provider_extension.md` and `docs/vision/legacy.md`.

## Extension shape

```yaml
extensions:
  greentic.ext.capabilities.v1:
    kind: greentic.ext.capabilities.v1
    version: 1.0.0
    inline:
      schema_version: 1
      offers:
        - offer_id: policy.pre.10
          cap_id: greentic.cap.op_hook.pre
          version: v1
          provider:
            component_ref: policy.hook
            op: hook.evaluate
          priority: 10
          requires_setup: false
          applies_to:
            op_names: [send]
```

## Recommended workflow

1. Scaffold a pack:

```bash
greentic-pack new <PACK_ID> --dir <DIR>
```

2. Add or sync components and flows:

```bash
greentic-pack update --in <DIR>
```

3. Add capability offers:

```bash
greentic-pack add-extension capability --pack-dir <DIR> \
  --offer-id <ID> \
  --cap-id <CAP_ID> \
  --component-ref <COMPONENT_ID> \
  --op <OP_ID> \
  --priority 10
```

4. Validate and build:

```bash
greentic-pack lint --in <DIR>
greentic-pack resolve --in <DIR>
greentic-pack build --in <DIR> --gtpack-out <DIR>/dist/pack.gtpack
greentic-pack doctor <DIR>/dist/pack.gtpack
```

## Validation rules for capability offers

- `schema_version` must be `1`.
- `provider.component_ref` must reference:
  - a component id from `pack.yaml`, or
  - a lock-backed component id from `pack.lock.cbor` (build path).
- if `requires_setup: true`:
  - `setup` is required;
  - `setup.qa_ref` must be non-empty and point to an existing file in the pack.

## Wizard catalog notes

`greentic-pack wizard` can load extension catalogs (`fixture://`, `file://`,
`oci://`) for scaffolding and editing extension entries.

For new extension packs, catalog entries should resolve to:

- `canonical_extension_key = greentic.ext.capabilities.v1`

Reference files:

- `docs/wizard_extension_catalog_v1.md`
- `docs/extensions_capability_packs.catalog.v1.json`

## Legacy commands (migration only)

Do not use these for new docs/examples:

- `greentic-pack providers ...`
- `greentic-pack add-extension provider ...`

They are kept only to migrate/maintain existing provider-extension/schema-core
deployments.
