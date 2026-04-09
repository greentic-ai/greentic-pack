# Components Extension (OCI refs)

Use the `extensions.greentic.components` entry to declare OCI component references that should be resolved externally (for example by a distributor). This keeps the core pack schema stable while allowing packs to point at registry-hosted components without embedding them.

## Shape

```yaml
extensions:
  greentic.components:
    kind: greentic.components
    version: v1
    inline:
      refs:
        - ghcr.io/org/name@sha256:<64-hex>   # required, may be empty list
      mode: eager | lazy                     # optional, preserved only
      allow_tags: false                      # optional, preserved only
```

- `refs` must be an array of strings. By default each entry must be digest pinned (`...@sha256:<64-hex>`).
- `mode` and `allow_tags` are advisory hints for installers; `packc`/`greentic-pack` only preserve them.

## Validation rules

- Digest-pinned refs are accepted by default.
- Tag refs (`ghcr.io/org/name:tag`) are rejected unless you opt in with `--allow-oci-tags`.
- Invalid shapes (missing `refs`, non-string entries, bad digest/tag formats) fail validation with actionable errors.

## CLI flag

When building/linting/inspecting from source:

```bash
packc build --in . --allow-oci-tags       # permit tag refs in the extension
packc lint --in . --allow-oci-tags
packc inspect --in . --allow-oci-tags
```

Archives (`.gtpack`) preserve the extension exactly; no download/pull occurs. Resolution is expected to be handled by installers/distributors that understand this extension.

## Capabilities Extension (v1)

Use `extensions.greentic.ext.capabilities.v1` to declare capability offers consumed by operator/runtime capability resolution.

### Shape

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

### Validation rules

- `schema_version` must be `1`.
- Each offer `provider.component_ref` must match either:
  - an id from `components[].id` in `pack.yaml`, or
  - a component id present in `pack.lock` (lock-backed component source ids).
- If `requires_setup: true`:
  - `setup` must be present;
  - `setup.qa_ref` must be non-empty and reference an existing file under the pack root.

Notes:

- `greentic-pack build` validates against `pack.yaml` + `pack.lock`, so lock-backed `provider.component_ref` ids are accepted there.
- `greentic-pack lint` validates source shape and still expects ids declared in `pack.yaml`.

## Static Routes Extension (v1)

Use `extensions.greentic.static-routes.v1` to declare public static mounts for
assets already packaged under `assets/...`.

### Shape

```yaml
extensions:
  greentic.static-routes.v1:
    kind: greentic.static-routes.v1
    version: 1.0.0
    inline:
      version: 1
      routes:
        - id: webchat-gui
          public_path: /v1/web/webchat/{tenant}
          source_root: assets/webchat-gui
          scope:
            tenant: true
            team: false
          index_file: index.html
          spa_fallback: index.html
          cache:
            strategy: public-max-age
            max_age_seconds: 3600
          exports:
            base_url: webchat_gui_base_url
            entry_url: webchat_gui_entry_url
```

### Validation rules

- `inline.version` must be `1`.
- `routes` must not be empty.
- Route ids must be unique within the pack.
- `public_path` must start with `/v1/web/`.
- `public_path` may contain only literal segments plus `{tenant}` / `{team}`.
- `source_root` must start with `assets/` and resolve to an existing directory-backed path in the pack.
- `index_file` / `spa_fallback`, when present, must exist under `source_root`.
- `scope.team=true` is rejected when `scope.tenant=false`.
- Exported URL names must be unique across the whole pack.
- `cache.strategy` must be `none` or `public-max-age`.
- `max_age_seconds` is only valid for `public-max-age` and required there.

Notes:

- The extension declares mount metadata only; it does not package files on its own.
- Runtime-facing static surfaces live under the reserved `/v1/web/...` namespace in v1.
