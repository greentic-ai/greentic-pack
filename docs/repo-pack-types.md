# Repo Pack Types

Repo-oriented pack metadata declares the provider role, its capabilities, and which WIT worlds/components it implements. This data lives alongside normal pack fields (`id`, `version`, flows, templates, etc.) and remains multi-tenant friendly (no tenant identifiers in manifests).

## Manifest requirements

```yaml
packVersion: 1
repo:
  kind: scanner            # source-provider | scanner | signing | attestation | policy-engine | oci-provider | billing-provider | search-provider | recommendation-provider
  capabilities:
    scan: ["sast", "deps", "license"]   # strict key set per kind; values are freeform, non-empty strings
  bindings:
    scan:
      - package: "greentic:scan"
        world: "scanner"
        version: "1.0.0"
        component: "scanner-snyk"
        entrypoint: "scan"
```

- `packVersion: 1` is required and validated; future versions can evolve independently.
- `capabilities` uses a fixed key per kind:
  - `source-provider` → `source`
  - `scanner` → `scan`
  - `signing` → `signing`
  - `attestation` → `attestation`
  - `policy-engine` → `policy`
  - `oci-provider` → `oci`
  - `billing-provider` → `billing`
  - `search-provider` → `search`
  - `recommendation-provider` → `reco`
- Keys not associated with the declared `kind` must stay empty; values are freeform strings (documented vocab only).
- `bindings` mirror the same key-per-kind rule. Each binding is one WIT/component pairing:
  - `package` – WIT package (e.g., `greentic:scan`)
  - `world` – world name inside the package (e.g., `scanner`)
  - `version` – world/package version string (e.g., `1.0.0`)
  - `component` – component identifier inside this pack
  - `entrypoint` – exported function for the world
  - `profile` – optional profile selector
- Optional `interfaces` (top-level array) list additional worlds outside the per-kind map; they use the same package/world/version triple (no component/entrypoint required).
- Reserved/unsupported in v1: `kind: rollout-strategy` (and other Distributor-oriented kinds). These must be rejected by tooling; they are reserved for a future phase.

Top-level metadata such as `homepage`, `support`, `license`, and `vendor` are allowed (type-checked only) plus a free-form `annotations` map for everything else.

## Examples

Scanner pack:

```yaml
packVersion: 1
repo:
  kind: scanner
  capabilities:
    scan: ["sast", "deps"]
  bindings:
    scan:
      - package: "greentic:scan"
        world: "scanner"
        version: "1.0.0"
        component: "scanner-snyk"
        entrypoint: "scan"
```

Signing pack:

```yaml
packVersion: 1
repo:
  kind: signing
  capabilities:
    signing: ["kms:aws"]
  bindings:
    signing:
      - package: "greentic:signing"
        world: "signer"
        version: "1.0.0"
        component: "signer-aws-kms"
        entrypoint: "sign"
```

Billing/search/recommendation packs follow the same pattern with their respective capability keys (`billing`, `search`, `reco`) and WIT bindings.

Billing pack:

```yaml
packVersion: 1
repo:
  kind: billing-provider
  capabilities:
    billing: ["metered", "flat"]
  bindings:
    billing:
      - package: "greentic:billing"
        world: "provider"
        version: "1.0.0"
        component: "billing-generic"
        entrypoint: "serve"
```

Search pack:

```yaml
packVersion: 1
repo:
  kind: search-provider
  capabilities:
    search: ["text", "vector"]
  bindings:
    search:
      - package: "greentic:search"
        world: "searcher"
        version: "1.0.0"
        component: "search-generic"
        entrypoint: "query"
```

Recommendation pack:

```yaml
packVersion: 1
repo:
  kind: recommendation-provider
  capabilities:
    reco: ["product"]
  bindings:
    reco:
      - package: "greentic:reco"
        world: "recommender"
        version: "1.0.0"
        component: "reco-generic"
        entrypoint: "recommend"
```

## Validation surface

- `packc lint` enforces `packVersion`, known kinds, strict capability key-per-kind, and presence of at least one binding for the declared role (fields must be non-empty).
- Schemas are generated from the Rust models (`pack.v1.schema.*` and `pack.schema.v1.*`).

No provider-specific packs (Slack/Teams/etc.) live here; keep fixtures generic or fake.
