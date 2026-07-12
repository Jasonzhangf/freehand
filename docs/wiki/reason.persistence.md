# Wiki: `reason.persistence`

Generated from `docs/mainline-calls/reason.persistence.json`. Do not edit by hand.

- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/persistence.rs`
- function map: `docs/function-maps/reason.persistence.md`
- generated wiki: `docs/wiki/reason.persistence.md`
- test design: `docs/testing/reason.persistence.md`

## Resource Operation Backlinks

- session.append_turn_to_turn

## Request Mainline

- runtime opens `~/.freehand/state/turns/<agent_id>/<session_id>/` as the authoritative reason state directory
- session-owned rewrite truth is restored from `SessionHistory` snapshots
- turn execution consumes restored session truth through `ReasonTurnEngine::start_turn`
- runtime persistence appends semantic reason-ledger rows before advancing durable snapshot cursors
- terminal turn close materializes immutable turn truth files and only then updates derived UI and index sidecars
- provider raw ledgers may be appended for debug, but they are not part of the authoritative request or recovery chain

## Response Mainline

- `SessionHistory` JSON/file helpers render and restore authoritative session rewrite truth
- reason persistence appends a reason-ledger row together with current session-history truth, then refreshes authoritative snapshots and derived sidecars
- reason persistence appends provider raw debug-ledger rows under `~/.freehand/ledgers/providers/<family>/<agent>/<session>/<turn>.jsonl` without mutating authoritative session truth
- reason persistence returns deterministic restore state from snapshot plus reason-ledger tail replay, or from reason-ledger-only rebuild when snapshots are missing or invalid
- terminal turn persistence yields immutable per-turn truth plus updated session cursor truth
- derived UI and index sidecars are regenerated from authoritative reason truth after durable writes complete
- session display metadata (`title`, `archived`) is persisted as reason-owned sidecar truth for multi-UI session management and stays separate from provider-visible session history
- session rollback appends a durable marker, filters effective transcript restore by logical turn key, and retains raw closed-turn files for audit

## Error Mainline

- invalid persisted snapshot JSON is rejected explicitly
- invalid persisted snapshot coherence is rejected explicitly
- reason-ledger sequence gaps or duplicate sequence numbers must block recovery
- provider raw payload availability alone must not mask missing authoritative reason truth
- UI sidecar presence alone must not be treated as session-truth recovery evidence
- session metadata mutation targets that do not exist fail explicitly
- rollback with no eligible target or with an active turn fails explicitly without deleting raw turn truth

## Shared Multi-Reference Functions

- `SessionHistory::persist_json`
  - owner: `crates/freehand-reason/src/session_history.rs`
  - purpose: render authoritative session rewrite truth as a persistable JSON snapshot
  - allowed callers: runtime persistence owner, replay/debug tools, owner-crate tests
  - related tests: persisted JSON round-trip, snapshot render/load tests
  - why shared: authoritative session snapshot rendering must stay centralized
- `SessionHistory::from_persisted_json`
  - owner: `crates/freehand-reason/src/session_history.rs`
  - purpose: restore authoritative session rewrite truth from JSON while validating coherence
  - allowed callers: runtime persistence owner, replay/debug tools, owner-crate tests
  - related tests: invalid persisted state rejection, persisted JSON round-trip
  - why shared: restore validation must stay aligned with the authoritative snapshot renderer
- `validate_rewrite_base_segments`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: validate stable rewrite base segments before session snapshots are accepted or restored
  - allowed callers: freehand-reason, owner-crate tests
  - related tests: rewrite-base validation, persisted coherence rejection
  - why shared: rewrite-base semantic validation must not be duplicated in persistence coordinators
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: compute metadata-side cache diagnostics stored in rewrite records and recovery evidence
  - allowed callers: freehand-reason, owner-crate tests, replay/debug tools
  - related tests: rewrite diagnostics snapshot tests, recovery audit tests
  - why shared: cache-shape evidence must stay aligned between planner runtime and persisted rewrite records
- `write_json_atomic`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: atomically replace authoritative snapshots and derived sidecars after durable ledger append
  - allowed callers: reason persistence owner, owner-crate tests
  - related tests: atomic snapshot replace, save/load smoke
  - why shared: all persistence file writes must use one atomic replacement path instead of ad hoc writes
- `ReasonPersistence::rollback_latest_session_turn`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: append one rollback marker for the latest effective logical user turn and return audit data without deleting raw turn files
  - allowed callers: runtime UI command dispatch owner, owner-crate tests
  - related tests: rollback_latest_session_turn_is_append_only_and_filters_effective_transcript, rollback_latest_session_turn_rejects_no_target_and_active_turn, repeated_rollback_steps_backward_through_effective_turns_then_fails
  - why shared: session rollback must be one durable reason-owned truth shared by WebUI, daemon, and CLI instead of a client-local transcript edit

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `SessionHistory::load_from_path` | `crates/freehand-reason/src/session_history.rs` | restore authoritative session rewrite snapshot from disk | session-history snapshot file | validated session rewrite truth | runtime/bootstrap | session-history owner |  |  |  | bound |
| 02 | `SessionHistory::from_persisted_json` | `crates/freehand-reason/src/session_history.rs` | validate restored session rewrite JSON | session-history JSON payload | validated session rewrite truth | persistence loader/debug tools | session-history owner |  |  |  | bound |
| 03 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | consume restored session truth for turn startup | restored session history plus turn input | initialized turn record plus provider payload | runtime/live bridge | reason owner |  |  |  | bound |
| 04 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize semantic outputs into turn truth before persistence projection | provider semantic output | updated turn truth | runtime/live bridge | reason owner |  |  |  | bound |
| 05 | `SessionHistory::commit_turn_start` | `crates/freehand-reason/src/session_history.rs` | consume one-shot non-ordinary rewrite state after successful startup | active turn id | updated session rewrite truth | reason owner | session-history owner |  |  |  | bound |
| 06 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | append turn-start ledger row, refresh active-turn snapshot, and update cursor/sidecars | session history plus started turn truth | durable reason state for running turn | runtime/live bridge/testkit | persistence owner | session | turn | session.append_turn_to_turn | bound |
| 07 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | append provider-output ledger row and refresh active-turn snapshot | session history plus updated turn truth plus provider semantic output | durable active-turn truth | runtime/live bridge/testkit | persistence owner |  |  |  | bound |
| 08 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | append schema-rejection ledger row and refresh active-turn rejection counter | session history plus updated turn truth plus rejection | durable rejection evidence | runtime/live bridge/testkit | persistence owner |  |  |  | bound |
| 09 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | append terminal ledger row, materialize immutable turn truth, clear active-turn snapshot, and update sidecars | session history plus terminal turn truth | durable closed-turn truth | runtime/live bridge/testkit | persistence owner |  |  |  | bound |
| 10 | `ReasonPersistence::record_rewrite_state_updated` | `crates/freehand-reason/src/persistence.rs` | append rewrite-state ledger row and refresh session snapshots | updated session-history truth | durable rewrite-state persistence | rewrite runtime / recovery path | persistence owner |  |  |  | bound |
| 11 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger rows without mutating authoritative turn/session truth | provider family + session/turn/trace identity + raw wire body + scene provenance | durable provider raw debug evidence | runtime/live bridge | persistence owner |  |  |  | bound |
| 12 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | rebuild authoritative state from snapshots plus reason-ledger tail, or from ledger alone | snapshot directory plus reason ledger | restored in-memory session and turn truth | runtime/bootstrap/testkit/CLI smoke | persistence owner |  |  |  | bound |
| 13 | `ReasonPersistence::restore_turn_snapshots_for_ui` | `crates/freehand-reason/src/persistence.rs` | rebuild latest per-turn snapshots from reason ledger while applying rollback markers to the effective UI transcript | reason ledger rows plus rollback markers | effective UI turn snapshots | runtime bootstrap / rollback refresh | persistence owner |  |  |  | bound |
| 14 | `ReasonPersistence::create_session_metadata / ReasonPersistence::rename_session / ReasonPersistence::archive_session / ReasonPersistence::restore_session / ReasonPersistence::delete_session` | `crates/freehand-reason/src/persistence.rs` | persist shared session display metadata mutations without mutating turn transcript truth | session id plus metadata mutation intent | updated session metadata sidecar | runtime UI command dispatch | persistence owner |  |  |  | bound |
| 15 | `ReasonPersistence::rollback_latest_session_turn` | `crates/freehand-reason/src/persistence.rs` | append latest-logical-turn rollback marker and advance effective cursor/projection state without deleting raw turn files | session id | rollback marker with target turn, previous effective head, and restored user text | runtime UI command dispatch | persistence owner |  |  |  | bound |

## Sync Status Against Mainline Call

- current code baseline now binds session-history JSON/file round-trip, reason-ledger append, provider-raw debug-ledger append, active-turn refresh, terminal turn materialization, derived sidecar writes, and snapshot-plus-tail / ledger-only recovery
- CLI and shared-harness smoke both bind to the persistence owner path without duplicating persistence semantics in the app layer
- live Anthropic `reason-live` path now persists start/output/rejection/terminal events plus provider raw debug bodies/events through `ReasonPersistence`
- generated wiki must be regenerated from `docs/mainline-calls/reason.persistence.json` when this function-map truth changes
