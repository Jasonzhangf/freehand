# Wiki: `ui.protocol`

Generated from `docs/mainline-calls/ui.protocol.json`. Do not edit by hand.

- owner crate: `crates/freehand-ui-protocol`
- owner module: `crates/freehand-ui-protocol/src/lib.rs`
- function map: `docs/function-maps/ui.protocol.md`
- generated wiki: `docs/wiki/ui.protocol.md`
- test design: `docs/testing/ui.protocol.md`

## Resource Operation Backlinks

- task.project_to_ui
- input_attachment.validate_submit_metadata

## Request Mainline

- UI commands enter one protocol truth shared by CLI and WebUI
- UI acts as an input ingress only; command submission does not make UI a truth writer
- command ingress acceptance is explicit and route-scoped: only mutation-intent commands may enter the ingress transport path
- accepted command ingress is wrapped into a dispatch envelope that declares the target owner feature/module before leaving the protocol boundary
- runtime-owned mutation commands such as checkpoint rewind stay explicit at the protocol envelope layer and do not become UI-owned semantics
- query and subscribe stay separate
- ADP WebSocket clients use protocol-owned typed frames for command, query, and subscribe requests instead of app-local JSON envelopes
- task list/history query commands are protocol-owned read-only ADP/query shapes while task truth remains runtime/task-owner supplied
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle queries are protocol-owned ADP/query command shapes while runtime/task owners supply truth
- task mutation commands (`CreateTask`, `CreateTaskAgent`, `AssignTask`, `ClaimNextTask`, `SubmitTaskReview`, `RejectTaskReview`, `ApproveTaskReview`, `CloseTask`) are protocol-owned mutation intents that validate required fields and route to task.orchestration through runtime; protocol does not write task truth
- `UiTaskDispatchCommand` lets a task create command explicitly choose self dispatch, agent dispatch, or no immediate dispatch so Phase 2A can create a waiting task before assignment
- Phase 1 ApplyExecutionFact and RunSchedulerTick are protocol-owned mutation intents routed to task.orchestration through runtime; protocol does not update task state or make scheduler decisions
- Phase 2B QueryEventInbox and RunMasterPoll are protocol-owned query/mutation-intent shapes routed to task.orchestration; protocol does not store event cursor truth, classify board state, or apply master business actions
- Phase 2C WorkerControl and QueryWorkerControl are protocol-owned command/query DTOs for already-running worker execution control; protocol validates ids, op names, and op-specific payloads before runtime dispatch
- Timer dashboard QueryTimerList, ScheduleTimer, and CancelTimer are protocol-owned query/mutation DTOs for independent timer truth; protocol validates timer ids, schedule mode, delay/run-at/repeat fields, reason, prompt, and source session id before runtime dispatch
- RunMasterPoll.replay_from_start is a protocol-owned cursor-mode field that defaults false for older clients and is rejected when combined with after_cursor
- error-center query/subscribe commands are protocol-owned read-only ADP/query shapes while metadata/error truth remains runtime/error-center supplied
- config status query is a protocol-owned read-only ADP/query shape carrying the complete safe provider registry plus current primary/fallback selection while config.core and runtime.ui-command-dispatch supply selected-agent truth
- provider definition upsert is a protocol-owned mutation command shape that carries only editable provider id/type/protocol/base URL/default model/env-var auth fields and routes to config.core without switching active selection
- provider selection is a protocol-owned mutation command shape that carries only agent name plus primary/fallback provider ids and routes to config.core without rewriting provider definitions
- provider web_search live test is a protocol-owned command shape that carries provider id plus optional query and routes to provider.reason-live-bridge without adding a local Freehand web_search tool
- legacy provider/model update remains a protocol-owned mutation command shape for existing CLI callers and carries no raw credential values
- Agent resource-count update is a protocol-owned mutation command shape that carries only agent name plus resource_count, rejects values outside `1..=5`, and routes valid intent to config.core
- subscriptions may target latest active turn, specific turn, specific turn debug state, or node/progress streams
- CancelLatestActiveTurn is a mutation-intent command for stopping the current active turn when a UI has not yet received a concrete turn_id
- SubmitUserInput may carry selected session_id and cwd; empty cwd is rejected by protocol validation
- SubmitUserInput may carry neutral metadata.attachments image payloads; protocol validates kind, id, filename, media type, and current-submit base64 payload before dispatch without embedding images in user text
- session management commands (`CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`, `RollbackLatestSessionTurn`) are mutation intents only; protocol validates and routes them while `reason.persistence` owns durable metadata and rollback truth
- task list subscribe commands are protocol-owned ADP/subscribe shapes while task truth remains runtime/task-owner supplied
- error-center subscribe commands are protocol-owned ADP/subscribe shapes while error truth remains runtime/error-center supplied

## Response Mainline

- query returns snapshots
- checkpoint query returns read-only checkpoint summary projections supplied by runtime owner code
- command ingress returns explicit dispatch receipt without claiming truth mutation success
- subscribe returns an initial snapshot followed by continuous incremental projections through a protocol-owned subscription channel, or waits for the first turn when the latest-turn stream is subscribed before any turn exists
- ADP subscribe returns an explicit SubscriptionAccepted frame before later SubscriptionEvent frames so UI-less clients can distinguish waiting from transport failure
- projections are read-only views over owner-written truth
- model request lifecycle is projected as UiModelRequestActivity inside UiTurnProjection for ordinary thinking, schema retry, and tool-result continuation; provider retry/failover are transport substate on the same activity, not separate reasoning-flow phases, and the activity clears when response/tool/usage/terminal/error projection arrives
- terminal completion shows only final projected text
- public conversation projection preserves the user prompt while stripping raw completion schema blocks and excluding reasoning, usage, provider payload, debug details, and verbose tool term text from the main user-visible stream
- debug state is projected as a read-only per-turn snapshot/stream with summary text plus ordered detail lines
- `ui.protocol` may ingest observation-only debug events from `debug.core` receivers and materialize only the snapshot projection into protocol state
- `ui.protocol` may ingest shared semantic/tool/usage/terminal/error contracts incrementally and update one turn projection without depending on `freehand-reason`
- tool-call lifecycle is projected as UiToolActivity inside UiTurnProjection: ReasonReq04ToolCall upserts a waiting activity, matching ReasonReq05ToolResultReentry marks the same activity completed or failed, and failed terminal truth marks still-waiting activities failed
- slave turn may surface as WebUI-only separate card while staying in one protocol truth
- client-specific projection gating stays inside the protocol owner, not in apps
- UI must be able to consume reason-turn state and debug-state projections without owning either truth source
- transport adapters may drain debug receivers and query protocol snapshots, but projection ownership stays in `freehand-ui-protocol`
- turn projections preserve terminal status separately from terminal text so UI clients can distinguish success, failed, blocked, interrupted, running, and cancelled terminal states
- turn projections carry metadata-only `attachments` rows for uploaded images so UI clients can show what was submitted after refresh without raw/base64 image payload
- public conversation terminal items derive status strings from terminal status instead of treating every terminal text as completed
- public conversation tool summaries carry tool_call_id so UI clients can update one tool card instead of rendering duplicate waiting/completed cards, and completed/failed public tool bodies expose protocol-projected tool result detail even when structured display fields are present
- cancel commands route to reason.turn whether they target an explicit turn_id or the latest active turn
- session list and transcript projections expose cwd bound by runtime/session truth
- session list projections expose only owner-supplied persisted session metadata as top-level active/archived sessions; internal framework sessions such as `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` remain directly transcript-queryable but are hidden from global session lists
- UiSessionSummary.active_turn_id is a live/progress identity only: it points only to the latest same-session turn when that turn is nonterminal; terminal latest turns remain visible as terminal summaries but do not appear active, and later nonterminal model/tool activity can become active again
- TaskBoard projections carry parent_session_id, observing attached_session_ids, canonical worker_session_id, and task-owner created_at so WebUI can scope current Worker work, correlate submit receipt truth, and open Worker transcripts without synthesizing ids or making workers top-level sessions
- task list/history query results use protocol-owned UI-safe DTOs supplied through `UiRuntimeQueryPort`
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle query results use protocol-owned UI-safe DTOs supplied through `UiRuntimeQueryPort`
- session list and transcript projections expose owner-supplied session title, archived state, cwd, and effective transcript projections after rollback
- rollback command ingress exposes append-only latest-turn rollback as a reason.persistence mutation intent; protocol does not remove turns or mutate local transcript truth
- error-center query results use protocol-owned UI-safe DTOs supplied through `UiRuntimeQueryPort`
- task list subscription events carry UI-safe task list projections published by runtime owner code
- config status query results expose the active agent/provider/model/auth-source fields, ordered paired agent name/mode/node/provider list, resource count, resource limit, shared provider id, and provider web_search configured/effective route diagnostics without pair tokens or provider credential values
- provider/model update command receipts report owner dispatch status only; restart-required and saved provider/model state are observed by follow-up config status projection
- provider web_search live-test command receipts report owner dispatch status only; provider success and exact provider rejection remain visible receipt text supplied by runtime/provider owners
- Agent resource-count update command receipts report owner dispatch status only; restart-required and saved topology state is observed by follow-up config status projection
- Phase 2B EventInbox and MasterPoll results expose UI-safe event rows, cursor values, compact classifications, and recommended semantic action labels supplied by runtime owner code through UiRuntimeQueryPort
- WorkerControl query results expose UI-safe persisted control events plus optional task/agent/lifecycle/task-event projections supplied by runtime owner code
- TimerList query results expose UI-safe timer schedule and ledger event projections supplied by runtime owner code

## Error Mainline

- invalid command, invalid stream selection, or unavailable source projection return explicit protocol errors
- query/subscribe commands sent to command-ingress route are explicit protocol misuse errors
- query commands sent as ADP command frames are explicit protocol misuse errors, not mutation attempts
- empty checkpoint rewind ids are rejected at the protocol boundary before runtime dispatch
- checkpoint query misses return an empty read-only snapshot, not an implicit recovery or filesystem fallback
- blank latest-turn subscribe does not fail early; it keeps waiting for the first matching turn
- source identity fields remain explicit across success and error paths
- UI-side commands may request mutations, but mutation success/failure is decided by owner modules and reflected back as projections or errors
- cancelled terminal projection must stay explicit and must not be collapsed into failed or completed UI status
- CancelLatestActiveTurn without any active or persisted turn returns explicit target-not-found from the owner module
- empty SubmitUserInput.cwd returns empty_session_cwd instead of falling back silently
- SubmitUserInput metadata attachments with missing id, filename, media type, base64 data, or unsupported kind return explicit attachment protocol rejections before runtime dispatch
- empty session ids and empty session titles are rejected at the protocol boundary for session management commands, including rollback
- empty task history ids return empty_task_id and task query commands sent as command ingress are rejected as query-route misuse
- task mutation commands reject empty task id/title/content/goal/review summary, empty worker agent id/capabilities, empty claim execution id, and empty review rejection fields before runtime dispatch instead of silently creating partial task truth
- Phase 1 execution fact commands reject empty ids and malformed facts before runtime dispatch; scheduler tick commands reject invalid threshold shape before runtime dispatch
- Phase 2B EventInbox and MasterPoll commands reject empty cursor strings before runtime dispatch and must not treat unknown cursors as empty results
- WorkerControl commands reject empty ids, unknown ops, missing ask_at_safe_point.question, missing add_constraint.constraint, and query-route misuse before runtime dispatch
- Timer commands reject empty timer ids, unknown schedule modes, missing relative delay, missing absolute run-at time, missing recurring repeat rule, invalid weekdays, invalid local seconds of day, invalid cron expression, empty reason, empty prompt, and query-route misuse before runtime dispatch
- RunMasterPoll rejects replay_from_start=true combined with after_cursor before dispatch
- empty error-center session ids return empty_session_id and error-center query commands sent as command ingress are rejected as query-route misuse
- task history remains query-only; task list and error-center subscribe reject non-subscription misuse through protocol stream matching
- provider/model update commands reject empty agent/provider/type/protocol/base URL/model/env-var fields and unsupported protocol values before dispatch; credential/API-key value fields do not exist in the DTO
- Agent resource-count update commands reject empty agent names and counts outside `1..=5` before dispatch; provider credentials and live process state do not exist in the DTO

## Shared Multi-Reference Functions

- `terminal_text_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: collapse terminal event to final user-visible text
  - allowed callers: query handlers, stream handlers, CLI/WebUI adapters
  - related tests: terminal result projection smoke
  - why shared: ensures CLI and WebUI project the same terminal text truth
- `public_conversation_items`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: derive user-visible conversation items from a full turn projection without exposing internal reasoning/debug/raw schema data while retaining the user prompt and tool_call_id identity
  - allowed callers: CLI/WebUI renderers, transport adapters
  - related tests: public conversation projection smoke
  - why shared: all UI clients need one public projection rule instead of per-client filtering
- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: gate slave substream visibility by UI client kind without changing turn truth
  - allowed callers: CLI/WebUI adapters, query handlers
  - related tests: slave turn subscription smoke
  - why shared: keeps client-specific projection rules centralized and protocol-owned
- `DebugStateSnapshot::new`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: construct reusable debug snapshots consumed by UI protocol without making UI the debug owner
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: debug state query/subscription smoke
  - why shared: keeps debug projection shape in `debug.core` instead of duplicating it inside UI protocol
- `accept_command_ingress`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: validate mutation-intent command ingress and return explicit acknowledgement without mutating truth
  - allowed callers: CLI/WebUI transport adapters
  - related tests: command ingress acceptance/rejection smoke
  - why shared: keeps ingress-route semantics inside the protocol owner instead of duplicating them in apps
- `build_command_dispatch_envelope`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: route accepted ingress command to the declared owner feature/module before dispatch
  - allowed callers: CLI/WebUI transport adapters, runtime owner adapters
  - related tests: command dispatch envelope owner-routing smoke
  - why shared: keeps command-to-owner routing out of app transport glue
- `checkpoint_projection_from_runtime_summary`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: convert runtime-owned checkpoint summaries into UI-safe read-only projection rows
  - allowed callers: runtime dispatcher bridge, app query handlers through protocol state
  - related tests: checkpoint summary query smoke
  - why shared: keeps checkpoint UI projection single-sourced without letting UI parse runtime manifests
- `UiAdpRequest`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: define protocol-owned WebSocket ADP frames for command, query, subscribe, event, and failure automation
  - allowed callers: WebUI transport adapters, Android transport adapters, CLI automation transports
  - related tests: ADP frame roundtrip smoke, daemon ADP command/query/subscribe smoke
  - why shared: all UI and headless clients need one typed control/status frame shape instead of per-client transport envelopes
- `UiRuntimeQueryPort::query_runtime`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: define a protocol-owned runtime query extension point for read-only owner-backed projections
  - allowed callers: WebUI ADP query transport, daemon ADP query transport
  - related tests: daemon_adp_queries_runtime_task_truth
  - why shared: keeps app transports protocol-only while allowing runtime owner read models
- `UiProtocolState::publish_task_list_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: publish runtime-supplied task list projection through the protocol subscription channel without owning task truth
  - allowed callers: runtime.ui-command-dispatch
  - related tests: task_list_subscription_matches_runtime_projection_only, daemon_adp_subscribes_runtime_task_truth
  - why shared: keeps ADP task push on the same protocol subscription bus as turn/debug updates
- `UiErrorCenterEventProjection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: define UI-safe error-center event DTOs that expose watermarked fields and raw hashes without raw provider/tool/request text
  - allowed callers: runtime.ui-command-dispatch, ADP transports, CLI automation
  - related tests: error_center_query_requires_session_id, error_center_subscription_matches_projection
  - why shared: keeps error-center read projection shape protocol-owned and transport-neutral
- `UiProtocolState::replace_session_turn_projections`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: replace one session transcript projection after persistence-owned rollback or selected-session refresh without making the UI a truth writer, erasing current latest-turn live activity, or preserving stale live activity on historical rounds
  - allowed callers: runtime.ui-command-dispatch
  - related tests: session_transcript_replacement_updates_query_projection, runtime_dispatches_session_rollback_into_effective_ui_projection, session_refresh_preserves_active_model_request_activity, session_refresh_preserves_active_tool_activity_cards, terminal_session_refresh_drops_stale_live_activity, session_list_active_turn_id_tracks_only_nonterminal_turns
  - why shared: runtime must refresh effective transcript projection centrally after rollback instead of letting each UI delete DOM rows locally
- `UiTaskEventInboxProjection / UiMasterPollProjection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: define UI-safe Phase 2B event inbox and master poll read DTOs without owning task event or cursor truth
  - allowed callers: runtime.ui-command-dispatch, ADP transports, CLI automation
  - related tests: phase2b_event_inbox_and_master_poll_validate_and_route_to_task_orchestration
  - why shared: keeps EventInbox and MasterPoll projection shape transport-neutral while Task Center remains the truth owner
- `UiWorkerControlCommand / UiWorkerControlProjection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: define transport-neutral Phase 2C worker-control command/query DTOs without owning task or control truth
  - allowed callers: runtime.ui-command-dispatch, ADP transports, CLI automation
  - related tests: worker_control_command_validates_and_routes_to_worker_control, worker_control_command_rejects_missing_fields, worker_control_adp_roundtrip_carries_projection
  - why shared: WebUI, Android, CLI, and headless tests need one worker-control frame shape while worker.control owns semantics

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | accept and validate UI command payload | UI command | validated command | CLI/WebUI | protocol boundary |  |  |  | bound |
| 01a | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | validate SubmitUserInput metadata image attachments before runtime dispatch | SubmitUserInput.metadata.attachments | validated image attachment metadata and transient base64 payload or explicit protocol rejection | CLI/WebUI/Android transports | protocol boundary | input_attachment | input_attachment | input_attachment.validate_submit_metadata | bound |
| 02 | `accept_command_ingress` | `crates/freehand-ui-protocol/src/lib.rs` | accept only mutation-intent ingress commands and return explicit ack | UI command | ingress ack | CLI/WebUI transport adapters | protocol boundary |  |  |  | bound |
| 03 | `protocol_rejection` | `crates/freehand-ui-protocol/src/lib.rs` | convert protocol error into transport-safe rejection payload | protocol error | rejection payload | CLI/WebUI transport adapters | protocol boundary |  |  |  | bound |
| 04 | `build_command_dispatch_envelope` | `crates/freehand-ui-protocol/src/lib.rs` | wrap accepted ingress command with declared owner routing | UI command | dispatch envelope | CLI/WebUI transport adapters | protocol boundary |  |  |  | bound |
| 04a | `validate_command / command_dispatch_target` | `crates/freehand-ui-protocol/src/lib.rs` | validate session-management mutation intents and route them to the session persistence owner | session CRUD or rollback command | owner-routed dispatch envelope or protocol rejection | CLI/WebUI/ADP transports | protocol boundary |  |  |  | bound |
| 04b | `UiProtocolState::replace_session_turn_projections / preserve_live_activity_on_nonterminal_refresh / merge_tool_activity` | `crates/freehand-ui-protocol/src/lib.rs` | replace one session's effective transcript projection after persistence-owned rollback or selected-session refresh while preserving live provider-transport/tool activity only for the latest nonterminal replacement turn and keeping terminal snapshots authoritative | session id plus effective turn projections | queryable session transcript excluding rolled-back turns, retaining current latest provider transport retry/tool-call observability until terminal truth, and clearing stale live activity on historical rounds | runtime.ui-command-dispatch | protocol state |  |  |  | bound |
| 04c | `session_list_projection / turn_is_nonterminal` | `crates/freehand-ui-protocol/src/lib.rs` | project persisted session summaries while allowing active turn identity only for the latest nonterminal live/progress turn | persisted session metadata plus turn projections plus latest protocol active turn id | session summary list where terminal latest turns remain visible but do not appear active | UiProtocolState::query | protocol state |  |  |  | bound |
| 05 | `UiProtocolState::query` | `crates/freehand-ui-protocol/src/lib.rs` | execute read-only query path | query command | snapshot projection | protocol boundary | query handler |  |  |  | bound |
| 06 | `UiProtocolState::subscribe` | `crates/freehand-ui-protocol/src/lib.rs` | expose the protocol-owned continuous subscription channel for app transports | none | UiSubscriptionEvent receiver | app/transport adapters | protocol state |  |  |  | bound |
| 07 | `subscription_selector` | `crates/freehand-ui-protocol/src/lib.rs` | build read-only subscribe selector | subscribe command | subscription selector | protocol boundary | stream handler |  |  |  | bound |
| 08 | `subscription_matches` | `crates/freehand-ui-protocol/src/lib.rs` | route incremental projection to matching subscription | subscription selector plus projection | delivery decision | stream handler | selector matcher |  |  |  | bound |
| 09 | `turn_projection_from_events` | `crates/freehand-ui-protocol/src/lib.rs` | project whole-turn state into UI snapshot, including tool lifecycle activities | semantic/tool/tool-result/usage/terminal/error/user inputs | UI turn projection | query/stream handler | projector |  |  |  | bound |
| 10 | `terminal_text_projection` | `crates/freehand-ui-protocol/src/lib.rs` | project terminal text | terminal semantic payload | UI terminal text | query/stream handler | projector |  |  |  | bound |
| 10a | `public_conversation_items / public_turn_projection` | `crates/freehand-ui-protocol/src/lib.rs` | derive public user-visible conversation stream, preserve user prompt, and strip raw completion schema | full turn projection | public turn projection | app transports/renderers | projector |  |  |  | bound |
| 11 | `UiProtocolState::apply_semantic_event / apply_tool_call / apply_tool_result / apply_usage_event / apply_terminal_event / apply_error_event` | `crates/freehand-ui-protocol/src/lib.rs` | incrementally update one turn projection from shared contract events and publish subscription updates | shared reason/error contracts | updated queryable/subscribable turn projection | runtime/debug bridges | protocol state |  |  |  | bound |
| 12 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate client-specific slave substream visibility | turn projection plus client kind | client-specific turn projection | CLI/WebUI adapter | projector |  |  |  | bound |
| 13 | `UiProtocolState::set_debug_state` | `crates/freehand-ui-protocol/src/lib.rs` | store per-turn read-only debug projection for UI consumption and publish subscription updates | freehand-debug snapshot | queryable/subscribable debug state | reason/node/debug bridge | protocol state |  |  |  | bound |
| 14 | `UiProtocolState::apply_debug_event` | `crates/freehand-ui-protocol/src/lib.rs` | ingest one observation-only debug event into UI protocol state when a snapshot is present | freehand-debug event | updated per-turn debug state or ignored event | reason/node/debug bridge | protocol state |  |  |  | bound |
| 15 | `UiProtocolState::drain_debug_receiver` | `crates/freehand-ui-protocol/src/lib.rs` | drain a debug.core receiver without making UI a truth writer | debug receiver | applied snapshot count | protocol transport/app adapters | protocol state |  |  |  | bound |
| 16 | `debug_projection_from_event` | `crates/freehand-ui-protocol/src/lib.rs` | map one debug event to read-only UI debug projection when snapshot exists | freehand-debug event | UiProjection::Debug | protocol tests/transport adapters | projector |  |  |  | bound |
| 17 | `UiProtocolState::set_checkpoint_snapshot / checkpoint_projection_from_runtime_summary` | `crates/freehand-ui-protocol/src/lib.rs` | store and query read-only checkpoint summaries supplied by runtime owner | runtime checkpoint summary DTO | checkpoint query result | runtime dispatcher / app query handlers | protocol state |  |  |  | bound |
| 18 | `UiAdpRequest` | `crates/freehand-ui-protocol/src/lib.rs` | define protocol-owned WebSocket ADP request frames for command/query/subscribe automation | ADP JSON frame | typed command/query/subscription request | WebUI/Android/CLI automation transports | protocol owner |  |  |  | bound |
| 19 | `UiAdpResponse` | `crates/freehand-ui-protocol/src/lib.rs` | define protocol-owned WebSocket ADP response frames for command/query/subscribe/event/failure automation | protocol command/query/subscription result | ADP JSON response frame | protocol owner | WebUI/Android/CLI automation transports |  |  |  | bound |
| 20 | `UiRuntimeQueryPort::query_runtime` | `crates/freehand-ui-protocol/src/lib.rs` | define runtime-backed read-only query port shape | UI query command | optional UI query result or dispatch failure | WebUI/daemon ADP query transport | runtime owner query implementation |  |  |  | bound |
| 20a | `UiCommand::QueryConfigStatus / UiQueryResult::ConfigStatus / UiConfigStatusProjection / UiConfigPeerProjection / UiProviderConfigSummaryProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define the secret-free selected config query/result DTO with complete provider registry, current primary/fallback provider selection, ordered peer list, and Agent resource capacity | config status query | active agent/provider/fallback/model/auth-source plus safe configured provider registry, ordered paired agent name/mode/node/provider projections, and Agent resource capacity | ADP query transport | runtime owner query implementation |  |  |  | bound |
| 10b | `UiProtocolState::apply_terminal_event / turn_projection_from_events` | `crates/freehand-ui-protocol/src/lib.rs` | preserve terminal status alongside terminal text in UI turn projections | terminal semantic event | terminal text plus terminal status projection | runtime/app protocol consumers | protocol projector |  |  |  | bound |
| 21 | `UiProtocolState::publish_task_list_projection` | `crates/freehand-ui-protocol/src/lib.rs` | publish runtime-supplied task list projection to subscribers | UI task list projection | UI subscription event | runtime.ui-command-dispatch | protocol subscription channel |  |  |  | bound |
| 22 | `subscription_selector / subscription_matches` | `crates/freehand-ui-protocol/src/lib.rs` | route task list subscription selectors to task list projection events | SubscribeTaskList command plus UI projection | subscription delivery decision | ADP/SSE subscription transport | protocol selector matcher |  |  |  | bound |
| 23 | `UiCommand::QueryErrorCenterEvents / UiCommand::SubscribeErrorCenterEvents / UiErrorCenterEventProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define read-only error-center ADP query/subscribe shapes and UI-safe event DTOs | session/trace/turn/domain filters | ErrorCenterEvents query result or subscription projection | ADP/CLI/WebUI transports | runtime query port / protocol selector matcher |  |  |  | bound |
| 24 | `UiProviderConfigUpdate / UiCommand::UpdateProviderConfig / UiCommand::UpsertProviderConfig` | `crates/freehand-ui-protocol/src/lib.rs` | define provider definition mutation DTOs without credential values and route them to the config owner | provider/model/base-url/env-var update | validated mutation intent routed to config.core | WebUI/CLI ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 24b | `UiAgentProviderSelectionUpdate / UiCommand::UpdateAgentProviderSelection` | `crates/freehand-ui-protocol/src/lib.rs` | define active provider selection mutation DTO without credential values and route it to the config owner | agent name plus primary/fallback provider ids | validated mutation intent routed to config.core | WebUI/CLI ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 24b2 | `UiCommand::TestProviderWebSearch` | `crates/freehand-ui-protocol/src/lib.rs` | define provider-hosted web_search live-test command DTO and route it to the runtime/provider bridge owner | provider id plus optional query | validated mutation intent routed to provider.reason-live-bridge | WebUI/CLI ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 24c | `UiAgentResourceConfigUpdate / UiCommand::UpdateAgentResourceConfig` | `crates/freehand-ui-protocol/src/lib.rs` | define Agent resource-count update command DTO without provider credentials and route it to the config owner | agent name plus `1..=5` resource count | validated mutation intent routed to config.core | WebUI/CLI ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 25 | `UiCommand::CreateTask / UiCommand::SubmitTaskReview / UiCommand::ApproveTaskReview / UiCommand::CloseTask` | `crates/freehand-ui-protocol/src/lib.rs` | define task mutation command DTOs and validate required fields before runtime dispatch | task create/review/approve/close mutation intent | validated mutation intent routed to task.orchestration or protocol rejection | WebUI/CLI ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 26 | `UiCommand::QueryTaskBoard / UiCommand::QueryAgentBoard / UiCommand::QueryAgentLifecycle` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 1 board and lifecycle query DTOs without owning task/lifecycle truth | board or lifecycle query filters | runtime-backed UI-safe board/lifecycle projections | ADP query transport | runtime query port | task | ui_projection | task.project_to_ui | bound |
| 27 | `UiCommand::ApplyExecutionFact / UiCommand::RunSchedulerTick` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 1 execution fact and scheduler tick mutation DTOs routed to task.orchestration | execution fact or scheduler tick command | validated mutation intent routed to task.orchestration | ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 28 | `UiTaskAgentCreateCommand / UiTaskAssignCommand / UiTaskClaimCommand / UiTaskReviewRejectionCommand / UiTaskDispatchCommand` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2A worker agent, assignment, claim, review rejection, and dispatch-mode DTOs routed to task.orchestration | worker/task mutation intent | validated mutation intent routed to task.orchestration or protocol rejection | ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 29 | `UiCommand::QueryEventInbox / UiQueryResult::EventInbox` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2B EventInbox query DTOs without owning task event truth or cursor truth | event inbox query cursor | runtime-backed UI-safe event inbox projection | ADP query transport | runtime.ui-command-dispatch |  |  |  | bound |
| 30 | `UiCommand::RunMasterPoll / UiQueryResult::MasterPoll` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2B MasterPoll command/result DTOs routed to task.orchestration, including replay_from_start cursor-mode validation | master poll command cursor | validated mutation intent routed to task.orchestration and owner-supplied poll projection | ADP command transport | runtime.ui-command-dispatch |  |  |  | bound |
| 28 | `UiWorkerControlCommand / UiCommand::WorkerControl / UiCommand::QueryWorkerControl / UiQueryResult::WorkerControl` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2C worker-control command/query DTOs, validate op-specific fields, and route mutation intent to worker.control | worker-control command or event query | validated mutation intent or runtime-backed control event projection | ADP command/query transport | runtime.ui-command-dispatch |  |  |  | bound |
| 31 | `UiTimerScheduleCommand / UiTimerRepeatCommand / UiCommand::QueryTimerList / UiCommand::ScheduleTimer / UiCommand::CancelTimer / UiQueryResult::TimerList / validate_timer_schedule_command` | `crates/freehand-ui-protocol/src/lib.rs` | define independent timer dashboard query/command DTOs, validate mode-specific timer schedule fields, and route mutation intent to runtime.master-worker-loop through runtime dispatch | timer list query, timer schedule command, or timer cancel command | validated timer query/result DTO or mutation intent routed to timer owner | ADP command/query transport | runtime.ui-command-dispatch |  |  |  | bound |

## Sync Status Against Mainline Call

- command validation, query selection, subscription routing, turn projection, and debug-state projection are bound in code
- command ingress acceptance, dispatch-envelope routing, and rejection payload mapping are now bound in code
- checkpoint rewind is now a protocol-owned mutation-intent command routed to `runtime.checkpoint-rewind`
- checkpoint summary projection/query is read-only protocol state and code-bound
- client-specific projection gating is now also bound in code
- UiProtocolState now owns a continuous subscription channel plus incremental shared-contract turn projection updates
- debug-state projection consumes freehand-debug::DebugStateSnapshot instead of a UI-owned duplicate DTO
- UI ingress versus truth-writer separation is now locked in the function map
- minimal per-turn debug-state query/subscribe plus receiver-drain bridge are now bound in UiProtocolState
- model request waiting projection is bound in UiProtocolState and clears on response/tool/usage/terminal/error projection
- selected-session transcript replacement preserves model_request and tool_activities already present in UiProtocolState only for the latest nonterminal replacement turn, while later-round or terminal refresh drops stale live activity
- session list active_turn_id projection is latest-nonterminal-only so terminal latest turns do not make completed/failed/cancelled sessions look active
- public turn projection is protocol-owned
- terminal status is now preserved in UiTurnProjection and public conversation status mapping
- tool activity status and result detail are now preserved in UiTurnProjection.tool_activities and public conversation status mapping
- public tool summaries preserve tool_call_id, duplicate same-id tool calls upsert into one public card, and completed/failed public tool bodies expose tool result detail
- CancelLatestActiveTurn is now accepted by command ingress and routed to reason.turn
- ADP request and response frames are now protocol-owned and JSON roundtrip tested for UI-less automation clients
- task list/history query command DTOs and runtime query-port shape are landed
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle query DTOs are landed and route only through runtime-backed query ports
- Phase 1 ApplyExecutionFact and RunSchedulerTick command DTOs are landed and route only through runtime-backed task.orchestration dispatch
- error-center query/subscribe command DTOs and UI-safe event projection are landed
- provider definition upsert and legacy provider/model update command DTOs are landed, owner-routed to config.core, and serialize without credential/API-key values
- active provider selection command DTO is landed, owner-routed to config.core, and serializes without credential/API-key values
- Agent resource-count update command DTO is landed, owner-routed to config.core, rejects out-of-range counts, and serializes without provider credentials or live-process state
- task mutation command DTOs are landed, owner-routed to task.orchestration, and rejected at the protocol boundary when required task, worker, execution, or review fields are empty
- Phase 2A worker agent, assignment, claim, review rejection, and dispatch-mode DTOs are landed and locked by protocol owner tests
- Phase 2B EventInbox/MasterPoll DTOs are landed and locked by phase2b_event_inbox_and_master_poll_validate_and_route_to_task_orchestration, including replay_from_start conflict validation
- Phase 2C WorkerControl command/query DTOs are landed and locked by protocol owner tests
