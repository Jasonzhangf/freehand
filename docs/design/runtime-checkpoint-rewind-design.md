# Runtime Checkpoint Rewind Design

## Scope

This doc locks the runtime-owned checkpoint and rewind lifecycle for writable tools.

- feature_id: `runtime.checkpoint-rewind`
- owner: `crates/freehand-runtime`
- upstream semantic owners:
  - `tool.preview`
  - `tool.registry`
  - `reason.persistence`
  - `reason.session-history`
- non-owners:
  - app crates
  - provider adapter crates
  - `crates/freehand-reason`

## Reference Evidence

Reasonix evidence:

- `../Deepseek-reasonix/internal/control/controller.go`
  - controller owns checkpoint store binding, pre-edit snapshot hook, and reopen-safe checkpoint metadata
- `../Deepseek-reasonix/internal/tool/builtin/preview.go`
  - preview runs before write so checkpoint scope is known before side effects

Current Freehand evidence:

- `crates/freehand-runtime`
  - owns live tool execution composition
- `crates/freehand-reason/src/persistence.rs`
  - owns session/turn truth persistence only, not workspace filesystem restore truth

## Core Truth

- `freehand-runtime` owns writable-tool checkpoint snapshots and rewind lifecycle.
- `freehand-reason` remains the only owner of session and turn truth.
- Checkpoint state is runtime filesystem truth, not session truth.
- A writable tool may enter checkpointed live execution only if:
  - the tool is implemented in `tool.registry`
  - the tool has preview truth in `tool.preview`
  - runtime successfully snapshots the previewed pre-image set

## Runtime Paths

Checkpoint runtime evidence is fixed to:

- `~/.freehand/state/checkpoints/<agent_id>/<session_id>/`
- `~/.freehand/ledgers/checkpoints/<agent_id>/<session_id>.jsonl`

First-version checkpoint state contains:

- manifest files keyed by checkpoint id
- pre-image file blobs or copied files for previewed writable paths
- append-only ledger rows for create / applied / restore / discard lifecycle

## First-Version Lifecycle

### Pre-write checkpoint

1. runtime receives a completed writable tool call from the live bridge
2. runtime asks `tool.preview` for canonical change truth
3. runtime derives the affected path set from preview truth
4. runtime snapshots the pre-image of each affected path under a checkpoint id bound to:
   - `agent_id`
   - `session_id`
   - `turn_id`
   - `tool_call_id`
5. runtime appends a checkpoint-created ledger row
6. runtime executes the writable tool through `tool.registry`
7. runtime appends applied or failed checkpoint outcome

### Rewind

1. runtime receives an explicit rewind request for one checkpoint id
2. runtime restores the recorded pre-image set into the locked workspace root
3. runtime appends a checkpoint-restored ledger row
4. runtime emits explicit runtime/debug status

## Boundary Rules

- reason persistence and checkpoint persistence are separate stores
- checkpoint restore must not mutate session history, turn truth, or UI truth directly
- provider raw ledgers remain unrelated to checkpoint recovery
- UI may render checkpoint status, but it must not perform restore logic
- conversation rewind or turn truncation is not part of v1; this lock is for workspace mutation rewind

## Error And Rewrite Interaction

- writable-tool failure must stay explicit; runtime must not auto-hide it by restoring silently
- rewind is an explicit action, not a fallback
- invalid preview, missing snapshot, restore mismatch, or ledger corruption must block explicitly
- rewrite/compaction remains owned by `reason.rewrite-policy` and `reason.context-planner`
- when rewrite summarizes history after tool failures or rewinds, the summary contract must preserve:
  - `Errors & fixes`
  - `Commands & outcomes`
  - `Pending & next step`

## Test Direction

- white-box:
  - checkpoint manifest render/load tests
  - preview-derived path-set snapshot tests
  - create / modify / delete pre-image capture tests
  - restore success tests
  - restore missing-manifest or missing-blob rejection tests
  - no-preview no-checkpoint rejection tests
- module black-box:
  - runtime writable execution creates checkpoint before tool execute
  - runtime explicit rewind restores prior workspace state
  - runtime restart can still inspect checkpoint ledger truth
- project black-box:
  - CLI or daemon live writable-tool smoke can mutate then rewind via runtime owner path

## Non-Goals For This Design Lock

- conversation fork UI
- approval-card UX
- distributed/shared checkpoint storage
- background-job workspace snapshots

## Update Rule

If writable-tool restore lifecycle changes, update in the same change set:

- `docs/architecture/feature-map.md`
- `docs/function-maps/runtime.checkpoint-rewind.md`
- `docs/testing/runtime.checkpoint-rewind.md`
- this design doc
- `docs/mainline-calls/runtime.checkpoint-rewind.json`
- generated wiki from `xtask mainlines generate`
