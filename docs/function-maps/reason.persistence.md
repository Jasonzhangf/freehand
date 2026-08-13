# Function Map: `reason.persistence`

- feature_id: `reason.persistence`
- owner crate: `crates/freehand-reason`
- owner module: `crates/freehand-reason/src/persistence.rs`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `session.restore`
  - `session.append_turn_to_turn`
  - `session.list_persisted`
- owner entry symbols:
  - `ReasonPersistence::record_turn_started`
  - `ReasonPersistence::record_provider_output_applied`
  - `ReasonPersistence::record_completion_rejected`
  - `ReasonPersistence::record_turn_closed`
  - `ReasonPersistence::record_rewrite_state_updated`
  - `ReasonPersistence::record_provider_raw_event`
  - `ReasonPersistence::restore`
  - `ReasonPersistence::restore_turn_start_snapshots`
  - `ReasonPersistence::restore_turn_snapshots_for_ui`
  - `ReasonPersistence::restore_turn_snapshots_page_for_ui`
  - `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui`
  - `ReasonPersistence::authoritative_turn_snapshots_fingerprint`
  - `SessionHistory::persist_json`
  - `SessionHistory::from_persisted_json`
  - `SessionHistory::persist_to_path`
  - `SessionHistory::load_from_path`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `session`
- touched resources:
  - `turn`
  - `ui_projection`
- resource operations:
  - `session.restore`
  - `session.append_turn_to_turn`
  - `session.list_persisted`
- forbidden shortcuts:
  - UI projection must not synthesize persisted sessions from turn-only or worker sessions.
  - Session truth must not be recovered from provider raw ledgers or UI sidecars.

## Request Mainline

- runtime opens `~/.freehand/state/turns/<agent_id>/<session_id>/` as the authoritative reason state directory
- session-owned rewrite truth is restored from `SessionHistory` snapshots
- turn execution consumes restored session truth through `ReasonTurnEngine::start_turn`, which stamps `TurnRecord.created_at` as turn-created truth
- runtime persistence holds a session-scoped persistence lock while reading cursor truth, allocating the next reason-ledger sequence, appending semantic reason-ledger rows, and advancing durable snapshot cursors
- terminal turn close materializes immutable turn truth files and only then updates derived UI and index sidecars
- provider raw ledgers may be appended for debug, but they are not part of the authoritative request or recovery chain

## Response Mainline

- `SessionHistory` JSON/file helpers render and restore authoritative session rewrite truth
- reason persistence appends a reason-ledger row together with current session-history truth and row `created_at`, then refreshes authoritative snapshots and derived sidecars
- same-session concurrent writers allocate strictly monotonic reason-ledger sequences because seq allocation and durable snapshot refresh are in the same session lock
- reason persistence appends provider raw debug-ledger rows under `~/.freehand/ledgers/providers/<family>/<agent>/<session>/<turn>.jsonl` without mutating authoritative session truth
- reason persistence returns deterministic restore state from snapshot plus reason-ledger tail replay, or from reason-ledger-only rebuild when snapshots are missing or invalid
- reason persistence filters provider-visible `historical_turn:*`
  session-memory segments to the same effective active/closed logical turn set
  used by rollback-aware transcript truth, while preserving ordinary stable
  memory
- reason persistence returns non-UI turn-start snapshots from authoritative
  reason-ledger `TurnStarted` rows, respecting rollback markers, so owner
  workflows can recover first-round user intent even when effective UI
  snapshots have advanced to later continuation or repair rounds
- reason persistence returns derived UI restore snapshots from authoritative closed/active turn files plus rollback-marker sidecar truth when those snapshots contain every observed round; old turn snapshots with missing `created_at` are backfilled from durable file metadata; incomplete multi-round authoritative snapshots are backfilled from reason-ledger rows only on selected transcript query paths; if a legacy selected transcript has surviving inactive authoritative snapshots but the reason ledger is empty or retained at a later sequence offset that cannot backfill missing rounds, restore returns those surviving snapshots with an explicit `reason_persistence_partial_ui_restore` integrity warning on the latest turn instead of claiming a complete transcript
- reason persistence returns a bounded latest/older page over the same effective UI snapshot order, requiring an owner-issued same-session turn cursor for older pages and returning explicit page facts without changing turn truth
- reason persistence also exposes an authoritative-only UI snapshot restore path for daemon bootstrap and background lifecycle polling, so global startup and Master parent-workset reconciliation do not replay every historical reason ledger
- closed-turn authoritative restore consumes only `*.json` turn snapshots; leftover atomic temp files such as `*.tmp-*` are write artifacts, not session truth, and must not wedge daemon bootstrap
- UI restore preserves one latest snapshot per exact turn id, so `runtime-turn-N` / `runtime-turn-N-rM` provider/tool/repair rounds remain chronological UI cards; only append-only rollback markers remove every round that belongs to the rolled-back logical turn key
- terminal turn persistence yields immutable per-turn truth plus updated session cursor truth
- derived UI and index sidecars are regenerated from authoritative reason truth after durable writes complete
- persisted session index reads expose only derived index rows and session metadata sidecars for UI list/search projection; worker task transcripts are not promoted to top-level persisted user sessions by this owner operation
- session display metadata (`title`, `archived`) is persisted as reason-owned sidecar truth for multi-UI session management; it is separate from provider-visible session history and turn transcript truth
- session rollback is persisted as an append-only reason-ledger marker; effective transcript restore filters rolled-back logical turns while raw closed-turn files remain on disk for audit; later durable writes rebuild derived sidecars from rollback-filtered effective turns while raw rolled-back files remain reserved for audit and turn-id allocation

## Error Mainline

- invalid persisted snapshot JSON is rejected explicitly
- invalid closed-turn `*.json` payloads are rejected explicitly with the file path in the parse error, while non-`.json` atomic temp files in the turns directory are ignored as non-authoritative write artifacts
- invalid persisted snapshot coherence is rejected explicitly
- reason-ledger sequence gaps or duplicate sequence numbers must block recovery
- stale duplicate ledger rows are recoverable only when a later authoritative row with the expected next sequence exists; otherwise duplicate or regressed sequence numbers still fail explicitly
- provider raw payload availability alone must not mask missing authoritative reason truth
- UI sidecar presence alone must not be treated as session-truth recovery evidence
- active incomplete authoritative UI snapshots with an empty or retained-offset reason ledger remain an explicit restore error; only inactive surviving snapshots may be displayed, and only with a visible partial-transcript warning
- session metadata mutations for unknown sessions fail explicitly unless they are creating a new metadata-only session
- deleting a session through UI protocol is a non-destructive archive operation until a physical deletion design is explicitly approved
- rollback with no eligible closed turn or with an active turn fails explicitly; it must not delete turn files or silently mutate UI-local state

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
  - allowed callers: `freehand-reason`, owner-crate tests
  - related tests: rewrite-base validation, persisted coherence rejection
  - why shared: rewrite-base semantic validation must not be duplicated in persistence coordinators
- `inspect_context_cache_diagnostics`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: compute metadata-side cache diagnostics stored in rewrite records and recovery evidence
  - allowed callers: `freehand-reason`, owner-crate tests, replay/debug tools
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
  - purpose: add an explicit integrity warning to the latest inactive surviving authoritative UI snapshot when earlier round truth is missing and the reason ledger is empty or retained at an unusable sequence offset
  - allowed callers: selected-session UI restore path only
  - related tests: `ui_restore_returns_inactive_partial_authoritative_snapshots_with_integrity_warning`, `ui_restore_returns_inactive_partial_authoritative_snapshots_when_retained_ledger_starts_after_one`, `ui_restore_keeps_active_incomplete_authoritative_snapshot_as_hard_error_without_ledger`, `ui_restore_keeps_active_incomplete_authoritative_snapshot_as_hard_error_with_retained_ledger`
  - why shared: partial transcript visibility must stay a reason-owned contract warning, not a WebUI/browser guess or provider-raw recovery path
- `ReasonPersistence::create_session_metadata` / `rename_session` / `archive_session` / `restore_session`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: persist multi-UI session management metadata without mutating turn transcript truth
  - allowed callers: runtime UI command dispatch owner
  - related tests: session metadata create/rename/archive/restore smoke plus unknown-session rejection
  - why shared: WebUI/Android/CLI must not each invent local session CRUD state
- `ReasonPersistence::rollback_latest_session_turn`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: append a rollback marker for the latest effective logical user turn and rebuild effective transcript without deleting raw turn truth
  - allowed callers: runtime UI command dispatch owner
  - related tests: append-only rollback marker plus effective transcript filtering, raw file retention, and later sidecar non-resurrection
  - why shared: rollback truth must be durable and shared across WebUI/CLI/daemon instead of being a client-local transcript edit
- `ReasonPersistence::restore_turn_start_snapshots`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: expose authoritative turn-start request truth without UI/effective-round coalescing, while still honoring rollback markers
  - allowed callers: runtime parent-goal evaluation and owner-crate tests
  - related tests: `restore_turn_start_snapshots_preserves_original_round_and_respects_rollback`, `turn_start_restore_uses_authoritative_snapshots_when_retained_ledger_starts_after_one`, `production_master_runner_recovers_parent_goal_from_first_round_turn_start_ledger`
  - why shared: background lifecycle owners sometimes need first-round user intent, not the latest repaired UI row; parsing reason ledgers outside the persistence owner would duplicate recovery semantics
- `ReasonPersistence::raw_authoritative_turn_snapshots` / `ReasonPersistence::reserved_authoritative_turn_ids`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: expose raw authoritative turn files and reserved turn ids, including rolled-back audit files, without making them effective transcript truth
  - allowed callers: runtime parent-workset turn allocator/idempotency repair and owner-crate tests
  - related tests: `rollback_marker_does_not_resurrect_raw_turns_when_later_turn_is_persisted`, `production_master_runner_rechecks_stale_blocked_parent_marker_after_rollback`
  - why shared: runtime must allocate a fresh follow-up turn after rollback without parsing reason-owner directories or resurrecting rolled-back transcript truth
- `filter_history_context_to_effective_turns`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: remove rolled-back or orphan `historical_turn:*`
    session-memory segments before restored session truth can feed a future
    provider request
  - allowed callers: persistence restore, ledger replay, durable row
    materialization, owner-crate tests
  - related tests:
    `rollback_filters_model_visible_history_to_effective_turn_truth`
  - why shared: snapshot restore, ledger-only rebuild, and rollback
    persistence must use one effective-turn filter instead of drifting
    independently

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `SessionHistory::load_from_path` | `crates/freehand-reason/src/session_history.rs` | restore authoritative session rewrite snapshot from disk | session-history snapshot file | validated session rewrite truth | runtime/bootstrap | session-history owner | bound |
| 02 | `SessionHistory::from_persisted_json` | `crates/freehand-reason/src/session_history.rs` | validate restored session rewrite JSON | session-history JSON payload | validated session rewrite truth | persistence loader/debug tools | session-history owner | bound |
| 03 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | consume restored session truth for turn startup and stamp turn-created time | restored session history + turn input | initialized turn record with `created_at` + provider payload | runtime/live bridge | reason owner | bound |
| 04 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | materialize semantic outputs into turn truth before persistence projection | provider semantic output | updated turn truth | runtime/live bridge | reason owner | bound |
| 05 | `SessionHistory::commit_turn_start` | `crates/freehand-reason/src/session_history.rs` | consume one-shot non-ordinary rewrite state after successful startup | active turn id | updated session rewrite truth | reason owner | session-history owner | bound |
| 06 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | append turn-start ledger row with row `created_at`, refresh active-turn snapshot, and update cursor/sidecars | session history + started turn truth | durable reason state for running turn with created-time truth | runtime/live bridge/testkit | persistence owner | bound |
| 07 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | append provider-output ledger row and refresh active-turn snapshot | session history + updated turn truth + provider semantic output | durable active-turn truth | runtime/live bridge/testkit | persistence owner | bound |
| 08 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | append schema-rejection ledger row and refresh active-turn rejection counter | session history + updated turn truth + rejection | durable rejection evidence | runtime/live bridge/testkit | persistence owner | bound |
| 09 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | append terminal ledger row, materialize immutable turn truth including `created_at`, clear active-turn snapshot, and update sidecars | session history + terminal turn truth | durable closed-turn truth | runtime/live bridge/testkit | persistence owner | bound |
| 10 | `ReasonPersistence::record_rewrite_state_updated` | `crates/freehand-reason/src/persistence.rs` | append rewrite-state ledger row and refresh session snapshots | updated session-history truth | durable rewrite-state persistence | rewrite runtime / recovery path | persistence owner | bound |
| 11 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger rows without mutating authoritative turn/session truth | provider family + session/turn/trace identity + raw wire body + scene provenance | durable provider raw debug evidence | runtime/live bridge | persistence owner | bound |
| 12 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | rebuild authoritative state from snapshots plus reason-ledger tail, or from ledger alone | snapshot directory + reason ledger | restored in-memory session and turn truth | runtime/bootstrap/testkit/CLI smoke | persistence owner | bound |
| 12h | `filter_history_context_to_effective_turns` | `crates/freehand-reason/src/persistence.rs` | filter model-visible historical-turn session memory to effective active/closed logical turn truth | restored session history + active/closed turns | session history without rolled-back or orphan historical-turn memory | restore / ledger replay / row persistence | persistence owner | bound |
| 13 | `ReasonPersistence::restore_turn_start_snapshots` | `crates/freehand-reason/src/persistence.rs` | restore authoritative turn-start snapshots from reason-ledger truth without UI coalescing and filter rolled-back logical turns | session id + reason ledger + rollback markers | ordered turn-start request truth | runtime parent-goal evaluation | persistence owner | bound |
| 14 | `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui` | `crates/freehand-reason/src/persistence.rs` | restore derived UI snapshots from authoritative closed/active `*.json` turn files plus rollback-marker sidecar truth without replaying reason ledgers, preserving exact round ids that are present on disk and ignoring leftover atomic temp files | authoritative turn snapshots + rollback markers | lightweight exact-per-file UI snapshots with created-time truth | runtime UI bootstrap + Master parent-workset reconciliation | persistence owner | bound |
| 14a | `ReasonPersistence::authoritative_turn_snapshots_fingerprint` | `crates/freehand-reason/src/persistence.rs` | return a deterministic change fingerprint over every authoritative file that `restore_authoritative_turn_snapshots_for_ui` reads: session history, cursor, active turn, closed turn snapshots, and rollback markers, plus the monotonic reason cursor sequence; `Ok(None)` when no authoritative truth exists yet, `Err` on real filesystem failures so runtime never reads file paths or coerces errors into valid cache keys; the fingerprint is used as the cache key so any authoritative mutation changes at least one (file name, mtime, size) triple or the cursor sequence, catching same-mtime/same-size content changes and snapshot renames | session id | `Ok(None)` or `Ok(Some(fingerprint_string))` or `Err` | runtime Master parent-workset reconciliation cache | persistence owner | bound |
| 15 | `ReasonPersistence::create_session_metadata` / `rename_session` / `archive_session` / `restore_session` | `crates/freehand-reason/src/persistence.rs` | persist session display metadata sidecar mutations for shared UI CRUD | session id + title/archive intent | updated session metadata sidecar | runtime UI command dispatch | persistence owner | bound |
| 16 | `ReasonPersistence::rollback_latest_session_turn` | `crates/freehand-reason/src/persistence.rs` | append latest-logical-turn rollback marker and update effective cursor/projection state without deleting raw turn files | session id | rollback marker with target turn, previous effective head, and restored user text | runtime UI command dispatch | persistence owner | bound |
| 14r | `ReasonPersistence::raw_authoritative_turn_snapshots` / `ReasonPersistence::reserved_authoritative_turn_ids` | `crates/freehand-reason/src/persistence.rs` | read raw authoritative closed/active turn files and cursor-reserved ids, including rolled-back logical turns, for id reservation and stale-idempotency diagnosis without changing effective transcript truth | session id + authoritative turn files + active snapshot + cursor sidecar | raw turn snapshots or reserved turn ids; callers must not project them as effective UI transcript truth | runtime Master parent-workset reconciliation | persistence owner | bound |
| 17 | `ReasonPersistence::restore_turn_snapshots_for_ui` | `crates/freehand-reason/src/persistence.rs` | restore selected transcript snapshots from authoritative turn files, rebuild exact per-round UI snapshots from reason ledger when authoritative snapshot truth is absent or missing earlier observed rounds, and allow inactive surviving authoritative snapshots only with an explicit partial-transcript warning when the ledger is empty or retained at an unusable sequence offset | authoritative turn snapshots + reason ledger rows + rollback markers | exact per-round UI snapshots with rolled-back logical turns filtered, or inactive partial snapshots carrying `reason_persistence_partial_ui_restore`; active incomplete snapshots still fail, including retained-offset ledgers | selected `QuerySessionTurns` / rollback refresh | persistence owner | bound |
| 17a | `ReasonPersistence::restore_turn_snapshots_page_for_ui` | `crates/freehand-reason/src/persistence.rs` | select bounded newest or strictly older effective UI snapshots for selected-session incremental transcript restore | session id + validated latest/older page request | bounded ordered turn page plus has-older and owner-issued oldest/newest cursor facts, or explicit invalid limit/cursor/recovery error | runtime `QuerySessionTurnsPage` bridge | persistence owner | bound |
| 18 | `ReasonPersistence::list_persisted_sessions` | `crates/freehand-reason/src/persistence.rs` | expose derived persisted session index rows and session metadata sidecar truth for UI-safe list/search projection without reading provider raw ledgers or treating worker transcripts as global sessions | session index sidecar plus metadata sidecar | persisted session index/metadata rows for runtime UI projection | runtime.ui-command-dispatch `QuerySessionList` / `QuerySessionSearch` | persistence owner | bound |

## Metadata / Request Isolation Notes

- authoritative snapshots store session and turn truth, not provider wire payloads
- reason-ledger rows may carry metadata-side diagnostics, but request-chain content remains separate from provider raw debug bodies
- provider raw ledgers are separate files under `~/.freehand/ledgers/providers` and must not be reinterpreted as authoritative request or turn truth
- derived UI sidecars and session indexes are downstream projections only and must not participate in recovery decisions
- session display metadata is downstream UI/session-management truth and must not participate in provider-visible context recovery decisions

## Sync Status Against Code

- current code baseline now binds session-history JSON/file round-trip,
  reason-ledger append, provider-raw debug-ledger append, active-turn refresh,
  terminal turn materialization, created-time preservation, derived sidecar
  writes, snapshot-plus-tail / ledger-only recovery, effective historical-turn
  session-memory filtering, and rollback-aware restore
- current code exposes exact-round UI restore so multi-round repair/tool activity survives selected transcript queries; complete authoritative snapshots avoid replaying huge reason ledgers during daemon bootstrap, Master parent-workset reconciliation, and retained-offset restore; incomplete authoritative snapshots are backfilled from reason-ledger truth only when the selected transcript is explicitly queried, inactive legacy partial snapshots with empty or retained-offset ledgers return with a visible integrity warning, and active incomplete snapshots remain hard restore errors
- current code exposes turn-start snapshot restore so parent-goal evaluation can recover the original first-round operator objective from reason-ledger truth even when the effective UI snapshot has advanced to a repaired round
- session metadata sidecar CRUD is bound for create, rename, archive, restore, and delete-as-archive while staying separate from authoritative turn transcript truth
- append-only latest-session-turn rollback is bound for durable marker write, effective transcript filtering, restart restore, raw turn file retention, reserved id allocation, and later sidecar non-resurrection
- CLI and shared-harness smoke both bind to the persistence owner path without duplicating persistence semantics in the app layer
- live Anthropic runtime path now records provider raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` while keeping those ledgers outside recovery truth
- explicit owner-bound regression coverage now locks ledger sequence gaps plus provider-raw-only and UI-sidecar-only missing-recovery rejection
- explicit owner-bound regression coverage now also locks invalid persisted snapshot JSON, invalid snapshot coherence, and duplicate-sequence recovery rejection
- explicit owner-bound regression coverage locks that leftover atomic temp files under a closed-turn directory do not enter authoritative UI/bootstrap restore
- migrated mainline-call source now lives at `docs/mainline-calls/reason.persistence.json` and generated wiki lives at `docs/wiki/reason.persistence.md`
