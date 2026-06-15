# Workspace Layout

## Layers

- `freehand-contracts`: global shared semantic source and shared error/ID contracts
- `freehand-blocks`: reusable pure functions
- `freehand-provider-*`: provider adapters only
- `freehand-reason`: turn orchestration, session-history / rewrite-gate truth, and event emission
- `freehand-node`: master/slave runtime
- `freehand-ui-protocol`: UI-facing contract surface
- `freehand-gates`: architecture enforcement
- `freehand-testkit`: shared test and replay helpers
  - includes project black-box runtime harnesses before production CLI/server loops exist

## Test Ownership

- owner crates keep white-box tests near owner truth
- module black-box tests may live in the owner crate or `freehand-testkit` when shared fixtures or runtime harnesses are required
- project black-box tests should converge on `freehand-testkit` and app/runtime smoke harnesses
- test fixtures, mock providers, replay inputs, and protocol fixtures should not be redefined in each orchestrator crate

## Runtime Home

- `~/.freehand/state`: node state, session state, durable local runtime data
- `~/.freehand/state/turns`: authoritative session-history snapshots, active-turn snapshots, terminal turn truth
- `~/.freehand/state/ui`: derived UI/session sidecars
- `~/.freehand/logs`: logs by subsystem
- `~/.freehand/ledgers`: append-only event, debug, and audit ledgers
- `~/.freehand/ledgers/reason`: append-only semantic turn and rewrite evidence
- `~/.freehand/ledgers/providers`: provider-family raw/debug evidence
- `~/.freehand/replays`: captured runtime exchanges for replay/debug
- `~/.freehand/cache`: runtime cache
- `~/.freehand/cache/session-index`: rebuildable session list and index caches
- `~/.freehand/tmp`: explicit temp workspace

## Rule

- Downstream crates may depend on contracts.
- Shared semantic logic moves into blocks.
- Before writing any new function, inspect existing function libraries and owner crates first.
- If a function is helper-like, reusable, semantic, parser-like, validator-like, builder-like, or projector-like, it belongs in `freehand-blocks`.
- Orchestrators may compose blocks, but must not redefine semantic logic.
- Orchestrators may keep entrypoint glue only. Do not park temporary helpers in `freehand-reason` or `freehand-node`.
- context planner builders, segment validators, and cache-shape calculators belong in `freehand-blocks`, not `freehand-reason`.
- UI crates consume projections and commands only.
- Function map drives owner lookup and debug entry. Do not start feature work without it.
- Config schema stays outside `freehand-contracts`.
- UI projection stays outside `freehand-contracts`.
- Debug/trace envelope stays outside `freehand-contracts`.
- Provider wire payloads stay outside `freehand-contracts` and outside shared semantic contracts.
- Session truth writes stay inside `freehand-reason`.
- Provider `finish_reason` is not final stop truth; Freehand completion schema is.
- UI protocol owns command/query/subscribe/projection truth; apps own rendering only.
- Test ownership follows the same single-truth rule as runtime semantics.
- `freehand-reason` must not depend on provider adapter crates.
- Provider adapter crates must not depend on `freehand-reason`.
- Metadata/debug/provider/cache fields and request-chain content fields must use separate types and separate builders.
- Metadata must not be embedded into request text unless an explicit context builder converted it into request data.
- subagent transcript truth stays outside parent prompt history; only typed final conclusion segments may be admitted into parent context.
- authoritative persistence is `freehand-reason` truth only; UI sidecars and provider raw ledgers are rebuildable derivatives
