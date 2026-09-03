# v2 Topology Plugin Owner Binding

Status: source_implemented
Owner: `v2-topology-plugin`
Governance: AppSDK `0.1.6`

## Scope

This module owns typed physical topology projection from machine -> node ->
Agent -> Channel, registry generation reconciliation and focus identity.

It does not own notification ranking, session ordering, transport or business
payloads.

## Boundary

```text
Registry projection
  -> load / reconcile
  -> publish
  -> focus
```

Allowed paths:

- `playground/experiments/v2/topology-plugin/**`
- `tests/v2/topology-plugin/**`
- `docs/design/v2-topology-plugin-owner-binding.md`

Forbidden paths:

- `active/lib/**`, `protected/**`, `generated/**`
- `docs/v2/v2-ui-design.md`

## Gates

- `cargo test --manifest-path playground/experiments/v2/topology-plugin/Cargo.toml --test v2_topology_plugin_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/topology-plugin/Cargo.toml --all-targets -- -D warnings`
- `appsdk verify --review-admission <project> --module v2-topology-plugin`
