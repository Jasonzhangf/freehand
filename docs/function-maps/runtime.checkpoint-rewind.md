# Function Map: `runtime.checkpoint-rewind`

- feature_id: `runtime.checkpoint-rewind`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- mainline call source: `docs/mainline-calls/runtime.checkpoint-rewind.json`
- generated wiki: `docs/wiki/runtime.checkpoint-rewind.md`
- owner entry symbols:
  - `RuntimeCheckpointStore::new`
  - `RuntimeCheckpointStore::create_from_preview`
  - `RuntimeCheckpointStore::list_summaries`
  - `list_checkpoints`
  - `execute_registry_tool_call`
  - `rewind_checkpoint`

## Request Mainline

- runtime receives a writable tool call during live execution
- runtime requests canonical preview truth from `tool.preview`
- runtime derives affected locked paths from preview truth
- runtime snapshots the pre-image set under one checkpoint id before tool execution
- runtime executes the writable tool only after checkpoint creation succeeds
- future explicit rewind requests route back into the runtime owner, not into UI or tool owners

## Response Mainline

- checkpoint creation returns manifest and ledger truth bound to agent/session/turn/tool-call identity
- writable tool execution is associated with one checkpoint lifecycle record and ledger rows for created/applied/failed
- read-only checkpoint query returns runtime-owned summaries from manifest plus ledger truth
- explicit rewind restores the pre-image set into the locked workspace root
- runtime emits checkpoint status for debug and future projection consumers without changing reason truth ownership

## Error Mainline

- preview-unavailable writable tools return explicit rejection
- snapshot failure blocks writable execution explicitly
- restore failure, missing manifest, missing blob, or ledger corruption block explicitly
- checkpoint query failure is explicit and never falls back to UI projection truth
- checkpoint truth must not be reconstructed from UI projections or provider raw ledgers

## Shared Multi-Reference Functions

- `RuntimeCheckpointStore::write_manifest`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: persist one runtime-owned checkpoint manifest and associated path set atomically
  - allowed callers: checkpoint create path, checkpoint restore path, runtime tests
  - related tests: checkpoint manifest round-trip tests
  - why shared: checkpoint metadata write semantics must stay single-sourced
- `RuntimeCheckpointStore::append_ledger_row`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: append create / apply / restore / discard lifecycle rows in one owner path
  - allowed callers: checkpoint create path, runtime restore path
  - related tests: checkpoint ledger lifecycle tests
  - why shared: runtime checkpoint audit must not be duplicated across entrypoints

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCheckpointStore::new` | `crates/freehand-runtime/src/lib.rs` | bind runtime home checkpoint directories for one selected agent runtime | runtime config plus selected agent | checkpoint store | runtime bootstrap and tests | checkpoint owner | bound |
| 02 | `BuiltinToolRegistry::preview` | `crates/freehand-tools/src/lib.rs` | request writable-tool preview before any side effect | writable tool call | canonical preview truth | live bridge/tool loop | tool preview owner | bound |
| 03 | `RuntimeCheckpointStore::create_from_preview` | `crates/freehand-runtime/src/lib.rs` | snapshot previewed pre-image set and write checkpoint manifest | preview truth plus turn identity | checkpoint manifest plus created ledger row | tool loop | checkpoint owner | bound |
| 04 | `execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | call `tool.registry` execute only after checkpoint succeeds for writable tools | checkpoint id plus writable tool call | tool result plus applied ledger row | tool loop | tool registry owner | bound |
| 05 | `rewind_checkpoint` | `crates/freehand-runtime/src/lib.rs` | restore one checkpoint pre-image set into the locked workspace root | checkpoint id | restored workspace plus restore ledger row | future CLI/UI/runtime command | checkpoint owner | bound |
| 06 | `list_checkpoints` / `RuntimeCheckpointStore::list_summaries` | `crates/freehand-runtime/src/lib.rs` | read manifest plus ledger truth into safe checkpoint summaries | runtime home plus agent/session ids | checkpoint summary list | runtime dispatcher and tests | checkpoint owner | bound |

## Metadata / Request Isolation Notes

- checkpoint manifests and ledgers are runtime filesystem truth, not reason turn truth
- checkpoint status may be projected to debug or UI later, but those projections are not recovery truth
- checkpoint metadata must not mutate session history or turn truth implicitly
- UI checkpoint summaries are read-only projections; rewind still reloads runtime checkpoint truth from disk

## Sync Status Against Code

- design truth is locked
- current runtime code has live tool execution with checkpoint + rewind owner path now code-bound
- checkpoint summary query/projection is runtime-owned and code-bound
- current reason persistence remains authoritative for session/turn truth and is intentionally separate from checkpoint restore truth
- generated wiki must be regenerated from `docs/mainline-calls/runtime.checkpoint-rewind.json` when this function-map truth changes
