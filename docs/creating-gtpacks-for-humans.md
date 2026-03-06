# Creating Different Types of GTPacks for Humans

This guide is the human-facing workflow for creating new `.gtpack` content with `greentic-pack`.

## Rule of thumb

Use wizard-first workflows for new packs and edits:

- `greentic-pack wizard` for interactive authoring
- `greentic-pack add-extension capability` for deterministic capability entry updates

## Prerequisites

- `greentic-pack` installed and on `PATH`
- `greentic-flow` and `greentic-component` installed (wizard delegates to both)

If `greentic-component` is missing, install it with:

```bash
cargo install cargo-binstall   # run once
cargo binstall greentic-component
```

If GitHub API rate limits block `cargo binstall`, use:

```bash
cargo install --locked greentic-component
```

## 1) Application pack (new)

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Create application pack`
2. Enter `pack id` and `pack dir`
3. In setup menu, use:
   - `Edit flows` (delegates to `greentic-flow wizard`)
   - `Add/edit components` (delegates to `greentic-component wizard`)
   - `Finalize` (`doctor` + `build`, optional sign)

## 2) Application pack (update)

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Update application pack`
2. Enter existing `pack dir`
3. Use:
   - `Edit flows`
   - `Add/edit components`
   - `Run update & validate` (`doctor` + `build`)
   - `Sign` (optional)

## 3) Extension pack (new from catalog)

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Create extension pack`
2. Answer `Check for a new version [Y/n]`
   - `Enter` / `Y`: wizard offers the GitHub docs URL for `extensions_capability_packs.catalog.v1.json` as the default and lets you overwrite it
   - `n`: wizard uses the bundled/local default catalog (`file://docs/extensions_capability_packs.catalog.v1.json`)
   - you can also paste an explicit catalog ref directly at that prompt (`fixture://...`, `file://...`, `oci://...`, or `https://...`)
   - if the default GitHub URL cannot be fetched, the wizard falls back to the bundled default catalog instead of failing
3. Select extension type and template
4. Enter output `pack dir`
5. Answer template and extension-entry questions
6. Finalize (`doctor` + `build`, optional sign)

New extension packs always start from the same base scaffold:

- directories: `flows/`, `components/`, `i18n/`, `assets/`, `qa/`, `extensions/`
- seed files: `assets/README.md`, `qa/README.md`

The selected extension template then adds its own `pack.yaml`, README, and any extra files on top of that base.

Templates can also use edit answers such as `component_ref` in scaffold paths
and contents. That means a control template can generate
`components/controller/...` while another provider can generate
`components/provider/...` without `greentic-pack` hardcoding those names.

## 4) Extension pack (update existing)

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Update extension pack`
2. Enter existing `pack dir`
3. Answer the same catalog prompt flow as create-extension (`Y/n`, editable GitHub URL, or explicit ref)
4. Use:
   - `Edit extension entries` (writes `extensions/<type>.json` and updates canonical `extensions.greentic.ext.capabilities.v1` data in `pack.yaml`)
   - `Edit flows`
   - `Add/edit components`
   - `Run update & validate`

## 5) Add extension to an existing pack

Interactive path:

```bash
greentic-pack wizard
```

Pick `Add extension to existing pack`, then select type and answer questions.

Deterministic capability-first path (recommended for CI and repeatability):

```bash
greentic-pack add-extension capability --pack-dir <DIR> \
  --offer-id <ID> \
  --cap-id <CAP_ID> \
  --component-ref <COMPONENT_ID> \
  --op <OP_ID> \
  --priority 10
```

The default extension catalog also includes a generic `Deployer` scaffold. That
path creates placeholder flows, schemas, examples, and a component bundle, and
persists generic deployer metadata in both `extensions/deployer.json` and
`pack.yaml -> extensions.greentic.deployer.v1`.

Direct deployer metadata path:

```bash
greentic-pack add-extension deployer --pack-dir <DIR> \
  --contract-id greentic.deployer.v1 \
  --op generate \
  --op plan
```

External extension dependency path:

```bash
greentic-pack add-extension dependency --pack-dir <DIR> \
  --id greentic.deployer.v1 \
  --role deployer \
  --ref oci://ghcr.io/greenticai/packs/deployer:0.6.0 \
  --allow-tags

greentic-pack extensions-lock --in <DIR>
```

This keeps author-edited refs in `pack.extensions.json` and writes pinned lock
entries to `pack.extensions.lock.json`.

If you change `pack.extensions.json` later, rerun `greentic-pack extensions-lock`
before linting or building. Those commands now reject drift between the editable
source file and the generated lock file.

`greentic-pack doctor --in <DIR>` now reports the same drift as pack validation
diagnostics, so CI and local checks can surface the problem without a separate
manual diff step.

## Final validation

For any pack type:

```bash
greentic-pack lint --in <DIR>
greentic-pack resolve --in <DIR>
greentic-pack build --in <DIR> --gtpack-out <DIR>/dist/pack.gtpack
greentic-pack doctor <DIR>/dist/pack.gtpack
```
