# Events Provider Packs

Packs can describe event providers so runtimes can discover brokers/sources/sinks/bridges without hard-coding transports. The `events` block in `pack.yaml` is optional and backwards compatible.

## Schema

```yaml
events:
  providers:
    - name: "nats-core"                  # required, unique per pack
      kind: broker                       # broker | source | sink | bridge
      component: "nats-provider@1.0.0"   # component id/version
      default_flow: "flows/events/nats/default.ygtc"  # optional canned flow
      custom_flow: "flows/events/nats/custom.ygtc"    # optional override
      capabilities:
        transport: nats                  # nats | kafka | sqs | webhook | email | other:<string>
        reliability: at_least_once       # at_most_once | at_least_once | effectively_once
        ordering: per_key                # none | per_key | global
        topics:
          - "greentic.*"                 # optional topic patterns
```

Fields map directly to the shared `EventProviderDescriptor` model:

- `name` – human-friendly identifier used in diagnostics and registries.
- `kind` – declares the provider role (broker/source/sink/bridge).
- `component` – the component instance that implements the provider.
- `default_flow` / `custom_flow` – references to flows (e.g. under `flows/events/...`) that wire the provider into the runtime.
- `capabilities` – optional hints about transport/reliability/ordering and supported topic patterns.

## Examples

NATS broker:

```yaml
events:
  providers:
    - name: "nats-core"
      kind: broker
      component: "nats-provider@1.0.0"
      default_flow: "flows/events/nats/default.ygtc"
      capabilities:
        transport: nats
        reliability: at_least_once
        ordering: per_key
        topics:
          - "greentic.>"
```

Kafka broker (conceptual):

```yaml
events:
  providers:
    - name: "kafka-core"
      kind: broker
      component: "kafka-provider@1.0.0"
      custom_flow: "flows/events/kafka/custom.ygtc"
      capabilities:
        transport: kafka
        reliability: at_least_once
        ordering: per_key
        topics:
          - "greentic.repo.*"
```

## Validation and discovery

- `packc lint --in <pack-dir>` validates the `events.providers` block alongside flows/templates.
- `greentic-pack events list <pack-path> [--format table|json|yaml]` lists declared providers from a source directory or `.gtpack`.

Treat the `events` block as optional; packs without it continue to parse and build normally.
