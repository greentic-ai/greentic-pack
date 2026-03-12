PR-PK-01 — Add generic static-routes extension to any pack
Title

Add `greentic.static-routes.v1` as a generic pack extension for public static asset mounting

Overall direction

We are implementing:

1. packs can declare static routes through a generic extension that can be added to any pack
2. `greentic-setup`, `greentic-start`, and `greentic-operator` support this
3. `greentic-messaging-providers` will package both `webchat` and `webchat-gui`
4. `webchat-gui` includes `greentic-webchat` and is configured out of the box to use the `webchat` Direct Line backend
5. all hard-coded webchat/directline routes are removed from operator

Goal

Allow any pack to declare public static routes by extension metadata, with assets already bundled under `assets/...`.

Why

This is the declaration point for hosted surfaces such as:

- `messaging.webchat-gui`
- docs portals
- admin UIs
- setup portals
- any future tenant-facing static surface

It must not be tied to GUI-only conventions.

Audit conclusions

- `greentic-bundle` does NOT need changes for v1.
- `greentic-pack` should add a new generic extension for static routes.
- `greentic-setup` should collect and persist stable hosting policy.
- `greentic-start` should detect static-route capability and gate startup compatibility.
- `greentic-operator` should get only a small runtime-only change to mount/serve static routes and remove hard-coded webchat/directline routes.

Scope

Keep the work surgical. Avoid broad redesigns beyond what is needed for:

- generic static-routes extension
- `setup` / `start` / `operator` support
- `webchat` / `webchat-gui` migration
- removal of hard-coded webchat routes from operator

Static route extension

Add a new generic extension, e.g.:

```yaml
extensions:
  greentic.static-routes.v1:
    kind: greentic.static-routes.v1
    version: 1.0.0
    inline:
      version: 1
      routes:
        - id: webchat-gui
          public_path: "/v1/web/webchat/{tenant}"
          source_root: "assets/webchat-gui"
          scope:
            tenant: true
            team: false
          index_file: "index.html"
          spa_fallback: "index.html"
          cache:
            strategy: "public-max-age"
            max_age_seconds: 3600
          exports:
            base_url: "webchat_gui_base_url"
            entry_url: "webchat_gui_entry_url"
```

Decisions:

- Add a new generic extension such as `greentic.static-routes.v1`.
- Do NOT reuse the current GUI convention as the canonical runtime model.
- Assets continue to be packaged as normal files under `assets/...`.
- The extension only declares mount metadata.

Important route-space decision

To avoid collisions with platform routes like messaging, events, oauth, and other dynamic endpoints, hosted static surfaces live under a reserved namespace:

`/v1/web/...`

Dynamic / provider / API routes stay in their own namespaces, e.g.:

- `/v1/messaging/...`
- `/v1/events/...`
- `/v1/oauth/...`

So for webchat:

- GUI: `/v1/web/webchat/{tenant}`
- backend / directline / provider ingress: `/v1/messaging/webchat/{tenant}/...`

Do NOT require `pack_id` in public paths

Public URLs should be semantic, not derived from pack ids.

Do NOT force paths like:

`/v1/web/<pack-id>/...`

Keep `pack_id` only for:

- diagnostics
- collision reporting
- runtime ownership / inventory
- logs

Public path must be explicitly declared by the pack.

Recommended v1 shape

Required:

- `version`
- `routes[]`
- `routes[].id`
- `routes[].public_path`
- `routes[].source_root`

Optional:

- `routes[].scope.tenant`
- `routes[].scope.team`
- `routes[].index_file`
- `routes[].spa_fallback`
- `routes[].cache`
- `routes[].exports`

Pack-layer decisions for PR-PK-01

`public_path` syntax

Keep v1 very small.

Allowed:

- literal segments
- `{tenant}`
- `{team}`

Disallowed in v1:

- `*`
- `**`
- regex-like segments
- arbitrary placeholders
- optional segments
- query strings
- fragments

Treat `public_path` as a mount prefix, not a route-pattern engine.

Additionally, for v1 static routes:

- `public_path` must start with `/v1/web/`

`source_root`

Use full asset namespace paths, e.g.:

`assets/webchat-gui`

Do NOT use paths relative to an implicit asset root.

Validation and docs should lock this in.

`cache` schema

Keep v1 minimal:

```yaml
cache:
  strategy: "none" | "public-max-age"
  max_age_seconds: 3600
```

Rules:

- `strategy` is required if `cache` exists
- `max_age_seconds` is only valid when `strategy` is `public-max-age`
- reject unknown strategies

Export name uniqueness

Exported URL names must be unique across the whole pack, not only per route.

Scope defaults and rules

Default scope:

```yaml
scope:
  tenant: false
  team: false
```

If omitted, route is global.

Disallow `team: true` when `tenant: false`.

`index_file` and `spa_fallback`

Both are relative to `source_root`.

Example:

```yaml
index_file: "index.html"
spa_fallback: "index.html"
```

`source_root` type

Prefer directory-backed mounts only in v1.

Validation requirements

Validate at build / lint time.

- `public_path` must start with `/`
- and in v1 must start with `/v1/web/`
- reject traversal and unsupported wildcard syntax
- only allow literal segments plus `{tenant}` / `{team}`
- `source_root` must start with `assets/`
- `source_root` must exist and resolve inside pack assets
- `index_file` must exist under `source_root` if declared
- `spa_fallback` must exist under `source_root` if declared
- reject duplicate route ids within a pack
- reject duplicate normalized public paths within a pack
- exported URL names unique across the whole pack
- reject invalid scope combinations

Inspect / doctor support

Extend:

- `inspect`
- `doctor`
- validation output

Human output should show a dedicated static routes section including:

- `id`
- `public_path`
- `source_root`
- `scope`
- `index_file`
- `spa_fallback`
- `cache`
- `exports`

JSON output should expose a stable `static_routes` array with those fields.

`greentic-pack`

Responsibilities:

- add `greentic.static-routes.v1`
- parse and validate the extension
- surface static routes in `inspect` / `doctor`
- document the extension and pack expectations

Non-goals:

- no runtime serving here
- no operator logic here
- no setup policy here
- no bundle-level aggregation here
- no WebChat-specific logic in the pack layer

Files likely touched

- pack config schema
- extension parsing / validation
- lint path
- inspect / doctor output
- docs for pack format / extensions

`greentic-setup`

`setup` should collect stable admin / deployment hosting policy, not per-pack route tables.

Add bundle / environment-level fields such as:

- `public_web_enabled`
- `public_base_url`
- `public_surface_policy`
- optional `default_route_prefix_policy`
- optional `tenant_path_policy`

`setup` should validate:

- `public_base_url` syntax / normalization
- required combinations, e.g. `public_web_enabled=true` requires `public_base_url`
- stable policy consistency
- environment / deployment compatibility

`setup` should persist a bundle-level artifact, e.g.:

`state/config/platform/static-routes.json`

Replay / update flows should include these fields.

`setup` should NOT own:

- route serving
- pack route schema
- runtime collision detection
- startup-only launch decisions

`greentic-start`

`start` should inspect pack metadata directly from bundled packs to detect `greentic.static-routes.v1`.

`start` should own launch compatibility checks before boot:

- bundle has static routes?
- public HTTP enabled?
- asset serving supported?
- `public_base_url` resolved when required?

If the bundle declares static routes but launch mode cannot support them, fail before boot.

`start` should pass a small resolved runtime contract onward, e.g.:

- `PUBLIC_HTTP_ENABLED`
- `STATIC_ROUTES_ENABLED`
- `ASSET_SERVING_ENABLED`
- `PUBLIC_BASE_URL`

`start` should NOT:

- serve routes
- perform full runtime collision detection

`greentic-operator`

Operator change should be small and runtime-only.

`operator` should:

- read static-route declarations from pack metadata during warm
- validate collisions against reserved routes
- build an active static route table
- serve static assets from bundle-readable paths
- swap / unmount routes on activate / rollback / complete-drain

`operator` should keep only genuinely operator-owned reserved routes.

Remove the current hard-coded webchat / directline routes:

- `/token`
- `/v3/directline/*`
- `/directline/*`

After this, webchat / directline backend routes must come from provider / pack metadata, not operator code.

`greentic-messaging-providers`

Keep `messaging.webchat` as backend-only Direct Line provider.

Add `messaging.webchat-gui` as backend + packaged GUI.

Extract or reuse a shared backend core so logic is not duplicated.

`webchat-gui` should:

- package built `greentic-webchat` assets under normal assets, e.g. `assets/webchat-gui/...`
- declare static routes via `greentic.static-routes.v1`
- use provider / pack-declared backend routes, not operator hard-coded routes
- be configured out of the box to use the matching directline backend automatically

Desired end state:

- operator has no webchat-specific routing logic
- all webchat routing comes from provider / pack declarations

Bundle

Do not change `greentic-bundle` for v1.

Existing bundles already preserve pack bytes and metadata unchanged.

`start` and `operator` can inspect pack manifests directly without bundle preprocessing.

Merge / implementation order

1. `greentic-pack`
2. `greentic-setup`
3. `greentic-start`
4. `greentic-operator`
5. `greentic-messaging-providers`

Acceptance criteria

- any pack can declare `greentic.static-routes.v1`
- assets remain normal `assets/...` payloads
- extension validates cleanly
- `inspect` / `doctor` surface it clearly
- `greentic-setup` persists stable hosting policy
- `greentic-start` gates incompatible launch modes before boot
- `greentic-operator` mounts generic static routes and removes hard-coded webchat/directline routes
- `greentic-messaging-providers` packages both `webchat` and `webchat-gui`
- `webchat-gui` is preconfigured to use the matching Direct Line backend
- no WebChat-specific logic exists in the pack layer
- no `greentic-bundle` changes are required for v1
