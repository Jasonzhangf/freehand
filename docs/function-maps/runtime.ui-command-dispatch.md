# Function Map: `runtime.ui-command-dispatch`

- feature_id: `runtime.ui-command-dispatch`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- owner entry symbols:
  - `RuntimeCommandDispatcher::new`
  - `RuntimeCommandDispatcher::from_selected_agent`
  - `RuntimeCommandDispatcher::from_default_config`
  - `RuntimeCommandDispatcher::dispatch`
  - `RuntimeCommandDispatcher::query_runtime`
  - `restore_session_turns_page_for_ui_query`
  - `RuntimeCommandDispatcher::ui_state`
  - `project_config_status_for_ui`
  - `run_live_reason_turn_with_hooks`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - runtime command dispatch and turn projection operations over `input_attachment`
- touched resources:
  - `input_attachment`
  - `provider_request`
  - `ui_projection`
  - `runtime_agent_activity`
  - `session`
  - `task`
  - `timer`
  - `tool_call`
  - `debug_trace`
  - `account_config_document`
- resource operations:
  - `input_attachment.prepare_provider_input` (`input_attachment` -> `provider_request`)
  - `input_attachment.project_to_ui` (`input_attachment` -> `ui_projection`)
  - `debug_trace.read_snapshot` (`debug_trace` -> `ui_projection`)
  - `turn.project_runtime_agent_activity` (`turn` -> `runtime_agent_activity`)
  - session transcript bridge references `reason.persistence` owner operation `session.restore`
  - timer bridge references `runtime.master-worker-loop` owner operations `timer.schedule`, `timer.cancel`, and `timer.list`
  - tool registry bridge references `tool.registry` owner operation `tool_call.project_registry_to_ui`
  - search bridge references `reason.persistence` owner operation `session.list_persisted` and `task.orchestration` parent-session truth for Worker child nesting
  - account-config pull/push applies synced mirror truth through `config.core` owner operation `account_config_document.apply_shared_account_config` into the live selected agent/effective config
- forbidden shortcuts:
  - Runtime must not persist image base64 in reason/session history or project it to UI history.
  - Runtime activity is a typed control-side projection only; it must not be copied into ADP or UI business payloads.
  - Provider adapters must consume only provider-neutral attachment semantics; runtime must not construct protocol-specific image wire payloads.

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from
  `~/.freehand/config.toml`; Master and Worker selections preserve the same
  configured node pair but bind reason/session projection to the selected
  Agent's own identity and node
- config-selected bootstrap consumes local node id and the ordered configured
  peer set, including each peer node id, allowed IP, and pair-token env, from
  `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- submit commands may carry an optional selected session id and selected cwd so a draft or explicitly chosen cwd-bound session can receive the new turn instead of always using the default session
- submit commands may carry protocol-validated image attachment metadata plus base64 payload; runtime maps that payload into provider-neutral current-request attachments and persists only id/name/media type/size metadata on the turn
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs, then recover or clear dead-owner Master active-work checkpoints before accepting user work
- runtime dispatch owner reads the declared owner target from the envelope
- session management commands route through runtime into `reason.persistence` session metadata and rollback APIs; runtime refreshes `UiProtocolState` from persistence-owned metadata/effective transcript projections after mutation
- live submit registers active turn cancellation state, persists a prepared
  active turn snapshot before context/provider work, and releases the runtime
  mutex before provider IO; Master selection uses Master active-work control
  truth and Master reason policy, while Worker selection uses Worker reason
  policy and must not create or clear Master active-work truth
- Worker-selected dispatch rejects Master-only task orchestration, Master poll,
  Worker control, and direct-to-Slave commands before owner mutation
- `RuntimeCommandDispatcher::current_agent_activity` projects direct-session
  activity from runtime-owned active turns; `RuntimeAgentActivityProjection::merge`
  combines it with lifecycle-owner delegated activity without reading UI or ADP
  payloads
- `CancelLatestActiveTurn` resolves to the newest active live turn before falling back to latest persisted runtime turn
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics
- ADP/read-only task query requests enter through `UiRuntimeQueryPort` and route to `TaskRuntime::list_tasks` or `TaskRuntime::task_history` without duplicating task filtering or ledger ordering in runtime; Worker-selected hosts preserve the Worker reason/session namespace while reading Task Center projections from the paired Master owner namespace
- `QuerySessionTurns` enters through `RuntimeCommandDispatcher::query_runtime`, searches only the master and configured Worker reason-persistence namespaces, restores the requested effective logical-turn snapshots with their owning agent/node source, hides framework-owned `worker-task-*` input prompts from `user_text`, and replaces the derived session transcript so parent and Worker task conversations are visible without daemon restart while preserving live provider/model waiting and tool activity already published into `UiProtocolState` only for the latest nonterminal replacement turn; if the latest background lifecycle turn has no live hook projection yet, the runtime derives same-session/same-turn model-waiting state from ErrorCenter metadata truth before projection, while terminal turn snapshots and historical earlier rounds remain authoritative and cannot be re-lit as active
- `QuerySessionTurnsPage` enters through the same runtime query owner, maps protocol page direction/cursor/limit to `reason.persistence`, and returns only the bounded owner-selected page plus page facts; invalid page requests remain explicit errors and never fall back to `QuerySessionTurns`
- `QuerySessionTurnsPage` applies the protocol owner's page-refresh preservation helper to the returned page, so in-flight same-turn model/tool activity is not lost by refresh while terminal page truth remains authoritative
- `QuerySessionList` and `QueryArchivedSessionList` refresh the selected Agent's persisted metadata and effective turn projections at query time before reading `UiProtocolState`; this makes background-created sessions visible without restart while the selected Agent's persistence namespace remains the only session-list source
- ADP/read-only error-center query requests enter through `UiRuntimeQueryPort` and route to runtime-owned metadata ledger projection without exposing raw provider/tool/request text
- ADP/read-only config status query requests enter through `UiRuntimeQueryPort` with a typed local/remote access scope, reload config-owner truth from the runtime home, and project the selected live agent/provider config plus complete safe provider/model-group registries; remote scope omits loopback Agent URLs, while neither scope exposes API keys, pair tokens, Relay tokens, or credential-bearing URLs
- `PullAccountConfig` enters through `RuntimeCommandDispatcher::dispatch`, resolves the authenticated Relay account through the account-config client, persists the synced or explicit not-configured device mirror, applies a synced mirror through `config.core::apply_shared_account_config` to refresh the live selected agent and effective config model, and returns an `account_config_pulled` receipt
- `PushAccountConfig` enters through `RuntimeCommandDispatcher::dispatch`, exports local non-secret config through `config.core::export_shared_account_config`, sends the mirror etag through the account-config client, persists the synced or conflict device mirror, applies a synced mirror through `config.core::apply_shared_account_config` to refresh the live selected agent and effective config model, and returns an `account_config_pushed` receipt
- provider web_search live-test commands enter through a protocol dispatch envelope, reload config-owner provider truth from runtime home, resolve the requested enabled provider via `LoadedConfig::select_provider_for_test`, and execute one provider-hosted-search test through `provider.reason-live-bridge` outside the runtime mutex
- provider definition upsert commands enter through a protocol dispatch envelope, route to `config.core::upsert_provider_config_in_path`, and must not duplicate config validation or persistence logic in runtime
- provider selection commands enter through a protocol dispatch envelope, route to `config.core::switch_agent_provider_in_path`, and must not duplicate config validation or persistence logic in runtime
- model group definition upsert commands enter through a protocol dispatch envelope, route to `config.core::upsert_model_group_config_in_path`, and must not duplicate route/provider validation or persistence logic in runtime
- model group selection commands enter through a protocol dispatch envelope, route to `config.core::switch_agent_model_group_in_path`, and must not duplicate enabled-group validation or persistence logic in runtime
- legacy provider/model update commands enter through a protocol dispatch envelope, route to `config.core::update_provider_config_in_path`, and remain supported for existing CLI callers
- Agent resource-count update commands enter through a protocol dispatch envelope, route to `config.core::update_agent_resource_config_in_path`, and must not duplicate topology validation or persistence logic in runtime
- successful task tool mutations publish a runtime-owned task list projection into `UiProtocolState` so ADP task list subscribers observe lifecycle changes without UI polling
- protocol-owned task mutation commands route through runtime into `TaskRuntime::create_task`, `TaskRuntime::create_agent`, `TaskRuntime::assign_task`, `TaskRuntime::claim_next_task`, `TaskRuntime::submit_review`, `TaskRuntime::reject_review`, `TaskRuntime::approve_review`, and `TaskRuntime::close_task`; runtime publishes updated task list projection after each accepted mutation
- Phase 1 board/lifecycle queries route `QueryTaskBoard`, `QueryAgentBoard`, and `QueryAgentLifecycle` through runtime into `TaskRuntime::query_task_board`, `TaskRuntime::query_agent_board`, and `TaskRuntime::query_agent_lifecycle`
- Phase 1 execution/timer commands route `ApplyExecutionFact` and `RunSchedulerTick` through runtime into `TaskRuntime::apply_execution_fact` and `TaskRuntime::run_scheduler_tick`; runtime remains a thin bridge and does not make scheduler business decisions
- Phase 2A master/worker sample commands route as thin ADP dispatch into Task
  Center owner truth; runtime does not decide business next actions
- Phase 2B EventInbox and MasterPoll query/command shapes route as thin ADP
  dispatch into Task Center owner truth; runtime does not classify or apply
  business actions locally
- runtime passes Phase 2B `replay_from_start` and `limit` to the owner without
  local cursor policy; closeout samples use replay plus omitted limit to consume
  all pending events instead of only continuing from a stale persisted cursor or
  the first page
- Phase 2C WorkerControl command and QueryWorkerControl query shapes route as
  thin ADP dispatch/query into `worker.control`; runtime converts DTOs and does
  not own control semantics, safe-point queues, or Task Center consequences
- Phase 2 Timer dashboard command and query shapes route through runtime into
  independent timer owner truth. Runtime converts protocol DTOs into timer
  schedule requests, calls TimerStore schedule/cancel/list APIs, and never
  mutates Task Center truth for timer actions.
- Phase 2 Tools dashboard `QueryToolRegistry` routes through runtime into
  `tool.registry` owner truth. Runtime only maps
  `BuiltinToolRegistry::registry_projection` rows into UI DTOs and does not
  execute tools, persist registry truth, or synthesize provider-hosted
  `web_search` as a local tool.
- Phase 2 Search dashboard `QuerySessionSearch` routes through runtime into
  `reason.persistence` persisted session index/metadata truth and
  `task.orchestration` TaskBoard parent-session truth. Runtime returns only
  persisted Master/user sessions as top-level results and nests Worker matches
  under the owning parent session.
- Diagnostics `QueryDiagnostics` routes through runtime into runtime-home log
  truth under `~/.freehand/logs`; runtime projects file metadata and bounded
  redacted tail lines only, without exposing absolute user paths, provider raw
  payloads, secrets, or non-log files.

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- reason-backed submit projects persisted attachment metadata with the turn while excluding image base64 from session history and UI projection
- live provider-backed submit incrementally writes reason/debug projection updates into `UiProtocolState` while the turn is still running
- live provider-backed submit maps `ReasonBroadcastEvent::ToolResult` into `UiProtocolState::apply_tool_result` so tool lifecycle completion is visible over turn SSE
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events so blank UI subscriptions can render a complete public conversation stream; framework-owned `worker-task-*` live pending projections use the same user-text hiding contract as persisted query projections
- accepted live provider-backed submits persist a `RewriteStateUpdated` active snapshot before provider request build so refresh/restart/session queries observe the accepted user turn instead of collapsing to an older or empty transcript; the live bridge filters that prepared current turn out of restored historical context before writing the canonical provider `TurnStarted`
- live provider-backed submit maps `RuntimeLive01ContextPlanningStarted`,
  `RuntimeLive01ContextPlanningCompleted`, and
  `RuntimeLive02ProviderRequestBuilt` debug events into
  `UiProtocolState::apply_model_request_waiting` so ADP clients can see
  pre-provider context preparation and request-sent/model-response-waiting
  state
- live provider-backed submit honors a selected session id when present and keeps the derived UI state pinned to that session instead of the latest global session
- live provider-backed submit honors a selected cwd when present, persists it on the turn record, and reuses the session cwd for later submits that omit cwd
- active live cancel requests set the active cancel token, immediately close the same prepared active snapshot as persisted `Cancelled` truth, clear the in-memory active turn and Master active-work checkpoint, and publish the cancelled projection without waiting for provider completion, provider retry backoff sleep, or pre-provider context construction; cancelled framework `worker-task-*` projections still hide internal task/continuation prompts from `user_text`
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete `turn_id`
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- final live multi-round UI projection preserves final-round visible text, usage, errors, and terminal status without aggregating tool-call/tool-result lifecycle activity from earlier runtime rounds
- live bootstrap restores only authoritative turn snapshots so daemon startup does not replay every historical reason ledger; complete authoritative multi-round snapshots still appear as chronological per-round cards
- live provider-backed submit refreshes `UiProtocolState` from authoritative persistence before returning dispatch failure when the live bridge materializes failed turn truth
- live provider-backed submit materializes an explicit failed turn if the live provider/protocol fails before reason persistence has recovery truth, so the selected session remains observable instead of collapsing into a transport-only dispatch failure
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from the maximum persisted ordinal across all sessions, including WebUI-created non-default sessions
- live bootstrap uses the Master active-work owner path to interrupt recoverable stale active snapshots or clear dead-owner checkpoint-only work; a stale checkpoint with missing session truth or no matching active turn snapshot must not block WebUI/ADP after restart
- live bootstrap rehydrates session cwd from persisted turn records into both `UiProtocolState` and runtime session cwd inheritance state
- selected-session `QuerySessionTurns` performs the heavier exact-round restore and backfills incomplete authoritative Worker/Master transcript snapshots from reason-ledger truth at query time; inactive legacy partial transcripts whose reason ledger is empty return surviving authoritative snapshots with a reason-owned integrity warning instead of a dispatch-port hard failure, while active incomplete snapshots remain hard restore errors
- runtime-owned UI state reflects derived projections only, not authoritative turn truth
- session metadata mutations return receipts only after the persistence owner accepts the create/rename/archive/restore/delete-as-archive operation and the protocol projection has been refreshed
- session rollback mutations return receipts only after the persistence owner writes an append-only rollback marker, runtime cancels non-terminal child tasks created by the rolled-back logical parent turn through `task.cancel`, and runtime replaces the selected session transcript with effective turn projections
- runtime-backed `QuerySessionTurns` refreshes the requested session transcript from its configured master/Worker persistence owner; a missing session stays missing, and a missing `worker-task-*` transcript is explicit target-not-found rather than an empty or different agent/global transcript. Selected transcript refresh preserves live provider transport retry/model waiting and tool activity already present in `UiProtocolState` only for the latest nonterminal replacement turn; when live hooks were missed, provider retry/failover is restored from ErrorCenter metadata as `model_request.transport` and schema retry remains a model request phase only for that latest nonterminal same-session turn. Parent-goal evaluation internal prompt turns hide their synthetic user text while preserving the final assistant decision, and framework-owned `worker-task-*` turns hide internal task contract and continuation prompts from `user_text` while preserving Worker assistant/tool/final truth and source-agent attribution. Inactive partial authoritative transcripts with empty reason ledgers project the surviving snapshots plus the reason-owned partial-transcript warning into UI, and do not become a global ADP connection failure.
- runtime-backed task list and task history queries return UI-safe task projections sourced from `task.orchestration` snapshot and ledger APIs
- runtime-backed task mutation commands return receipts only after `task.orchestration` accepts the mutation and task list projection publication succeeds
- runtime-backed worker claim receipts include the claimed task id and execution id; no-task is explicit and not a success mutation
- runtime-backed TaskBoard and AgentBoard queries return UI-safe board projections sourced from `task.orchestration` and `agent.lifecycle`; TaskBoard task projections preserve task `created_at` for UI submit-receipt correlation
- execution fact and scheduler tick dispatch receipts return only after `task.orchestration` accepts owner truth and task-list publication succeeds
- runtime-backed error-center queries return UI-safe projections sourced from `metadata.core` ledger rows written by `error.center`
- runtime-backed config status queries return UI-safe projections sourced from
  `config.core` selected agent truth and include the ordered peer
  name/mode/node-id list, auth source type, provider web_search effective
  status/reason, active model group id, complete safe model group registry, and route summary only
- account-config pull returns `account_config_pulled` with the account id and server revision when the server document is accepted, and `status=not_configured` when the server reports no document; a synced mirror refreshes `live.selected_agent` and the effective config model through `config.core::apply_shared_account_config`
- account-config push returns `account_config_pushed` only after the server accepts the exported non-secret content and a synced mirror refreshes `live.selected_agent` and the effective config model; conflict is an explicit dispatch failure with the server document persisted in the mirror and is never applied
- runtime-backed provider web_search tests return explicit owner receipts:
  success requires a provider-hosted search observation; provider rejection or
  missing hosted observation is returned as a visible dispatch failure
- successful provider/model updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and leave the active runtime/live provider config unchanged until daemon restart
- successful model group definition and selection updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and leave the active runtime/live provider config unchanged until daemon restart
- successful Agent resource-count updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and do not fabricate live AgentBoard workers before daemon/Worker restart
- runtime-backed task list subscription updates reuse the same projection helper and source task truth from `TaskRuntime::list_tasks`
- runtime-backed EventInbox query returns UI-safe event rows sourced from
  `TaskRuntime::query_event_inbox`
- runtime-backed MasterPoll command returns UI-safe poll outcome sourced from
  `TaskRuntime::run_master_poll`, including classifications and persisted
  cursor evidence without task status mutations
- runtime-backed EventInbox/MasterPoll closeout proof must use
  `replay_from_start=true` plus omitted limit; explicit finite limits are
  pagination and cannot prove no events remain after cursor persistence
- runtime-backed WorkerControl dispatch returns owner receipt evidence only
  after `worker.control` accepts the command, and QueryWorkerControl returns
  persisted control events for same-id restart verification
- runtime-backed Timer dashboard dispatch returns owner receipt evidence only
  after TimerStore persists schedule/cancel truth; QueryTimerList returns
  UI-safe schedule and ledger rows from timer owner truth
- runtime-backed Tools dashboard query returns UI-safe registry rows from
  `BuiltinToolRegistry::registry_projection`, including schema, examples,
  guidance, scope, implemented/read-only state, and Master/Worker exposure
  flags without tool execution
- runtime-backed Search dashboard query returns UI-safe persisted session
  results from `ReasonPersistence::list_persisted_sessions` plus metadata
  sidecars and nests Worker hits via TaskBoard `worker_session_id` /
  `parent_session_id` truth instead of exposing Worker sessions at top level
- runtime-backed Diagnostics query returns UI-safe log file metadata and bounded
  redacted tail lines from runtime-home logs, sorted newest-first, without raw
  provider payloads, secrets, absolute user paths, or browser-local guesses

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- unknown session metadata mutation targets return explicit target-not-found failures
- rollback with no eligible target or with an active turn returns explicit dispatch failure; runtime must not delete UI turns locally to pretend success
- selected-session restore failures remain explicit query failures for that session; inactive partial authoritative restore is not a failure once reason persistence returns surviving snapshots with an integrity warning
- missing turn targets for cancel/resume return explicit dispatch-port failures
- cancelled live turns return explicit cancelled dispatch failure to the original submitter and must not overwrite cancelled UI projection with later provider success
- live provider/tool loops check cancellation at round, stream callback, provider-output, tool-execution, and terminal-write boundaries
- live provider/tool failures that materialize failed turn truth are returned as explicit dispatch failures only after the failed projection has been refreshed into `UiProtocolState`
- live provider/protocol failures before persistence starts still write a failed turn and return an explicit dispatch failure; runtime must not fall back to non-live submit or leave the selected session empty
- `CancelLatestActiveTurn` with no active or persisted turn returns explicit target-not-found
- missing checkpoint manifests return explicit dispatch target-not-found failures
- wrong slave target node returns explicit dispatch-port failures
- missing config, invalid agent selection, paired-token mismatch, or slave-mode host selection return explicit bootstrap failures
- invalid persisted recovery truth or node-metadata bootstrap failure returns explicit runtime bootstrap failure
- unwritable shared node metadata ledgers fail bootstrap explicitly as `NodeRuntimeInit` and must not materialize a runtime dispatcher
- task query misses map to explicit dispatch target-not-found failures; invalid task status filters and task persistence failures map to dispatch failures
- error-center metadata ledger load/parse failures map to explicit dispatch failures; incomplete rows are skipped instead of being repaired into guessed semantics
- config status query without live selected config returns no runtime result rather than inventing app-local config truth
- provider/model update without a live runtime home or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake hot reload
- model group update/selection without a live runtime home or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake hot reload
- Agent resource-count update without a live runtime home, with a non-Master target, or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake live resources
- task list publication failures after task mutation are explicit dispatch failures and must not be silently swallowed as a successful task tool result
- task mutation dispatch requires a live runtime home, maps missing tasks/agents to target-not-found, and must not create task truth outside `task.orchestration`
- invalid task board filters, missing agent lifecycle ids, invalid execution facts, and invalid scheduler thresholds map to explicit dispatch failures from the owner APIs
- invalid EventInbox cursor or MasterPoll cursor persistence failures map to
  explicit dispatch failures from `task.orchestration`
- invalid worker-control targets, unknown operations, missing op-specific
  payloads, terminal tasks, and Task Center consequence failures map to
  explicit dispatch failures from `worker.control`
- invalid timer schedules, unknown timer ids, missing live runtime home, and
  TimerStore persistence failures map to explicit dispatch failures; runtime
  must not create task truth or fake a browser-local timer projection
- tool registry projection failure maps to explicit query failure; runtime must
  not fall back to a hardcoded browser/runtime tool list
- persisted session search failures map to explicit query failure; runtime must
  not fall back to browser-local session filtering or id-prefix guessing
- account-config pull/push without a live runtime home or without an authenticated Relay agent returns an explicit unsupported dispatch failure and never contacts the server or writes a mirror
- account-config apply failure after a synced pull/push returns an explicit dispatch failure, records the apply error in the mirror projection, and never publishes a half-applied effective selected agent

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
- `query_error_center_events_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe error-center event projections from metadata ledger truth
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`
  - related tests: `runtime_query_reads_error_center_metadata_without_raw_text`, `daemon_adp_queries_runtime_error_center_truth`
  - why shared: keeps metadata ledger reads runtime-owned while app transports stay protocol-only
- `apply_error_center_live_activity_to_session_projections`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: restore query-time model waiting/retry state from ErrorCenter metadata only for the latest same-session nonterminal turn when background lifecycle hooks were not present in `UiProtocolState`
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`
  - related tests: `runtime_query_session_turns_projects_background_provider_retry_from_error_center`, `runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry`, `runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round`
  - why shared: keeps ErrorCenter-to-UI projection in the runtime query bridge instead of making UI guess from raw error text or stale AgentBoard state
- `project_config_status_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe active config projection plus complete provider and model group registries from `config.core` selected agent truth
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`, `RuntimeCommandDispatcher::dispatch_update_provider_config`, `RuntimeCommandDispatcher::dispatch_upsert_provider_config`, `RuntimeCommandDispatcher::dispatch_update_agent_provider_selection`, `RuntimeCommandDispatcher::dispatch_upsert_model_group_config`, `RuntimeCommandDispatcher::dispatch_update_agent_model_group_selection`
  - related tests: `runtime_query_projects_config_status_without_secrets`, `runtime_dispatch_updates_provider_config_without_hot_reloading_active_model`, `runtime_dispatch_upserts_provider_registry_without_switching_active_selection`, `runtime_dispatch_switches_agent_provider_selection_without_hot_reload`, `runtime_dispatch_upserts_and_selects_model_group_without_hot_reload`
  - why shared: keeps config-to-UI projection in runtime owner while app transports stay protocol-only
- `project_task_board_for_ui` / `project_agent_board_for_ui` / `project_agent_lifecycle_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 1 task and agent lifecycle owner projections into UI-safe DTOs
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`
  - related tests: `runtime_query_reads_phase1_task_and_agent_boards`
  - why shared: keeps board projection in the runtime bridge while Task Center and Agent Lifecycle remain owner truth
- `project_event_inbox_for_ui` / `project_master_poll_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 2B EventInbox and MasterPoll owner projections into
    UI-safe DTOs
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`,
    `RuntimeCommandDispatcher::dispatch`
  - related tests: `runtime_dispatches_phase2b_master_poll_and_event_inbox`
  - why shared: keeps runtime as a DTO bridge and prevents app-local event
    classification or cursor mutation
- `project_worker_control_for_ui` / `project_worker_control_events_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Phase 2C worker-control owner projections and persisted
    events into UI-safe DTOs
  - allowed callers: `RuntimeCommandDispatcher::dispatch`,
    `RuntimeCommandDispatcher::query_runtime`
  - related tests: `runtime_dispatches_worker_control_to_task_owner`,
    `runtime_worker_control_invalid_target_returns_explicit_failure`
  - why shared: keeps runtime as a DTO bridge while `worker.control` remains the
    ledger and validation owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch and live node metadata bootstrap | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected agent config | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner + runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter, including config-owner provider/model-group mutations | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node/config owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug/tool-result callbacks to runtime-owned consumers | selected live config + live request + callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state, bind optional requested session id/cwd, and persist accepted-turn visibility before provider execution | runtime state + submitted user text + optional requested session id/cwd | prepared live submit + active cancel token + canonical cwd + prepared active turn truth | `RuntimeCommandDispatcher::dispatch` | runtime owner + reason persistence | bound |
| 07p | `persist_prepared_live_submit_active_turn` | `crates/freehand-runtime/src/turn_projection.rs` | persist a prepared active turn snapshot through `ReasonPersistence::record_rewrite_state_updated` without writing duplicate `TurnStarted` truth | prepared live submit + current session history | queryable active turn projection and reason active snapshot | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `ReasonPersistence::record_rewrite_state_updated` + UI projection | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed live turn outside runtime mutex while honoring active cancel token | prepared live submit | live receipt or cancelled dispatch failure | `RuntimeCommandDispatcher::dispatch` | `run_live_reason_turn_with_hooks` | bound |
| 08a | `restore_or_materialize_failed_live_submit` | `crates/freehand-runtime/src/turn_projection.rs` | restore bridge-materialized failed turn truth or close the same prepared active snapshot before returning dispatch failure | prepared live submit + live bridge error | failed turn projection and persisted failed turn truth | `RuntimeCommandDispatcher::finish_live_submit` | reason persistence + UI projection | bound |
| 08b | `restore_or_materialize_cancelled_live_submit` | `crates/freehand-runtime/src/turn_projection.rs` | restore bridge-materialized cancelled turn truth or close the same prepared active snapshot before returning dispatch cancellation | prepared live submit + cancellation summary | cancelled turn projection and persisted cancelled turn truth | `RuntimeCommandDispatcher::finish_live_submit` | reason persistence + UI projection | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics, immediate prepared-active snapshot materialization, Master active-work cleanup, and UI projection | cancel command turn id | cancel receipt + persisted cancelled turn + cancelled projection | `RuntimeCommandDispatcher::dispatch` | reason owner / active cancel registry / `restore_or_materialize_cancelled_live_submit` / `master_runner::clear_master_active_work_if_current` | bound |
| 10 | `restore_all_persisted_sessions_into_ui` | `crates/freehand-runtime/src/turn_projection.rs` | rehydrate UI protocol state from reason-owned authoritative UI turn snapshots without replaying every historical reason ledger during daemon bootstrap | persisted reason sessions | lightweight derived UI session list/transcripts from authoritative snapshots; complete multi-round snapshots retain per-round tool activity | runtime bootstrap | `ReasonPersistence::restore_authoritative_turn_snapshots_for_ui` + UI protocol | bound |
| 10a | `RuntimeCommandDispatcher::recover_stale_master_active_work_on_bootstrap` / `RuntimeCommandDispatcher::recover_stale_master_active_work_before_live_submit` | `crates/freehand-runtime/src/lib.rs` | recover or clear dead-owner Master active-work before the runtime accepts new WebUI/ADP work | Master active-work checkpoint plus optional reason active snapshot | interrupted stale active turn projection or cleared checkpoint-only stale work | `RuntimeCommandDispatcher::new` / live submit preparation | `master_runner::recoverable_stale_master_active_work` + `ReasonPersistence::restore` + `master_runner::clear_master_active_work_if_current` | bound |
| 11 | `RuntimeCommandDispatcher::dispatch_session_management` | `crates/freehand-runtime/src/lib.rs` | route protocol-owned session CRUD and rollback commands into reason persistence APIs, cancel rolled-back child task truth, and refresh UI projection | session CRUD or rollback dispatch envelope | dispatch receipt or target-not-found failure | `RuntimeCommandDispatcher::dispatch` | `ReasonPersistence` session metadata/rollback owner + rollback child cleanup | bound |
| 11a | `cancel_tasks_for_session_rollback` | `crates/freehand-runtime/src/lib.rs` | cancel non-terminal child tasks whose parent turn shares the rolled-back logical turn while leaving retained-turn child tasks untouched | runtime home + master agent id + rollback marker | Task Center truth with rolled-back children `Cancelled` or explicit owner failure | `RuntimeCommandDispatcher::dispatch_session_management` | `TaskRuntime::query_task_board` + `TaskRuntime::cancel_task` | bound |
| 11b | `UiProtocolState::replace_session_turn_projections` | `crates/freehand-ui-protocol/src/lib.rs` | replace a session transcript with persistence-owned effective projections after rollback | session id + effective turn projections | queryable transcript excluding rolled-back logical turn | `RuntimeCommandDispatcher::dispatch_session_management` | ui.protocol state | bound |
| 12 | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | route read-only runtime queries such as session turns, config status, task list/history, and error-center events into owner APIs | UI query command | optional query result or explicit dispatch failure | WebUI/daemon ADP query transport | reason persistence / selected config / task runtime owner / metadata center | bound |
| 12s | `RuntimeCommandDispatcher::query_runtime` / `restore_session_turns_for_ui_query` / `apply_error_center_live_activity_to_session_projections` / `UiProtocolState::replace_session_turn_projections` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-ui-protocol/src/lib.rs` | restore exact-round `QuerySessionTurns` snapshots from the master plus configured Worker reason-persistence namespaces, derive latest-turn nonterminal retry/waiting state from ErrorCenter metadata when live hooks are absent, and replace the derived transcript at query time without projecting framework-owned parent/Worker prompts as user-authored text or reactivating historical retry rows; inactive partial authoritative transcripts with empty reason ledgers surface the reason-owned integrity warning on the surviving turns | session id | fresh queryable transcript with `runtime-turn-N` / `runtime-turn-N-rM` rounds preserved, per-round tool activity visible, partial-transcript warning visible when reason ledger is empty, background provider retry/failover observable only as model-request transport substate on the latest nonterminal owning turn, schema waiting observable as model-request phase, terminal and historical earlier turns not reactivated, internal parent-goal evaluation user text hidden, and Worker task transcript rows sourced to the configured Worker with internal task prompts hidden from `user_text` | ADP query transport / runtime query tests | `ReasonPersistence::restore_turn_snapshots_for_ui` + configured agent/node map + turn projection + ErrorCenter metadata projection + ui.protocol state | bound |
| 12u | `project_runtime_turn_history` / `ui_user_text_for_turn` / `merge_hosted_search_activities` | `crates/freehand-runtime/src/turn_projection.rs` / `crates/freehand-ui-protocol/src/projection.rs` | convert runtime turn truth into UI-safe turn projection while hiding framework-owned parent/Worker prompt text and merging typed hosted search tool activities from persisted search evidence | turn record plus source agent/node and optional cwd | UI turn projection with normal user text preserved, internal framework prompt text hidden, and hosted search tool activities merged | runtime query/dispatch projection paths | `turn_projection_from_events` / `turn_projection_for_client` / ui.protocol projection | bound |
| 12t | `publish_live_pending_user_projection` / `publish_live_cancelled_projection` / `ui_user_text_projection_for_session_user_text` / `ui_should_hide_user_text` | `crates/freehand-runtime/src/turn_projection.rs` | apply the same internal-prompt hiding contract to live pending and cancelled turn projections before they enter `UiProtocolState` | active session id plus submitted/internal prompt text | live UI projection whose `user_text` is absent for framework parent/Worker prompts and present for normal user sessions | live submit/cancel path / runtime projection tests | ui.protocol state | bound |
| 12a | `project_config_status_for_ui` / `project_config_status_from_path_for_ui` / `safe_provider_base_url_for_projection` / `provider_base_url_host_for_projection` / `provider_web_search_effective_status` / `provider_web_search_route_summary` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-config/src/lib.rs` | convert selected live agent config and config-owner provider/model group registries into UI-safe config status including web_search configured/effective route diagnostics and typed local/remote endpoint visibility | selected agent config plus loaded config registry plus `UiQueryAccessScope` | secret-free config status projection with active primary/fallback, active model group, complete provider registry, complete model group registry, web_search route status, and no loopback URLs in remote scope | `RuntimeCommandDispatcher::query_runtime_with_scope` / config mutation dispatchers | UI protocol DTO | bound |
| 13 | `project_task_list_for_ui` / `project_task_history_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert task owner snapshots and ledger events into protocol DTOs without changing task truth | task snapshots or ledger events | UI-safe task query projection | `RuntimeCommandDispatcher::query_runtime` | UI protocol DTOs | bound |
| 14 | `task_list_projection_from_runtime` / `UiProtocolState::publish_task_list_projection` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-ui-protocol/src/lib.rs` | publish runtime-owned task list projection after successful task tool mutation | task runtime snapshot | UI task list subscription event | live task tool bridge | ui.protocol subscription channel | bound |
| 15 | `query_error_center_events_for_ui` | `crates/freehand-runtime/src/lib.rs` | read session metadata ledger and filter `error.center` rows by trace, turn, and domain | runtime home plus query filters | UI-safe error-center event list | `RuntimeCommandDispatcher::query_runtime` | metadata center | bound |
| 16 | `project_error_center_event_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert one watermarked error-center metadata row into ADP DTO fields | metadata envelope | `UiErrorCenterEventProjection` or skipped row | runtime query bridge | ui.protocol DTO | bound |
| 17 | `RuntimeCommandDispatcher::dispatch_update_provider_config` | `crates/freehand-runtime/src/lib.rs` | route provider/model update dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | `UiProviderConfigUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `update_provider_config_in_path` | bound |
| 17a | `RuntimeCommandDispatcher::dispatch_upsert_provider_config` | `crates/freehand-runtime/src/lib.rs` | route provider definition upsert dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | `UiProviderConfigUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `upsert_provider_config_in_path` | bound |
| 17b | `RuntimeCommandDispatcher::dispatch_update_agent_provider_selection` | `crates/freehand-runtime/src/lib.rs` | route active provider selection dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | `UiAgentProviderSelectionUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `switch_agent_provider_in_path` | bound |
| 17c | `RuntimeCommandDispatcher::dispatch_test_provider_web_search` / `execute_provider_web_search_test` | `crates/freehand-runtime/src/lib.rs` | route provider-hosted web_search live-test dispatch into config-selected provider execution without holding runtime state lock or exposing provider wire DTOs | `UiCommand::TestProviderWebSearch` dispatch envelope | provider test receipt or explicit provider/runtime failure | `RuntimeCommandDispatcher::dispatch` | `LoadedConfig::select_provider_for_test` + live provider driver | bound |
| 18 | `update_provider_config_in_path` / `upsert_provider_config_in_path` / `switch_agent_provider_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist provider definition updates and active provider selection through canonical config owner | runtime config path + provider definition or selection update | selected agent config projection from saved TOML | config mutation dispatchers | config.core persistence | bound |
| 18m | `RuntimeCommandDispatcher::dispatch_upsert_model_group_config` / `RuntimeCommandDispatcher::dispatch_update_agent_model_group_selection` | `crates/freehand-runtime/src/lib.rs` | route model group definition and active group selection dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | `UiModelGroupConfigUpdate` or `UiAgentModelGroupSelectionUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `upsert_model_group_config_in_path` / `switch_agent_model_group_in_path` | bound |
| 18n | `upsert_model_group_config_in_path` / `switch_agent_model_group_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist model group definitions and active model-group selection through canonical config owner | runtime config path + model group definition or selection update | selected agent config projection from saved TOML | model group mutation dispatchers | config.core persistence | bound |
| 18a | `RuntimeCommandDispatcher::dispatch_update_agent_resource_config` | `crates/freehand-runtime/src/lib.rs` | route Agent resource-count update dispatch into the config owner and store pending restart-required UI projection without fabricating live workers | `UiAgentResourceConfigUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `update_agent_resource_config_in_path` | bound |
| 18b | `update_agent_resource_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist reciprocal Master/Worker topology update through canonical config owner | runtime config path + resource-count update | selected agent config projection from saved TOML | `RuntimeCommandDispatcher::dispatch_update_agent_resource_config` | config.core persistence | bound |
| 19 | `project_task_board_for_ui` / `project_task_snapshot_for_ui` / `project_agent_board_for_ui` / `project_agent_lifecycle_for_ui` | `crates/freehand-runtime/src/lib.rs` | project Phase 1 TaskBoard, AgentBoard, and AgentLifecycle owner truth into protocol DTOs, preserving task `created_at` from owner snapshots | task/agent owner projections | UI-safe board/lifecycle query results | `RuntimeCommandDispatcher::query_runtime` | ui.protocol DTOs | bound |
| 20 | `RuntimeCommandDispatcher::dispatch_apply_execution_fact` / `dispatch_run_scheduler_tick` | `crates/freehand-runtime/src/lib.rs` | route Phase 1 execution facts and scheduler ticks into task.orchestration without making business decisions | execution fact or scheduler tick dispatch envelope | dispatch receipt or owner failure | `RuntimeCommandDispatcher::dispatch` | `TaskRuntime::apply_execution_fact` / `TaskRuntime::run_scheduler_tick` | bound |
| 21 | `RuntimeCommandDispatcher::dispatch_create_task_agent` / `dispatch_assign_task` / `dispatch_claim_next_task` / `dispatch_reject_task_review` | `crates/freehand-runtime/src/lib.rs` | route Phase 2A worker registry, assignment, claim, and review rejection commands into task.orchestration | protocol task mutation command | dispatch receipt plus task list projection | `RuntimeCommandDispatcher::dispatch` | `TaskRuntime` task owner APIs | bound |
| 22 | `project_event_inbox_for_ui` / `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | route Phase 2B EventInbox query into task.orchestration and project UI-safe event rows; omitted limit drains all matching rows | inbox query command | EventInbox query projection | runtime query dispatch | task owner | bound |
| 23 | `RuntimeCommandDispatcher::dispatch_run_master_poll` / `project_master_poll_for_ui` | `crates/freehand-runtime/src/lib.rs` | route Phase 2B MasterPoll command into task.orchestration and project classifications without business mutations; replay_from_start plus omitted limit drains all rows before cursor persistence | master poll command | MasterPoll projection and persisted cursor receipt | runtime ADP command dispatch / CLI sample | task owner | bound |
| 24 | `RuntimeCommandDispatcher::dispatch_worker_control` / `project_worker_control_for_ui` | `crates/freehand-runtime/src/lib.rs` | route Phase 2C WorkerControl commands into worker.control and project accepted event receipt without owning control semantics | worker-control dispatch envelope | dispatch receipt with control id/op/status | `RuntimeCommandDispatcher::dispatch` | `TaskRuntime::apply_worker_control` | bound |
| 25 | `RuntimeCommandDispatcher::query_runtime` / `project_worker_control_events_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryWorkerControl into worker.control persisted event truth for restart verification | task id plus execution id query | `UiWorkerControlProjection` event list | ADP query transport | `TaskRuntime::query_worker_control_events` | bound |
| 26 | `RuntimeCommandDispatcher::query_runtime` / `project_timer_list_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryTimerList into timer owner schedule and ledger truth, then project UI-safe TimerList rows | include_terminal timer query flag | `UiTimerListProjection` or explicit timer-store failure | ADP query transport | `TimerStore::load_schedules` / `TimerStore::load_events` | bound |
| 27 | `RuntimeCommandDispatcher::dispatch_schedule_timer` / `RuntimeCommandDispatcher::dispatch_cancel_timer` | `crates/freehand-runtime/src/lib.rs` | route ScheduleTimer and CancelTimer command envelopes into TimerStore owner mutation APIs and return user-safe timer receipts | validated timer schedule or cancel command | `timer_scheduled` or `timer_cancelled` receipt, or explicit owner failure | `RuntimeCommandDispatcher::dispatch` | `TimerStore::schedule_from_request` / `TimerStore::upsert_schedule` / `TimerStore::cancel` | bound |
| 28 | `RuntimeCommandDispatcher::query_runtime` / `project_tool_registry_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryToolRegistry into tool.registry owner projection and convert rows into UI-safe protocol DTOs without executing tools | tool registry query | `UiToolRegistryProjection` or explicit query failure | ADP query transport | `BuiltinToolRegistry::registry_projection` | bound |
| 29 | `RuntimeCommandDispatcher::query_runtime` / `query_session_search_for_ui` / `worker_parent_session_map` | `crates/freehand-runtime/src/lib.rs` | route QuerySessionSearch into reason.persistence persisted session index/metadata truth, join Worker matches through TaskBoard parent-session truth, and return only persisted Master/user sessions as top-level results | search query plus optional limit | `UiSessionSearchProjection` or explicit search/query failure | ADP query transport | `ReasonPersistence::list_persisted_sessions` / `ReasonPersistence::load_session_metadata` / `TaskRuntime::query_task_board` | bound |
| 30 | `project_diagnostics_for_ui` | `crates/freehand-runtime/src/lib.rs` | route QueryDiagnostics into runtime-home diagnostics log projection, include only .log metadata plus bounded redacted tail lines, and keep raw payloads, secrets, and absolute user paths out of UI DTOs | diagnostics query plus live runtime home | `UiDiagnosticsProjection` or explicit query failure | ADP query transport | runtime logs under `~/.freehand/logs` | bound |
| 31 | `RuntimeCommandDispatcher::dispatch_compact_session_context` | `crates/freehand-runtime/src/lib.rs` | route CompactSessionContext through reason.rewrite-policy truth by restoring the persisted session history and running `ReasonRewriteRuntime::apply_compaction_policy`, returning an explicit hold/soft-notice/stale-prune/staged receipt without fabricating a rewrite result | CompactSessionContext command plus request reason | compaction policy receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `ReasonPersistence::restore` / `ReasonRewriteRuntime::apply_compaction_policy` | bound |
| 32 | `RuntimeCommandDispatcher::dispatch_pull_account_config` | `crates/freehand-runtime/src/lib.rs` | route PullAccountConfig through the authenticated Relay account-config client, persist the synced or not-configured device mirror, apply a synced mirror through `config.core::apply_shared_account_config` to refresh the live selected agent, and return the runtime receipt | PullAccountConfig command envelope plus live selected Relay connection | `account_config_pulled` receipt with refreshed effective selected agent or explicit failure | `RuntimeCommandDispatcher::dispatch` | `AccountConfigClient::pull` / `AccountConfigMirror::synced` / `AccountConfigMirror::not_configured` / `config.core::apply_shared_account_config` / `refresh_selected_agent_from_account_config` | bound |
| 33 | `RuntimeCommandDispatcher::dispatch_push_account_config` | `crates/freehand-runtime/src/lib.rs` | route PushAccountConfig through config.core export plus the account-config client, persist the synced or conflict device mirror, apply a synced mirror through `config.core::apply_shared_account_config` to refresh the live selected agent, and return the runtime receipt | PushAccountConfig command envelope plus live selected Relay connection and local config | `account_config_pushed` receipt with refreshed effective selected agent or explicit conflict/apply/dispatch failure | `RuntimeCommandDispatcher::dispatch` | `export_shared_account_config` / `AccountConfigClient::push` / `AccountConfigMirror::synced` / `AccountConfigMirror::conflict` / `config.core::apply_shared_account_config` / `refresh_selected_agent_from_account_config` | bound |

## Sync Status Against Code

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- live provider submit now binds requested/session cwd into pending, final, and restored UI projection state
- live provider submit now projects context-planning and provider-request-built
  debug events into `UiProtocolState.model_request` before model response
  arrives
- live provider submit now streams tool-result updates into `UiProtocolState` so tool activities can transition to completed before final receipt
- live submit now releases the runtime mutex before provider IO so `CancelTurn` can enter concurrently
- active live cancel now persists cancelled terminal truth and clears active-work immediately, so a stuck pre-provider context builder cannot leave the session active after cancel; later provider success cannot overwrite the cancelled projection
- runtime dispatch now supports `CancelLatestActiveTurn` for current-turn stop without requiring the UI to know `turn_id`
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and terminal persistence
- missing `CancelTurn`, empty `CancelLatestActiveTurn`, and wrong-node direct-message dispatch paths now stay explicit target-not-found failures
- direct slave message dispatch routes through `node.master-slave`
- explicit checkpoint rewind dispatch now routes through `runtime.checkpoint-rewind`
- missing checkpoint rewind manifests now stay explicit target-not-found dispatch failures instead of being collapsed into generic success or fallback projection
- resume dispatch remains an explicit unsupported runtime path
- config-selected runtime bootstrap is now bound in code
- config-selected runtime bootstrap uses explicit peer-topology config instead of synthetic paired node ids
- config-selected live bootstrap now seeds a shared metadata ledger path into `node.master-slave` before the first command runs
- unwritable shared node metadata ledgers are now regression-locked as explicit bootstrap failures
- config-selected live bootstrap restores persisted turn projection and next runtime turn ordinal from all persisted sessions when recovery truth exists
- config-selected live bootstrap now restores authoritative UI snapshots without scanning every historical reason ledger; selected `QuerySessionTurns` remains the exact per-round ledger-backfill path for incomplete old Worker/Master transcripts
- config-selected live bootstrap now recovers or clears dead-owner Master active-work checkpoints, including checkpoint-only stale work with missing session truth or no matching active turn snapshot
- selected `QuerySessionTurns` now preserves active live provider transport retry/model waiting and tool activity already published into `UiProtocolState` only for the latest nonterminal replacement turn; it also reconstructs background lifecycle provider retry/failover as model-request transport substate and schema waiting as model-request phase from ErrorCenter metadata only for that latest nonterminal turn, and refuses to reactivate terminal or historical earlier turns. Covered by `runtime_query_session_turns_preserves_live_provider_retry_activity`, `runtime_query_session_turns_preserves_live_tool_activity`, `runtime_query_session_turns_projects_background_provider_retry_from_error_center`, `runtime_query_session_turns_does_not_reactivate_terminal_error_center_retry`, and `runtime_query_session_turns_does_not_reactivate_historical_retry_before_later_terminal_round`.
- config-selected live bootstrap now restores persisted session cwd from turn records and preserves cwd for later same-session submits
- runtime session-management dispatch is bound as a thin route to `reason.persistence`
- runtime rollback dispatch is bound as a thin route to `reason.persistence::rollback_latest_session_turn` plus UI effective transcript replacement
- runtime task query dispatch is bound as a thin read-only route to `task.orchestration`
- runtime Phase 1 TaskBoard and AgentBoard query dispatch is bound as a thin read-only route to `task.orchestration` and `agent.lifecycle`
- runtime Phase 1 execution fact and scheduler tick dispatch is bound as a thin mutation route into `task.orchestration`; scheduler ticks emit facts/recommendations only
- runtime error-center query dispatch is bound as a thin read-only route to `metadata.core` rows written by `error.center`
- runtime config status query dispatch is bound as a thin read-only route from selected `config.core` truth to `UiConfigStatusProjection`, including complete provider registry and current fallback id
- runtime PullAccountConfig/PushAccountConfig dispatch is bound as a thin route to `config.account-config-sync`; synced mirror content is applied through `config.core::apply_shared_account_config` into the live selected agent/effective config model and is covered by `runtime_account_config_sync_pull_applies_shared_provider_to_effective_config` plus `runtime_account_config_sync_bootstrap_applies_existing_synced_mirror`, while Relay authentication remains runtime-selected connection truth and client validation/mirror persistence stays in the account-config owner
- runtime provider definition upsert and active provider selection dispatch are bound as thin mutation routes into `config.core`; successful saves project restart-required pending status and active runtime config remains unchanged until restart
- runtime legacy provider/model update dispatch remains bound as a thin mutation route into `config.core` for existing callers
- runtime Agent resource-count update dispatch is bound as a thin mutation route into `config.core`; successful saves project restart-required pending status and active AgentBoard truth remains unchanged until restart/process startup
- runtime task list projection publication is bound as a thin route from task mutation to `ui.protocol`
- runtime task mutation dispatch is bound as a thin route from protocol commands to `task.orchestration` create/create_agent/assign/claim/review/reject/approve/close APIs, with `ui_task_actor` kept separate from model/tool `task_actor(turn)`
- runtime Phase 2B EventInbox query and MasterPoll command are implemented as
  thin routes to `task.orchestration`; online S-profile closeout is still
  required before claiming the phase complete
- runtime Phase 2C WorkerControl command/query bridge is implemented as a thin
  route to `worker.control`; online S-profile closeout is still required before
  claiming Phase 2C complete
- runtime Tools dashboard query bridge is implemented as a thin projection route
  to `tool.registry` and is covered by
  `runtime_query_projects_tool_registry_owner_truth`
- runtime Search dashboard query bridge is implemented as a thin projection route
  to reason.persistence plus task parent truth and is covered by
  `runtime_query_session_search_returns_worker_hits_under_parent_session`
- runtime Diagnostics query bridge is implemented as a thin projection route to
  runtime log metadata/redacted tail truth and is covered by
  `runtime_query_projects_diagnostics_without_raw_secrets_or_absolute_home`
- final live projection now keeps each runtime round as its own UI turn so earlier-round tool activity cannot be merged into the final latest turn
- failed live bridge tool execution now refreshes runtime UI state from persisted failed turn truth before returning the dispatch error, so WebUI query/SSE can observe failure instead of waiting forever
- early live provider/protocol failure now creates a persisted failed turn and session projection instead of falling back to non-live submit or returning transport-only dispatch failure
- context compaction dispatch restores the persisted session history from reason.persistence, runs `ReasonRewriteRuntime::apply_compaction_policy` through `reason.rewrite-policy` truth with the request reason, and returns an explicit hold/soft-notice/stale-prune/staged receipt; missing persisted recovery truth fails dispatch explicitly instead of falling back to an empty history, and the reason payload is preserved in the receipt status
- migrated mainline-call source now lives at `docs/mainline-calls/runtime.ui-command-dispatch.json` and generated wiki lives at `docs/wiki/runtime.ui-command-dispatch.md`
