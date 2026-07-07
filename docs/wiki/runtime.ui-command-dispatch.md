# Wiki: `runtime.ui-command-dispatch`

Generated from `docs/mainline-calls/runtime.ui-command-dispatch.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.ui-command-dispatch.md`
- generated wiki: `docs/wiki/runtime.ui-command-dispatch.md`
- test design: `docs/testing/runtime.ui-command-dispatch.md`

## Request Mainline

- accepted UI command ingress arrives as a `UiCommandDispatchEnvelope`
- runtime bootstrap may first select one configured agent from `~/.freehand/config.toml`
- config-selected bootstrap consumes local node id, paired node id, paired allowed IP, and paired token env from `config.core`
- config-selected live bootstrap may also seed one shared metadata ledger path for node-owned bootstrap and pairing provenance
- live bootstrap may restore persisted session truth and prior turn projections before the next command runs
- runtime dispatch owner reads the declared owner target from the envelope
- session management commands route through runtime into `reason.persistence` session metadata and rollback APIs; runtime refreshes `UiProtocolState` from persistence-owned metadata/effective transcript projections after mutation
- runtime dispatch routes the command into reason, node, or checkpoint owner adapters without letting the app own those semantics
- runtime read-only task queries enter through `UiRuntimeQueryPort` and call task owner list/history APIs
- runtime read-only error-center queries enter through `UiRuntimeQueryPort` and read watermarked metadata rows through the runtime metadata projection owner
- provider/model update commands enter through a protocol dispatch envelope, route to config.core::update_provider_config_in_path, and must not duplicate config validation or persistence logic in runtime
- live submit registers an active turn cancel token before provider execution and releases the runtime mutex before running provider IO
- CancelLatestActiveTurn resolves to the newest active live turn before falling back to latest persisted runtime turn
- submit commands may carry selected cwd; runtime canonicalizes and binds cwd to the selected session
- successful task tool mutations publish task list projection events through runtime-owned UI protocol state

## Response Mainline

- reason-backed submit/cancel commands return dispatch receipts and update derived UI turn projections, including the original user prompt and explicit cancelled terminal status for public conversation projection
- live provider-backed submit publishes the user prompt into `UiProtocolState` before provider events, projects RuntimeLive02ProviderRequestBuilt as model-response waiting state, and incrementally writes reason/debug/tool-result projection updates while the turn is still running
- live provider-backed multi-round turns keep the original operator prompt in public UI projection instead of exposing internal continuation prompts
- final live multi-round UI projection preserves final-round visible text, usage, errors, and terminal status without aggregating tool-call/tool-result lifecycle activity from earlier runtime rounds
- live provider-backed submit refreshes UiProtocolState from authoritative persistence before returning dispatch failure when the live bridge materializes failed turn truth
- node-backed direct-message commands return dispatch receipts after owner validation
- runtime-backed rewind commands restore checkpointed workspace state without mutating reason/session/UI truth directly
- config-selected runtime bootstrap returns one dispatcher for the requested agent
- config-selected live bootstrap may materialize node-owned bootstrap and pairing metadata into the shared metadata ledger before the first command runs
- live bootstrap rehydrates `UiProtocolState` from persisted turn truth and resumes runtime turn-id allocation from persisted ordinals
- runtime-owned UI state reflects derived projections only, not authoritative turn truth
- active live cancel requests set the active cancel token immediately and publish a cancelled UI projection without waiting for provider completion
- latest-active cancellation supports Esc during the short window before WebUI has received a concrete turn_id
- selected session cwd is persisted on turn records, projected to UiProtocolState, and restored for later same-session inheritance
- session metadata mutations return receipts only after the persistence owner accepts the create/rename/archive/restore/delete-as-archive operation and protocol projection is refreshed
- session rollback mutations return receipts only after the persistence owner writes an append-only rollback marker and runtime replaces the selected session transcript with effective turn projections
- runtime task list/history queries return UI-safe projections built from task owner snapshots and ledger events
- runtime error-center queries return UI-safe projections built from watermarked metadata rows and omit raw error/request/provider text
- runtime task list subscription updates reuse the same UI-safe projection helper as task list queries
- successful provider/model updates persist through the canonical config owner, store a pending restart-required UI-safe projection, and leave active runtime/live provider config unchanged until daemon restart

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
- CancelLatestActiveTurn with no active or persisted turn returns explicit target-not-found
- unknown session metadata mutation targets return explicit target-not-found failures
- rollback with no eligible target or with an active turn returns explicit dispatch failure; runtime must not delete UI turns locally to pretend success
- missing task history targets return explicit target-not-found and invalid task filters return dispatch failures
- invalid error-center query filters or metadata read failures return explicit dispatch failures
- task list publication failures after task mutation are explicit live bridge failures
- provider/model update without a live runtime home or with invalid config owner input returns an explicit dispatch failure; failed updates must not overwrite config or fake hot reload

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
- `project_config_status_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: build UI-safe active or pending config projection from config.core selected agent truth
  - allowed callers: RuntimeCommandDispatcher::query_runtime, RuntimeCommandDispatcher::dispatch_update_provider_config
  - related tests: runtime_query_projects_config_status_without_secrets, runtime_dispatch_updates_provider_config_without_hot_reloading_active_model
  - why shared: keeps config-to-UI projection in runtime owner while app transports stay protocol-only

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `RuntimeCommandDispatcher::new` | `crates/freehand-runtime/src/lib.rs` | compose first runtime owner wiring for reason/node command dispatch and live node metadata bootstrap | runtime config | runtime dispatcher | runtime bootstrap/tests | runtime owner | bound |
| 02 | `RuntimeCommandDispatcher::from_selected_agent` | `crates/freehand-runtime/src/lib.rs` | derive runtime bootstrap config from one selected agent config | selected agent config | runtime dispatcher | daemon/bootstrap tests | runtime bootstrap | bound |
| 03 | `RuntimeCommandDispatcher::from_default_config` | `crates/freehand-runtime/src/lib.rs` | load default config and bootstrap one runtime dispatcher | agent name | runtime dispatcher | daemon host | config owner plus runtime bootstrap | bound |
| 04 | `RuntimeCommandDispatcher::dispatch` | `crates/freehand-runtime/src/lib.rs` | execute protocol-owned dispatch envelope through the correct owner adapter | dispatch envelope | dispatch receipt or failure | app/daemon runtime boundary | reason/node owner adapter | bound |
| 05 | `RuntimeCommandDispatcher::ui_state` | `crates/freehand-runtime/src/lib.rs` | expose derived UI projection state for runtime-side consumers/tests | runtime dispatcher | shared derived UI state | runtime tests/future daemon | UI protocol state | bound |
| 06 | `run_live_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute a live provider turn while streaming reason/debug/tool-result callbacks to runtime-owned consumers | selected live config plus live request plus callbacks | live turn outcome plus incremental callbacks | runtime dispatch/tests | live bridge owner | bound |
| 07 | `RuntimeCommandDispatcher::prepare_live_submit_user_input` | `crates/freehand-runtime/src/lib.rs` | register active live turn cancellation state before provider execution | runtime state plus submitted user text | prepared live submit plus active cancel token | RuntimeCommandDispatcher::dispatch | runtime owner | bound |
| 08 | `RuntimeCommandDispatcher::dispatch_prepared_live_submit` | `crates/freehand-runtime/src/lib.rs` | run provider-backed live turn outside runtime mutex while honoring active cancel token | prepared live submit | live receipt or cancelled dispatch failure | RuntimeCommandDispatcher::dispatch | run_live_reason_turn_with_hooks | bound |
| 09 | `RuntimeCommandDispatcher::dispatch_cancel_turn` | `crates/freehand-runtime/src/lib.rs` | cancel active or persisted turns through reason-owned terminal semantics and UI projection | cancel command turn id | cancel receipt plus cancelled projection | RuntimeCommandDispatcher::dispatch | reason owner / active cancel registry | bound |
| 10 | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | execute runtime-backed read-only task list/history queries | UI query command | optional UI query result or dispatch failure | ADP query transport | task runtime owner | bound |
| 11 | `project_task_list_for_ui / project_task_history_for_ui` | `crates/freehand-runtime/src/lib.rs` | project task owner snapshots and ledger events into UI-safe DTOs | task snapshots or task ledger events | task query projection | RuntimeCommandDispatcher::query_runtime | UI protocol DTO | bound |
| 12 | `task_list_projection_from_runtime` | `crates/freehand-runtime/src/lib.rs` | build and publish task list projection from task runtime after successful task mutation | runtime home plus task filters | UI task list projection | run_live_reason_turn_with_hooks / RuntimeCommandDispatcher::query_runtime | TaskRuntime::list_tasks | bound |
| 13 | `query_error_center_events_for_ui / project_error_center_event_for_ui` | `crates/freehand-runtime/src/lib.rs` | read watermarked error-center metadata and project UI-safe event DTOs | QueryErrorCenterEvents filters | ErrorCenterEvents query projection | RuntimeCommandDispatcher::query_runtime | metadata.core ledger plus ui.protocol DTO | bound |
| 14 | `RuntimeCommandDispatcher::dispatch_session_management` | `crates/freehand-runtime/src/lib.rs` | route protocol-owned session CRUD and rollback commands into reason persistence APIs and refresh shared UI projection | session CRUD or rollback dispatch envelope | dispatch receipt or explicit target-not-found/failure | RuntimeCommandDispatcher::dispatch | ReasonPersistence session metadata/rollback owner | bound |
| 15 | `UiProtocolState::replace_session_turn_projections` | `crates/freehand-ui-protocol/src/lib.rs` | replace one session transcript with persistence-owned effective projections after rollback | session id plus effective turn projections | queryable transcript excluding rolled-back logical turns | RuntimeCommandDispatcher::dispatch_session_management | ui.protocol state | bound |
| 16 | `RuntimeCommandDispatcher::dispatch_update_provider_config` | `crates/freehand-runtime/src/lib.rs` | route provider/model update dispatch into the config owner and store pending restart-required UI projection without hot-reloading active runtime | UiProviderConfigUpdate dispatch envelope | dispatch receipt or explicit dispatch failure | RuntimeCommandDispatcher::dispatch | update_provider_config_in_path | bound |
| 17 | `update_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist provider/model update through canonical config owner | runtime config path plus provider update | selected agent config projection from saved TOML | RuntimeCommandDispatcher::dispatch_update_provider_config | config.core persistence | bound |

## Sync Status Against Mainline Call

- runtime dispatch owner baseline is now bound in code
- provider-backed submit input and cancel dispatch through `reason.turn` and update derived UI turn projections
- live provider submit now streams reason/debug updates into `UiProtocolState` before final receipt is returned
- live provider submit now projects provider-request-built debug events into UiProtocolState.model_request before model response arrives
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
- final live projection now keeps each runtime round as its own UI turn so earlier-round tool activity cannot be merged into the final latest turn
- generated wiki must be regenerated from `docs/mainline-calls/runtime.ui-command-dispatch.json` when this function-map truth changes
- live submit now releases the runtime mutex before provider IO so CancelTurn can enter concurrently
- active live cancel now publishes cancelled UI projection immediately and later provider success cannot overwrite it
- runtime dispatch now supports CancelLatestActiveTurn for current-turn stop without requiring the UI to know turn_id
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and terminal persistence
- missing CancelTurn, empty CancelLatestActiveTurn, and wrong-node direct-message dispatch paths now stay explicit target-not-found failures
- runtime task query bridge routes list/history through task.orchestration and is covered by runtime and daemon ADP tests
- runtime error-center query bridge routes metadata rows through a UI-safe projection and is covered by runtime and daemon ADP tests
- runtime provider/model update dispatch is bound as a thin mutation route into config.core; successful saves project restart-required pending status and active runtime config remains unchanged until restart
