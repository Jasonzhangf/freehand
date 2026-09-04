# v2 Cordis Ecosystem Owner Binding

Status: source_implemented
Owner: `v2-cordis-ecosystem`
Governance: AppSDK `0.1.6`

## Scope

This module owns the Freehand bindings around a Cordis-shaped plugin
composition surface: runtime scope identity, capability registration,
typed event path and safe replacement/unload boundaries.

It does not own capability semantics, Session Log truth, reasoning decisions,
UI rendering or network truth. Cordis itself is treated as the external plugin
substrate the Freehand MVP exposes typed ports for.

`CordisContext` also owns the compiled in-memory plugin registration surface:
every executable, replaceable or externally connected v2 component registers a
stable `plugin_id`, a `PluginRole` and a contract version before it is loadable
or replaceable through Cordis. Capability implementations still register their
CapabilityManifest separately through the capability owner; the Cordis plugin
registration is the control-plane identity, not a second capability truth.

## Boundary

```text
CordisRoot / CordisContext
  -> PluginRegistration (role + contract version)
  -> CapabilityPlugin manifest
  -> invoke
  -> EventLedger control event
  -> CompositionResult
```

Allowed paths:

- `playground/experiments/v2/cordis-ecosystem/**`
- `tests/v2/cordis-ecosystem/**`
- `docs/design/v2-plugin-capabilities-owner-binding.md`
- `docs/design/v2-cordis-ecosystem-owner-binding.md`

Forbidden paths:

- `docs/v2/v2-ui-design.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `active/lib/**`, `protected/**`, `generated/**`

## Contracts

`CordisContext` composes capability plugins and control events. A successful
composition emits invoked/completed control events without embedding business
payload bytes. A failing composition records an explicit error-event chain and
does not leak in-flight correlation state.

Replacement and unload are allowed only when no operation is pending. Unknown
capability, duplicate registration and failed invocation fail closed.
Plugin registration is deterministic, duplicate `plugin_id`s are rejected, and
unknown roles or unsupported contract versions fail before any mutation.

## Gates

- `cargo test --manifest-path playground/experiments/v2/cordis-ecosystem/Cargo.toml --test v2_cordis_ecosystem_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/cordis-ecosystem/Cargo.toml --all-targets -- -D warnings`
- `cargo fmt --check --manifest-path playground/experiments/v2/cordis-ecosystem/Cargo.toml`
- `appsdk verify --review-admission <project> --module v2-cordis-ecosystem`
