# v2 UI Plugin Family Owner Binding

Status: source_implemented
Owner: `v2-ui-plugin-family`
Governance: AppSDK `0.1.6`

## Scope

This module owns the Cordis UI slot registry and replaceable UI plugin family.
It registers UI plugins by stable `slot_id`, mounts one implementation per slot,
renders typed projections from the UI adaptor, preserves selections across
replacement, and releases slots on unmount.

It does not own rendering frameworks, browser state, session truth, reason
truth, search/memory records, channel transport or capability invocation.

## Boundary

```text
UiPluginDefinition
  -> UiPluginSlotRegistry::mount
  -> mount implementation into stable slot

UiProjection + selection
  -> UiPluginSlotRegistry::render
  -> UiPluginView

replacement requested
  -> preserve selection and latest projection
  -> mount replacement in same slot
  -> expose new UiPluginView
```

Allowed paths:

- `playground/experiments/v2/ui-plugins/**`
- `tests/v2/ui-plugins/**`
- `docs/design/v2-ui-plugin-family-owner-binding.md`

Forbidden paths:

- `docs/v2/v2-ui-design.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `active/lib/**`, `protected/**`, `generated/**`

## Contracts

`UiPluginDefinition` requires stable plugin/slot identity, instance identity,
positive contract version and at least one declared capability. The registry
rejects duplicate mounting and unknown-slot replacement. Replacement rebuilds
from the current typed projection without mutating owner truth.

## Gates

- `cargo test --manifest-path playground/experiments/v2/ui-plugins/Cargo.toml --test v2_ui_plugin_family_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/ui-plugins/Cargo.toml --all-targets -- -D warnings`
- `cargo fmt --check --manifest-path playground/experiments/v2/ui-plugins/Cargo.toml`
- `appsdk verify --review-admission <project> --module v2-ui-plugin-family`
