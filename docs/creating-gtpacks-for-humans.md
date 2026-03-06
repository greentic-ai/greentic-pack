# Creating Different Types of GTPacks for Humans

This guide is the human-facing workflow for creating new `.gtpack` content with `greentic-pack`.

## Rule of thumb

Use wizard-first workflows for new packs and edits:

- `greentic-pack wizard` for interactive authoring
- `greentic-pack add-extension capability` for deterministic capability entry updates

## Prerequisites

- `greentic-pack` installed and on `PATH`
- `greentic-flow` and `greentic-component` installed (wizard delegates to both)

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
2. Enter catalog ref (`fixture://extensions.json`, `file://...`, or `oci://...`)
3. Select extension type and template
4. Enter output `pack dir`
5. Finalize (`doctor` + `build`, optional sign)

## 4) Extension pack (update existing)

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Update extension pack`
2. Enter existing `pack dir`
3. Enter catalog ref
4. Use:
   - `Edit extension entries` (writes `extensions/<type>.json` and updates `pack.yaml`)
   - `Edit flows`
   - `Add/edit components`
   - `Run update & validate`

## 5) Add extension to an existing pack

Interactive path:

```bash
greentic-pack wizard
```

Pick `Add extension`, then select type and answer questions.

Deterministic capability-first path (recommended for CI and repeatability):

```bash
greentic-pack add-extension capability --pack-dir <DIR> \
  --offer-id <ID> \
  --cap-id <CAP_ID> \
  --component-ref <COMPONENT_ID> \
  --op <OP_ID> \
  --priority 10
```

## Final validation

For any pack type:

```bash
greentic-pack lint --in <DIR>
greentic-pack resolve --in <DIR>
greentic-pack build --in <DIR> --gtpack-out <DIR>/dist/pack.gtpack
greentic-pack doctor <DIR>/dist/pack.gtpack
```
