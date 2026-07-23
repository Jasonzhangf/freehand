# Test Design: `runtime.ui-command-dispatch`

- feature_id: `runtime.ui-command-dispatch`
- owner: `crates/freehand-runtime`
- lifecycle path under test:
  - config-selected bootstrap becomes one runtime dispatcher
  - config-selected live bootstrap may seed one shared node metadata ledger before the first command
  - accepted command ingress becomes a dispatch envelope
  - submit commands may carry an optional selected session id and selected cwd and must create or continue that cwd-bound session instead of silently flattening everything into the default session
  - runtime dispatch routes to the declared owner module
  - runtime-backed read-only task queries route to task owner APIs and return UI-safe projections without becoming task truth writers
  - runtime-backed Phase 1 TaskBoard, AgentBoard, and AgentLifecycle queries route to owner APIs and return UI-safe projections without becoming task or lifecycle truth writers; task snapshot `created_at` must be preserved from task owner truth
  - runtime-backed task mutation commands route to task owner APIs for create/create_agent/assign/claim/review/reject/approve/close and publish task list projections after accepted mutations
  - runtime-backed Phase 1 execution facts and scheduler ticks route to `task.orchestration`; recovering must not terminalize a task, and scheduler ticks must emit facts/recommendations without making task failure decisions
  - runtime-backed Phase 2B EventInbox and MasterPoll route to
    `task.orchestration`; runtime only projects owner DTOs and does not classify
    or apply master business actions locally
  - Phase 2B closeout/recovery proof uses `replay_from_start=true` to ignore a
    stale persisted cursor plus omitted `limit` to drain all pending EventInbox
    rows; explicit finite limit remains pagination and cannot prove no events
    remain after the persisted cursor
  - runtime-backed Phase 2C WorkerControl and QueryWorkerControl route to
    `worker.control`; runtime only projects owner DTOs and does not own
    safe-point control semantics or Task Center consequences
  - runtime-backed read-only error-center queries route to metadata ledger projection and return UI-safe rows without becoming error truth writers
  - runtime-backed read-only config status queries reload config-owner truth and project the complete safe provider registry plus current primary/fallback selection without becoming config truth writers
  - runtime-backed provider definition upsert commands route to `config.core` persistence without changing the active provider binding, then expose pending restart-required projection without hot-reloading active runtime config
  - runtime-backed provider selection commands route to `config.core` persistence without rewriting provider definitions, then expose pending restart-required projection without hot-reloading active runtime config
  - runtime-backed Agent resource-count commands route to `config.core`, expose pending `1..=5` shared-provider topology projection, and do not fabricate live AgentBoard processes before restart
  - successful task tool mutations publish a runtime-owned task list projection into shared UI protocol state so ADP task subscribers receive owner-backed lifecycle changes
  - session-management dispatch routes create/rename/archive/restore/delete-as-archive commands to reason persistence metadata APIs and refreshes the shared UI projection
  - session rollback dispatch routes `RollbackLatestSessionTurn` to reason persistence, cancels non-terminal child tasks from the rolled-back logical parent turn through `task.cancel`, reloads effective turn snapshots, and replaces the shared UI session transcript projection
  - runtime-backed `QuerySessionTurns` reloads exact per-round snapshots from the master plus configured Worker reason-persistence namespaces and replaces the shared UI session transcript projection so background-written parent turns and Worker task conversations are visible without daemon restart while every `runtime-turn-N` / `runtime-turn-N-rM` tool round remains observable, active live provider transport/model waiting and tool activity are preserved only on the latest nonterminal replacement turn, missed background lifecycle provider retry/failover is reconstructed from ErrorCenter metadata as model-request transport substate and schema waiting as model-request phase only for that latest nonterminal turn, and internal parent-evaluation / `worker-task-*` framework prompts are not projected as user-authored text
  - missing Worker task sessions return explicit target-not-found and never resolve to an empty transcript, another configured agent, or a global session
  - live bootstrap restores persisted turn projection and next runtime turn ordinal from all persisted sessions when recovery truth exists
  - live bootstrap restores authoritative turn snapshots without replaying every historical reason ledger; complete authoritative multi-round snapshots remain separate derived UI cards, while selected-session query performs exact-round ledger backfill for incomplete old snapshots
  - live bootstrap consumes reason-persistence authoritative closed-turn `*.json` truth and must not fail when a previous atomic write left a non-`.json` temp file in the turns directory
  - live bootstrap recovers or clears dead-owner Master active-work checkpoints before user work is accepted; checkpoint-only stale work with missing session truth or no matching active turn snapshot must not survive restart
  - reason-backed submit/cancel update derived UI state
  - reason-backed submit with a selected session id keeps the session transcript queryable under that session
  - reason-backed submit with selected cwd projects cwd into session transcript and later same-session submits inherit it
  - live submit persists a prepared active turn snapshot before pre-provider context admission so refresh/restart/session queries can observe the accepted user turn even if instruction capability or provider request build has not completed yet
  - active live cancel sets a cancel token, immediately persists cancelled terminal truth for the same prepared active snapshot, clears in-memory active_turn and Master active-work, and publishes cancelled projection without waiting for provider completion or pre-provider context construction
  - latest-active cancel resolves the current active live turn when UI has not received a concrete `turn_id`
  - cancelled live provider success must not overwrite the cancelled UI projection or commit a success outcome
  - reason-backed submit projects the original user prompt into derived UI public conversation truth
- live provider submit incrementally updates derived UI turn/debug state before terminal receipt
- live provider submit maps tool-result broadcasts into derived UI state so tool activity can transition to completed over SSE
- live provider submit maps context-planning and provider-request-built debug
  events into derived UI state so pre-provider context preparation and
  model-response waiting are visible before provider response arrives
- live provider submit maps completion-schema rejection broadcasts into derived UI state so clients can query `SchemaRetry` plus either missing tag guidance or concrete invalid-schema fields before the model repair completes
- live provider submit maps bridge-materialized tool execution failure into derived UI state before returning dispatch failure, so query/SSE do not stay waiting
- live provider/protocol failure before reason persistence starts still creates a failed selected-session turn, persists it, and returns explicit dispatch failure without falling back to non-live submit
  - live provider submit publishes the user prompt before provider events so blank WebUI streams can render the user side of the conversation immediately, but framework-owned `worker-task-*` live pending/cancelled projections must not expose internal task or continuation prompts as user-authored text
  - live provider submit keeps the selected session id attached to the derived UI truth when one is supplied
  - live provider submit keeps the selected cwd attached to derived UI truth and tool execution workspace when one is supplied
  - live provider multi-round continuation prompts must not replace the public user prompt projection
  - final live multi-round projection keeps only the final round's own content while earlier rounds remain separate cards with their own tool activity
  - node-backed direct message returns owner-backed receipt
  - unsupported resume path fails explicitly
  - unknown session metadata mutation targets fail explicitly as target-not-found
- white-box plan:
  - runtime bootstrap coverage
  - config-selected bootstrap coverage
  - config-selected live shared node-metadata-ledger bootstrap coverage
  - config-selected live shared node-metadata-ledger bootstrap failure coverage
  - selected-session live submit coverage
  - selected-session cwd inheritance coverage
  - session metadata dispatch coverage for create, rename, archive, restore, and delete-as-archive
  - session rollback dispatch coverage for append-only marker write, rolled-back child-task cancellation, retained-turn child non-cancellation, and effective transcript refresh
  - runtime query-session-turns coverage for persistence-backed transcript
    refresh, including parent-goal evaluation internal prompt hiding and final
    assistant decision visibility plus Worker task prompt hiding with Worker
    assistant/tool/final truth and source-agent attribution still visible
  - runtime query-session-turns coverage for background lifecycle retry observability: a latest nonterminal persisted parent/Master turn with same-session ErrorCenter `retry_same_step` metadata projects `model_request.transport.kind=ProviderRetry`, updates the session list to `waiting_model`, and exposes the owning turn id; a terminal persisted turn with the same metadata remains terminal and has no `model_request`; an earlier historical retry turn followed by a later terminal round remains historical and is not reactivated
  - session metadata negative coverage for unknown session id and empty title rejection before runtime dispatch
  - live tool execution with requested session cwd coverage
  - persisted latest-turn restore coverage
  - next runtime turn ordinal restore coverage, including selected non-default sessions created by WebUI
  - submit/cancel reason dispatch coverage
  - pre-provider prepared active turn persistence coverage proving `QuerySessionTurns` sees the submitted turn before provider request build and does not collapse to an older transcript after refresh/restart
  - active live cancel immediate receipt and persistent pre-provider terminalization coverage
  - latest-active cancel coverage
  - cancelled live submit negative coverage proving later provider success cannot replace cancelled projection
  - live bridge cancellation checkpoint coverage before provider output, tool execution, and terminal write
  - checkpoint rewind dispatch coverage
  - checkpoint rewind missing-manifest target-not-found coverage
  - task list/history runtime query coverage
  - Phase 1 TaskBoard, AgentBoard, and AgentLifecycle runtime query coverage
  - Phase 1 ApplyExecutionFact and RunSchedulerTick runtime dispatch coverage, including recovering, blocked, review-ready, stale, and scheduler non-decision behavior
  - Phase 2B EventInbox query and MasterPoll dispatch coverage, including
    persisted cursor, compact classifications, no task status mutation, and a
    backlog larger than 100 events to prove replay plus omitted limit is a full
    drain
  - Phase 2C WorkerControl dispatch/query coverage, including safe-point event
    projection, pause/resume/cancel consequence evidence, invalid target
    failure, and no runtime-local success projection
  - missing task history query target-not-found coverage
  - error-center runtime query coverage, including trace/turn/domain filters and no raw text in projection
  - config status runtime query coverage, including ordered multi-peer
    projection, complete configured provider registry, current primary/fallback
    ids, sanitized base URL/host projection, auth source projection,
    provider web_search configured/effective route diagnostics, and no API
    key/pair-token leakage
  - provider web_search live-test dispatch coverage, including hosted-only request shape, explicit no-observation failure, and no local `web_search` function tool
  - provider definition upsert dispatch coverage, including adding a provider without changing selection, invalid no-overwrite, no secret projection, and active runtime model/provider unchanged until restart
  - provider selection dispatch coverage, including switching to an existing enabled provider, preserving provider definitions, rejecting invalid/same-fallback selection without overwrite, and active runtime model/provider unchanged until restart
  - Agent resource-count update dispatch coverage, including valid grow/shrink, invalid no-overwrite, pending safe projection refresh, and active runtime peers unchanged until restart
  - task list publication coverage after successful task tool mutation
  - task mutation command dispatch coverage for create/review/approve/close, including task list projection publication and missing task failures
  - Phase 2A task mutation command dispatch coverage for create worker agent, assign task, claim next with execution id, reject review, retry via execution fact, approve, and close
- live reason hook-to-ui-state coverage
- live context-planning/provider-request-built debug-to-model-waiting UI coverage
- live completion-schema rejection feedback-to-client coverage, including no-schema missing tag feedback, invalid-schema missing field names, and retry index
- live reason tool-result hook-to-ui-state coverage
  - live reason prompt-first projection coverage
  - live Worker task prompt projection coverage proves `live_worker_task_projection_hides_internal_user_text` removes internal `worker-task-*` prompt text from `user_text`, while `live_regular_session_projection_keeps_user_text` proves normal sessions still render the real user prompt
  - live reason final projection keeps original user prompt after tool-result continuation
  - live reason projection keeps earlier-round tool activity visible on the earlier round after tool-result continuation
  - live bootstrap projection keeps earlier-round tool activity on its original round after restart
  - live bootstrap negative coverage poisons an incomplete historical reason ledger and proves daemon bootstrap succeeds from authoritative snapshots without parsing that ledger
  - live bootstrap negative coverage covers leftover atomic temp files under reason closed-turn directories through `reason.persistence` restore tests before daemon startup is considered blocked
  - live reason final projection negative coverage proves intermediate continuation text is not exposed in the final public conversation
  - live reason dispatch failure projection coverage proves bridge-materialized failed turns update `UiProtocolState` before the dispatch error is returned
  - live reason pre-provider active snapshot coverage proves cancellation, cancel command dispatch, and pre-provider failure close the same prepared turn instead of losing it, leaving active-work behind, or replacing it with a previous session turn
  - live reason pre-provider success coverage proves the prepared active snapshot writes `RewriteStateUpdated` only, provider execution writes exactly one canonical `TurnStarted`, and the prepared current turn is not replayed as historical context to the model
  - early live provider/protocol failure projection coverage proves the selected session keeps the original user prompt and a persisted failed terminal turn even when the provider bridge exits before recovery truth exists
  - runtime query-session-turns projection coverage proves `runtime_query_session_turns_restores_background_parent_evaluation` restores a background parent evaluation turn from reason persistence, hides the `<freehand_parent_evaluation>` synthetic user text, and keeps the Master evaluation decision/final assistant answer visible
  - runtime query-session-turns projection coverage proves `runtime_query_session_turns_restores_worker_task_namespace` restores Worker-owned `worker-task-*` turns, hides internal task/continuation prompts from `user_text`, keeps Worker source-agent attribution, keeps terminal/final text visible, and still fails missing Worker sessions explicitly
  - node direct-message dispatch coverage
  - unsupported/missing-target dispatch failure coverage
- module black-box plan:
  - command dispatch receipt smoke
  - owner-routing smoke
  - runtime-derived UI latest-turn smoke
  - runtime-derived UI latest-turn user prompt projection smoke
  - runtime-derived UI session cwd projection smoke
  - runtime-derived cancelled terminal projection smoke
  - config-selected runtime bootstrap smoke
  - config-selected live restart/restore smoke, including incomplete historical authoritative snapshots with a poisoned reason ledger
  - config-selected live multi-round tool restore smoke
  - config-selected live active-work restart recovery smoke, including dead-owner checkpoint-only state without an active snapshot
  - config-selected live node-metadata-ledger bootstrap smoke
  - runtime checkpoint rewind receipt smoke
  - runtime session CRUD receipt smoke over the shared UI protocol state
  - runtime session rollback receipt smoke over the shared UI protocol state
  - daemon ADP task list/history query smoke over the shared runtime query port
  - daemon ADP error-center query smoke over the shared runtime query port
  - daemon ADP config status query smoke over the shared runtime query port
  - daemon/WebUI provider registry, definition upsert, and active-provider selection smoke over the shared ADP command/query path, including visible invalid error, restart-required success state, and post-restart activation proof
  - daemon ADP task list subscription smoke over the shared runtime projection channel
  - daemon ADP EventInbox/MasterPoll smoke over the shared runtime query/command
    path, with same-cursor proof using replay plus omitted limit rather than a
    finite page limit
  - daemon ADP WorkerControl smoke over the shared runtime query/command path,
    with same-id proof using persisted worker-control ledger truth
- project black-box impact:
  - runtime command execution stays outside app boundary while remaining compatible with protocol-owned transport contracts
- fixtures / replay inputs / runtime evidence paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/state/tasks`
  - `~/.freehand/ledgers/tasks`
- known gaps:
  - production daemon/process transport exists only as the current HTTP/SSE smoke host
  - real websocket pairing transport is not wired yet
- sync status between design and implementation:
  - runtime dispatch owner baseline is landed
  - config-selected bootstrap is landed and consumes explicit peer-topology config truth
  - config-selected live bootstrap now seeds node-owned metadata records into the shared metadata ledger before first command ingress
  - config-selected live bootstrap now rejects unwritable shared node metadata ledgers explicitly before a dispatcher can materialize
  - provider-backed submit dispatch plus persisted restore/bootstrap is covered
  - selected non-default session restore now has regression coverage proving the next live submit does not reuse an existing `runtime-turn-N` after daemon restart
- selected-session cwd projection and inheritance are covered
- session metadata dispatch coverage is implemented through runtime dispatch into `reason.persistence` and shared UI projection queries
- session rollback dispatch coverage is implemented through runtime dispatch into `reason.persistence`, `task.cancel` child cleanup for the rolled-back logical turn, and shared UI transcript replacement queries
- live provider submit now streams incremental UI state updates through runtime-owned hooks
- live provider submit now projects provider-request-built lifecycle state into UI before model response arrives
- live provider submit now projects both missing-schema and invalid-schema retry lifecycle feedback into UI before the repair response completes
- live provider submit now streams tool-result UI updates and final projection does not merge earlier-round tool activity into the final turn
  - live provider submit now executes registry tools against the requested session cwd and projects that cwd into UI state
  - live bootstrap now restores complete authoritative earlier-round tool activity on its original UI turn without scanning historical ledgers, while selected `QuerySessionTurns` backfills incomplete old transcripts from ledger truth
  - live bootstrap now clears dead-owner Master active-work checkpoints when no matching active turn snapshot or session truth remains, covered by `live_bootstrap_clears_dead_owner_master_active_work_without_active_snapshot` and `live_bootstrap_clears_dead_owner_master_active_work_without_session_truth`
  - selected `QuerySessionTurns` refresh now preserves active provider transport retry/model waiting and tool activity only for the latest nonterminal replacement turn through `runtime_query_session_turns_preserves_live_provider_retry_activity` and `runtime_query_session_turns_preserves_live_tool_activity`; background lifecycle ErrorCenter-derived latest nonterminal retry transport projection is covered by `runtime_query_session_turns_projects_background_provider_retry_from_error_center`, terminal reactivation is blocked by `runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry`, and historical retry reactivation before a later terminal round is blocked by `runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round`
  - live submit prepared active snapshot persistence is covered by `runtime_live_submit_persists_pre_provider_active_turn_for_refresh`, `runtime_live_submit_materializes_cancelled_turn_before_provider_request`, `cancel_latest_active_live_turn_materializes_pre_provider_terminal_truth`, and `runtime_live_submit_success_does_not_duplicate_prepared_turn_started`
  - live bootstrap now restores persisted session cwd from turn records for UI projection and same-session inheritance
  - live provider submit now refreshes failed bridge truth from persistence before returning dispatch failure, preventing silent waiting UI state
  - reason-backed cancel dispatch is covered
  - active live cancel no longer waits behind provider IO or pre-provider context construction because dispatch cancellation materializes the prepared active snapshot as `Cancelled`, clears `active_turns`, and releases Master active-work immediately
  - active live cancel no longer waits behind provider retry backoff sleep because `sleep_provider_retry` checks the live cancel token; covered by `provider_retry_backoff_sleep_observes_live_cancel_token`
  - latest-active cancel is covered for current-turn stop without a UI-known `turn_id`
  - active live cancel blocks later provider success projection after cancellation
  - runtime live bridge cancellation checkpoint coverage before tool execution and terminal persistence is landed
  - checkpoint rewind missing-manifest dispatch failure is covered as explicit target-not-found
  - missing `CancelTurn`, empty `CancelLatestActiveTurn`, and wrong-node direct-message dispatch failures are covered as explicit target-not-found
  - node-backed direct-message dispatch is covered
  - explicit unsupported resume dispatch is covered
  - runtime task query bridge is covered by `runtime_query_reads_task_truth_from_task_runtime`
  - runtime Phase 1 TaskBoard/AgentBoard/Lifecycle query bridge is covered by `runtime_query_reads_phase1_task_and_agent_boards`
  - runtime Phase 1 ExecutionFact/SchedulerTick dispatch bridge is covered by `runtime_dispatch_execution_fact_and_scheduler_tick_update_task_truth`
  - runtime Phase 2B EventInbox/MasterPoll bridge is covered by
    `runtime_dispatches_phase2b_master_poll_and_event_inbox`
  - runtime Phase 2C WorkerControl dispatch/query bridge is covered by
    `runtime_dispatches_worker_control_to_task_owner` and
    `runtime_worker_control_invalid_target_returns_explicit_failure`
  - runtime task mutation command bridge is covered through CLI ADP lifecycle smoke and must remain a thin route to `task.orchestration`
  - Phase 2A master-worker command bridge is covered by
    `runtime_dispatches_phase2a_master_worker_loop_into_task_truth`
    and `master-worker-foundation-sample`
  - daemon ADP task query bridge is covered by `daemon_adp_queries_runtime_task_truth`
  - runtime error-center query bridge is covered by `runtime_query_reads_error_center_metadata_without_raw_text`
  - runtime config status query bridge is covered by `runtime_query_projects_config_status_without_secrets`
  - runtime provider web_search test dispatch is covered by `provider_web_search_test_declares_hosted_tool_and_requires_observation` and `provider_web_search_test_fails_when_provider_does_not_observe_hosted_search`
  - runtime provider/model update dispatch is covered by `runtime_dispatch_updates_provider_config_without_hot_reloading_active_model` and `runtime_dispatch_rejects_invalid_provider_config_without_overwrite`
  - daemon ADP error-center query bridge is covered by `daemon_adp_queries_runtime_error_center_truth`
  - runtime task list push bridge is planned for `runtime_task_tool_mutation_publishes_task_list_projection`
  - daemon ADP task list subscribe bridge is planned for `daemon_adp_subscribes_runtime_task_truth`
  - migrated mainline-call source and generated wiki are kept in sync with this test design
