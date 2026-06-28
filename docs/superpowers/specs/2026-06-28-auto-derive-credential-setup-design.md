# Auto-Derive Credential Setup Form — Design Spec

**Date:** 2026-06-28
**Status:** Approved (brainstorming complete)
**Primary repo:** `greentic-pack` (`crates/packc`)
**Touches:** `greentic-pack` (new code), `greentic-demo` (remove hand-authored asset), `greentic-llm` (read-only, drift-test reference)

## Context

When `greentic-pack` builds an **application** pack that declares `agents:`, the
operator-facing credential form (rendered by `gtc setup` / `greentic-setup`) is
driven by two asset files inside the pack:

- `assets/setup.yaml` — the question spec (LLM key, tool keys)
- `assets/secret-requirements.json` — the list of secret keys the pack needs

Today **both files are hand-authored** and injected via the build-answer
`pack_overlay` (see `greentic-demo/crates/agentic-research-tavily-demo/`). The
machine-readable source of truth the form *should* derive from already exists and
is correct:

- `pack.yaml` `agents.<id>.llm.{provider, credential_ref}` — the LLM credential
- `pack.yaml` `agents.<id>.tools[].{extension_id, tool_name}` — the tools used
- each tool extension's `describe.json` →
  `contributions.tools[].secret_requirements[]` (`{key, description, format,
  required}`) — the tool secrets

But no code turns that source into the setup form; a human re-types it per demo.
This means a pack published from the designer does **not** automatically carry
its credential questions — the demo is not faithful to the designer pipeline.

**Goal:** during pack build, auto-derive `assets/setup.yaml` +
`assets/secret-requirements.json` from `agents[].llm` and the tool extensions
each agent uses, so any application pack (CLI- or designer-built) carries its own
credential form with no hand-authoring.

## Decisions (from brainstorming)

1. **Scope:** full — derive both the LLM credential question and every tool
   secret question.
2. **Override:** auto-derive is the default; a hand-authored `assets/setup.yaml`
   present in the pack source **wins** (generator skips when it is present).
3. **LLM metadata source:** derive the question for **every** provider from
   `agent.llm.provider` (always present in `pack.yaml`); enrich popular providers
   with a small static overlay (`docs_url`, `placeholder`, pretty label); a CI
   drift-test cross-checks `greentic-llm`'s `ProviderKind::all()` so a provider
   that the overlay does not recognise is caught. No heavy runtime dependency on
   `greentic-llm`.
4. **Tool metadata source:** the tool extension's `describe.json`
   `secret_requirements` (`key`/`description`/`format`/`required`). No change to
   the `describe` schema.

## Architecture

### Where it hooks

New module `crates/packc/src/setup_gen.rs`, called from
`crates/packc/src/build.rs::run()` **after `assemble_manifest()` and before
`package_gtpack()`** — the same seam the existing
`write_secret_requirements_file()` / `aggregate_secret_requirements()` pattern
(`build.rs`) uses for component secret requirements. Generated files are written
to `pack_root/.packc/` and pushed onto `build.assets` as `AssetFile { logical_path,
source }`; `package_gtpack()` prefixes `assets/` when zipping. Running here means
both the CLI (`gtc wizard` / build) and the designer (which shells to
greentic-pack/bundle) get the behaviour with no extra wiring.

### Data flow

```
pack.yaml agents{}  (BTreeMap<String, serde_json::Value> in PackConfig)
  ├─ agent.llm.{provider, credential_ref}
  │        └─► LLM question  (group: "LLM")   + requirement key  llm/<credential_ref>
  └─ agent.tools[].{extension_id, tool_name}
           └─ resolve <extension_id> → .gtxpack  (extensions.lock.json / store)
                 └─ describe.json contributions.tools[ where tool_name matches ]
                       .secret_requirements[] {key, description, format, required}
                          └─► Tool question  (group: "Tools")  + requirement key = <key>
  ► dedupe questions + requirements by secret key   (Tavily declares api_key on
    both tavily_search and tavily_extract → one question, one requirement)
  ► emit assets/setup.yaml  +  assets/secret-requirements.json
```

### Units

- `setup_gen::generate(config, resolved_extensions) -> GeneratedSetup` — pure
  function over already-parsed inputs (the agents JSON + a map of
  `extension_id -> Vec<ToolSecretRequirement>` resolved by the caller). Returns
  `{ setup_yaml: String, secret_requirements_json: String }` or `Err` on an
  unresolvable declared tool extension. Pure → unit-testable without a build.
- `setup_gen::resolve_tool_secret_requirements(extension_id, tool_names, resolver)
  -> Result<Vec<ToolSecretRequirement>>` — reads the extension's `describe.json`
  (via the existing ext-resolver path that already loads `.gtxpack` /
  `component.json`) and extracts `contributions.tools[tool_name].secret_requirements`.
  Parsed with a **minimal local serde struct** (only `contributions.tools[].{name,
  secret_requirements[]}`), not the full `greentic-extension-sdk-contract` type —
  consistent with how packc already reads `component.json` ad hoc, and avoids
  coupling to the SDK contract's version churn.
- `setup_gen::llm_overlay(provider: &str) -> Option<ProviderOverlay>` — static
  table of `{ label, docs_url, placeholder }` for popular providers
  (openai/anthropic/deepseek/gemini/cohere/groq/perplexity/xai/mistral/…).
  Unknown provider → `None` → minimal-but-valid question.

### Output schemas (must match the consumers exactly)

`assets/setup.yaml` deserializes into `greentic-setup`'s
`SetupSpec { title?, description?, questions: Vec<SetupQuestion> }`
(`greentic-setup/src/setup_input.rs`). Each question:

```yaml
- name: <string>           # → secret URI final segment (canonicalised)
  title: <string>          # display label
  kind: string             # always "string" for credentials
  required: <bool>         # from source (LLM: true; tool: from describe.required)
  secret: true             # credentials are always secret
  help: <string?>          # LLM: overlay/generic; tool: describe.description
  group: "LLM" | "Tools"
  docs_url: <string?>      # LLM overlay only; omitted for tools / unknown providers
  placeholder: <string?>   # LLM overlay only
```

`assets/secret-requirements.json` deserializes into
`greentic-setup`'s `Vec<PackSecretRequirement> { key, required, description? }`
(`greentic-setup/src/secrets.rs`).

### Name → secret mapping (keep runtime resolution working)

The mapping is taken verbatim from the working hand-authored demo so the
zero-env bridge (`greentic-start` `store_scope_candidates`, runner
`StoreToolSecretsBackend`), `canonical_secret_uri`, and requirement-alias seeding
all continue to resolve unchanged:

| Source | question `name` | requirement `key` | group |
|--------|-----------------|-------------------|-------|
| LLM `credential_ref = deepseek` | `deepseek` | `llm/deepseek` | LLM |
| Tool secret `tavily/api_key` | `api_key` (last segment) | `tavily/api_key` | Tools |

- Runtime reads: LLM at `secrets://default/<tenant>/_/llm/<credential_ref>`;
  tool at `secret://<provider>/<key>`. Setup persists question `name` at
  `secrets://<env>/<tenant>/_/<pack_id>/<name>`; the requirement key seeds the
  alias that bridges the two. Using `name = credential_ref` (LLM) and
  `name = <key last segment>` (tool) reproduces the proven demo URIs.
- **Collision** (two extensions declare the same last segment, e.g. both expose
  `api_key`): disambiguate the question `name` to `<provider>_<key>` while keeping
  the full `provider/key` as the requirement key, so the runtime tool lookup
  (`secret://<provider>/<key>`) still resolves and the two questions stay distinct.

### Override

If `assets/setup.yaml` already exists in the pack source (hand-authored), the
generator **skips** generation for that file and leaves it untouched
(`secret-requirements.json` is still generated unless it too is hand-present).
Default path (no hand-authored file) generates both.

### Error handling

- A tool extension declared by an agent that **cannot be resolved** at build time
  (absent from `extensions.lock.json` and the store) is a hard **error** — a pack
  with an incomplete credential form is a bug and must not ship silently.
- An LLM provider not present in the overlay → **warn** and emit the minimal
  valid question (title derived from the provider id, no docs_url/placeholder).
  The build still succeeds.

## Testing

- **Unit (`setup_gen`)** — fixtures of agents + resolved tool requirements →
  expected `setup.yaml` / `secret-requirements.json`: dedupe (Tavily 2×→1),
  name mapping, collision disambiguation, LLM overlay hit vs miss, override-skip.
- **Drift-test** — assert every `greentic-llm::ProviderKind::all()` id either has
  an overlay entry or is explicitly allow-listed as "minimal" — fails when
  greentic-llm gains a provider the overlay author has not triaged. (`greentic-llm`
  as a `dev-dependency` of this test only.)
- **Integration** — build the tavily demo pack **without** its hand-authored
  `assets/setup.yaml` and assert the generated `setup.yaml` +
  `secret-requirements.json` equal the current hand-authored files (parity proof).
- **E2E** — `gtc setup` over the generated pack → form renders the DeepSeek + Tavily
  questions → secrets persist → `gtc start` zero-env answers grounded (reuses the
  existing zero-env demo path).

## Demo change (greentic-demo)

Remove the hand-authored `assets/setup.yaml` (and `assets/secret-requirements.json`)
from `crates/agentic-research-tavily-demo/` and rebuild the gtpack via the
generator. The generated output must equal the removed hand-authored files —
this is both the integration test and the proof the demo now faithfully mirrors
designer output.

## Out of scope (YAGNI)

- Changing the `describe` schema to carry display hints (`title`/`docs_url`/
  `placeholder`) for tools — deferred; tools use `description`/`format` only.
- Moving `ProviderKind` into a lightweight `greentic-llm` types crate / feature —
  deferred; the drift-test covers completeness without the dependency.
- Non-credential setup questions (mode, public_base_url, etc.) — the generator
  only emits credential questions; other questions remain author-supplied via the
  override path.
