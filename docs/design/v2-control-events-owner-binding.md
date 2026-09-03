# Freehand v2 Control Events Owner Binding

Status: active
Project: `freehand-v2`
Module: `v2-control-events`
Governance: AppSDK `0.1.6`
Base: `origin/v2`

## 1. Purpose

This module owns the typed control-event ledger used by later V2 plugins. It
must keep control/error facts separate from business payloads, expose ordered
event admission, acknowledgement, replay, terminal/non-terminal state, and an
explicit error chain.

## 2. Module Owner

- module_id: `v2-control-events`
- source_owner: `v2-control-events`
- owned paths:
  - `playground/experiments/v2/control-events/**`
  - `tests/v2/control-events/**`
  - `docs/design/v2-control-events-owner-binding.md`
- forbidden paths:
  - `active/lib/**`
  - `protected/**`
  - `generated/**`
  - `docs/v2/v2-ui-design.md`
  - `docs/v2/v2-cordis-reasoning-channel-architecture.md`

## 3. Resource Map

| resource_id | owner | truth_store | operations |
| --- | --- | --- | --- |
| `v2_control_event` | `v2-control-events` | typed event ledger | emit, reject, ack, complete, replay, route |

Forbidden direct edges:

- UI -> `v2_control_event`
- Cordis composition -> `v2_control_event`
- Search/Memory -> `v2_control_event`
- network/channel -> `v2_control_event`

Business payload bytes must never be embedded in control or error records.

## 4. Function Map

Planned and implemented symbols:

```text
v2_control_events::EventLedger::emit
v2_control_events::EventLedger::reject
v2_control_events::EventLedger::acknowledge
v2_control_events::EventLedger::complete
v2_control_events::EventLedger::replay_from
v2_control_events::EventLedger::owner_events
```

## 5. Mainline Call Map

Request mainline:

```text
admit control event
  -> duplicate/terminal validation
  -> ordered append or typed rejection
  -> owner route projection
```

Error mainline:

```text
reject/error chain
  -> typed ErrorRecord
  -> unchanged event ledger
  -> explicit failure projection
```

## 6. Verification Map

Required gates:

```text
cargo test --manifest-path playground/experiments/v2/control-events/Cargo.toml --test v2_control_event_boundary
cargo fmt --check
cargo clippy --manifest-path playground/experiments/v2/control-events/Cargo.toml --all-targets -- -D warnings
appsdk verify --review-admission <project> --module v2-control-events
```

Positive and negative coverage:

- accepted events route to declared owner in order;
- duplicate event ids are rejected before mutation;
- acknowledgement moves state and replay returns suffix;
- terminal correlation rejects later events and already-terminal completion;
- already-acknowledged events are rejected;
- error records preserve source event identity without mutating the event log;
- payload references remain reference-only and do not contain body bytes.
