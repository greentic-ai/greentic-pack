# Repo Pack Types

Repo-oriented packs describe their role, capabilities, and which WIT worlds/components they expose. This is optional and coexists with existing pack metadata.

## Schema shape

```yaml
repo:
  kind: scanner            # source-provider | scanner | signing | attestation | policy-engine | oci-provider
  capabilities:
    source: ["git", "git:github"]
    scan: ["sast", "deps", "secrets"]
    signing: ["kms:aws", "kms:azure"]
    attestation: ["slsa", "enterprise-audit"]
    policy: ["opa", "cel"]
    oci: ["registry:ecr", "registry:ghcr"]
  bindings:
    scan:
      - world: "greentic:scan/scanner"
        component_id: "scanner-snyk"
    signing:
      - world: "greentic:signing/signer"
        component_id: "signer-aws-kms"
```

- `capabilities` is a map of lists; category keys must be one of `source`, `scan`, `signing`, `attestation`, `policy`, `oci`. Values are non-empty strings; no rigid taxonomy enforced in code.
- `bindings` is a map keyed by the same categories. Each entry lists bindings with explicit `world` (e.g., `greentic:scan/scanner`) and `component_id` (the component inside this pack). An optional `profile` may be included if needed.
- Validation requires at least one capability and one binding for the declared `kind`.

## Examples

Scanner pack:

```yaml
repo:
  kind: scanner
  capabilities:
    scan: ["sast", "deps"]
  bindings:
    scan:
      - world: "greentic:scan/scanner"
        component_id: "scanner-snyk"
```

Signing pack:

```yaml
repo:
  kind: signing
  capabilities:
    signing: ["kms:aws"]
  bindings:
    signing:
      - world: "greentic:signing/signer"
        component_id: "signer-aws-kms"
```

Source-provider pack:

```yaml
repo:
  kind: source-provider
  capabilities:
    source: ["git"]
  bindings:
    source:
      - world: "greentic:source/source-sync"
        component_id: "source-git-main"
```

## Validation surface

- `packc lint` enforces known kinds, capability shape, and binding presence (world + component_id) for the declared role.
- Schemas are generated from the Rust models (`pack.v1.schema.*`).

No provider-specific packs (Slack/Teams/etc.) live here; keep fixtures generic or fake.
