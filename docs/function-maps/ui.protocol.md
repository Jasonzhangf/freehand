# Function Map: `ui.protocol`

- feature_id: `ui.protocol`
- owner crate: `crates/freehand-ui-protocol`
- owner module: `crates/freehand-ui-protocol/src/lib.rs` with protocol DTOs in `src/dto.rs`, ADP wire contracts in `src/adp_wire.rs`, and command descriptors in `src/adp_descriptor.rs`
- module registry: `docs/module-registry/ui.protocol.json`
- verification map: `docs/verification-maps/ui.protocol.json`
- mainline call source: `docs/mainline-calls/ui.protocol.json`
- generated wiki: `docs/wiki/ui.protocol.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `task.project_to_ui`
  - `input_attachment.validate_submit_metadata`
  - `debug_trace.read_snapshot`
  - `search_evidence.project_to_ui`
- owner entry symbols:
  - `UiTurnProjection`
  - `TurnProjectionInput`
  - `validate_command`
  - `accept_command_ingress`
  - `protocol_rejection`
  - `build_command_dispatch_envelope`
  - `dispatch_port_failure`
  - `UiAdpRequest`
  - `UiAdpResponse`
  - `adp_protocol_version`
  - `adp_server_capabilities`
  - `adp_protocol_manifest`
  - `adp_protocol_manifest_json`
  - `adp_protocol_webui_module`
  - `subscription_selector`
  - `subscription_matches`
  - `debug_projection_from_event`
  - `merge_hosted_search_activities`
  - `UiProtocolState::apply_search_evidence`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `ui_projection`
  - `input_attachment`
- touched resources:
  - `task`
  - `session`
  - `node_pairing`
  - `debug_trace`
  - `search_evidence`
- resource operations:
  - `task.project_to_ui`
  - `input_attachment.validate_submit_metadata`
  - `debug_trace.read_snapshot`
  - `search_evidence.project_to_ui`
- forbidden shortcuts:
  - UI projection must not mutate task truth directly.
  - UI projection must not synthesize persisted sessions from temporary subagent turns.
  - UI projection must not mutate or synthesize node pairing truth directly.
  - `turn_projection_from_events`
  - `public_conversation_items`
  - `public_turn_projection`
  - `checkpoint_projection_from_runtime_summary`
  - `terminal_text_projection`
  - `UiProtocolState::subscribe`
  - `UiProtocolState::query`
  - `UiProtocolState::preserve_live_activity_on_page_refresh`
  - `UiRuntimeQueryPort::query_runtime`
  - `UiRuntimeQueryPort::query_runtime_with_scope`
  - `UiQueryAccessScope`
  - `UiProviderConfigUpdate`
  - `UiModelGroupConfigUpdate`
  - `UiAgentModelGroupSelectionUpdate`
  - `UiToolRegistryProjection`
  - `session_list_projection`
  - `turn_is_nonterminal`
- `UiProtocolState::apply_semantic_event`
  - `UiProtocolState::apply_tool_call`
  - `UiProtocolState::apply_tool_result`
- `UiProtocolState::apply_usage_event`
  - `UiProtocolState::apply_terminal_event`
  - `UiProtocolState::apply_error_event`
  - `UiProtocolState::apply_debug_event`
  - `UiProtocolState::drain_debug_receiver`
  - `turn_projection_for_client`

## Request Mainline

- UI commands enter one protocol truth shared by CLI and WebUI
- UI acts as an input ingress only; command submission does not make UI a truth writer
- command ingress acceptance is explicit and route-scoped: only mutation-intent commands may enter the ingress transport path
- accepted command ingress is wrapped into a dispatch envelope that declares the target owner feature/module before leaving the protocol boundary
- runtime-owned mutation commands such as checkpoint rewind stay explicit at the protocol envelope layer and do not become UI-owned semantics
- `CancelLatestActiveTurn` is a mutation-intent command for stopping the current active turn when a UI has not yet received a concrete `turn_id`
- `SubmitUserInput` may carry an optional selected `session_id` and selected `cwd` so the protocol can route a new turn into an explicitly chosen cwd-bound session or draft session
- `SubmitUserInput.metadata.attachments` is the neutral attachment ingress for image payloads; protocol validates non-empty id/name/media-type/base64 and rejects unsupported kinds before runtime dispatch, while images do not enter `text`
- session management commands (`CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`, `RollbackLatestSessionTurn`) are mutation intents only; the protocol validates and routes them while `reason.persistence` owns durable session metadata and rollback truth
- query and subscribe stay separate
- ADP WebSocket clients use protocol-owned typed frames with top-level `protocol_version=3`; the first client frame must be `Handshake`, and command/query/subscribe frames are valid only after a server `HandshakeAccepted` capability response
- ADP command/frame metadata is single-sourced from the protocol-owned `UI_COMMAND_DESCRIPTORS` table; generated JSON manifest and WebUI constructors must be exported from Rust instead of handwritten in JavaScript
- task list and task history queries are protocol-owned ADP/query command shapes, but the protocol only defines UI-safe DTOs and query-port routing; persisted task truth remains owned by `task.orchestration`
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle queries are protocol-owned ADP/query command shapes; protocol defines UI-safe DTOs while runtime/task owners supply truth, including task `created_at` for submit-receipt correlation without relying on heartbeat `updated_at`
- public task mutation commands (`CreateTask`, `CreateTaskAgent`, `AssignTask`, `SubmitTaskReview`, `RejectTaskReview`, `ApproveTaskReview`, `CloseTask`) are protocol-owned mutation intents that validate required fields and route to `task.orchestration` through runtime; protocol does not write task truth. Internal scheduler/worker commands (`ClaimNextTask`, `ApplyExecutionFact`, `RunSchedulerTick`, `RunMasterPoll`) remain typed protocol DTOs for internal runtime/CLI harness paths, but are excluded from the public ADP manifest and rejected by public server command ingress before runtime dispatch.
- `UiTaskDispatchCommand` lets a task create command explicitly choose self dispatch, agent dispatch, or no immediate dispatch so Phase 2A can create a waiting task before assignment
- Phase 2B `QueryEventInbox` and read-only `QueryMasterPoll` are public protocol-owned query shapes routed to `task.orchestration`; internal `RunMasterPoll` remains a typed mutation DTO for runtime/CLI harness paths. Protocol does not store event cursor truth, classify board state, or apply master business actions.
- `RunMasterPoll` is an internal Command-frame mutation intent; `QueryMasterPoll` is public Query-frame read-only owner projection via `preview_master_poll`.
- `RunMasterPoll.replay_from_start` is a protocol-owned cursor-mode field;
  protocol validates that it is not combined with `after_cursor`, while
  `task.orchestration` owns the actual replay and cursor persistence semantics
- turn projection input may carry owner-created turn time; protocol preserves
  that timestamp as optional projection truth and does not synthesize wall-clock
  message time from client render time
- Phase 2C `WorkerControl` and `QueryWorkerControl` are protocol-owned
  command/query shapes for already-running worker execution control; protocol
  validates IDs/op payloads and routes mutation intent to `worker.control`
  while task/runtime owners supply control-event truth
- Timer dashboard `QueryTimerList`, `ScheduleTimer`, and `CancelTimer` are
  protocol-owned query/mutation shapes for independent timer truth. Protocol
  validates timer ids, schedule mode, relative/absolute/recurring fields,
  reason, prompt, and source session id before runtime dispatch; protocol does
  not write timer schedules or infer timer state.
- Tools dashboard `QueryToolRegistry` is a protocol-owned read-only ADP query
  shape. Protocol defines `UiToolRegistryProjection` and
  `UiToolRegistryToolProjection`, but the registry rows, examples, guidance,
  execution scopes, and Master/Worker exposure flags must be supplied by the
  runtime/tool registry owner path, not protocol-local state.
- Search dashboard `QuerySessionSearch` is a protocol-owned read-only ADP
  query shape. Protocol defines `UiSessionSearchProjection` plus child-match
  DTOs, while persisted index rows, metadata, and Worker-parent attachment
  truth must be supplied by runtime/reason/task owners, not protocol-local
  state.
- Diagnostics `QueryDiagnostics` is a protocol-owned read-only ADP query shape.
  Protocol defines `UiDiagnosticsProjection` and
  `UiDiagnosticLogFileProjection`, while runtime/debug owners supply safe log
  metadata and redacted tail lines without raw provider payloads, secrets, or
  absolute user paths.
- task list subscriptions are protocol-owned ADP/subscribe command shapes; task list projection contents must be supplied by runtime/task owners and remain read-only UI DTOs
- error-center event queries and subscriptions are protocol-owned ADP/query/subscribe command shapes, but metadata truth remains owned by `metadata.core` and classified by `error.center`
- config status query is a protocol-owned ADP/query command shape carrying the complete safe provider registry plus current primary/fallback selection, but selected config truth remains owned by `config.core` and supplied through `runtime.ui-command-dispatch`
- provider definition upsert is a protocol-owned mutation command shape (`UpsertProviderConfig`) that carries only editable provider id/type/protocol/base URL/default model/env-var auth fields and routes to `config.core` without switching active selection
- provider selection is a protocol-owned mutation command shape (`UpdateAgentProviderSelection`) that carries only agent name plus primary/fallback provider ids and routes to `config.core` without rewriting provider definitions
- model group definition upsert is a protocol-owned mutation command shape (`UpsertModelGroupConfig`) that carries only group id, enabled flag, label, primary/sub/search/title/fallback/load-balance route ids/models and routes to `config.core` without writing provider secrets or switching active provider selection
- model group selection is a protocol-owned mutation command shape (`UpdateAgentModelGroupSelection`) that carries only agent name plus optional model group id and routes to `config.core` without rewriting provider definitions or group definitions
- provider web_search live test is a protocol-owned command shape (`TestProviderWebSearch`) carrying provider id plus optional query; it routes to `provider.reason-live-bridge` and must not be handled as a UI-local query or local Freehand tool call
- legacy provider/model update remains a protocol-owned mutation command shape (`UpdateProviderConfig`) for existing CLI callers; WebUI and CLI must not write config files directly or send raw API-key values
- Agent resource-count update is a protocol-owned mutation command shape (`UpdateAgentResourceConfig`) that carries only agent name plus `resource_count`; protocol rejects values outside `1..=5` before runtime dispatch and routes valid intent to `config.core`
- subscriptions may target latest active turn, specific turn, specific turn debug state, or node/progress streams

## Response Mainline

- `UiTurnProjection.search_evidence` projects typed persisted search evidence; UI protocol does not parse provider observations or camo stdout

- query returns snapshots
- checkpoint query returns read-only checkpoint summary projections supplied by runtime owner code
- command ingress returns explicit dispatch receipt without claiming truth mutation success
- cancel commands route to `reason.turn` whether they target an explicit `turn_id` or the latest active turn
- subscribe returns an initial snapshot followed by continuous incremental projections through a protocol-owned subscription channel
- ADP handshake returns an explicit `HandshakeAccepted` frame with server capabilities before any command/query/subscribe response; ADP subscribe then returns an explicit `SubscriptionAccepted` frame before later `SubscriptionEvent` frames so UI-less clients can distinguish waiting from transport failure
- ADP protocol export returns a deterministic public manifest with `protocol_version`, handshake capability, request/response kinds, command serde names, frame classes, and owner feature ids, plus a WebUI module exposing `adpQueryOf`, `adpCommandOf`, and `adpSubscribeOf` constructors; internal owner module paths stay out of served/generated artifacts and public `CommandReceipt` responses
- projections are read-only views over owner-written truth
- model request lifecycle is projected as typed `UiModelRequestActivity` inside `UiTurnProjection` when runtime reports the provider request has been built and sent, when completion-schema validation rejects a model terminal block and runtime sends repair feedback back to the model, or when tool results have been paired and the model continuation request is waiting; `kind` distinguishes `Thinking`, `SchemaRetry`, and `ToolResultContinuation`, while provider retry/failover are transport substate in `UiModelRequestActivity.transport` and never become a separate reasoning-flow phase; the activity clears when response/tool/usage/terminal/error projection arrives
- selected-session transcript replacement preserves same-turn live-only `model_request` and `tool_activities` only for the latest replacement turn when that turn is nonterminal, so a persistence-backed refresh cannot erase current provider transport retry/waiting or per-tool-call activity, but older rounds stop looking live once a later round or terminal snapshot exists
- terminal completion shows only final projected text
- turn projections preserve terminal status separately from terminal text so UI clients can distinguish success, failed, blocked, interrupted, running, and cancelled terminal states
- turn projections preserve owner-created turn time as
  `UiTurnProjection.created_at` when runtime/reason supplies it, allowing UI
  clients to render per-message local-time labels without becoming timestamp
  owners
- turn projections expose metadata-only `UiTurnProjection.attachments` so clients can render submitted image metadata after refresh without raw/base64 payloads
- public conversation projection strips raw completion schema blocks and excludes reasoning, usage, provider payload, debug details, and verbose tool term text from the main user-visible stream
- public conversation tool summaries preserve owner-projected `UiToolActivity.detail` even when `UiToolActivity.display` is present, so failed tool cards show the actual execution error instead of only display parameters
- public conversation projection preserves the user prompt while still stripping raw completion schema blocks and excluding reasoning, usage, provider payload, debug details, and verbose tool term text from the main user-visible stream
- public conversation session selection stays explicit: submit can target a selected session id, and session-level transcript queries stay separate from the global latest turn
- session list and transcript projections expose session `cwd`, and turn projections carry `cwd` when the runtime owner has bound a session to a workspace
- session list projections expose only owner-supplied persisted session metadata (`CreateSession` / session metadata truth) as top-level active or archived sessions, so WebUI, Android, CLI, and headless ADP clients share one CRUD truth instead of deriving global sessions from raw turns
- `UiSessionSummary.active_turn_id` is a live/progress identity only: it may point only to the same-session latest turn when that turn is nonterminal with model/tool activity; terminal snapshots are authoritative and clear the session active identity even if an older round still has historical retry metadata
- session list projections hide internal framework sessions such as `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` from user-facing active and archived lists, while explicit `QuerySessionTurns { session_id }` remains queryable for debug/replay truth
- bounded selected-session page projections preserve same-turn live activity only for nonterminal replacement truth; terminal page truth clears stale activity
- task board projections carry `parent_session_id`, observing `attached_session_ids`, and canonical `worker_session_id` so WebUI can scope tasks to the selected parent/observer session and open the Worker transcript without synthesizing an id
- rollback command ingress exposes append-only latest-turn rollback as a reason.persistence mutation intent; protocol does not remove turns or mutate local transcript truth
- task list and task history query results expose UI-safe task snapshot and ledger-event projections supplied by runtime owner code through `UiRuntimeQueryPort`
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle query results expose UI-safe board/lifecycle projections supplied by runtime owner code through `UiRuntimeQueryPort`; TaskBoard task snapshots carry parent scope, canonical Worker session id, and `created_at`
- Phase 2B EventInbox and MasterPoll results expose UI-safe event rows,
  cursor values, compact classifications, and recommended semantic action
  labels supplied by runtime owner code through `UiRuntimeQueryPort`
- Phase 2C WorkerControl query results expose UI-safe control events and
  optional task/agent/lifecycle/task-event projections supplied by runtime
  owner code through `UiRuntimeQueryPort`
- TimerList query results expose UI-safe timer schedules and ledger event rows
  supplied by runtime owner code through `UiRuntimeQueryPort`
- ToolRegistry query results expose UI-safe built-in tool schema/guidance rows
  supplied by runtime owner code through `UiRuntimeQueryPort`; protocol does
  not execute tools, persist tool truth, or expose provider-hosted broad search
  as a local `web_search` function tool
- SessionSearch query results expose UI-safe persisted session search rows
  supplied by runtime owner code. Worker matches are nested under their owning
  persisted Master session and never projected as top-level session results.
- Diagnostics query results expose UI-safe runtime log metadata and redacted tail
  lines supplied by runtime owner code through `UiRuntimeQueryPort`; protocol
  does not read filesystem logs, expose absolute paths, or project raw provider
  payloads/secrets.
- task list subscription events expose the same UI-safe task list projection as query results so task panels can refresh from push without polling or app-local task state
- error-center event query results expose UI-safe watermarked metadata fields plus raw hash only; raw provider/tool/request/user/assistant text is not part of the protocol DTO
- config status query results expose UI-safe active agent/provider/model fields,
  an ordered peer list of agent name/mode/node id, and auth source type only;
  API keys, pair-token env/value fields, provider raw payloads, and full
  credential-bearing URLs are not part of the DTO
- config status query results expose UI-safe model group registry rows and current active model group id, but not provider credential values or raw config TOML
- config status query results expose provider web_search configured/effective
  route status and reason strings as UI-safe capability diagnostics, not as
  secrets or provider wire payloads
- provider/model update command receipts report owner dispatch status only; the restart-required and saved provider/model state is observed by a follow-up config status query/projection, not by protocol-local config truth
- model group definition/selection command receipts report owner dispatch status only; the restart-required and saved group state is observed by a follow-up config status query/projection, not by protocol-local config truth
- provider web_search test command receipts report owner dispatch status only;
  provider success and exact provider rejection remain visible receipt text
  supplied by runtime/provider owners
- Agent resource-count update command receipts report owner dispatch status only; the restart-required and saved topology state is observed by a follow-up config status query/projection, not by protocol-local config truth
- error-center subscription initial snapshots use the same `UiErrorCenterEventListProjection` as query results
- public conversation tool summaries carry `tool_call_id` so UI clients can update one tool card instead of rendering duplicate waiting/completed cards; tool status/outcome is conveyed by the status field while the public body stays semantic and target-focused instead of echoing success/failure result text
- public conversation tool summaries carry `tool.display` structured semantic projection from `tool.display`, so UI clients render category/action/target/parameters/result without parsing raw tool terms
- public conversation terminal items derive status strings from terminal status instead of treating every terminal text as completed; `TerminalStatus::ToolPending` projects as title `Lifecycle` and status `running`, not `Final`/`completed`
- Android APK renders no local conversation shell. It loads the daemon-hosted WebUI with `client=android-webview`; WebUI and browser shells therefore share the same protocol projection truth directly.
- debug state is projected as a read-only per-turn snapshot/stream with summary text plus ordered detail lines
- `ui.protocol` may ingest observation-only debug events from `debug.core` receivers and materialize only the snapshot projection into protocol state
- `ui.protocol` may ingest shared semantic/tool/usage/terminal/error contracts incrementally and update one turn projection without depending on `freehand-reason`
- tool-call lifecycle is projected as `UiToolActivity` inside `UiTurnProjection`: `ReasonReq04ToolCall` upserts a `waiting` activity, matching `ReasonReq05ToolResultReentry` marks the same activity `completed` or `failed`, and failed terminal truth only marks still-waiting tool activities `failed`
- `public_conversation_items` combines structured `UiToolActivity.display` with `UiToolActivity.detail` instead of letting display suppress the raw execution result/error detail
- `UiToolActivity.display` is produced by `freehand-blocks::project_tool_call_display` and updated by `freehand-blocks::project_tool_result_display`; UI apps must not reimplement tool classification
- slave turn may surface as WebUI-only separate card while staying in one protocol truth
- client-specific projection gating stays inside the protocol owner, not in apps
- UI must be able to consume reason-turn state and debug-state projections without owning either truth source
- transport adapters may drain debug receivers and query protocol snapshots, but projection ownership stays in `freehand-ui-protocol`

## Error Mainline

- invalid command, invalid stream selection, or unavailable source projection return explicit protocol errors
- empty `SubmitUserInput.cwd` is rejected at the protocol boundary instead of being treated as runtime default
- malformed or unsupported input attachment metadata is rejected at the protocol boundary instead of being passed to runtime/provider adapters or appended to user text
- empty session ids and empty session titles are rejected at the protocol boundary for session management commands, including rollback
- query/subscribe commands sent to command-ingress route are explicit protocol misuse errors
- ADP request/response frames missing `protocol_version` or carrying an unsupported version are rejected during protocol deserialization before route handling
- stale or missing generated ADP manifest/constructor artifacts are gate failures; generated artifacts and public command receipts must not expose internal owner module paths; WebUI frame-class misuse throws before a request leaves the browser constructor helper
- query commands sent as ADP command frames are explicit protocol misuse errors, not mutation attempts
- `CancelLatestActiveTurn` without any active or persisted turn returns explicit target-not-found from the owner module
- empty checkpoint rewind ids are rejected at the protocol boundary before runtime dispatch
- empty task history ids are rejected at the protocol boundary as `empty_task_id`
- empty error-center session ids are rejected at the protocol boundary as `empty_session_id`
- task list/history commands sent to command ingress are rejected as query-route misuse instead of mutating task truth
- config status command sent to command ingress is rejected as query-route misuse instead of becoming a UI-local config read or mutation
- `QueryToolRegistry` sent to command ingress or local protocol-state query is
  rejected as route/source misuse instead of returning a browser/protocol-local
  hardcoded tool list
- `QueryDiagnostics` sent to command ingress or local protocol-state query is
  rejected as route/source misuse instead of returning a browser/protocol-local
  diagnostics list or absolute-path log tail
- empty `QuerySessionSearch.query`, command-ingress use, or local
  protocol-state handling is rejected explicitly instead of returning
  browser/protocol-local session search results
- provider/model update commands reject empty agent/provider/type/protocol/base URL/model/env-var fields and unsupported protocol values before dispatch; credential/API-key value fields do not exist in the DTO
- model group commands reject empty agent/group ids, empty route providers/models, and zero load-balance weights before dispatch; credential/API-key value fields do not exist in the DTO
- Agent resource-count update commands reject empty agent names and counts outside `1..=5` before dispatch; provider credentials and process-start state do not exist in the DTO
- Public ADP command ingress rejects internal scheduler/worker commands (`ClaimNextTask`, `ApplyExecutionFact`, `RunSchedulerTick`, `RunMasterPoll`) with `adp_command_not_public` before building a runtime dispatch envelope.
- Internal `RunMasterPoll` rejects empty cursors and rejects `replay_from_start=true`
  combined with `after_cursor` before dispatch
- task history remains query-only; task list subscribe accepts only list filters and must reject history/query misuse on the subscribe route
- task mutation commands reject empty task id/title/content/goal/review summary, empty worker agent id/capabilities, empty claim execution id, and empty review rejection fields before runtime dispatch instead of silently creating partial task truth
- Phase 1 execution fact commands reject empty ids and malformed facts before runtime dispatch; scheduler tick commands reject invalid threshold shape before runtime dispatch
- Phase 2B EventInbox and MasterPoll commands reject empty cursor strings before
  runtime dispatch and must not treat unknown cursors as empty results
- checkpoint query misses return an empty read-only snapshot, not an implicit recovery or filesystem fallback
- Phase 2C WorkerControl commands reject empty ids, unknown ops, missing
  `ask_at_safe_point.question`, and missing `add_constraint.constraint` before
  runtime dispatch; `QueryWorkerControl` stays query-only
- Timer commands reject empty timer ids, unknown modes, missing relative delay,
  missing absolute run-at time, missing recurring repeat rule, invalid repeat
  parameters, empty reason, empty prompt, and query-route misuse before runtime
  dispatch
- source identity fields remain explicit across success and error paths
- cancelled terminal projection stays explicit and is not collapsed into failed or completed UI status
- UI-side commands may request mutations, but mutation success/failure is decided by owner modules and reflected back as projections or errors

## Shared Multi-Reference Functions

- `terminal_text_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: collapse terminal event to final user-visible text
  - allowed callers: query handlers, stream handlers, CLI/WebUI adapters
  - related tests: terminal result projection smoke
  - why shared: ensures CLI and WebUI project the same terminal text truth
- `public_conversation_items`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: derive user-visible conversation items from a full turn projection without exposing internal reasoning/debug/raw schema data while retaining the user prompt, tool_call_id identity, tool lifecycle status, and tool result detail
  - allowed callers: CLI/WebUI renderers and transport adapters
  - related tests: public conversation projection smoke
  - why shared: all UI clients need one public projection rule instead of per-client filtering
- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: gate slave substream visibility by UI client kind without changing turn truth
  - allowed callers: CLI/WebUI/Android adapters, query handlers
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
- `adp_protocol_version` / `adp_server_capabilities`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: single-source the current ADP wire version and handshake capability list for Rust clients and transport adapters
  - allowed callers: WebUI/daemon ADP transport, CLI automation, tests
  - related tests: `adp_request_and_response_frames_roundtrip`, `adp_frames_require_supported_protocol_version`, server ADP handshake tests
  - why shared: every UI/control client must negotiate the same versioned transport contract before command/query/subscribe frames are accepted
- `adp_protocol_manifest` / `adp_command_manifest_entries` / `adp_protocol_manifest_json` / `adp_protocol_webui_module`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: single-source ADP protocol version, handshake capability, command serde names, frame classes, and public owner feature ids into deterministic JSON and WebUI constructor artifacts while excluding internal owner module paths
  - allowed callers: `export-adp-protocol` bin, `xtask gates check`, WebUI asset smoke, protocol tests
  - related tests: `adp_protocol_manifest_covers_all_command_variants`, `verify_adp_protocol_artifacts`
  - why shared: all Rust and WebUI ADP clients need one command/frame contract instead of handwritten JavaScript mirrors or duplicated command classification logic
- `UiRuntimeQueryPort::query_runtime`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: let app transports ask runtime owners for read-only query projections such as config status, task list/history, Phase 1 boards/lifecycle, worker-control events, and error-center events before falling back to protocol-state queries
  - allowed callers: WebUI/daemon ADP query transport
  - related tests: daemon ADP task query smoke, daemon ADP error-center query smoke, Phase 1 foundation CLI smoke
  - why shared: keeps app transports protocol-bound while allowing runtime-owned read models without importing runtime into `freehand-server`
- `UiQueryAccessScope` / `UiRuntimeQueryPort::query_runtime_with_scope`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: carry local-loopback versus remote projection scope as a typed side-channel so remote ConfigStatus cannot expose loopback endpoints or secrets through response payloads
  - allowed callers: WebUI ADP transport, Relay bridge, runtime query owner
  - related tests: `cargo test -p freehand-server query_access_scope -- --nocapture`, `cargo test -p freehand-runtime runtime_query_projects_config_status_without_secrets -- --nocapture --test-threads=1`
  - why shared: keeps access visibility policy out of business query payloads and out of app-specific URL filtering
- `project_tool_call_display` / `project_tool_result_display`
  - owner: `crates/freehand-blocks/src/tool_display.rs`
  - purpose: parse tool call/result contracts into UI-safe semantic display projection
  - allowed callers: UI protocol projector and tests
  - related tests: tool display parser tests, public tool summary projection tests
  - why shared: keeps WebUI/Android/CLI from guessing tool classes from raw tool text

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | accept and validate UI command payload | UI command | validated command | CLI/WebUI | protocol boundary | bound |
| 01a | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | validate SubmitUserInput image metadata attachments and reject missing id/name/media type/base64 or unsupported kinds | `SubmitUserInput.metadata.attachments` | validated neutral attachment metadata or explicit protocol rejection | CLI/WebUI/Android transports | protocol boundary | bound |
| 01 note | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | validate ingress intent only; does not mutate reason/debug truth | UI command | validated command | CLI/WebUI | protocol boundary | bound |
| 02 | `accept_command_ingress` | `crates/freehand-ui-protocol/src/lib.rs` | accept only mutation-intent ingress commands and return explicit ack | UI command | ingress ack | CLI/WebUI transport adapters | protocol boundary | bound |
| 03 | `protocol_rejection` | `crates/freehand-ui-protocol/src/lib.rs` | convert protocol error into transport-safe rejection payload | protocol error | rejection payload | CLI/WebUI transport adapters | protocol boundary | bound |
| 04 | `build_command_dispatch_envelope` | `crates/freehand-ui-protocol/src/lib.rs` | wrap accepted ingress command with declared owner routing | UI command | dispatch envelope | CLI/WebUI transport adapters | protocol boundary | bound |
| 05 | `UiProtocolState::query` | `crates/freehand-ui-protocol/src/lib.rs` | execute read-only query path | query command | snapshot projection | protocol boundary | query handler | bound |
| 06 | `UiProtocolState::subscribe` | `crates/freehand-ui-protocol/src/lib.rs` | expose the protocol-owned continuous subscription channel for app transports | none | `UiSubscriptionEvent` receiver | app/transport adapters | protocol state | bound |
| 07 | `subscription_selector` | `crates/freehand-ui-protocol/src/lib.rs` | build read-only subscribe selector | subscribe command | subscription selector | protocol boundary | stream handler | bound |
| 08 | `subscription_matches` | `crates/freehand-ui-protocol/src/lib.rs` | route incremental projection to matching subscription | subscription selector + projection | delivery decision | stream handler | selector matcher | bound |
| 09 | `TurnProjectionInput` / `turn_projection_from_events` / `UiTurnProjection` | `crates/freehand-ui-protocol/src/adp_wire.rs` / `crates/freehand-ui-protocol/src/adp_descriptor.rs` / `crates/freehand-ui-protocol/src/dto.rs` | project whole-turn state into UI snapshot, including owner-created turn time and tool lifecycle activities | semantic/tool/tool-result/usage/terminal/error/user inputs plus optional owner-created timestamp | UI turn projection with `created_at` when supplied by runtime/reason truth | query/stream handler | projector | bound |
| 10 | `terminal_text_projection` | `crates/freehand-ui-protocol/src/lib.rs` | project terminal text | terminal semantic payload | UI terminal text | query/stream handler | projector | bound |
| 10a | `public_conversation_items` / `public_turn_projection` | `crates/freehand-ui-protocol/src/lib.rs` | derive public user-visible conversation stream, preserve user prompt, and strip raw completion schema | full turn projection | public turn projection | app transports/renderers | projector | bound |
| 10b | `UiProtocolState::apply_terminal_event` / `turn_projection_from_events` | `crates/freehand-ui-protocol/src/lib.rs` | preserve terminal status alongside terminal text in UI turn projections | terminal semantic event | terminal text + terminal status projection | runtime/app protocol consumers | protocol projector | bound |
| 11 | `UiProtocolState::apply_semantic_event` / `apply_tool_call` / `apply_tool_result` / `apply_usage_event` / `apply_terminal_event` / `apply_error_event` | `crates/freehand-ui-protocol/src/lib.rs` | incrementally update one turn projection from shared contract events and publish subscription updates | shared reason/error contracts | updated queryable/subscribable turn projection | runtime/debug bridges | protocol state | bound |
| 11b | `project_tool_call_display` / `project_tool_result_display` | `crates/freehand-blocks/src/tool_display.rs` | attach structured tool display projection before UI consumption | tool call/result contracts | `ToolDisplayProjection` | ui.protocol | tool.display owner | bound |
| 11a | `UiProtocolState::apply_model_request_waiting` / `apply_model_request_waiting_kind` | `crates/freehand-ui-protocol/src/lib.rs` | project provider-request-sent and continuation lifecycle state before model response arrives | runtime provider request built or continuation signal | queryable/subscribable turn projection with typed model request waiting kind | runtime debug bridge | protocol state | bound |
| 11c | `UiProtocolState::apply_completion_schema_retry_waiting` | `crates/freehand-ui-protocol/src/lib.rs` | project completion-schema mismatch as user-visible model polishing wait state | schema mismatch retry count + issue summary | queryable/subscribable turn projection with `kind=SchemaRetry` and compact `schema polishing #N: issue` detail | runtime bridge | protocol state | bound |
| 11d | `UiProtocolState::apply_search_evidence` / `merge_hosted_search_activities` | `crates/freehand-ui-protocol/src/lib.rs` / `crates/freehand-ui-protocol/src/projection.rs` | store typed persisted search evidence projection and merge hosted search tool activities into the turn projection | `SearchEvidenceTurnDelivery` | updated turn projection with `search_evidence` and `web_search` tool activities | runtime/reason bridge | protocol state / projector | bound |
| 12 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate client-specific slave substream visibility | turn projection + client kind | client-specific turn projection | CLI/WebUI adapter | projector | bound |
| 13 | `UiProtocolState::set_debug_state` | `crates/freehand-ui-protocol/src/lib.rs` | store per-turn read-only debug projection for UI consumption and publish subscription updates | `freehand-debug` snapshot | queryable/subscribable debug state | reason/node/debug bridge | protocol state | bound |
| 14 | `UiProtocolState::apply_debug_event` | `crates/freehand-ui-protocol/src/lib.rs` | ingest one observation-only debug event into UI protocol state when a snapshot is present | `freehand-debug` event | updated per-turn debug state or ignored event | reason/node/debug bridge | protocol state | bound |
| 15 | `UiProtocolState::drain_debug_receiver` | `crates/freehand-ui-protocol/src/lib.rs` | drain a `debug.core` receiver without making UI a truth writer | debug receiver | applied snapshot count | protocol transport/app adapters | protocol state | bound |
| 16 | `debug_projection_from_event` | `crates/freehand-ui-protocol/src/lib.rs` | map one debug event to read-only UI debug projection when snapshot exists | `freehand-debug` event | `UiProjection::Debug` | protocol tests/transport adapters | projector | bound |
| 17 | `UiProtocolState::set_checkpoint_snapshot` / `checkpoint_projection_from_runtime_summary` | `crates/freehand-ui-protocol/src/lib.rs` | store and query read-only checkpoint summaries supplied by runtime owner | runtime checkpoint summary DTO | checkpoint query result | runtime dispatcher / app query handlers | protocol state | bound |
| 18 | `UiAdpRequest` / `UiAdpResponse` / `adp_protocol_version` / `adp_server_capabilities` | `crates/freehand-ui-protocol/src/adp_wire.rs` | define protocol-owned versioned WebSocket ADP frames and handshake capability metadata for command/query/subscribe automation | ADP JSON frame with top-level `protocol_version` and first-frame handshake | typed handshake, command/query/subscription response, event, or failure | WebUI/Android/CLI automation transports | protocol owner | bound |
| 18a | `UI_COMMAND_DESCRIPTORS` / `command_descriptor` / `adp_protocol_manifest` / `adp_protocol_webui_module` | `crates/freehand-ui-protocol/src/adp_descriptor.rs` / `crates/freehand-ui-protocol/src/adp_wire.rs` | derive the ADP manifest and WebUI constructor module from the exhaustive protocol command descriptor table | `UiCommand` variants plus protocol version and handshake capability constants | deterministic command manifest entries, frame classes, public owner feature ids, and JavaScript constructor helpers without internal crate paths | `export-adp-protocol` / protocol tests / `xtask` gate | protocol owner descriptor table | bound |
| 18b | `write_output` / `main` | `crates/freehand-ui-protocol/src/bin/export-adp-protocol.rs` | write the generated ADP JSON manifest or WebUI constructor module to the requested artifact path | `--json` or `--js` output path | generated `adp-protocol.schema.json` or `adp-protocol.js` artifact | developer / `xtask` gate | freehand-ui-protocol manifest exporters | bound |
| 19 | `validate_command` / `command_dispatch_target` | `crates/freehand-ui-protocol/src/lib.rs` | validate session-management mutation intents and route them to the session persistence owner | session CRUD or rollback command | owner-routed dispatch envelope or protocol rejection | WebUI/Android/CLI transports | runtime/reason persistence owner path | bound |
| 19a | `UiProtocolState::replace_session_turn_projections` / `preserve_live_activity_on_nonterminal_refresh` / `merge_tool_activity` | `crates/freehand-ui-protocol/src/lib.rs` | replace one session's effective transcript projection after persistence-owned rollback or selected-session refresh while preserving live provider-transport/tool activity only for the latest nonterminal replacement turn and keeping terminal snapshots authoritative | session id plus effective turn projections plus current protocol state | queryable session transcript without rolled-back turns, without stale live activity on historical rounds, and without losing current provider transport retry/tool-call observability | runtime.ui-command-dispatch | protocol state | bound |
| 19b | `UiProtocolState::merge_persisted_turn_projections_without_publish` | `crates/freehand-ui-protocol/src/lib.rs` | merge persistence-owned background session snapshots for list queries without publishing replay events, changing latest-active identity, or replacing a newer live nonterminal projection | persisted turn projections plus current protocol state | silently refreshed queryable session rows with live activity preserved | runtime.ui-command-dispatch | protocol state | bound |
| 19b | `session_list_projection` / `turn_is_nonterminal` | `crates/freehand-ui-protocol/src/lib.rs` | project persisted session summaries while allowing active turn identity only for the latest nonterminal live/progress turn | persisted session metadata plus turn projections plus latest protocol active turn id | session summary list where terminal latest turns remain visible but do not appear active | protocol query handler | protocol state | bound |
| 20 | `UiRuntimeQueryPort::query_runtime` | `crates/freehand-ui-protocol/src/lib.rs` | define runtime-backed read-only query extension point without making app transports import runtime owners | UI query command | optional runtime-owned query result or explicit dispatch failure | WebUI/daemon ADP query transport | runtime query owner | bound |
| 20d | `UiRuntimeQueryPort::query_runtime_with_scope` / `UiQueryAccessScope` | `crates/freehand-ui-protocol/src/lib.rs` | carry typed local/remote visibility scope beside ConfigStatus query execution without placing control semantics in query payloads | UI query command + typed access scope | scoped runtime-owned UI projection or explicit dispatch failure | WebUI/Relay ADP transport | runtime query owner | bound |
| 20a | `UiCommand::QueryConfigStatus` / `UiQueryResult::ConfigStatus` / `UiConfigStatusProjection` / `UiProviderConfigSummaryProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define UI-safe active config query/result DTO with complete provider registry and no secrets | config status query | active agent/provider/fallback/model/auth-source/resource-count projection plus safe configured provider registry | ADP query transport | runtime query port | bound |
| 20b | `UiProviderConfigUpdate` / `UiCommand::UpdateProviderConfig` / `UiCommand::UpsertProviderConfig` | `crates/freehand-ui-protocol/src/lib.rs` | define provider definition mutation DTOs without credential values and route them to the config owner | provider/model/base-url/env-var update | validated mutation intent routed to `config.core` | WebUI/CLI ADP command transport | runtime.ui-command-dispatch | bound |
| 20b1 | `UiAgentProviderSelectionUpdate` / `UiCommand::UpdateAgentProviderSelection` | `crates/freehand-ui-protocol/src/lib.rs` | define active provider selection mutation DTO without credential values and route it to the config owner | agent name plus primary/fallback provider ids | validated mutation intent routed to `config.core` | WebUI/CLI ADP command transport | runtime.ui-command-dispatch | bound |
| 20b2 | `UiCommand::TestProviderWebSearch` | `crates/freehand-ui-protocol/src/lib.rs` | define provider-hosted web_search live-test command DTO and route it to the runtime/provider bridge owner | provider id plus optional query | validated mutation intent routed to `provider.reason-live-bridge` | WebUI/CLI ADP command transport | runtime.ui-command-dispatch | bound |
| 20b3 | `UiModelGroupConfigUpdate` / `UiAgentModelGroupSelectionUpdate` / `UiCommand::UpsertModelGroupConfig` / `UiCommand::UpdateAgentModelGroupSelection` / `UiModelGroupConfigProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define model group registry/selection DTOs without credential values and route them to the config owner | group id, route provider/model fields, load-balance weights, and optional active group id | validated mutation intent routed to `config.core` plus UI-safe config projection DTOs | WebUI/CLI ADP command/query transport | runtime.ui-command-dispatch | bound |
| 20c | `UiAgentResourceConfigUpdate` / `UiCommand::UpdateAgentResourceConfig` | `crates/freehand-ui-protocol/src/lib.rs` | define Agent resource-count update command DTO without provider credentials and route it to the config owner | agent name + `1..=5` resource count | validated mutation intent routed to `config.core` | WebUI/CLI ADP command transport | runtime.ui-command-dispatch | bound |
| 21 | `UiCommand::QueryErrorCenterEvents` / `UiQueryResult::ErrorCenterEvents` | `crates/freehand-ui-protocol/src/lib.rs` | define read-only error-center query/result DTOs | session/trace/turn/domain filters | UI-safe error-center projection | ADP query transport | runtime query port | bound |
| 22 | `UiCommand::SubscribeErrorCenterEvents` / `UiProjection::ErrorCenterEvents` | `crates/freehand-ui-protocol/src/lib.rs` | define error-center subscription command and projection event shape | error-center subscription filters | UI-safe error-center subscription event | ADP subscribe transport | protocol selector matcher | bound |
| 23 | `UiCommand::QueryTaskBoard` / `UiCommand::QueryAgentBoard` / `UiCommand::QueryAgentLifecycle` / `UiTaskSnapshotProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 1 board and lifecycle query DTOs without owning task/lifecycle truth; TaskBoard task snapshots expose `created_at` from task owner truth for UI receipt correlation | board or lifecycle query filters | runtime-backed UI-safe board/lifecycle projections | ADP query transport | runtime query port | bound |
| 24 | `UiCommand::ApplyExecutionFact` / `UiCommand::RunSchedulerTick` | `crates/freehand-ui-protocol/src/lib.rs` | define internal Phase 1 execution fact and scheduler tick mutation DTOs routed to task.orchestration while excluding them from the public ADP command surface | execution fact or scheduler tick command | validated internal mutation intent routed to task.orchestration or public-ingress rejection | internal runtime/CLI harness dispatch, not public ADP command ingress | runtime.ui-command-dispatch | bound |
| 25 | `UiTaskAgentCreateCommand` / `UiTaskAssignCommand` / `UiTaskClaimCommand` / `UiTaskReviewRejectionCommand` / `UiTaskDispatchCommand` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2A worker agent, assignment, review rejection, and dispatch-mode DTOs routed to task.orchestration; keep ClaimNextTask internal-only for worker/runtime claim paths | worker/task mutation intent; public ADP excludes direct worker claim | validated mutation intent routed to task.orchestration or protocol/public-ingress rejection | ADP command transport | runtime.ui-command-dispatch | bound |
| 26 | `UiCommand::QueryEventInbox` / `UiQueryResult::EventInbox` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2B EventInbox query DTOs without owning task event truth or cursor truth | event inbox query cursor | runtime-backed UI-safe event inbox projection | ADP query transport | runtime.ui-command-dispatch | bound |
| 27 | `UiCommand::RunMasterPoll` / `UiQueryResult::MasterPoll` | `crates/freehand-ui-protocol/src/lib.rs` | define internal Phase 2B RunMasterPoll mutation DTO plus public read-only MasterPoll result DTO; public ADP uses QueryMasterPoll and rejects RunMasterPoll command ingress | master poll command cursor mode | validated internal mutation intent or public read-only owner-supplied poll projection | internal runtime/CLI harness dispatch for RunMasterPoll; ADP query transport for QueryMasterPoll | runtime.ui-command-dispatch | bound |
| 28 | `UiWorkerControlCommand` / `UiCommand::WorkerControl` / `UiCommand::QueryWorkerControl` / `UiQueryResult::WorkerControl` | `crates/freehand-ui-protocol/src/lib.rs` | define Phase 2C worker-control command/query DTOs, validate op-specific fields, and route mutation intent to worker.control | worker-control command or event query | validated mutation intent or runtime-backed control event projection | ADP command/query transport | runtime.ui-command-dispatch | bound |
| 29 | `UiTimerScheduleCommand` / `UiTimerRepeatCommand` / `UiCommand::QueryTimerList` / `UiCommand::ScheduleTimer` / `UiCommand::CancelTimer` / `UiQueryResult::TimerList` / `validate_timer_schedule_command` | `crates/freehand-ui-protocol/src/lib.rs` | define independent timer dashboard query/command DTOs, validate mode-specific schedule fields, and route mutation intent to runtime.master-worker-loop through runtime dispatch | timer list query, schedule command, or cancel command | validated timer query/result DTO or mutation intent routed to timer owner | ADP command/query transport | runtime.ui-command-dispatch | bound |
| 30 | `UiCommand::QueryToolRegistry` / `UiQueryResult::ToolRegistry` / `UiToolRegistryProjection` / `UiToolRegistryToolProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define read-only Tools dashboard query/result DTOs without owning tool registry truth or executing tools | tool registry query | runtime-backed UI-safe built-in tool registry projection or protocol-state route rejection | ADP query transport | runtime.ui-command-dispatch | bound |
| 31 | `UiCommand::QuerySessionSearch` / `UiQueryResult::SessionSearch` / `UiSessionSearchProjection` / `UiSessionSearchResultProjection` / `UiSessionSearchChildProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define read-only persisted-session Search dashboard query/result DTOs without owning session index truth or promoting Worker sessions to top-level results | search query text plus optional limit | runtime-backed UI-safe persisted session search projection or protocol-state route rejection | ADP query transport | runtime.ui-command-dispatch | bound |
| 32 | `UiCommand::QueryDiagnostics` / `UiQueryResult::Diagnostics` / `UiDiagnosticsProjection` / `UiDiagnosticLogFileProjection` | `crates/freehand-ui-protocol/src/lib.rs` | define read-only diagnostics query/result DTOs without owning debug/log truth or exposing raw provider payloads, secrets, or absolute user paths | diagnostics query | runtime-backed UI-safe diagnostics projection or protocol-state route rejection | ADP query transport | runtime.ui-command-dispatch | bound |
| 33 | `UiConfigStatusProjection` / `UiLocalAgentProjection` | `crates/freehand-ui-protocol/src/lib.rs` | carry the config-owned local Agent endpoint directory without owning endpoint selection or session truth | runtime-owned config status projection | credential-free Agent name, role, node, and WebUI URL rows | ADP query transport | runtime.ui-command-dispatch | bound |

## Sync Status Against Code

- command validation, query selection, subscription routing, turn projection, and debug-state projection are bound in code
- command ingress acceptance, dispatch-envelope routing, `cwd` validation, and rejection payload mapping are now bound in code
- checkpoint rewind is now a protocol-owned mutation-intent command routed to `runtime.checkpoint-rewind`
- checkpoint summary projection/query is read-only protocol state and code-bound
- client-specific projection gating is now also bound in code
- `UiProtocolState` now owns a continuous subscription channel plus incremental shared-contract turn projection updates
- `UiProtocolState` now projects provider-request-sent, schema-retry, and tool-result-continuation waiting states into typed `UiTurnProjection.model_request.kind` and clears it on response/tool/usage/terminal/error projection
- `UiProtocolState::replace_session_turn_projections` now preserves same-turn provider transport retry/model waiting and tool activity cards only on the latest nonterminal replacement turn, while terminal or later-round refresh drops stale live waiting state; covered by `session_refresh_preserves_active_model_request_activity`, `session_refresh_preserves_active_tool_activity_cards`, `terminal_session_refresh_drops_stale_live_activity`, and `session_list_active_turn_id_tracks_only_nonterminal_turns`
- `session_list_projection` now exposes `active_turn_id` only for the latest nonterminal live/progress turn; terminal latest turns keep their terminal summary/status but do not make the session look active. Covered by `session_list_active_turn_id_tracks_only_nonterminal_turns`
- debug-state projection consumes `freehand-debug::DebugStateSnapshot` instead of a UI-owned duplicate DTO
- UI ingress versus truth-writer separation is now locked in the function map
- minimal per-turn debug-state query/subscribe plus receiver-drain bridge are now bound in `UiProtocolState`
- tool activity status and raw result detail are preserved in `UiTurnProjection.tool_activities`, but public conversation body prefers semantic target/diff display while outcome stays in the status mapping, including failed tool-result projection and failed terminal projection for still-waiting tool calls
- tool activities now carry structured `display` projection from `tool.display`; public tool cards expose semantic action/target/parameter/diff summaries instead of making UI infer categories from raw tool detail or echo success/failure result text as primary content
- public tool summaries now preserve `tool_call_id`, expose semantic tool display body, and duplicate tool-call projections upsert into one activity before public rendering
- ADP request/response frames are now protocol-owned and JSON roundtrip tested for UI-less automation clients
- `UiTurnProjection.created_at` is landed from `TurnProjectionInput.created_at`
  and covered by `command_to_projection_smoke`
- session cwd projection is landed for `UiTurnProjection`, `UiSessionSummary`, and `UiSessionTranscriptProjection`
- session CRUD protocol routing is bound for create, rename, archive, restore, and delete-as-archive commands through `runtime.ui-command-dispatch` into `reason.persistence`
- rollback latest session turn protocol routing is bound through `RollbackLatestSessionTurn` into `reason.persistence`, and `UiProtocolState::replace_session_turn_projections` lets runtime refresh effective transcript projection without UI-local deletion
- task list/history query DTOs and runtime query-port routing are protocol-bound; `UiProtocolState::query` rejects them so runtime owner code must supply task truth
- Phase 1 TaskBoard, AgentBoard, and AgentLifecycle query DTOs are protocol-bound and route only through runtime-backed query ports
- Phase 1 ApplyExecutionFact and RunSchedulerTick command DTOs are protocol-bound for internal runtime-backed task.orchestration dispatch and are excluded from public ADP command ingress
- task list subscription projection is protocol-bound; runtime owner code publishes task list projection events into `UiProtocolState`
- error-center query/subscription DTOs and runtime query-port routing are protocol-bound; runtime owner code supplies metadata-backed read projections
- config status query/result DTO is protocol-bound; runtime owner code supplies complete provider registry plus selected primary/fallback projection and protocol state rejects local handling
- provider web_search effective status fields and `TestProviderWebSearch` command routing are protocol-bound without adding a local `web_search` function tool
- provider definition upsert and legacy provider/model update command DTOs are protocol-bound, owner-routed to `config.core`, reject invalid/empty fields, and serialize without credential/API-key values
- active provider selection command DTO is protocol-bound, owner-routed to `config.core`, rejects empty provider ids, and serializes without credential/API-key values
- model group config and active selection command DTOs are protocol-bound, owner-routed to `config.core`, reject empty route fields/zero weights, and serialize without credential/API-key values
- Agent resource-count update command DTO is protocol-bound, owner-routed to `config.core`, rejects out-of-range counts, and serializes without provider credentials or live-process state
- task mutation command DTOs are protocol-bound, owner-routed to `task.orchestration`, and rejected at the protocol boundary when required task, worker, execution, or review fields are empty
- Phase 2A worker agent, assignment, claim, review rejection, and dispatch-mode DTOs are landed and locked by protocol owner tests
- Phase 2C worker-control command/query DTOs are landed and locked by protocol owner tests
- Timer dashboard command/query DTOs are landed and locked by protocol owner tests
- Tools dashboard query/result DTOs are landed and locked by `cargo test -p freehand-ui-protocol tool_registry -- --nocapture`; protocol-state local query rejection proves the runtime/tool owner supplies the registry projection
- Search dashboard query/result DTOs are landed and locked by `cargo test -p freehand-ui-protocol session_search -- --nocapture --test-threads=1`; protocol-state local query rejection proves runtime/reason owners supply the search projection
- Diagnostics query/result DTOs are landed and locked by `cargo test -p freehand-ui-protocol diagnostics_query -- --nocapture`; protocol-state local query rejection proves runtime/debug owners supply the diagnostics projection
- context compaction command DTO (`CompactSessionContext`) is protocol-bound, owner-routed to `reason.rewrite-policy`, and validated for non-empty session id; runtime supplies the policy/rewrite outcome and must not fabricate a compaction result
- per-turn usage projection (`UiUsageProjection` on `UiTurnProjection.usage_projection`) carries input/output/reasoning/cache-creation/cache-read tokens, cache-hit-rate bps, context tokens, and compacted-token counters projected from provider usage events; cache-hit-rate bps is cache-read tokens over total input tokens, so uncached input cannot be silently shown as a 100% hit
- the generated wiki must be regenerated from `docs/mainline-calls/ui.protocol.json` when this function-map truth changes

- `accept_query_ingress` is the ADP Query-frame gate: only `UiCommandFrameClass::Query` may enter `handle_adp_query`; mutation frames return `direct_task_mutation_forbidden` before runtime query ports run.
