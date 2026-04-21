# Greentic Pack Usage Guide

This document is the lower-level workflow reference.

For a friendlier, human-first introduction, read the repository
`README.md` first.

For coding agents and deterministic wizard replay, read
`docs/coding-agents.md`.

## Canonical Source-Pack Workflow

Use this sequence for most source-pack work:

```bash
greentic-pack update --in <DIR>
greentic-pack lint --in <DIR>
greentic-pack resolve --in <DIR>
greentic-pack build --in <DIR> --gtpack-out <DIR>/dist/<PACK>.gtpack
greentic-pack doctor <DIR>/dist/<PACK>.gtpack
```

What each step does:

- `update`: refreshes pack metadata from the current source tree
- `lint`: validates the pack source
- `resolve`: writes or refreshes `pack.lock.cbor`
- `build`: produces the distributable archive
- `doctor`: inspects the result

## Install

Install the CLI:

```bash
cargo install cargo-binstall
cargo binstall greentic-pack
```

If you use wizard delegation for flows or components, also install:

```bash
cargo binstall greentic-flow
cargo binstall greentic-component
```

## Create A New Pack

Wizard-first:

```bash
greentic-pack wizard
```

Scaffold-first:

```bash
greentic-pack new hello-pack --dir ./hello-pack
cd hello-pack
greentic-pack update --in .
```

## Build A Pack

```bash
greentic-pack build --in examples/weather-demo --gtpack-out dist/weather-demo.gtpack
```

Useful build options:

- `--manifest <FILE>`: custom manifest output path
- `--gtpack-out <FILE>`: custom archive path
- `--bundle <cache|none>`: embed component runtime artifacts or keep refs only
- `--dry-run`: validate without writing outputs
- `--no-update`: skip the pre-build update pass
- `--no-extra-dirs`: only include `flows/`, `components/`, and `assets/`
- `--dev`: keep extra debugging/source artifacts in the archive

## Inspect A Pack

Inspect a built archive:

```bash
greentic-pack doctor dist/weather-demo.gtpack
```

Inspect a source tree directly:

```bash
greentic-pack doctor --in examples/weather-demo
```

Use `--json` when you need machine-readable output.

## Resolve Sidecars And `pack.lock.cbor`

Flow authoring may use `*.ygtc.resolve.json` sidecars to map flow nodes to
component sources.

Recommended sequence:

```bash
greentic-pack update --in <DIR> --strict
greentic-pack resolve --in <DIR>
```

Builds expect a valid `pack.lock.cbor`. If it is missing or stale, run
`resolve` again.

## Bundle Modes

`build` supports two main bundle strategies:

- `--bundle=cache`: embed runtime artifacts for components
- `--bundle=none`: keep component refs and digests without embedding runtime
  artifacts

Use `cache` when you want a more self-contained archive.
Use `none` when you want a refs-only pack.

## Planning

Generate a deployment plan:

```bash
greentic-pack plan dist/demo.gtpack \
  --tenant tenant-demo \
  --environment prod
```

You can also point `plan` at a source directory, and the CLI will build a
temporary archive first.

## Flow And Component Work

For human interactive work, prefer the pack wizard from the root README.

For direct lower-level work:

```bash
greentic-flow wizard ./my-pack
greentic-component wizard --project-root ./my-pack
```

After direct edits, run:

```bash
greentic-pack update --in ./my-pack
greentic-pack lint --in ./my-pack
```

## Extension Work

For new v0.6 extension authoring, see:

- `docs/extension-provider-packs-howto.md`
- `docs/pack_extensions_components.md`

For legacy compatibility paths, see:

- `docs/vision/legacy.md`
- `docs/provider_extension.md`
