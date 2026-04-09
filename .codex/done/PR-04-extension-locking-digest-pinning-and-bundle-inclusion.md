# PR-04 — extension locking, digest pinning, and deterministic bundle inclusion

## Goal
Make extension inclusion deterministic and clear, especially for deployer extensions and other external extension dependencies.

## Scope lock (2026-03-06)

This repo already has component locking via:
- `pack.lock.cbor`
- `greentic-pack resolve`
- `greentic-pack inspect-lock`

That existing lock flow is for resolved component artifacts referenced by flow
sidecars. This PR must **not** redesign or replace that machinery.

This PR is specifically about **extension dependency references** and how they
should be modeled, locked, and validated alongside the existing component lock.

## Scope
This PR is about how `greentic-pack`:
- records extension refs
- locks them to digests
- validates them
- exposes them for bundle build/use

It is not about implementing the external deployment runtime itself.

It is also not about changing the current component lock schema unless an
explicit extension-dependency linkage requires an additive extension.

## Developer-edited source file
Keep a human-editable file like:

- `pack.extensions.json`

This file may contain tag refs for author convenience.

Example conceptual structure:

```json
{
  "version": 1,
  "extensions": [
    {
      "id": "greentic.deployer.v1",
      "role": "deployer",
      "source": {
        "kind": "oci",
        "ref": "oci://ghcr.io/greenticai/packs/deployer:0.6.0",
        "allow_tags": true
      }
    }
  ]
}
```

## Machine-generated lock file
Generate a deterministic lock file like:

- `pack.lock`

Prefer aligning with the existing `pack.lock.cbor` model unless there is a
strong reason to split extension dependency locks into a separate file. If a
second file is introduced, the PR must justify why the current CBOR lock cannot
be extended cleanly.

This must record:
- digest-pinned ref
- digest
- media type
- size
- optional signature/verification metadata

## Manifest linkage
The actual pack manifest should avoid unstable tag refs and instead point to lock entries by logical id/reference.

## `greentic-pack` command responsibilities
The PR should define/clarify behavior for commands such as:

- add extension
- lock
- doctor

If a new extension-lock command is introduced, it must be clearly separated from
the current `resolve` command, whose meaning today is component/sidecar
resolution.

### add extension
- records logical dependency in editable file
- validates coarse shape
- does not pretend tag refs are production-safe

Current repo note:
- `add-extension capability` already exists, but it edits canonical extension
  data in `pack.yaml`; it is not an external extension-ref manager.
- This PR must avoid overloading that current command with incompatible
  responsibilities unless the UX is redesigned explicitly.

### lock
- resolves tag refs to digest refs
- records media type and size
- writes lockfile
- optionally records signature verification metadata if available

### doctor
- validates that manifest/logical refs and lock entries align
- validates media types
- validates required fields for deployer extension refs
- warns or errors on unpinned refs depending on mode/policy

## Deployer-specific validation
When a referenced extension is a deployer extension, validation should also confirm:
- capability id aligns
- target/ops metadata is present when available from descriptor data
- incompatible deployer contract versions are flagged early

## Bundle inclusion expectations
The docs/help should explain that bundle builders consume the digest-pinned lockfile entries to retrieve exact artifacts deterministically.

This should be written in a way that distinguishes:
- component bundle inclusion already supported today
- future extension dependency bundle inclusion introduced by this PR

## Tests
- add-extension tests
- tag-to-digest lock tests
- doctor validation tests
- deployer-reference validation tests
- snapshot tests for editable file + lockfile

## Acceptance note

This PR should leave the existing component lock flow intact and add a coherent,
deterministic model for extension dependency refs rather than creating a second
competing concept of “lock” by accident.
