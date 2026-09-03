# v2 Reasoning Backend Owner Binding

Status: source_implemented
Owner: `v2-reasoning-backend`
Governance: AppSDK `0.1.6`

## Scope

This module owns the provider-neutral Reasoning Service seam,
runtime-group backend binding, normalized reasoning events and the
Freehand native and OpenCode adaptor boundaries.

It does not own Session Log truth, UI projections, capability invocation,
network transport or provider wire DTOs. It consumes freehand-v2-contracts
IDs and immutable payload values but must not expose provider storage as
Freehand truth.

## Boundary

```text
runtime-group binding
  -> ReasoningService
  -> active ReasoningBackend
  -> normalized ReasoningEvent / ReasoningError
  -> caller appends to Session Log at its own owner boundary
```

Allowed paths:

- `playground/experiments/v2/reasoning-backend/**`
- `tests/v2/reasoning-backend/**`
- `docs/design/v2-reasoning-backend-owner-binding.md`

Forbidden paths:

- `docs/v2/v2-ui-design.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `active/lib/**`, `protected/**`, `generated/**`

## Contracts

`ReasoningService` owns active backend binding per runtime group and
in-flight session tracking. One runtime group has one active backend at a
time. Multiple runtime groups can use different backends.

`ReasoningBackend` is the provider-neutral seam. The first two
implementations are `NativeBackend` and `OpenCodeBackend`. Both normalize
start/resume/interrupt/inspect/subscribe behavior into
`ReasoningEvent` or `ReasoningError`.

`Arc<ImmutablePayload>` is the local adjacent handoff shape. The native
fixture keeps the same allocation across start/consumer boundaries; the
OpenCode fixture stands for an explicit adaptor boundary that may
reconstruct or serialize as its integration requires.

## Gates

- `cargo test --manifest-path playground/experiments/v2/reasoning-backend/Cargo.toml --test v2_reasoning_backend_boundary`
- `cargo clippy --manifest-path playground/experiments/v2/reasoning-backend/Cargo.toml --all-targets -- -D warnings`
- `cargo fmt --check --manifest-path playground/experiments/v2/reasoning-backend/Cargo.toml`
- `appsdk verify --review-admission <project> --module v2-reasoning-backend`

## Known Extension

The OpenCode transport and Cordis injection are implementation work behind
this seam. They must not create a second Session Log truth or migrate an
in-flight reasoning operation during backend replacement.
