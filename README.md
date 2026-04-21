# Greentic Pack

`greentic-pack` helps you create, edit, validate, and ship Greentic packs.

A pack is a folder, and later a `.gtpack` file, that contains the things your
Greentic project needs:

- flows
- components
- assets
- translations
- metadata about how everything fits together

If you are not a systems programmer, that is completely fine. The normal way to
work with this tool is:

1. Create or open a pack with the wizard
2. Add or edit flows
3. Add or edit components
4. Run validation
5. Build a `.gtpack`

This README is written for humans first, especially people who are comfortable
writing product logic but do not want to reverse-engineer the CLI.

If you are a coding agent or you are scripting pack creation, do not invent
wizard payloads from this README. Use the live schema and deterministic replay
workflow in [docs/coding-agents.md](/projects/ai/greentic-ng/greentic-pack/docs/coding-agents.md).

## What A Pack Is

Think of a pack as a project folder for one Greentic capability.

Examples:

- a customer-support assistant
- a research workflow
- a provider or extension pack that adds new runtime capability

Common files and directories inside a pack:

- `pack.yaml`: the pack manifest
- `flows/`: conversation and orchestration flows, usually `.ygtc` files
- `components/`: component folders or component references
- `assets/`: cards, prompts, schemas, examples, images, and other support files
- `i18n/`: translations
- `extensions/`: extension-specific data files for extension packs

You usually do not need to hand-author everything from scratch. The wizard can
scaffold the basic structure for you.

## Install

Install the main CLI:

```bash
cargo install cargo-binstall
cargo binstall greentic-pack
```

The pack wizard can delegate to other tools. If you plan to edit flows or
components through the wizard, also install:

```bash
cargo binstall greentic-flow
cargo binstall greentic-component
```

If `cargo binstall` is not available in your environment, you can use:

```bash
cargo install --locked greentic-pack
cargo install --locked greentic-flow
cargo install --locked greentic-component
```

Check that the tools are available:

```bash
greentic-pack --help
greentic-flow --help
greentic-component --help
```

## Quick Start

Create a new pack:

```bash
greentic-pack wizard
```

Choose `Create application pack`, then:

1. enter a pack id
2. choose a directory
3. use `Edit flows` to open the flow wizard
4. use `Add/edit components` to open the component wizard
5. finish with validation and build

If you prefer a non-interactive scaffold first:

```bash
greentic-pack new acme.weather --dir ./acme-weather
cd ./acme-weather
greentic-pack update --in .
greentic-pack build --in .
```

## The Easiest Mental Model

You do not need to learn every command at once. Most day-to-day work falls into
these jobs:

- `wizard`: guided authoring
- `update`: refresh pack metadata after changing files
- `lint`: validate source pack contents
- `resolve`: write or refresh `pack.lock.cbor`
- `build`: create the distributable `.gtpack`
- `doctor`: inspect a source pack or built archive

## Typical Human Workflow

### 1. Create a new application pack

Run:

```bash
greentic-pack wizard
```

Pick:

1. `Create application pack`
2. choose a pack id like `acme.weather`
3. choose an output directory like `./acme-weather`

The wizard creates the basic pack structure for you.

### 2. Add or edit flows

From the pack wizard, choose `Edit flows`.

This opens `greentic-flow wizard` for you. Use it when you want to:

- add a new flow file
- add or update steps in a flow
- set routing between steps
- configure step inputs and outputs

If you already have a pack and want to work on flows directly:

```bash
cd ./acme-weather
greentic-flow wizard .
```

### 3. Add or edit components

From the pack wizard, choose `Add/edit components`.

This opens `greentic-component wizard`, which helps you:

- create a new component
- update component configuration
- answer component QA/setup questions
- keep component metadata aligned with the pack

If you want to run it directly:

```bash
cd ./acme-weather
greentic-component wizard --project-root .
```

### 4. Refresh pack metadata

After editing files manually, run:

```bash
greentic-pack update --in .
```

This syncs `pack.yaml` with the current `flows/` and `components/` content.

### 5. Validate before building

Run:

```bash
greentic-pack lint --in .
greentic-pack resolve --in .
```

Why both?

- `lint` checks source-level correctness
- `resolve` produces `pack.lock.cbor`, which pins resolved component sources

### 6. Build the pack

Run:

```bash
greentic-pack build --in . --gtpack-out ./dist/acme-weather.gtpack
```

This creates the distributable archive you can inspect, test, or publish.

### 7. Inspect the built result

Run:

```bash
greentic-pack doctor ./dist/acme-weather.gtpack
```

This is a good final check before sharing the pack with anyone else.

## Understanding The Main Commands

### `greentic-pack wizard`

Use this when:

- you are new to pack authoring
- you want the safest path
- you want to create or update a pack interactively
- you want the CLI to open the flow and component wizards for you

### `greentic-pack new`

Use this when:

- you already know you want a new empty scaffold
- you are comfortable filling in files yourself after scaffolding

### `greentic-pack update`

Use this after:

- adding a new flow file
- adding or removing component files
- changing pack structure by hand

### `greentic-pack lint`

Use this early and often. It is the fast “did I break the pack?” check.

### `greentic-pack resolve`

Use this when your flows or components depend on resolved references and you
need to refresh `pack.lock.cbor`.

### `greentic-pack build`

Use this when you want the actual archive that will be shared, inspected, or
deployed.

### `greentic-pack doctor`

Use this when you want a readable report about a pack or `.gtpack`.

## Common Tasks

### Create a new pack

```bash
greentic-pack wizard
```

### Open an existing pack and continue editing

```bash
greentic-pack wizard
```

Choose `Update application pack` or `Update extension pack`.

### Build a pack from a directory

```bash
greentic-pack build --in ./my-pack --gtpack-out ./dist/my-pack.gtpack
```

### Check a built archive

```bash
greentic-pack doctor ./dist/my-pack.gtpack
```

### Work directly on flows

```bash
cd ./my-pack
greentic-flow wizard .
```

### Work directly on components

```bash
cd ./my-pack
greentic-component wizard --project-root .
```

## Extension Packs

If you are creating a provider or extension pack, the wizard is still the best
starting point.

Run:

```bash
greentic-pack wizard
```

Then choose one of:

- `Create extension pack`
- `Update extension pack`
- `Add extension to existing pack`

For extension-specific background and lower-level details, see:

- [docs/extension-provider-packs-howto.md](/projects/ai/greentic-ng/greentic-pack/docs/extension-provider-packs-howto.md)
- [docs/pack_extensions_components.md](/projects/ai/greentic-ng/greentic-pack/docs/pack_extensions_components.md)

## Recommended Simple Workflow

If you want one copy-pasteable routine for ordinary work:

```bash
greentic-pack wizard
greentic-pack update --in ./my-pack
greentic-pack lint --in ./my-pack
greentic-pack resolve --in ./my-pack
greentic-pack build --in ./my-pack --gtpack-out ./dist/my-pack.gtpack
greentic-pack doctor ./dist/my-pack.gtpack
```

## Which Document Should I Read Next?

Read this README if you are:

- learning the tool
- using the wizard manually
- trying to understand the overall workflow

Read [docs/coding-agents.md](/projects/ai/greentic-ng/greentic-pack/docs/coding-agents.md) if you are:

- a coding agent
- generating `AnswerDocument` files
- using `wizard --schema`
- using `wizard run --dry-run --emit-answers`
- applying deterministic wizard replays

Read [docs/cli.md](/projects/ai/greentic-ng/greentic-pack/docs/cli.md) if you need:

- exact flags
- command reference
- lower-level behavior details

Read [docs/pack-format.md](/projects/ai/greentic-ng/greentic-pack/docs/pack-format.md) if you need:

- archive structure details
- deterministic packaging details

## Examples

Examples in this repository include:

- `examples/weather-demo`
- `examples/adaptive-mcp-oauth-demo`

A good way to learn is to inspect one example and run:

```bash
greentic-pack doctor --in examples/weather-demo
greentic-pack build --in examples/weather-demo --gtpack-out ./dist/weather-demo.gtpack
greentic-pack doctor ./dist/weather-demo.gtpack
```

## Documentation Map

- [docs/README.md](/projects/ai/greentic-ng/greentic-pack/docs/README.md): documentation index
- [docs/cli.md](/projects/ai/greentic-ng/greentic-pack/docs/cli.md): command reference
- [docs/coding-agents.md](/projects/ai/greentic-ng/greentic-pack/docs/coding-agents.md): deterministic agent workflow
- [docs/extension-provider-packs-howto.md](/projects/ai/greentic-ng/greentic-pack/docs/extension-provider-packs-howto.md): extension-specific authoring
- [docs/pack_extensions_components.md](/projects/ai/greentic-ng/greentic-pack/docs/pack_extensions_components.md): component and extension details
- [docs/publishing.md](/projects/ai/greentic-ng/greentic-pack/docs/publishing.md): release and publishing guidance

## Contributing And Security

- [SECURITY.md](/projects/ai/greentic-ng/greentic-pack/SECURITY.md)
