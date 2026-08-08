# Wiki: `runtime.ui-command-dispatch`

Generated from `docs/mainline-calls/runtime.ui-command-dispatch.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.ui-command-dispatch.md`
- generated wiki: `docs/wiki/runtime.ui-command-dispatch.md`
- test design: `docs/testing/runtime.ui-command-dispatch.md`

## Resource Operation Backlinks

- input_attachment.prepare_provider_input
- input_attachment.project_to_ui
- session.restore
- debug_trace.read_snapshot
- turn.project_runtime_agent_activity

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from `~/.freehand/config.toml`
- config-selected bootstrap consumes local node id and the ordered configured peer set, including each peer node id, allowed IP, and pair-token env, from `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- live bootstrap restores prior UI projections from authoritative turn snapshots only, without replaying every historical reason ledger before the next command runs, then recovers or clears dead-owner Master active-work checkpoints before accepting user work
- runtime dispatch owner reads the declared owner target from the envelope
- session management commands route through runtime into `reason.persistence` session metadata and rollback APIs; rollback also cancels non-terminal child tasks from the rolled-back logical parent turn through task.orchestration before runtime refreshes `UiProtocolState` from persistence-owned metadata/effective transcript projections
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics
- `QuerySessionTurns` enters through RuntimeCommandDispatcher::query_runtime, restores effective logical-turn snapshots from reason.persistence, hides framework-owned parent-evaluation and worker-task prompts from user_text, derives same-session/same-turn model waiting from ErrorCenter metadata only for the latest nonterminal turn when background lifecycle live hooks are absent, and replaces the derived session transcript so background-written turns are visible without daemon restart while preserving live provider/model waiting and tool activity already published into UiProtocolState only for the latest nonterminal replacement turn; inactive partial authoritative transcripts with empty reason ledgers become visible surviving snapshots with a reason-owned warning, while active incomplete snapshots remain explicit restore failures
- live pending and cancelled turn projections use the same internal-prompt hiding helper before publishing to UiProtocolState, so framework-owned worker-task prompts never become live user_text while ordinary user sessions remain visible
- runtime read-only task queries enter through `UiRuntimeQueryPort` and call task owner list/history APIs
- protocol-owned task mutation commands route through runtime into `TaskRuntime::create_task`, `TaskRuntime::create_agent`, `TaskRuntime::assign_task`, `TaskRuntime::claim_next_task`, `TaskRuntime::submit_review`, `TaskRuntime::reject_review`, `TaskRuntime::approve_review`, and `TaskRuntime::close_task`; runtime publishes updated task list projection after each accepted mutation
- Phase 1 board and lifecycle queries route `QueryTaskBoard`, `QueryAgentBoard`, and `QueryAgentLifecycle` through runtime into `TaskRuntime::query_task_board`, `TaskRuntime::query_agent_board`, and `TaskRuntime::query_agent_lifecycle`
- Phase 1 execution fact and scheduler tick commands route through runtime into `TaskRuntime::apply_execution_fact` and `TaskRuntime::run_scheduler_tick`; runtime stays a thin bridge and does not make scheduler business decisions
- runtime read-only error-center queries enter through `UiRuntimeQueryPort` and read watermarked metadata rows through the runtime metadata projection owner
- runtime read-only config status queries reload config-owner truth from the runtime home and project the selected live agent/provider config plus complete safe provider registry, complete safe model group registry, active model group id, and web_search configured/effective route diagnostics without exposing API keys, pair tokens, or credential-bearing URLs
- provider web_search live-test commands reload config-owner provider truth from runtime home, resolve the requested enabled provider through LoadedConfig::select_provider_for_test, and execute one provider-hosted-search test through provider.reason-live-bridge outside the runtime mutex
- provider definition upsert commands enter through a protocol dispatch envelope, route to config.core::upsert_provider_config_in_path, and must not duplicate config validation or persistence logic in runtime
- provider selection commands enter through a protocol dispatch envelope, route to config.core::switch_agent_provider_in_path, and must not duplicate config validation or persistence logic in runtime
- model group definition upsert commands enter through a protocol dispatch envelope, route to config.core::upsert_model_group_config_in_path, and must not duplicate route/provider validation or persistence logic in runtime
- model group selection commands enter through a protocol dispatch envelope, route to config.core::switch_agent_model_group_in_path, and must not duplicate enabled-group validation or persistence logic in runtime
- legacy provider/model update commands enter through a protocol dispatch envelope, route to config.core::update_provider_config_in_path, and remain supported for existing CLI callers
- Agent resource-count update commands enter through a protocol dispatch envelope, route to config.core::update_agent_resource_config_in_path, and must not duplicate topology validation or persistence logic in runtime
- live submit registers an active turn cancel token, persists a prepared active turn snapshot before context/provider execution, and releases the runtime mutex before running provider IO
- CancelLatestActiveTurn resolves to the newest active live turn before falling back to latest persisted runtime turn
- submit commands may carry selected cwd; runtime canonicalizes and binds cwd to the selected session
- SubmitUserInput metadata may carry transient current-submit image attachments; runtime converts raw base64 only into provider-neutral current-request attachments and keeps raw image bytes out of session history
- successful task tool mutations publish task list projection events through runtime-owned UI protocol state
- Phase 2A master/worker sample commands route as thin ADP dispatch into Task Center owner truth; runtime does not decide business next actions
- Phase 2B EventInbox and MasterPoll query/command shapes route as thin ADP dispatch into Task Center owner truth; runtime does not classify or apply business actions locally
- Phase 2C WorkerControl command and QueryWorkerControl query shapes route as thin ADP dispatch/query into worker.control owner truth; runtime converts DTOs and does not own control semantics
- Phase 2 timer dashboard QueryTimerList, ScheduleTimer, and CancelTimer route through runtime into timer owner truth; runtime converts protocol DTOs, persists through TimerStore, and does not encode timers as Task Center truth
- Phase 2 Tools dashboard QueryToolRegistry routes through runtime into tool.registry owner truth; runtime maps BuiltinToolRegistry::registry_projection rows into UI DTOs without executing tools, persisting registry truth, or synthesizing provider-hosted web_search as a local tool
- Phase 2 Search dashboard QuerySessionSearch routes through runtime into reason.persistence persisted session index/metadata truth and task.orchestration TaskBoard parent-session truth; runtime returns persisted Master/user sessions only as top-level results and nests Worker matches under the owning parent session.
- Diagnostics QueryDiagnostics routes through runtime into runtime-home log truth under ~/.freehand/logs; runtime projects file metadata and bounded redacted tail lines only, without exposing absolute user paths, provider raw payloads, secrets, or non-log files.
- runtime passes Phase 2B replay_from_start and maps omitted limit to the owner full-drain sentinel so closeout samples can ignore stale persisted cursors and consume all pending events instead of only the first page

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events, persists a RewriteStateUpdated prepared active snapshot before provider request build so refresh/restart/session queries observe the accepted user turn, projects RuntimeLive01ContextPlanningStarted, RuntimeLive01ContextPlanningCompleted, and RuntimeLive02ProviderRequestBuilt as model-response waiting state, and incrementally writes reason/debug/tool-result projection updates while the turn is still running; framework-owned worker-task live prompts are hidden from user_text before publication
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- final live multi-round UI projection preserves final-round visible text, usage, errors, and terminal status without aggregating tool-call/tool-result lifecycle activity from earlier runtime rounds
- live provider-backed submit refreshes UiProtocolState from authoritative persistence before returning dispatch failure when the live bridge materializes failed turn truth
- live provider-backed submit materializes an explicit failed turn if provider/protocol failure occurs before reason persistence has recovery truth, preserving the selected session transcript instead of collapsing into transport-only failure
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from persisted ordinals
- live bootstrap uses the Master active-work owner path to interrupt recoverable stale active snapshots or clear dead-owner checkpoint-only work; stale checkpoints with missing session truth or no matching active turn snapshot must not block WebUI/ADP after restart
- runtime-owned UI state reflects derived projections only, not authoritative turn truth
- active live cancel requests set the active cancel token, immediately persist cancelled terminal truth for the same prepared active snapshot, clear the in-memory active turn and Master active-work checkpoint, and publish the cancelled projection without waiting for provider completion, provider retry backoff sleep, or pre-provider context construction; framework-owned worker-task cancelled prompts remain hidden from user_text
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete turn_id
- selected session cwd is persisted on turn records, projected to UiProtocolState, and restored for later same-session inheritance
- current-submit image attachments are sent to the provider request once while turn/session truth stores only id, kind, name, media type, and size metadata for UI projection after refresh
- session metadata mutations return receipts only after the persistence owner accepts the create/rename/archive/restore/delete-as-archive operation and protocol projection is refreshed
- session rollback mutations return receipts only after the persistence owner writes an append-only rollback marker, runtime cancels non-terminal child tasks created by the rolled-back logical parent turn through task.orchestration, and runtime replaces the selected session transcript with effective turn projections
- runtime-backed QuerySessionTurns refreshes the requested session transcript from reason persistence truth at query time; every runtime-turn-N / runtime-turn-N-rM round remains chronological and retains its own tool activity, inactive partial authoritative transcripts with empty reason ledgers project surviving snapshots plus the reason-owned partial-transcript warning, active provider transport retry/model waiting and tool activity already in UiProtocolState are preserved only for the latest nonterminal replacement turn, missed background lifecycle provider retry/failover is restored from ErrorCenter metadata as model-request transport substate only on that latest nonterminal same-session turn, and framework parent/Worker internal prompts stay hidden from user_text
- runtime task list/history queries return UI-safe projections built from task owner snapshots and ledger events
- runtime Phase 1 TaskBoard and AgentBoard queries return UI-safe board projections sourced from task.orchestration and agent.lifecycle, preserving task created_at for UI submit-receipt correlation
- runtime Phase 1 execution fact and scheduler tick dispatch receipts return only after task.orchestration accepts owner truth and task-list projection publication succeeds
- runtime-backed task mutation commands return receipts only after task.orchestration accepts the mutation and task list projection publication succeeds
- runtime-backed worker claim receipts include the claimed task id and execution id; no-task is explicit and not a success mutation
- runtime error-center queries return UI-safe projections built from watermarked metadata rows and omit raw error/request/provider text
- runtime config status queries return UI-safe projections sourced from config.core selected-agent truth with the ordered peer name/mode/node-id/provider list, complete provider registry, complete model group registry, current primary/fallback provider ids, active model group id, resource count, resource limit, shared provider id, auth source type, and provider web_search effective route diagnostics only
- runtime-backed provider web_search tests return explicit owner receipts: success requires a provider-hosted search observation; provider rejection or missing hosted observation is returned as a visible dispatch failure
- runtime task list subscription updates reuse the same UI-safe projection helper as task list queries
- successful provider definition upserts persist through the canonical config owner, store a pending restart-required UI-safe projection, preserve active primary/fallback selection, and leave active runtime/live provider config unchanged until daemon restart
- successful provider selection updates persist only the agent primary/fallback binding through the canonical config owner, store a pending restart-required UI-safe projection, preserve provider definitions, and leave active runtime/live provider config unchanged until daemon restart
- successful model group definition and selection updates persist through the canonical config owner, store a pending restart-required UI-safe projection, preserve provider definitions, and leave active runtime/live provider config unchanged until daemon restart
- successful legacy provider/model updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and leave active runtime/live provider config unchanged until daemon restart
- successful Agent resource-count updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and do not fabricate live AgentBoard workers before daemon/Worker restart
- runtime-backed EventInbox query and MasterPoll command return UI-safe projections sourced from TaskRuntime::query_event_inbox and TaskRuntime::run_master_poll, including classifications and persisted cursor evidence without task status mutations
- runtime-backed WorkerControl dispatch returns owner receipt evidence after worker.control persists accepted control truth, and QueryWorkerControl returns UI-safe persisted control events for same-id restart verification
- runtime-backed Timer dashboard commands return owner receipt evidence only after TimerStore accepts schedule or cancel truth, and QueryTimerList returns UI-safe timer schedule and ledger projections from timer owner truth
- runtime-backed Tools dashboard query returns UI-safe registry rows from BuiltinToolRegistry::registry_projection, including schema, examples, guidance, scope, implemented/read-only state, and Master/Worker exposure flags without tool execution
- runtime-backed Search dashboard query returns UI-safe persisted session results from ReasonPersistence::list_persisted_sessions plus metadata sidecars and nests Worker hits via TaskBoard worker_session_id / parent_session_id truth instead of exposing Worker sessions at top level
- runtime-backed Diagnostics query returns UI-safe log file metadata and bounded redacted tail lines from runtime-home logs, sorted newest-first, without raw provider payloads, secrets, absolute user paths, or browser-local guesses.
- runtime-backed EventInbox/MasterPoll closeout proof uses replay_from_start=true plus omitted limit; explicit finite limits are pagination and cannot prove no events remain after cursor persistence

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- missing turn targets for cancel/resume return explicit dispatch-port failures
- missing checkpoint manifests return explicit dispatch target-not-found failures
- wrong slave target node returns explicit dispatch-port failures
- missing config, invalid agent selection, paired-token mismatch, or slave-mode host selection return explicit bootstrap failures
- unwritable shared node metadata ledgers fail bootstrap explicitly as NodeRuntimeInit and must not materialize a runtime dispatcher
- invalid persisted recovery truth or node-metadata bootstrap failure returns explicit runtime bootstrap failure
- cancelled live turns return explicit cancelled dispatch failure to the original submitter and must not overwrite cancelled UI projection with later provider success
- live provider/tool loops check cancellation at round, stream callback, provider-output, tool-execution, and terminal-write boundaries
- live provider/tool failures that materialize failed turn truth are returned as explicit dispatch failures only after the failed projection has been refreshed into UiProtocolState
- live provider/protocol failures before persistence starts still write a failed turn and return explicit dispatch failure; runtime must not fall back to non-live submit or leave the selected session empty
- CancelLatestActiveTurn with no active or persisted turn returns explicit target-not-found
- unknown session metadata mutation targets return explicit target-not-found failures
- rollback with no eligible target or with an active turn returns explicit dispatch failure; runtime must not delete UI turns locally to pretend success
- selected-session restore failures remain explicit query failures for that session; inactive partial authoritative restore is not a failure once reason persistence returns surviving snapshots with an integrity warning
- missing task history targets return explicit target-not-found and invalid task filters return dispatch failures
- invalid error-center query filters or metadata read failures return explicit dispatch failures
- task list publication failures after task mutation are explicit live bridge failures
- task mutation dispatch requires a live runtime home, maps missing tasks to target-not-found, and must not create task truth outside task.orchestration
- invalid task board filters, missing agent lifecycle ids, invalid execution facts, and invalid scheduler thresholds map to explicit dispatch failures from the owner APIs
- provider/model update without a live runtime home or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake hot reload
- model group update/selection without a live runtime home or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake hot reload
- Agent resource-count update without a live runtime home, with a non-Master target, or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake live resources
- invalid EventInbox cursor, conflicting MasterPoll cursor mode, or MasterPoll cursor persistence failures map to explicit dispatch failures from task.orchestration
- invalid worker-control targets, unknown operations, missing op-specific payloads, terminal tasks, and Task Center consequence failures map to explicit dispatch failures without runtime-local success projection
- timer schedule/cancel/list without a live runtime home, with invalid timer id, or with timer-store persistence failure maps to explicit dispatch failure; runtime must not create task truth or fake a timer projection
- tool registry projection failure maps to explicit query failure; runtime must not fall back to a hardcoded browser/runtime tool list
- persisted session search failures map to explicit query failure; runtime must not fall back to browser-local session filtering or id-prefix guessing
- diagnostics log directory/metadata/tail read failures map to explicit query failure; runtime must not fall back to unredacted absolute paths, raw log dumps, or browser-local diagnostics rows.

## Shared Multi-Reference Functions

- `build_command_dispatch_envelope`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: declare command owner routing before runtime dispatch
  - allowed callers: runtime dispatch ports, transport adapters
  - related tests: command dispatch envelope owner-routing smoke
  - why shared: keeps command-to-owner routing out of app/runtime glue duplication
- `load_default_config`
  - owner: `crates/freehand-config/src/lib.rs`
  - purpose: load the default config file before runtime host bootstrap
  - allowed callers: runtime bootstrap helpers, CLI/startup tests
  - related tests: config load smoke, runtime bootstrap smoke
  - why shared: keeps config loading and selection in the config owner
- `TaskRuntime::list_tasks`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: return filtered task snapshots for runtime-backed UI task list queries
  - allowed callers: RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_query_reads_task_truth_from_task_runtime, daemon_adp_queries_runtime_task_truth
  - why shared: keeps task filtering in task.orchestration instead of duplicating it in runtime or UI
- `TaskRuntime::task_history`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: return ordered task ledger events for runtime-backed UI task history queries
  - allowed callers: RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_query_reads_task_truth_from_task_runtime, daemon_adp_queries_runtime_task_truth
  - why shared: keeps ledger ordering in task.orchestration instead of duplicating it in runtime or UI
- `TaskRuntime::create_task / TaskRuntime::create_agent / TaskRuntime::assign_task / TaskRuntime::claim_next_task / TaskRuntime::submit_review / TaskRuntime::reject_review / TaskRuntime::approve_review / TaskRuntime::close_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: perform task lifecycle and Phase 2A worker loop mutations for runtime-backed UI task commands
  - allowed callers: RuntimeCommandDispatcher::dispatch_create_task, RuntimeCommandDispatcher::dispatch_create_task_agent, RuntimeCommandDispatcher::dispatch_assign_task, RuntimeCommandDispatcher::dispatch_claim_next_task, RuntimeCommandDispatcher::dispatch_submit_task_review, RuntimeCommandDispatcher::dispatch_reject_task_review, RuntimeCommandDispatcher::dispatch_approve_task_review, RuntimeCommandDispatcher::dispatch_close_task
  - related tests: CLI ADP task lifecycle sample mock WebSocket smoke, S-profile task-lifecycle-sample, runtime_dispatches_phase2a_master_worker_loop_into_task_truth
  - why shared: keeps task lifecycle mutation in task.orchestration while runtime remains a thin dispatch bridge
- `task_list_projection_from_runtime`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe task list projection from task owner snapshots for query and push surfaces
  - allowed callers: RuntimeCommandDispatcher::query_runtime, run_live_reason_turn_with_hooks task projection hook
  - related tests: runtime_query_reads_task_truth_from_task_runtime, runtime_task_tool_mutation_publishes_task_list_projection, daemon_adp_subscribes_runtime_task_truth
  - why shared: keeps task UI projection single-sourced while task filtering remains in task.orchestration
- `query_error_center_events_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: read and project error-center metadata into UI-safe query results
  - allowed callers: RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_query_reads_error_center_metadata_without_raw_text, daemon_adp_queries_runtime_error_center_truth
  - why shared: keeps metadata/error-center read projection in runtime owner instead of app transports
- `apply_error_center_live_activity_to_session_projections`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: restore query-time provider retry/failover as model-request transport substate and schema waiting as model-request phase from ErrorCenter metadata only for the latest same-session nonterminal turn when background lifecycle hooks were not already present in UiProtocolState
  - allowed callers: RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_query_session_turns_projects_background_provider_retry_from_error_center, runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry, runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round
  - why shared: keeps ErrorCenter-to-UI retry projection in the runtime query bridge instead of making UI guess from raw error text or AgentBoard state
- `project_config_status_for_ui / project_config_status_from_path_for_ui / RuntimeCommandDispatcher::query_runtime_with_scope`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe active or pending config projection plus complete provider and model group registries from config.core selected agent truth while omitting loopback Agent endpoints for typed remote scope
  - allowed callers: RuntimeCommandDispatcher::query_runtime_with_scope, RuntimeCommandDispatcher::dispatch_update_provider_config, RuntimeCommandDispatcher::dispatch_upsert_provider_config, RuntimeCommandDispatcher::dispatch_update_agent_provider_selection, RuntimeCommandDispatcher::dispatch_upsert_model_group_config, RuntimeCommandDispatcher::dispatch_update_agent_model_group_selection
  - related tests: runtime_query_projects_config_status_without_secrets, provider_web_search_test_declares_hosted_tool_and_requires_observation, provider_web_search_test_fails_when_provider_does_not_observe_hosted_search, runtime_dispatch_updates_provider_config_without_hot_reloading_active_model, runtime_dispatch_upserts_provider_registry_without_switching_active_selection, runtime_dispatch_switches_agent_provider_selection_without_hot_reload, runtime_dispatch_upserts_and_selects_model_group_without_hot_reload
  - why shared: keeps config-to-UI projection in runtime owner while app transports stay protocol-only
- `project_task_board_for_ui / project_agent_board_for_ui / project_agent_lifecycle_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 1 task and agent lifecycle owner projections into UI-safe DTOs
  - allowed callers: RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_query_reads_phase1_task_and_agent_boards
  - why shared: keeps board projection in the runtime bridge while Task Center and Agent Lifecycle remain owner truth
- `task_dispatch_from_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert protocol-owned create-task dispatch mode into task.orchestration dispatch request
  - allowed callers: RuntimeCommandDispatcher::dispatch_create_task
  - related tests: runtime_dispatches_phase2a_master_worker_loop_into_task_truth, cli_runs_master_worker_foundation_sample_against_mock_websocket
  - why shared: keeps UI dispatch-mode DTO conversion in runtime bridge while Task Center owns resulting task truth
- `project_event_inbox_for_ui / project_master_poll_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 2B EventInbox and MasterPoll owner projections into UI-safe DTOs
  - allowed callers: RuntimeCommandDispatcher::query_runtime, RuntimeCommandDispatcher::dispatch
  - related tests: runtime_dispatches_phase2b_master_poll_and_event_inbox
  - why shared: keeps runtime as a DTO bridge and prevents app-local event classification or cursor mutation
- `project_worker_control_for_ui / project_worker_control_events_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 2C worker-control owner projections and persisted events into UI-safe DTOs
  - allowed callers: RuntimeCommandDispatcher::dispatch, RuntimeCommandDispatcher::query_runtime
  - related tests: runtime_dispatches_worker_control_to_task_owner, runtime_worker_control_invalid_target_returns_explicit_failure
  - why shared: keeps runtime as a DTO bridge while worker.control remains the ledger and validation owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch and live node metadata bootstrap | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner |  |  |  | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected Master or Worker while preserving the configured pair and selected reason identity | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap |  |  |  | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner plus runtime bootstrap |  |  |  | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter, including config-owner provider/model-group mutations | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node/config owner adapter |  |  |  | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state |  |  |  | bound |
| 06 | `run_live_reason_turn_with_hooks / run_worker_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a role-correct live provider turn while streaming reason/debug/tool-result callbacks to runtime-owned consumers | selected live config plus live request plus callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner |  |  |  | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state, bind selected session cwd, and persist accepted-turn visibility before provider execution | runtime state plus submitted user text plus optional selected session/cwd | prepared live submit plus active cancel token plus queryable prepared active turn truth | RuntimeCommandDispatcher::dispatch | runtime owner plus reason persistence |  |  |  | bound |
| 07a | `submit_attachment_inputs` | `crates/freehand-runtime/src/lib.rs` | convert SubmitUserInput metadata image payloads into provider-neutral current-submit attachments while deriving metadata-only session truth | SubmitUserInput.metadata.attachments with base64 image payloads | ProviderInputAttachment list plus InputAttachmentMetadata list without raw bytes | RuntimeCommandDispatcher::prepare_live_submit_user_input / submit_user_input_non_live | provider request builder and reason turn metadata projection | input_attachment | provider_request | input_attachment.prepare_provider_input | bound |
| 07p | `persist_prepared_live_submit_active_turn` | `crates/freehand-runtime/src/turn_projection.rs` | persist a prepared active turn snapshot through ReasonPersistence::record_rewrite_state_updated without writing duplicate TurnStarted truth | prepared live submit plus current session history | queryable active turn projection and reason active snapshot | RuntimeCommandDispatcher::prepare_live_submit_user_input | ReasonPersistence::record_rewrite_state_updated plus UI projection |  |  |  | bound |
| 07b | `project_runtime_turn_history / persist_prepared_live_submit_active_turn` | `crates/freehand-runtime/src/turn_projection.rs` | project persisted input attachment metadata into UiTurnProjection without raw/base64 payload | TurnRecord.attachments metadata-only truth | UiTurnProjection.attachments metadata rows for refreshed clients | runtime session query, live pending projection, and bootstrap restore | ui.protocol turn projection | input_attachment | ui_projection | input_attachment.project_to_ui | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed Master or Worker live turn outside runtime mutex while honoring active cancel token and keeping Master active-work truth Master-only | prepared live submit | live receipt or cancelled dispatch failure | RuntimeCommandDispatcher::dispatch | run_live_reason_turn_with_hooks / run_worker_live_reason_turn_with_hooks |  |  |  | bound |
| 08a | `restore_or_materialize_failed_live_submit` | `crates/freehand-runtime/src/turn_projection.rs` | restore bridge-materialized failed turn truth or close the same prepared active snapshot before returning dispatch failure | prepared live submit plus live bridge error | failed turn projection and persisted failed turn truth | RuntimeCommandDispatcher::finish_live_submit | reason persistence plus UI projection |  |  |  | bound |
| 08b | `restore_or_materialize_cancelled_live_submit` | `crates/freehand-runtime/src/turn_projection.rs` | restore bridge-materialized cancelled turn truth or close the same prepared active snapshot before returning dispatch cancellation | prepared live submit plus cancellation summary | cancelled turn projection and persisted cancelled turn truth | RuntimeCommandDispatcher::finish_live_submit | reason persistence plus UI projection |  |  |  | bound |
| 08p | `RuntimeCommandDispatcher::current_agent_activity / RuntimeAgentActivityProjection::merge` | `crates/freehand-runtime/src/lib.rs` | project direct-session activity and merge it with a separate owner-projected activity source without reading UI or ADP payloads | runtime active-turn truth plus typed external activity projection | typed status and saturating active-session count projection | daemon Relay presence wiring | Relay control-side heartbeat projection | turn | runtime_agent_activity | turn.project_runtime_agent_activity | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics, immediate prepared-active snapshot materialization, Master active-work cleanup, and UI projection | cancel command turn id | cancel receipt plus persisted cancelled turn plus cancelled projection | RuntimeCommandDispatcher::dispatch | reason owner / active cancel registry / restore_or_materialize_cancelled_live_submit / master_runner::clear_master_active_work_if_current |  |  |  | bound |
| 10 | `restore_all_persisted_sessions_into_ui` | `crates/freehand-runtime/src/turn_projection.rs` | restore authoritative UI turn snapshots for every persisted session without replaying historical reason ledgers during daemon bootstrap | persisted reason sessions plus authoritative turn snapshots | lightweight bootstrapped UI session transcripts and maximum turn ordinal | RuntimeCommandDispatcher::new | ReasonPersistence::restore_authoritative_turn_snapshots_for_ui plus ui.protocol state |  |  |  | bound |
| 10a | `RuntimeCommandDispatcher::recover_stale_master_active_work_on_bootstrap / RuntimeCommandDispatcher::recover_stale_master_active_work_before_live_submit` | `crates/freehand-runtime/src/lib.rs` | recover or clear dead-owner Master active-work before the runtime accepts new WebUI/ADP work | Master active-work checkpoint plus optional reason active snapshot | interrupted stale active turn projection or cleared checkpoint-only stale work | RuntimeCommandDispatcher::new / live submit preparation | master_runner::recoverable_stale_master_active_work + ReasonPersistence::restore + master_runner::clear_master_active_work_if_current |  |  |  | bound |
| 10q | `RuntimeCommandDispatcher::query_runtime / RuntimeCommandDispatcher::query_runtime_with_scope / RuntimeCommandDispatcher::query_runtime_for_scope` | `crates/freehand-runtime/src/lib.rs` | execute runtime-backed read-only session-turn, task list/history, config, and error-center queries while carrying typed local/remote scope to ConfigStatus projection | UI query command plus typed access scope | optional scoped UI query result or dispatch failure | ADP query transport | reason persistence / task runtime owner / config projection / metadata center |  |  |  | bound |
| 10s | `UiProtocolState::replace_session_turn_projections` | `crates/freehand-ui-protocol/src/state.rs` | restore exact-round QuerySessionTurns snapshots from the master plus configured Worker reason-persistence namespaces, derive latest-turn nonterminal retry/waiting state from ErrorCenter metadata when live hooks are absent, and replace the derived transcript at query time without projecting framework-owned parent/Worker prompts as user-authored text or reactivating historical retry rows; inactive partial authoritative transcripts with empty reason ledgers surface the reason-owned integrity warning on the surviving turns | session id | fresh queryable transcript with runtime-turn-N / runtime-turn-N-rM rounds preserved, per-round tool activity visible, partial-transcript warning visible when reason ledger is empty, background provider retry/failover observable only as model-request transport substate on the latest nonterminal owning turn, schema waiting observable as model-request phase, terminal and historical earlier turns not reactivated, active live provider/tool activity preserved only on latest-turn nonterminal refresh, internal parent-goal evaluation user text hidden, and Worker task transcript rows sourced to the configured Worker with internal task prompts hidden from user_text | ADP query transport / runtime query tests | ReasonPersistence::restore_turn_snapshots_for_ui + configured agent/node map + turn_projection + ErrorCenter metadata projection + ui.protocol state |  |  |  | bound |
| 10u | `project_runtime_turn_history / ui_user_text_for_turn` | `crates/freehand-runtime/src/turn_projection.rs` | convert runtime turn truth into UI-safe turn projection while hiding framework-owned parent/Worker prompt text | turn record plus source agent/node and optional cwd | UI turn projection with normal user text preserved and internal framework prompt text hidden | RuntimeCommandDispatcher query/dispatch projection paths | turn_projection_from_events / turn_projection_for_client |  |  |  | bound |
| 10t | `publish_live_pending_user_projection / publish_live_cancelled_projection / ui_user_text_projection_for_session_user_text / ui_should_hide_user_text` | `crates/freehand-runtime/src/turn_projection.rs` | apply the same internal-prompt hiding contract to live pending and cancelled turn projections before they enter UiProtocolState | active session id plus submitted or internal prompt text | live UI projection whose user_text is absent for framework parent/Worker prompts and present for normal user sessions | live submit/cancel path / runtime projection tests | ui.protocol state |  |  |  | bound |
| 11 | `project_task_list_for_ui / project_task_history_for_ui` | `crates/freehand-runtime/src/lib.rs` | project task owner snapshots and ledger events into UI-safe DTOs | task snapshots or task ledger events | task query projection | RuntimeCommandDispatcher::query_runtime | UI protocol DTO |  |  |  | bound |
| 12 | `task_list_projection_from_runtime` | `crates/freehand-runtime/src/lib.rs` | build and publish task list projection from task runtime after successful task mutation | runtime home plus task filters | UI task list projection | run_live_reason_turn_with_hooks / RuntimeCommandDispatcher::query_runtime | TaskRuntime::list_tasks |  |  |  | bound |
| 12a | `RuntimeCommandDispatcher::dispatch_create_task / dispatch_submit_task_review / dispatch_approve_task_review / dispatch_close_task` | `crates/freehand-runtime/src/lib.rs` | route protocol-owned task mutation commands into task.orchestration and publish refreshed task list projections | task create/review/approve/close dispatch envelope | dispatch receipt or explicit task dispatch failure | RuntimeCommandDispatcher::dispatch | TaskRuntime lifecycle APIs |  |  |  | bound |
| 13 | `query_error_center_events_for_ui / project_error_center_event_for_ui` | `crates/freehand-runtime/src/lib.rs` | read watermarked error-center metadata and project UI-safe event DTOs | QueryErrorCenterEvents filters | ErrorCenterEvents query projection | RuntimeCommandDispatcher::query_runtime | metadata.core ledger plus ui.protocol DTO |  |  |  | bound |
| 14 | `RuntimeCommandDispatcher::dispatch_session_management` | `crates/freehand-runtime/src/lib.rs` | route protocol-owned session CRUD and rollback commands into reason persistence APIs and refresh shared UI projection | session CRUD or rollback dispatch envelope | dispatch receipt or explicit target-not-found/failure | RuntimeCommandDispatcher::dispatch | ReasonPersistence session metadata/rollback owner plus rollback child task cleanup |  |  |  | bound |
| 14a | `cancel_tasks_for_session_rollback` | `crates/freehand-runtime/src/lib.rs` | after a reason rollback marker, cancel non-terminal child tasks whose parent turn shares the rolled-back logical turn while leaving retained-turn child tasks untouched | runtime home, master agent id, and SessionRollbackMarker | Task Center truth with rolled-back child tasks Cancelled or explicit owner failure | RuntimeCommandDispatcher::dispatch_session_management | TaskRuntime::query_task_board and TaskRuntime::cancel_task |  |  |  | bound |
| 15 | `UiProtocolState::replace_session_turn_projections` | `crates/freehand-ui-protocol/src/state.rs` | replace one session transcript with persistence-owned effective projections after rollback or selected-session refresh while preserving live provider/tool activity only for the latest nonterminal replacement turn | session id plus effective turn projections | queryable transcript excluding rolled-back logical turns and preserving active live activity until terminal truth arrives | RuntimeCommandDispatcher::dispatch_session_management | ui.protocol state |  |  |  | bound |
| 16 | `RuntimeCommandDispatcher::dispatch_update_provider_config` | `crates/freehand-runtime/src/lib.rs` | route legacy provider/model update dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | UiProviderConfigUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | update_provider_config_in_path |  |  |  | bound |
| 16a | `RuntimeCommandDispatcher::dispatch_upsert_provider_config` | `crates/freehand-runtime/src/lib.rs` | route provider definition upsert dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | UiProviderConfigUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | upsert_provider_config_in_path |  |  |  | bound |
| 16b | `RuntimeCommandDispatcher::dispatch_update_agent_provider_selection` | `crates/freehand-runtime/src/lib.rs` | route active provider selection dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | UiAgentProviderSelectionUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | switch_agent_provider_in_path |  |  |  | bound |
| 16c | `RuntimeCommandDispatcher::dispatch_test_provider_web_search / execute_provider_web_search_test` | `crates/freehand-runtime/src/lib.rs` | route provider-hosted web_search live-test dispatch into config-selected provider execution without holding runtime state lock or exposing provider wire DTOs | UiCommand::TestProviderWebSearch dispatch envelope | provider test receipt or explicit provider/runtime failure | RuntimeCommandDispatcher::dispatch | LoadedConfig::select_provider_for_test + live provider driver |  |  |  | bound |
| 16d | `RuntimeCommandDispatcher::dispatch_upsert_model_group_config / RuntimeCommandDispatcher::dispatch_update_agent_model_group_selection` | `crates/freehand-runtime/src/lib.rs` | route model group definition and active group selection dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | UiModelGroupConfigUpdate or UiAgentModelGroupSelectionUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | upsert_model_group_config_in_path / switch_agent_model_group_in_path |  |  |  | bound |
| 17 | `update_provider_config_in_path / upsert_provider_config_in_path / switch_agent_provider_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist provider definition updates and active provider selection through canonical config owner | runtime config path plus provider definition or selection update | selected agent config projection from saved TOML | config mutation dispatchers | config.core persistence |  |  |  | bound |
| 17m | `upsert_model_group_config_in_path / switch_agent_model_group_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist model group definitions and active model-group selection through canonical config owner | runtime config path plus model group definition or selection update | selected agent config projection from saved TOML | model group mutation dispatchers | config.core persistence |  |  |  | bound |
| 17a | `RuntimeCommandDispatcher::dispatch_update_agent_resource_config` | `crates/freehand-runtime/src/lib.rs` | route Agent resource-count update dispatch into the config owner and store pending restart-required UI projection without fabricating live workers | UiAgentResourceConfigUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | update_agent_resource_config_in_path |  |  |  | bound |
| 17b | `update_agent_resource_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist reciprocal Master/Worker topology update through canonical config owner | runtime config path plus Agent resource update | selected agent config projection from saved TOML | RuntimeCommandDispatcher::dispatch_update_agent_resource_config | config.core persistence |  |  |  | bound |
| 18 | `project_task_board_for_ui / project_task_snapshot_for_ui / project_agent_board_for_ui / project_agent_lifecycle_for_ui` | `crates/freehand-runtime/src/lib.rs` | project Phase 1 TaskBoard, AgentBoard, and AgentLifecycle owner truth into protocol DTOs, preserving task created_at from owner snapshots | task/agent owner projections | UI-safe board/lifecycle query results | RuntimeCommandDispatcher::query_runtime | ui.protocol DTOs |  |  |  | bound |
| 19 | `RuntimeCommandDispatcher::dispatch_apply_execution_fact / dispatch_run_scheduler_tick` | `crates/freehand-runtime/src/lib.rs` | route Phase 1 execution facts and scheduler ticks into task.orchestration without making business decisions | execution fact or scheduler tick dispatch envelope | dispatch receipt or owner failure | RuntimeCommandDispatcher::dispatch | TaskRuntime::apply_execution_fact / TaskRuntime::run_scheduler_tick |  |  |  | bound |
| 20 | `RuntimeCommandDispatcher::dispatch_create_task_agent / dispatch_assign_task / dispatch_claim_next_task / dispatch_reject_task_review` | `crates/freehand-runtime/src/lib.rs` | route Phase 2A worker registry, assignment, claim, and review rejection commands into task.orchestration | protocol task mutation command | dispatch receipt plus task list projection | RuntimeCommandDispatcher::dispatch | TaskRuntime task owner APIs |  |  |  | bound |
| 21 | `project_event_inbox_for_ui / RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | route Phase 2B EventInbox query into task.orchestration and project UI-safe event rows; omitted limit drains all pending rows | inbox query command | EventInbox query projection | runtime query dispatch | task owner |  |  |  | bound |
| 22 | `RuntimeCommandDispatcher::dispatch_run_master_poll / project_master_poll_for_ui` | `crates/freehand-runtime/src/lib.rs` | route Phase 2B MasterPoll command into task.orchestration and project classifications without business mutations; replay_from_start plus omitted limit drains all pending rows before cursor persistence | master poll command with cursor mode | MasterPoll projection and persisted cursor receipt | RuntimeCommandDispatcher::dispatch | TaskRuntime::run_master_poll |  |  |  | bound |
| 23 | `RuntimeCommandDispatcher::dispatch_worker_control / project_worker_control_for_ui` | `crates/freehand-runtime/src/lib.rs` | route Phase 2C WorkerControl commands into worker.control and project the accepted event without owning control semantics | worker-control dispatch envelope | dispatch receipt with control id/op/status | RuntimeCommandDispatcher::dispatch | TaskRuntime::apply_worker_control |  |  |  | bound |
| 24 | `RuntimeCommandDispatcher::query_runtime / project_worker_control_events_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryWorkerControl into worker.control persisted event truth for restart verification | task id plus execution id query | UiWorkerControlProjection event list | ADP query transport | TaskRuntime::query_worker_control_events |  |  |  | bound |
| 25 | `RuntimeCommandDispatcher::query_runtime / project_timer_list_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryTimerList into timer owner schedule and ledger truth, then project UI-safe TimerList rows | include_terminal timer query flag | UiTimerListProjection or explicit timer-store failure | ADP query transport | TimerStore::load_schedules / TimerStore::load_events |  |  |  | bound |
| 26 | `RuntimeCommandDispatcher::dispatch_schedule_timer / RuntimeCommandDispatcher::dispatch_cancel_timer` | `crates/freehand-runtime/src/lib.rs` | route ScheduleTimer and CancelTimer command envelopes into TimerStore owner mutation APIs and return user-safe timer receipts | validated timer schedule or cancel command | timer_scheduled or timer_cancelled receipt, or explicit owner failure | RuntimeCommandDispatcher::dispatch | TimerStore::schedule_from_request / TimerStore::upsert_schedule / TimerStore::cancel |  |  |  | bound |
| 27 | `RuntimeCommandDispatcher::query_runtime / project_tool_registry_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryToolRegistry into tool.registry owner projection and convert rows into UI-safe protocol DTOs without executing tools | tool registry query | UiToolRegistryProjection or explicit query failure | ADP query transport | BuiltinToolRegistry::registry_projection |  |  |  | bound |
| 28a | `RuntimeCommandDispatcher::query_runtime / query_session_search_for_ui / worker_parent_session_map` | `crates/freehand-runtime/src/lib.rs` | route QuerySessionSearch into reason.persistence persisted session index/metadata truth, join Worker matches through TaskBoard parent-session truth, and return only persisted Master/user sessions as top-level results | search query plus optional limit | UiSessionSearchProjection or explicit search/query failure | ADP query transport | ReasonPersistence::list_persisted_sessions / ReasonPersistence::load_session_metadata / TaskRuntime::query_task_board |  |  |  | bound |
| 30 | `project_diagnostics_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryDiagnostics into runtime-home diagnostics log projection, include only .log metadata plus bounded redacted tail lines, and keep raw payloads, secrets, and absolute user paths out of UI DTOs | diagnostics query plus live runtime home | UiDiagnosticsProjection or explicit query failure | ADP query transport | runtime logs under ~/.freehand/logs | debug_trace | ui_projection | debug_trace.read_snapshot | bound |

## Sync Status Against Mainline Call

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- live provider submit now projects context-planning and provider-request-built debug events into UiProtocolState.model_request before model response arrives
- live provider submit now streams tool-result updates into UiProtocolState so tool activities can transition to completed before final receipt
- direct slave message dispatch routes through `node.master-slave`
- explicit checkpoint rewind dispatch now routes through `runtime.checkpoint-rewind`
- missing checkpoint rewind manifests now stay explicit target-not-found dispatch failures instead of being collapsed into generic success or fallback projection
- resume dispatch remains an explicit unsupported runtime path
- config-selected runtime bootstrap is now bound in code
- config-selected runtime bootstrap uses explicit peer-topology config instead of synthetic paired node ids
- config-selected live bootstrap now seeds a shared metadata ledger path into `node.master-slave` before the first command runs
- unwritable shared node metadata ledgers are now regression-locked as explicit bootstrap failures
- config-selected live bootstrap restores persisted turn projection and next runtime turn ordinal when recovery truth exists
- config-selected live bootstrap recovers or clears dead-owner Master active-work checkpoints, including checkpoint-only stale work with missing session truth or no matching active turn snapshot
- selected QuerySessionTurns refresh preserves active provider transport retry/model waiting and tool activity already published into UiProtocolState only for the latest nonterminal replacement turn and reconstructs missed background lifecycle retry state as model-request transport substate from ErrorCenter metadata only for that latest nonterminal turn without reactivating terminal or historical earlier turns; inactive partial authoritative restore with empty reason ledger is covered by the session-unlock online verifier and active incomplete restore remains a reason-owned hard error
- final live projection now keeps each runtime round as its own UI turn so earlier-round tool activity cannot be merged into the final latest turn
- generated wiki must be regenerated from `docs/mainline-calls/runtime.ui-command-dispatch.json` when this function-map truth changes
- live submit now releases the runtime mutex before provider IO so CancelTurn can enter concurrently
- active live cancel now persists cancelled terminal truth and clears active-work immediately, so a stuck pre-provider context builder cannot leave the session active after cancel; later provider success cannot overwrite it
- runtime dispatch now supports CancelLatestActiveTurn for current-turn stop without requiring the UI to know turn_id
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and terminal persistence
- missing CancelTurn, empty CancelLatestActiveTurn, and wrong-node direct-message dispatch paths now stay explicit target-not-found failures
- runtime task query bridge routes list/history through task.orchestration and is covered by runtime and daemon ADP tests
- runtime Phase 1 TaskBoard and AgentBoard query bridge routes through task.orchestration and agent.lifecycle and is covered by runtime tests
- runtime Phase 1 ExecutionFact and SchedulerTick dispatch bridge routes through task.orchestration and is covered by runtime tests
- runtime task mutation command bridge is bound as a thin route from protocol commands to task.orchestration create/create_agent/assign/claim/review/reject/approve/close APIs, with ui_task_actor kept separate from model/tool task_actor(turn)
- runtime Phase 2A master-worker command bridge is covered by runtime_dispatches_phase2a_master_worker_loop_into_task_truth and master-worker-foundation-sample
- runtime error-center query bridge routes metadata rows through a UI-safe projection and is covered by runtime and daemon ADP tests
- runtime provider definition upsert and active provider selection dispatch are bound as thin mutation routes into config.core; successful saves project restart-required pending status and active runtime config remains unchanged until restart
- runtime model group definition and active group selection dispatch are bound as thin mutation routes into config.core; successful saves project restart-required pending status and active runtime config remains unchanged until restart
- runtime legacy provider/model update dispatch remains bound as a thin mutation route into config.core for existing callers
- runtime Agent resource-count update dispatch is bound as a thin mutation route into config.core; successful saves project restart-required pending status and active AgentBoard truth remains unchanged until restart/process startup
- runtime Phase 2B EventInbox and MasterPoll bridge is covered by runtime_dispatches_phase2b_master_poll_and_event_inbox, including replay_from_start and a backlog larger than the old default page size
- runtime Phase 2C WorkerControl dispatch/query bridge is covered by runtime_dispatches_worker_control_to_task_owner and runtime_worker_control_invalid_target_returns_explicit_failure
- runtime Tools dashboard query bridge is covered by runtime_query_projects_tool_registry_owner_truth
- runtime Search dashboard query bridge is covered by runtime_query_session_search_returns_worker_hits_under_parent_session
- runtime Diagnostics query bridge is implemented as a thin projection route to runtime log metadata/redacted tail truth and is covered by runtime_query_projects_diagnostics_without_raw_secrets_or_absolute_home.
