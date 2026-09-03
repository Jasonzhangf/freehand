# v2 Plugin Capabilities Owner Binding

Status: source_implemented
Owner: `v2-plugin-capabilities`
Governance: AppSDK `0.1.6`

## Scope

This module owns typed capability manifests, deterministic capability
registration, local Rust leaf/composition plugin identity and invocation
results.

It does not own reasoning decisions, Session Log truth, UI projection,
network transport or Cordis runtime internals.

## Boundary

```text
CapabilityManifest
  -> CapabilityRegistry
  -> CapabilityPlugin
  -> CapabilityInvocation / CapabilityError
```

Allowed paths:

- `playground/experiments/v2/plugin-capabilities/**`
- `tests/v2/plugin-capabilities/**`
- `docs/design/v2-plugin-capabilities-owner-binding.md`

Forbidden paths:

- `docs/v2/v2-ui-design.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `active/lib/**`, `protected/**`, `generated/**`

## Contracts

`CapabilityManifest` declares plugin/capability identity, input/output
contract IDs, emitted event names, permission scopes and optional scope.
`CapabilityRegistry` rejects duplicate registration and unloads only known
capabilities. `LocalCapabilityPlugin` is the first deterministic Rust leaf
capability.

`CapabilityInvocation` returns an immutable payload via `Arc` and an explicit
success/failure result. No control/event fields enter the business payload.

## Gates

- `cargo test --manifest-path playground/experiments/v2/plugin-capabilities/Cargo.toml --test v2_plugin_capabilities_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/plugin-capabilities/Cargo.toml --all-targets -- -D warnings`
- `cargo fmt --check --manifest-path playground/experiments/v2/plugin-capabilities/Cargo.toml`
- `appsdk verify --review-admission <project> --module v2-plugin-capabilities`
