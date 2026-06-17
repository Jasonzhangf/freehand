# Workspace Layout

## Layers

- `freehand-contracts`: global shared semantic source and shared error/ID contracts
- `freehand-blocks`: reusable pure functions
- `freehand-provider-*`: provider adapters only
- `freehand-tools`: built-in tool registry, tool specs, and tool execution owner
- `freehand-reason`: turn orchestration, session-history / rewrite-gate truth, and event emission
- `freehand-node`: master/slave runtime
- `freehand-debug`: debug/trace envelope, debug snapshot, replay-facing observation contracts
- `freehand-metadata`: internal control/provenance metadata center with writer owner and write-node validation
- `freehand-ui-protocol`: UI-facing contract surface
- `freehand-runtime`: runtime wiring owner that composes reason/node owners without turning apps into business owners
  - may also own config-selected runtime bootstrap helpers so host apps stay thin
- `freehand-gates`: architecture enforcement
- `freehand-testkit`: shared test and replay helpers
  - includes project black-box runtime harnesses before production CLI/server loops exist
- `apps/freehand-server`: protocol-only HTTP/SSE transport owner and smoke entrypoint
- `apps/freehand-daemon`: runtime host app that injects `freehand-runtime` into the shared transport

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
- Runtime/demo tools must not live in orchestrator crates. Built-in tool specs and execution ownership stay in `freehand-tools`.
- Every built-in tool must have one owner spec entry with explicit `implemented` state before runtime or provider exposure.
- Tool registry exposure follows function-map and test-design gates; do not advertise a new tool from runtime first and document it later.
- context planner builders, segment validators, and cache-shape calculators belong in `freehand-blocks`, not `freehand-reason`.
- UI crates consume projections and commands only.
- UI crates are input ingress plus read-only consumers; they must not become reason/debug/session truth writers.
- Debug crates own debug/trace envelopes and replay-facing observation contracts only; they must not become session or request truth writers.
- Function map drives owner lookup and debug entry. Do not start feature work without it.
- Config schema stays outside `freehand-contracts`.
- UI projection stays outside `freehand-contracts`.
- Debug/trace envelope stays outside `freehand-contracts`.
- Provider wire payloads stay outside `freehand-contracts` and outside shared semantic contracts.
- Session truth writes stay inside `freehand-reason`.
- Provider `finish_reason` is not final stop truth; Freehand completion schema is.
- UI protocol owns command/query/subscribe/projection truth; apps own rendering only.
- UI-submitted commands may ask the system to act, but owner modules still own all turn/debug/session truth mutation.
- transport reuse is allowed only when the shared transport implementation stays protocol-only.
- Debug snapshots and trace envelopes belong in `freehand-debug`, not in `freehand-contracts`, `freehand-reason`, or UI apps.
- Internal control/provenance metadata belongs in `freehand-metadata`; metadata entries must include writer owner and write-node provenance.
- Metadata must not carry request text, prompt content, message arrays, provider request payloads, or context segment content.
- Debug may observe or link metadata, but debug is not the metadata write owner.
- Test ownership follows the same single-truth rule as runtime semantics.
- `freehand-reason` must not depend on provider adapter crates.
- Provider adapter crates must not depend on `freehand-reason`.
- Metadata/debug/provider/cache fields and request-chain content fields must use separate types and separate builders.
- Metadata must not be embedded into request text unless an explicit context builder converted it into request data.
- subagent transcript truth stays outside parent prompt history; only typed final conclusion segments may be admitted into parent context.
- authoritative persistence is `freehand-reason` truth only; UI sidecars and provider raw ledgers are rebuildable derivatives
