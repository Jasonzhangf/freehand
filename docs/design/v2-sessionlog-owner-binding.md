# Freehand v2 Session Log Owner Binding

Status: design-freeze-candidate
Project: `freehand-v2`
Module: `v2-sessionlog`
Governance: AppSDK `0.1.6`
Branch: `codex/v2-sessionlog-20260903`
Base: `origin/v2=e4651e9c4b7b36f971b143753985070528e5a8bf`
Related vectors: `docs/v2/v2-sessionlog-test-vectors.md`
Related test design: `docs/v2/v2-test-design.md`
Related project black-box: `docs/v2/v2-project-blackbox-verification.md`

## 1. Purpose

This document freezes the M2 canonical Session Log owner binding before source
implementation. It does not claim that M2 source exists or that any vector has
passed. It is the machine-checkable starting point for the next implementation
milestone.

The current dependency gate remains explicit: M1b control events have not yet
produced a remote `v2` mainline receipt. M2 source work should not begin until
that predecessor receipt is present or Jason explicitly authorizes an isolated
Playground source experiment that does not enter remote `v2` delivery before
the roadmap gate is satisfied.

## 2. Module Owner

- module_id: `v2-sessionlog`
- source_owner: `v2-sessionlog`
- feature_id: `v2-sessionlog`
- owner worktree: `<project-root>/playground/v2-sessionlog-20260903`
- branch: `codex/v2-sessionlog-20260903`

Planned owned paths once implementation begins:

```text
playground/experiments/v2/sessionlog/**
tests/v2/sessionlog/**
docs/design/v2-sessionlog-owner-binding.md
docs/v2/v2-sessionlog-test-vectors.md
docs/v2/v2-test-design.md
docs/v2/v2-project-blackbox-verification.md
```

Forbidden paths:

```text
UI, Cordis composition, channel/network, Search, Memory, reasoning provider
implementation, provider-native raw history stores
```

## 3. Resource Map

| resource_id | owner | truth_store | operations |
| --- | --- | --- | --- |
| `v2_session_log` | `v2-sessionlog` | canonical local Session Log adapter owned by `v2-sessionlog` | create_header, append_event, read, replay, derive_surface, replace_surface, reorder_surface, undo_fact, fork_session, restore, export |

Projections:

- ordered event replay projection
- deterministic model-visible surface
- cursor/restore projection
- export projection

Forbidden direct edges:

- UI -> `v2_session_log`
- Cordis composition -> `v2_session_log`
- Search/Memory -> `v2_session_log`
- network/channel -> `v2_session_log`

All reads and writes must cross the canonical Session Log service boundary.

## 4. Function Map

Planned entry symbols before implementation:

```text
v2_sessionlog::create_session
v2_sessionlog::append_event
v2_sessionlog::read_session
v2_sessionlog::replay_from
v2_sessionlog::derive_surface
v2_sessionlog::replace_surface
v2_sessionlog::reorder_surface
v2_sessionlog::record_undo
v2_sessionlog::fork_from
v2_sessionlog::restore_session
v2_sessionlog::export_session
v2_sessionlog::recover_interrupted
```

These symbols are binding-pending until the crate exists.

## 5. Mainline Call Map

Request mainline:

```text
SessionLogCommand
  -> canonical adapter validation
  -> contiguous sequence admission
  -> durable append or typed rejection
  -> surface derivation
  -> ordered projection
```

Response mainline:

```text
SessionLogResult
  -> event/cursor/surface/export
  -> source event identities
  -> deterministic projection digest
```

Error mainline:

```text
SessionLogError
  -> duplicate/gap/corrupt/version/lock/permission/cursor/fork failure
  -> unchanged-log assertion
  -> no successful terminal projection
```

## 6. Verification Map

Development gates before implementation:

```text
appsdk verify <project>
cargo test --manifest-path playground/experiments/v2/sessionlog/Cargo.toml
cargo fmt --check
cargo clippy --manifest-path playground/experiments/v2/sessionlog/Cargo.toml --all-targets -- -D warnings
```

The M2 regression gate must bind every vector in
`docs/v2/v2-sessionlog-test-vectors.md`, with both positive and negative
assertions. Release admission, install, restart, deployed blackbox and review
remain gated by AppSDK lifecycle and by the user-facing public entrypoint.

## 7. Freeze Decision

For the next development step, M2 stays as:

- one canonical Session Log owner;
- one canonical append path;
- one local persistence adapter;
- append-only replacement, reorder and undo;
- explicit interrupted-turn recovery;
- typed storage and integrity errors;
- `Arc` only for immutable local payload sharing, not as durable truth.

No second log, UI cache truth, provider-native truth or network fallback is
permitted.
