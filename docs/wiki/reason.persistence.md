# Wiki: `reason.persistence`

Generated from `docs/mainline-calls/reason.persistence.json`. Do not edit by hand.

- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/persistence.rs`
- function map: `docs/function-maps/reason.persistence.md`
- generated wiki: `docs/wiki/reason.persistence.md`
- test design: `docs/testing/reason.persistence.md`

## Resource Operation Backlinks

- runtime_command.append_tool_result
- session.restore
- session.append_turn_to_turn
- session.list_persisted
- session.list_persisted_page

## Request Mainline

- runtime opens `~/.freehand/state/turns/<agent_id>/<session_id>/` as the authoritative reason state directory
- session-owned rewrite truth is restored from `SessionHistory` snapshots
- turn execution consumes restored session truth through `ReasonTurnEngine::start_turn`
- runtime persistence appends semantic reason-ledger rows before advancing durable snapshot cursors
- terminal turn close materializes immutable turn truth files and only then updates derived UI and index sidecars
- provider raw ledgers may be appended for debug, but they are not part of the authoritative request or recovery chain
- tool-result memory entries append the complete owner-projected Markdown content as independent JSONL records under the configured memory path, outside session truth

## Response Mainline

- `SessionHistory` JSON/file helpers render and restore authoritative session rewrite truth
- reason persistence appends a reason-ledger row together with current session-history truth, then refreshes authoritative snapshots and derived sidecars
- reason persistence appends provider raw debug-ledger rows under `~/.freehand/ledgers/providers/<family>/<agent>/<session>/<turn>.jsonl` without mutating authoritative session truth
- reason persistence returns deterministic restore state from snapshot plus reason-ledger tail replay, or from reason-ledger-only rebuild when snapshots are missing or invalid
- reason persistence filters provider-visible `historical_turn:*` session-memory segments to the same effective active/closed logical turn set used by rollback-aware transcript truth, while preserving ordinary stable memory
- reason persistence returns non-UI turn-start snapshots from authoritative reason-ledger TurnStarted rows, respecting rollback markers, so owner workflows can recover first-round user intent even when effective UI snapshots coalesce later repair rounds
- reason persistence returns authoritative-only UI restore snapshots for daemon bootstrap without replaying historical reason ledgers; selected transcript queries preserve exact runtime-turn-N / runtime-turn-N-rM rounds, backfill incomplete authoritative snapshots from reason-ledger truth when available, and return inactive surviving authoritative snapshots with a visible reason_persistence_partial_ui_restore warning when the reason ledger is empty instead of claiming a complete transcript
- authoritative closed-turn restore consumes only `*.json` turn snapshots; leftover atomic temp files such as `*.tmp-*` are write artifacts, not session truth, and must not wedge daemon bootstrap
- terminal turn persistence yields immutable per-turn truth plus updated session cursor truth
- derived UI and index sidecars are regenerated from authoritative reason truth after durable writes complete
- persisted session index reads expose only derived index rows and session metadata sidecars for UI list/search projection; worker task transcripts are not promoted to top-level persisted user sessions by this owner operation
- session display metadata (`title`, `archived`) is persisted as reason-owned sidecar truth for multi-UI session management and stays separate from provider-visible session history
- session rollback appends a durable marker, filters effective transcript restore by logical turn key, and retains raw closed-turn files for audit; later durable writes rebuild derived sidecars from rollback-filtered effective turns while raw rolled-back files remain reserved for audit and id allocation
- tool-result memory append creates parent directories, appends one serialized entry, syncs the file, and reloads independently after session archive/delete or dispatcher restart

## Error Mainline

- invalid persisted snapshot JSON is rejected explicitly
- invalid closed-turn `*.json` payloads are rejected explicitly with the file path in the parse error, while non-`.json` atomic temp files in the turns directory are ignored as non-authoritative write artifacts
- invalid persisted snapshot coherence is rejected explicitly
- reason-ledger sequence gaps or duplicate sequence numbers must block recovery
- provider raw payload availability alone must not mask missing authoritative reason truth
- UI sidecar presence alone must not be treated as session-truth recovery evidence
- active incomplete authoritative UI snapshots with an empty or retained-offset reason ledger remain explicit restore errors; only inactive surviving snapshots may be displayed, and only with a visible partial-transcript warning
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
- `annotate_incomplete_authoritative_ui_restore`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: add an explicit integrity warning to the latest inactive surviving authoritative UI snapshot when earlier round truth is missing and the reason ledger is empty
  - allowed callers: selected-session UI restore path only
  - related tests: ui_restore_returns_inactive_partial_authoritative_snapshots_with_integrity_warning, ui_restore_keeps_active_incomplete_authoritative_snapshot_as_hard_error_without_ledger
  - why shared: partial transcript visibility must stay a reason-owned contract warning, not a WebUI/browser guess or provider-raw recovery path
- `ReasonPersistence::rollback_latest_session_turn`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: append one rollback marker for the latest effective logical user turn and return audit data without deleting raw turn files
  - allowed callers: runtime UI command dispatch owner, owner-crate tests
  - related tests: rollback_latest_session_turn_is_append_only_and_filters_effective_transcript, rollback_latest_session_turn_rejects_no_target_and_active_turn, repeated_rollback_steps_backward_through_effective_turns_then_fails
  - why shared: session rollback must be one durable reason-owned truth shared by WebUI, daemon, and CLI instead of a client-local transcript edit
- `ReasonPersistence::restore_turn_start_snapshots`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: expose authoritative turn-start request truth without UI/effective-round coalescing while honoring rollback markers
  - allowed callers: runtime parent-goal evaluation, owner-crate tests
  - related tests: restore_turn_start_snapshots_preserves_original_round_and_respects_rollback, production_master_runner_recovers_parent_goal_from_first_round_turn_start_ledger
  - why shared: background lifecycle owners sometimes need first-round user intent, not the latest repaired UI row; parsing reason ledgers outside the persistence owner would duplicate recovery semantics
- `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: expose authoritative UI turn snapshots for daemon bootstrap without parsing historical reason ledgers or atomic temp files
  - allowed callers: runtime UI bootstrap, owner-crate tests, Master parent-workset reconciliation
  - related tests: live_bootstrap_does_not_replay_incomplete_historical_reason_ledgers, restore_ignores_leftover_atomic_tmp_turn_files, production_master_runner_rechecks_stale_blocked_parent_marker_after_rollback
  - why shared: global startup must remain bounded by authoritative snapshot files, while selected transcript queries own heavier ledger backfill
- `ReasonPersistence::raw_authoritative_turn_snapshots / ReasonPersistence::reserved_authoritative_turn_ids`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: expose raw authoritative turn files and reserved turn ids, including rolled-back audit files, without making them effective transcript truth
  - allowed callers: runtime parent-workset turn allocator/idempotency repair, owner-crate tests
  - related tests: rollback_marker_does_not_resurrect_raw_turns_when_later_turn_is_persisted, production_master_runner_rechecks_stale_blocked_parent_marker_after_rollback
  - why shared: runtime must allocate a fresh follow-up turn after rollback without parsing reason-owner directories or resurrecting rolled-back transcript truth
- `filter_history_context_to_effective_turns`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: remove rolled-back or orphan `historical_turn:*` session-memory segments before restored session truth can feed a future provider request
  - allowed callers: persistence restore, ledger replay, durable row materialization, owner-crate tests
  - related tests: rollback_filters_model_visible_history_to_effective_turn_truth
  - why shared: snapshot restore, ledger-only rebuild, and rollback persistence must use one effective-turn filter instead of drifting independently
- `ReasonPersistence::authoritative_turn_snapshots_fingerprint`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: serve a deterministic change fingerprint over every authoritative turn-snapshot file (history, cursor, active, closed turns, rollback markers) plus the monotonic cursor sequence so runtime can use it as a conservative cache key; file names participate in the key so same-mtime/same-size content changes and snapshot renames invalidate the entry; runtime never reads the persistence file layout or coerces IO errors into valid cache keys
  - allowed callers: Master parent-workset reconciliation, owner-crate tests
  - related tests: authoritative_turn_snapshots_fingerprint_covers_all_files
  - why shared: authoritative session-history metadata must be served by the persistence owner so runtime never reads the file path itself or coerces IO errors into valid cache keys

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
| 12h | `filter_history_context_to_effective_turns` | `crates/freehand-reason/src/persistence.rs` | filter model-visible historical-turn session memory to effective active/closed logical turn truth | restored session history plus active/closed turns | session history without rolled-back or orphan historical-turn memory | restore / ledger replay / row persistence | persistence owner |  |  |  | bound |
| 13 | `ReasonPersistence::restore_turn_start_snapshots` | `crates/freehand-reason/src/persistence.rs` | restore authoritative turn-start snapshots from reason-ledger truth without UI coalescing and filter rolled-back logical turns | session id plus reason ledger and rollback markers | ordered turn-start request truth | runtime parent-goal evaluation | persistence owner |  |  |  | bound |
| 14 | `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui` | `crates/freehand-reason/src/persistence.rs` | restore derived UI snapshots from authoritative closed/active `*.json` turn files plus rollback-marker sidecar truth without replaying the reason ledger or parsing leftover atomic temp files | authoritative turn snapshots plus rollback markers | lightweight exact-per-file UI snapshots with created-time truth | runtime bootstrap | persistence owner |  |  |  | bound |
| 14a | `ReasonPersistence::authoritative_turn_snapshots_fingerprint` | `crates/freehand-reason/src/persistence.rs` | return a deterministic change fingerprint over every authoritative turn-snapshot file plus the monotonic cursor sequence; Ok(None) when no authoritative truth exists yet, Err on real filesystem failures; file names participate in the key so same-mtime/same-size content changes and renames invalidate cache entries | session id | Ok(None) or Ok(Some((mtime_nanos, size_bytes))) or Err | runtime Master parent-workset reconciliation cache | persistence owner |  |  |  | bound |
| 14r | `ReasonPersistence::raw_authoritative_turn_snapshots / ReasonPersistence::reserved_authoritative_turn_ids` | `crates/freehand-reason/src/persistence.rs` | read raw authoritative closed/active turn files and cursor-reserved ids, including rolled-back logical turns, for id reservation and stale-idempotency diagnosis without changing effective transcript truth | session id plus authoritative turn files, active snapshot, and cursor sidecar | raw turn snapshots or reserved turn ids; callers must not project them as effective UI transcript truth | runtime Master parent-workset reconciliation | persistence owner |  |  |  | bound |
| 14q | `ReasonPersistence::restore_turn_snapshots_for_ui` | `crates/freehand-reason/src/persistence.rs` | restore selected transcript snapshots from authoritative turn files, rebuild exact per-round UI snapshots from reason ledger when authoritative snapshot truth is absent or missing earlier observed rounds, and allow inactive surviving authoritative snapshots only with an explicit partial-transcript warning when the ledger is empty or retained at an unusable sequence offset | session id plus authoritative turn snapshots, reason ledger rows, and rollback markers | exact runtime-turn-N / runtime-turn-N-rM UI snapshots with rolled-back logical turns filtered, or inactive partial snapshots carrying reason_persistence_partial_ui_restore; active incomplete snapshots still fail, including retained-offset ledgers | selected QuerySessionTurns / rollback refresh | persistence owner |  |  |  | bound |
| 15 | `ReasonPersistence::create_session_metadata / ReasonPersistence::rename_session / ReasonPersistence::archive_session / ReasonPersistence::restore_session / ReasonPersistence::delete_session` | `crates/freehand-reason/src/persistence.rs` | persist shared session display metadata mutations without mutating turn transcript truth | session id plus metadata mutation intent | updated session metadata sidecar | runtime UI command dispatch | persistence owner |  |  |  | bound |
| 16 | `ReasonPersistence::rollback_latest_session_turn` | `crates/freehand-reason/src/persistence.rs` | append latest-logical-turn rollback marker and advance effective cursor/projection state without deleting raw turn files | session id | rollback marker with target turn, previous effective head, and restored user text | runtime UI command dispatch | persistence owner |  |  |  | bound |
| 18 | `ReasonPersistence::list_persisted_sessions` | `crates/freehand-reason/src/persistence.rs` | expose derived persisted session index rows and session metadata sidecar truth for runtime search projection, startup stale-lifecycle recovery, and bootstrap UI projection restore without reading provider raw ledgers or treating worker transcripts as global sessions | session index sidecar plus metadata sidecar | persisted session index/metadata rows for runtime search projection | runtime.ui-command-dispatch QuerySessionSearch / recover_stale_lifecycle_waits_on_bootstrap / restore_all_persisted_sessions_into_ui | persistence owner | session | ui_projection | session.list_persisted | bound |
| 18p | `ReasonPersistence::list_persisted_sessions_page` | `crates/freehand-reason/src/persistence.rs` | maintain the versioned session summary index and return one ordered metadata-only page with an opaque cursor; unavailable poisoned sessions stay explicit facts without blocking other rows | archived space plus latest/older request plus summary index/metadata truth | bounded ReasonSessionListPage with page facts and unavailable ids | runtime.ui-command-dispatch QuerySessionListPage | persistence owner | session | ui_projection | session.list_persisted_page | bound |
| 19m | `ReasonPersistence::append_tool_result_memory` | `crates/freehand-reason/src/persistence.rs` | append the complete tool-result Markdown plus typed session/turn/tool identity to the config-selected durable memory JSONL | memory path plus session id, optional turn id, optional tool call id, and complete content | persisted ToolResultMemoryEntry with explicit created_at_unix_seconds | runtime.ui-command-dispatch AddToMemory | persistence owner | runtime_command | memory | runtime_command.append_tool_result | bound |

## Sync Status Against Mainline Call

- current code baseline now binds session-history JSON/file round-trip, reason-ledger append, provider-raw debug-ledger append, active-turn refresh, terminal turn materialization, derived sidecar writes, snapshot-plus-tail / ledger-only recovery, effective historical-turn session-memory filtering, turn-start snapshot recovery, authoritative-only UI bootstrap restore, selected transcript exact-round ledger backfill, inactive partial restore warnings, active incomplete restore rejection, and atomic temp exclusion from closed-turn truth
- CLI and shared-harness smoke both bind to the persistence owner path without duplicating persistence semantics in the app layer
- live Anthropic `reason-live` path now persists start/output/rejection/terminal events plus provider raw debug bodies/events through `ReasonPersistence`
- generated wiki must be regenerated from `docs/mainline-calls/reason.persistence.json` when this function-map truth changes
- session.list_persisted is bound through ReasonPersistence::list_persisted_sessions and load_session_metadata for UI-safe list/search projections
- session.list_persisted_page is bound through ReasonPersistence::list_persisted_sessions_page and load_or_migrate_session_summary_index for metadata-only cursor paging
- reserved raw turn ids and retained-offset partial UI restore are covered by reason/runtime stale lifecycle tests
