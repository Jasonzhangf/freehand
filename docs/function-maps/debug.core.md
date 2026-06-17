# Function Map: `debug.core`

- feature_id: `debug.core`
- owner crate: `crates/freehand-debug`
- owner module: `crates/freehand-debug/src/lib.rs`
- owner entry symbols:
  - `DebugObservationFailure`
  - `DebugSemanticPosition`
  - `DebugScenePosition`
  - `DebugTraceEnvelope`
  - `DebugStateSnapshot`
  - `DebugStateSnapshot::new`
  - `DebugEvent`
  - `DebugSinkKind`
  - `DebugHub`
  - `DebugHub::emit`
  - `DebugHub::subscribe`
  - `DebugHub::subscribe_failures`

## Request Mainline

- owner modules create debug semantic position and scene position from their own truth
- owner modules build debug trace envelopes or debug snapshots through `freehand-debug`
- debug material is observation data, not request content
- owner modules emit debug events into `DebugHub`
- sink-dispatch failures are surfaced through a dedicated observation-failure stream without mutating owner truth

## Response Mainline

- debug snapshots may be consumed by UI protocol as read-only projection data
- trace envelopes may be persisted or replayed by future debug/runtime writers
- consumers preserve semantic and scene coordinates together
- `DebugHub` fans out to subscribers and sinks without mutating owner truth
- failure subscribers can observe sink-dispatch failures with the original event envelope and sink classification
- file sinks append replay-safe JSONL entries instead of overwriting prior debug evidence

## Error Mainline

- empty required debug fields are explicit construction errors only when builder helpers validate them
- debug artifacts must not be promoted into successful reason/session/request truth
- missing debug data is an observation gap, not a fallback source for business state
- sink failures are explicit, do not rewrite owner truth, and are emitted through `DebugHub::subscribe_failures`
- disabled hubs do not dispatch to subscribers or sinks

## Shared Multi-Reference Functions

- `DebugStateSnapshot::new`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: construct the minimal reusable debug projection shared by UI protocol and future module emitters
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: debug snapshot serialization and UI debug query/subscription tests
  - why shared: avoids each module defining a private debug snapshot DTO
- `DebugHub::emit`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: ingest debug events and fan them out to subscribers and sinks
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: fanout and sink dispatch tests
  - why shared: centralizes observation delivery instead of duplicating buses in each owner crate
- `DebugHub::subscribe_failures`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: expose a dedicated observation-failure stream for sink-dispatch failures
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: sink failure surfacing tests, reason producer observation-failure smoke
  - why shared: keeps observation failures separate from business error truth while remaining queryable

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `DebugSemanticPosition` | `crates/freehand-debug/src/lib.rs` | carry semantic coordinates for debug correlation | feature/session/turn/trace/node identifiers | semantic debug position | module emitters | debug contract | bound |
| 02 | `DebugScenePosition` | `crates/freehand-debug/src/lib.rs` | carry scene coordinates for debug correlation | crate/file/function/artifact coordinates | scene debug position | module emitters | debug contract | bound |
| 03 | `DebugTraceEnvelope` | `crates/freehand-debug/src/lib.rs` | combine semantic and scene coordinates with optional hashes/artifact/timestamp | debug coordinates + hash/timestamp metadata | trace envelope | module emitters/replay tools | debug contract | bound |
| 04 | `DebugStateSnapshot::new` | `crates/freehand-debug/src/lib.rs` | build UI-consumable read-only debug snapshot | semantic position + scene position + status/detail text | debug snapshot | module emitters/UI protocol tests | debug contract | bound |
| 05 | `DebugHub::emit` | `crates/freehand-debug/src/lib.rs` | fan out emitted debug events to subscribers and sinks | debug event | delivered debug event | owner modules | debug hub | bound |
| 06 | `DebugHub::subscribe` | `crates/freehand-debug/src/lib.rs` | register read-only debug subscribers | subscriber request | subscription handle | UI/debug tools | debug hub | bound |
| 07 | `DebugHub::subscribe_failures` | `crates/freehand-debug/src/lib.rs` | register read-only subscribers for observation failures | failure-subscriber request | observation-failure subscription handle | reason/provider/node/UI debug tools | debug hub | bound |

## Sync Status Against Code

- debug core crate, reusable snapshot/envelope contracts, hub fanout, subscriber registration, and file/stdout sink classes are bound in code
- dedicated observation-failure stream is bound in code through `DebugObservationFailure` and `DebugHub::subscribe_failures`
- current landed emitters are `freehand-reason` lifecycle milestones plus runtime-owned `provider.reason-live-bridge` restore/request/tool/terminal boundaries; node producers and direct provider-adapter emitters remain future integration work
- sink failures are explicit at `DebugHub::emit`, surface through the dedicated observation-failure stream, and current reason-side integration keeps them observation-only without promoting them into reason truth
- direct white-box locks now cover file-sink append semantics, real file-io failure surfacing, and disabled-hub no-dispatch behavior
- migrated mainline-call source now lives at `docs/mainline-calls/debug.core.json` and generated wiki lives at `docs/wiki/debug.core.md`
