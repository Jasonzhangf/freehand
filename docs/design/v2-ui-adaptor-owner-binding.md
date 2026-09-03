# v2 UI Adaptor Owner Binding

Status: source_implemented
Owner: `v2-ui-adaptor`
Governance: AppSDK `0.1.6`

## Scope

This module owns the typed public UI adaptor boundary. It validates and
projects typed UI commands, queries, subscriptions, projections, connection
state, capability availability and UI control events.

It does not own rendering state, browser storage, reason truth, session log
truth, network transport, plugin registry semantics or provider wire DTOs.

## Boundary

```text
UI command
  -> UiAdaptor::accept_command
  -> UiCommandReceipt
  -> UiControlEvent

owner projection
  -> UiAdaptor::publish_projection
  -> Arc shared UiProjection
  -> view/query/subscribe projection
```

Allowed paths:

- `playground/experiments/v2/ui-adaptor/**`
- `tests/v2/ui-adaptor/**`
- `docs/design/v2-ui-adaptor-owner-binding.md`

Forbidden paths:

- `docs/v2/v2-ui-design.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `active/lib/**`, `protected/**`, `generated/**`

## Contracts

`UiAdaptor` accepts a `UiCommand` only once per correlation, publishes typed
projections through `Arc<ImmutablePayload>`, queries the latest projection for a
slot, and attaches typed subscriptions only to existing slots. UI control events
capture identity and kind but never embed business payload bytes.

## Gates

- `cargo test --manifest-path playground/experiments/v2/ui-adaptor/Cargo.toml --test v2_ui_adaptor_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/ui-adaptor/Cargo.toml --all-targets -- -D warnings`
- `cargo fmt --check --manifest-path playground/experiments/v2/ui-adaptor/Cargo.toml`
- `appsdk verify --review-admission <project> --module v2-ui-adaptor`
