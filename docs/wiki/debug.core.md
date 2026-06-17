# Wiki: `debug.core`

Generated from `docs/mainline-calls/debug.core.json`. Do not edit by hand.

- owner crate: `crates/freehand-debug`
- owner module: `crates/freehand-debug/src/lib.rs`
- function map: `docs/function-maps/debug.core.md`
- generated wiki: `docs/wiki/debug.core.md`
- test design: `docs/testing/debug.core.md`

## Request Mainline

- owner modules create debug semantic position and scene position from their own truth
- owner modules build debug trace envelopes or debug snapshots through `freehand-debug`
- debug material is observation data, not request content
- owner modules emit debug events into `DebugHub`

## Response Mainline

- debug snapshots may be consumed by UI protocol as read-only projection data
- trace envelopes may be persisted or replayed by future debug/runtime writers
- consumers preserve semantic and scene coordinates together
- `DebugHub` fans out to subscribers and sinks without mutating owner truth

## Error Mainline

- empty required debug fields are explicit construction errors only when builder helpers validate them
- debug artifacts must not be promoted into successful reason/session/request truth
- missing debug data is an observation gap, not a fallback source for business state
- sink failures are explicit and do not rewrite owner truth

## Shared Multi-Reference Functions

- `DebugStateSnapshot::new`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: construct the minimal reusable debug projection shared by UI protocol and future module emitters
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: debug snapshot serialization, UI debug query/subscription tests
  - why shared: avoids each module defining a private debug snapshot DTO
- `DebugHub::emit`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: ingest debug events and fan them out to subscribers and sinks
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: fanout tests, sink dispatch tests
  - why shared: centralizes observation delivery instead of duplicating buses in each owner crate

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `DebugSemanticPosition` | `crates/freehand-debug/src/lib.rs` | carry semantic coordinates for debug correlation | feature/session/turn/trace/node identifiers | semantic debug position | module emitters | debug contract | bound |
| 02 | `DebugScenePosition` | `crates/freehand-debug/src/lib.rs` | carry scene coordinates for debug correlation | crate/file/function/artifact coordinates | scene debug position | module emitters | debug contract | bound |
| 03 | `DebugTraceEnvelope` | `crates/freehand-debug/src/lib.rs` | combine semantic and scene coordinates with optional hashes/artifact/timestamp | debug coordinates plus hash/timestamp metadata | trace envelope | module emitters/replay tools | debug contract | bound |
| 04 | `DebugStateSnapshot::new` | `crates/freehand-debug/src/lib.rs` | build UI-consumable read-only debug snapshot | semantic position plus scene position plus status/detail text | debug snapshot | module emitters/UI protocol tests | debug contract | bound |
| 05 | `DebugHub::emit` | `crates/freehand-debug/src/lib.rs` | fan out emitted debug events to subscribers and sinks | debug event | delivered debug event | owner modules | debug hub | bound |
| 06 | `DebugHub::subscribe` | `crates/freehand-debug/src/lib.rs` | register read-only debug subscribers | subscriber request | subscription handle | UI/debug tools | debug hub | bound |

## Sync Status Against Mainline Call

- debug core crate, reusable snapshot/envelope contracts, hub fanout, subscriber registration, and file/stdout sink classes are bound in code
- current landed emitters are `freehand-reason` lifecycle milestones; provider/node producers remain future integration work
- sink failures are explicit at `DebugHub::emit`, but current reason-side integration keeps them observation-only and does not promote them into reason truth
- generated wiki must be regenerated from `docs/mainline-calls/debug.core.json` when this function-map truth changes
