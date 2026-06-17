# Wiki: `runtime.checkpoint-rewind`

Generated from `docs/mainline-calls/runtime.checkpoint-rewind.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.checkpoint-rewind.md`
- generated wiki: `docs/wiki/runtime.checkpoint-rewind.md`
- test design: `docs/testing/runtime.checkpoint-rewind.md`

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
- explicit rewind restores the pre-image set into the locked workspace root
- runtime emits checkpoint status for debug and future projection consumers without changing reason truth ownership

## Error Mainline

- preview-unavailable writable tools return explicit rejection
- snapshot failure blocks writable execution explicitly
- restore failure, missing manifest, missing blob, or ledger corruption block explicitly
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

## Sync Status Against Mainline Call

- design truth is locked
- current runtime code has live tool execution with checkpoint + rewind owner path now code-bound
- current reason persistence remains authoritative for session/turn truth and is intentionally separate from checkpoint restore truth
- generated wiki must be regenerated from `docs/mainline-calls/runtime.checkpoint-rewind.json` when this function-map truth changes
