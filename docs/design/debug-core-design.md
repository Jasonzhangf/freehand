# Debug Core Design

## Status

Baseline landed.

## Purpose

`debug.core` is the independent observation module for Freehand.

It exists so reason, provider, node, UI transport, and future modules can share one debug/trace contract without coupling their business semantics to each other.

## Ownership

- owner crate: `crates/freehand-debug`
- feature id: `debug.core`

`debug.core` owns:

- trace envelope shape
- semantic debug position
- scene debug position
- per-turn debug snapshot shape
- debug snapshot builder helpers
- debug hub and subscription fanout
- sink classification for stdout, file, and replay capture

`debug.core` does not own:

- request truth
- session truth
- reason turn truth
- provider semantic truth
- provider raw wire payload ownership
- UI rendering

In other words:

- `debug.core` does not own request truth
- `debug.core` does not own session truth
- `debug.core` does not own reason turn truth

## Dependency Direction

Allowed:

- `freehand-debug` depends on `freehand-contracts`
- reason/provider/node/testkit/UI protocol may depend on `freehand-debug`

Forbidden:

- `freehand-debug` depending on reason/provider/node/UI implementation crates
- rebuilding authoritative truth from debug artifacts
- embedding request content or provider raw payload as hidden debug fields
- treating UI debug projections as debug truth writers

## First-Version Types

### DebugSemanticPosition

Carries semantic coordinates:

- `feature_id`
- `session_id`
- `turn_id`
- `trace_id`
- optional `agent_id`
- optional `pipeline_node`

### DebugScenePosition

Carries scene coordinates:

- `crate_name`
- `file`
- `function`
- optional `line`
- optional `artifact_path`
- optional `raw_exchange_id`

### DebugTraceEnvelope

Combines semantic and scene coordinates with hashes and timestamp.

The envelope is for observation and replay correlation only.

### DebugStateSnapshot

The minimal UI-consumable debug projection:

- semantic position
- scene position
- `status_text`
- ordered `detail_lines`

The snapshot is intentionally not raw ledger truth.

### DebugEvent

The runtime emission form for observation data:

- semantic position
- scene position
- optional trace envelope
- optional debug snapshot
- sink classification

`DebugEvent` is the payload that modules emit into the debug hub.

Current landed producer:

- `freehand-reason` lifecycle milestones

### DebugSinkKind

Supported sink kinds:

- MemorySubscriber
- Stdout
- FileLedger
- ReplayCapture

### DebugHub

The debug runtime bus:

- accepts emitted debug events
- fans out to subscribers
- dispatches events to sinks
- never mutates owner truth

The hub is the only place where observation delivery is coordinated.

## UI Consumption

UI consumes debug state through `freehand-ui-protocol`.

UI may query or subscribe to debug snapshots, but it must not directly mutate debug truth or use debug data as session/reason truth.

## Runtime Paths

Debug artifacts should converge under:

- `~/.freehand/ledgers`
- `~/.freehand/replays`
- `~/.freehand/logs`

Exact writer layout remains owned by the producing module and future persistence/debug work.

Current sink primitives already exist:

- `StdoutDebugSink`
- `FileDebugSink`

## Update Trigger

Update this design when:

- trace envelope fields change
- debug snapshot fields change
- module dependency direction changes
- hub or sink classes change
- debug ledger/replay ownership changes
- UI debug projection semantics change
