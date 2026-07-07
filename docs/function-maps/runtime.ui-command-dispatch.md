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
  - `RuntimeCommandDispatcher::ui_state`
  - `run_live_reason_turn_with_hooks`

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from `~/.freehand/config.toml`
- config-selected bootstrap consumes local node id, paired node id, paired allowed IP, and paired token env from `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- submit commands may carry an optional selected session id and selected cwd so a draft or explicitly chosen cwd-bound session can receive the new turn instead of always using the default session
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs
- runtime dispatch owner reads the declared owner target from the envelope
- session management commands route through runtime into `reason.persistence` session metadata and rollback APIs; runtime refreshes `UiProtocolState` from persistence-owned metadata/effective transcript projections after mutation
- live submit registers active turn cancellation state before provider execution and releases the runtime mutex before provider IO
- `CancelLatestActiveTurn` resolves to the newest active live turn before falling back to latest persisted runtime turn
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics
- ADP/read-only task query requests enter through `UiRuntimeQueryPort` and route to `TaskRuntime::list_tasks` or `TaskRuntime::task_history` without duplicating task filtering or ledger ordering in runtime
- ADP/read-only error-center query requests enter through `UiRuntimeQueryPort` and route to runtime-owned metadata ledger projection without exposing raw provider/tool/request text
- ADP/read-only config status query requests enter through `UiRuntimeQueryPort` and project the selected live agent/provider config without exposing API keys, pair tokens, or credential-bearing URLs
- provider/model update commands enter through a protocol dispatch envelope, route to `config.core::update_provider_config_in_path`, and must not duplicate config validation or persistence logic in runtime
- successful task tool mutations publish a runtime-owned task list projection into `UiProtocolState` so ADP task list subscribers observe lifecycle changes without UI polling

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- live provider-backed submit incrementally writes reason/debug projection updates into `UiProtocolState` while the turn is still running
- live provider-backed submit maps `ReasonBroadcastEvent::ToolResult` into `UiProtocolState::apply_tool_result` so tool lifecycle completion is visible over turn SSE
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events so blank UI subscriptions can render a complete public conversation stream
- live provider-backed submit maps `RuntimeLive02ProviderRequestBuilt` debug events into `UiProtocolState::apply_model_request_waiting` so ADP clients can see request-sent/model-response-waiting state
- live provider-backed submit honors a selected session id when present and keeps the derived UI state pinned to that session instead of the latest global session
- live provider-backed submit honors a selected cwd when present, persists it on the turn record, and reuses the session cwd for later submits that omit cwd
- active live cancel requests set the active cancel token immediately and publish a cancelled UI projection without waiting for provider completion
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete `turn_id`
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- final live multi-round UI projection preserves final-round visible text, usage, errors, and terminal status without aggregating tool-call/tool-result lifecycle activity from earlier runtime rounds
- live bootstrap restores multi-round UI projections from reason-ledger snapshots as chronological per-round cards so earlier-round tool activity remains on its own turn after daemon restart
- live provider-backed submit refreshes `UiProtocolState` from authoritative persistence before returning dispatch failure when the live bridge materializes failed turn truth
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from the maximum persisted ordinal across all sessions, including WebUI-created non-default sessions
- live bootstrap rehydrates session cwd from persisted turn records into both `UiProtocolState` and runtime session cwd inheritance state
- live bootstrap groups persisted `runtime-turn-N` round snapshots into one UI projection when restoring session transcripts, while keeping authoritative closed-turn recovery unchanged
- runtime-owned UI state reflects derived projections only, not authoritative turn truth
- session metadata mutations return receipts only after the persistence owner accepts the create/rename/archive/restore/delete-as-archive operation and the protocol projection has been refreshed
- session rollback mutations return receipts only after the persistence owner writes an append-only rollback marker and runtime replaces the selected session transcript with effective turn projections
- runtime-backed task list and task history queries return UI-safe task projections sourced from `task.orchestration` snapshot and ledger APIs
- runtime-backed error-center queries return UI-safe projections sourced from `metadata.core` ledger rows written by `error.center`
- runtime-backed config status queries return UI-safe projections sourced from `config.core` selected agent truth and include auth source type only
- successful provider/model updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and leave the active runtime/live provider config unchanged until daemon restart
- runtime-backed task list subscription updates reuse the same projection helper and source task truth from `TaskRuntime::list_tasks`

## Error Mainline

- unsupported runtime command paths return explicit dispatch-port failures
- unknown session metadata mutation targets return explicit target-not-found failures
- rollback with no eligible target or with an active turn returns explicit dispatch failure; runtime must not delete UI turns locally to pretend success
- missing turn targets for cancel/resume return explicit dispatch-port failures
- cancelled live turns return explicit cancelled dispatch failure to the original submitter and must not overwrite cancelled UI projection with later provider success
- live provider/tool loops check cancellation at round, stream callback, provider-output, tool-execution, and terminal-write boundaries
- live provider/tool failures that materialize failed turn truth are returned as explicit dispatch failures only after the failed projection has been refreshed into `UiProtocolState`
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
- task list publication failures after task mutation are explicit dispatch failures and must not be silently swallowed as a successful task tool result

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
- `project_config_status_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe active config projection from `config.core` selected agent truth
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`, `RuntimeCommandDispatcher::dispatch_update_provider_config`
  - related tests: `runtime_query_projects_config_status_without_secrets`, `runtime_dispatch_updates_provider_config_without_hot_reloading_active_model`
  - why shared: keeps config-to-UI projection in runtime owner while app transports stay protocol-only

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch and live node metadata bootstrap | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected agent config | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner + runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug/tool-result callbacks to runtime-owned consumers | selected live config + live request + callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state before provider execution and bind optional requested session id/cwd | runtime state + submitted user text + optional requested session id/cwd | prepared live submit + active cancel token + canonical cwd | `RuntimeCommandDispatcher::dispatch` | runtime owner | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed live turn outside runtime mutex while honoring active cancel token | prepared live submit | live receipt or cancelled dispatch failure | `RuntimeCommandDispatcher::dispatch` | `run_live_reason_turn_with_hooks` | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics and UI projection | cancel command turn id | cancel receipt + cancelled projection | `RuntimeCommandDispatcher::dispatch` | reason owner / active cancel registry | bound |
| 10 | `restore_all_persisted_sessions_into_ui` | `crates/freehand-runtime/src/lib.rs` | rehydrate UI protocol state from reason-ledger turn snapshots and group same-ordinal runtime rounds for transcript projection | persisted reason sessions | derived UI session list/transcripts with retained tool activity | runtime bootstrap | reason persistence + UI protocol | bound |
| 11 | `RuntimeCommandDispatcher::dispatch_session_management` | `crates/freehand-runtime/src/lib.rs` | route protocol-owned session CRUD and rollback commands into reason persistence APIs and refresh UI projection | session CRUD or rollback dispatch envelope | dispatch receipt or target-not-found failure | `RuntimeCommandDispatcher::dispatch` | `ReasonPersistence` session metadata/rollback owner | bound |
| 11a | `UiProtocolState::replace_session_turn_projections` | `crates/freehand-ui-protocol/src/lib.rs` | replace a session transcript with persistence-owned effective projections after rollback | session id + effective turn projections | queryable transcript excluding rolled-back logical turn | `RuntimeCommandDispatcher::dispatch_session_management` | ui.protocol state | bound |
| 12 | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | route read-only runtime queries such as config status, task list/history, and error-center events into owner APIs | UI query command | optional query result or explicit dispatch failure | WebUI/daemon ADP query transport | selected config / task runtime owner / metadata center | bound |
| 12a | `project_config_status_for_ui` / `config_base_url_host_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert selected live agent config into UI-safe active config status | selected agent config | secret-free config status projection | `RuntimeCommandDispatcher::query_runtime` | UI protocol DTO | bound |
| 13 | `project_task_list_for_ui` / `project_task_history_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert task owner snapshots and ledger events into protocol DTOs without changing task truth | task snapshots or ledger events | UI-safe task query projection | `RuntimeCommandDispatcher::query_runtime` | UI protocol DTOs | bound |
| 14 | `task_list_projection_from_runtime` / `UiProtocolState::publish_task_list_projection` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-ui-protocol/src/lib.rs` | publish runtime-owned task list projection after successful task tool mutation | task runtime snapshot | UI task list subscription event | live task tool bridge | ui.protocol subscription channel | bound |
| 15 | `query_error_center_events_for_ui` | `crates/freehand-runtime/src/lib.rs` | read session metadata ledger and filter `error.center` rows by trace, turn, and domain | runtime home plus query filters | UI-safe error-center event list | `RuntimeCommandDispatcher::query_runtime` | metadata center | bound |
| 16 | `project_error_center_event_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert one watermarked error-center metadata row into ADP DTO fields | metadata envelope | `UiErrorCenterEventProjection` or skipped row | runtime query bridge | ui.protocol DTO | bound |
| 17 | `RuntimeCommandDispatcher::dispatch_update_provider_config` | `crates/freehand-runtime/src/lib.rs` | route provider/model update dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | `UiProviderConfigUpdate` dispatch envelope | dispatch receipt or explicit dispatch failure | `RuntimeCommandDispatcher::dispatch` | `update_provider_config_in_path` | bound |
| 18 | `update_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist provider/model update through canonical config owner | runtime config path + provider update | selected agent config projection from saved TOML | `RuntimeCommandDispatcher::dispatch_update_provider_config` | config.core persistence | bound |

## Sync Status Against Code

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- live provider submit now binds requested/session cwd into pending, final, and restored UI projection state
- live provider submit now projects provider-request-built debug events into `UiProtocolState.model_request` before model response arrives
- live provider submit now streams tool-result updates into `UiProtocolState` so tool activities can transition to completed before final receipt
- live submit now releases the runtime mutex before provider IO so `CancelTurn` can enter concurrently
- active live cancel now publishes cancelled UI projection immediately and later provider success cannot overwrite it
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
- config-selected live bootstrap now restores multi-round tool activity into UI session transcripts after daemon restart
- config-selected live bootstrap now restores persisted session cwd from turn records and preserves cwd for later same-session submits
- runtime session-management dispatch is bound as a thin route to `reason.persistence`
- runtime rollback dispatch is bound as a thin route to `reason.persistence::rollback_latest_session_turn` plus UI effective transcript replacement
- runtime task query dispatch is bound as a thin read-only route to `task.orchestration`
- runtime error-center query dispatch is bound as a thin read-only route to `metadata.core` rows written by `error.center`
- runtime config status query dispatch is bound as a thin read-only route from selected `config.core` truth to `UiConfigStatusProjection`
- runtime provider/model update dispatch is bound as a thin mutation route into `config.core`; successful saves project restart-required pending status and active runtime config remains unchanged until restart
- runtime task list projection publication is bound as a thin route from task mutation to `ui.protocol`
- final live projection now keeps each runtime round as its own UI turn so earlier-round tool activity cannot be merged into the final latest turn
- failed live bridge tool execution now refreshes runtime UI state from persisted failed turn truth before returning the dispatch error, so WebUI query/SSE can observe failure instead of waiting forever
- migrated mainline-call source now lives at `docs/mainline-calls/runtime.ui-command-dispatch.json` and generated wiki lives at `docs/wiki/runtime.ui-command-dispatch.md`
