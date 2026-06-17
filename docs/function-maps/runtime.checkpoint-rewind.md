# Function Map: `runtime.checkpoint-rewind`

- feature_id: `runtime.checkpoint-rewind`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- mainline call source: `docs/mainline-calls/runtime.checkpoint-rewind.json`
- generated wiki: `docs/wiki/runtime.checkpoint-rewind.md`
- owner entry symbols:
  - `pending: checkpoint store bootstrap in freehand-runtime`
  - `pending: writable tool pre-execute checkpoint path in freehand-runtime`
  - `pending: explicit rewind entrypoint in freehand-runtime`

## Request Mainline

- runtime receives a writable tool call during live execution
- runtime requests canonical preview truth from `tool.preview`
- runtime derives affected locked paths from preview truth
- runtime snapshots the pre-image set under one checkpoint id before tool execution
- runtime executes the writable tool only after checkpoint creation succeeds
- future explicit rewind requests route back into the runtime owner, not into UI or tool owners

## Response Mainline

- checkpoint creation returns manifest and ledger truth bound to agent/session/turn/tool-call identity
- writable tool execution is associated with one checkpoint lifecycle record
- explicit rewind restores the pre-image set into the locked workspace root
- runtime emits checkpoint status for debug and future projection consumers without changing reason truth ownership

## Error Mainline

- preview-unavailable writable tools return explicit rejection
- snapshot failure blocks writable execution explicitly
- restore failure, missing manifest, or ledger corruption block explicitly
- checkpoint truth must not be reconstructed from UI projections or provider raw ledgers

## Shared Multi-Reference Functions

- `pending: checkpoint manifest writer`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: persist one runtime-owned checkpoint manifest and associated path set atomically
  - allowed callers: checkpoint create path, checkpoint restore path, runtime tests
  - related tests: checkpoint manifest round-trip tests
  - why shared: checkpoint metadata write semantics must stay single-sourced
- `pending: checkpoint ledger append helper`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: append create / apply / restore / discard lifecycle rows in one owner path
  - allowed callers: checkpoint create path, runtime restore path
  - related tests: checkpoint ledger lifecycle tests
  - why shared: runtime checkpoint audit must not be duplicated across entrypoints

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `pending: checkpoint store bootstrap` | `crates/freehand-runtime/src/lib.rs` | bind runtime home checkpoint directories for one selected agent runtime | runtime config plus selected agent | checkpoint store | runtime bootstrap | checkpoint owner | pending |
| 02 | `pending: preview-before-write path` | `crates/freehand-runtime/src/lib.rs` | request writable-tool preview before any side effect | writable tool call | canonical preview truth | live bridge/tool loop | tool preview owner | pending |
| 03 | `pending: create checkpoint from preview` | `crates/freehand-runtime/src/lib.rs` | snapshot previewed pre-image set and write checkpoint manifest | preview truth plus turn identity | checkpoint manifest plus created ledger row | tool loop | checkpoint owner | pending |
| 04 | `pending: execute writable tool with checkpoint` | `crates/freehand-runtime/src/lib.rs` | call `tool.registry` execute only after checkpoint succeeds | checkpoint id plus writable tool call | tool result plus applied ledger row | tool loop | tool registry owner | pending |
| 05 | `pending: rewind checkpoint` | `crates/freehand-runtime/src/lib.rs` | restore one checkpoint pre-image set into the locked workspace root | checkpoint id | restored workspace plus restore ledger row | future CLI/UI/runtime command | checkpoint owner | pending |

## Metadata / Request Isolation Notes

- checkpoint manifests and ledgers are runtime filesystem truth, not reason turn truth
- checkpoint status may be projected to debug or UI later, but those projections are not recovery truth
- checkpoint metadata must not mutate session history or turn truth implicitly

## Sync Status Against Code

- design truth is locked
- current runtime code has live tool execution but no code-bound checkpoint or rewind owner path yet
- current reason persistence remains authoritative for session/turn truth and is intentionally separate from checkpoint restore truth
- generated wiki must be regenerated from `docs/mainline-calls/runtime.checkpoint-rewind.json` when this function-map truth changes
