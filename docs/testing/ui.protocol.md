# Test Design: `ui.protocol`

- feature_id: `ui.protocol`
- owner: `crates/freehand-ui-protocol`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `task.project_to_ui`
  - `input_attachment.validate_submit_metadata`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `task.project_to_ui` | bound | `cargo test -p freehand-ui-protocol -- --nocapture` covers DTO validation, task/session projection filtering, command receipt, and worker-child session projection tests | `cargo test -p freehand-cli master_worker_autonomy -- --nocapture` covers ADP query/subscribe/command protocol smokes for task board, agent board, event inbox, task history, and session list | `make verify-webui-online` covers WebUI/CLI S-profile projection proofs that task truth renders through protocol projections without UI owning task state |
| `input_attachment.validate_submit_metadata` | bound | `cargo test -p freehand-ui-protocol image -- --nocapture` covers image-only submit admission and missing-payload rejection | `cargo test -p freehand-server --lib android -- --nocapture` covers WebUI asset wiring for attachment metadata submit helpers and Android bridge payload fields | `node scripts/verify-webui-image-attachment-online.mjs` proves online WebUI submit sends `SubmitUserInput.metadata.attachments` with image base64 while persisted turn projection keeps metadata only |

- lifecycle path under test:
  - commands enter protocol boundary
  - commands act as ingress only and do not make UI a truth writer
  - command ingress accepts only mutation-intent commands and rejects query-route misuse explicitly
  - accepted command ingress is routed to declared owner feature/module before transport dispatch
  - submit command validation accepts an optional selected session id and optional selected cwd without weakening empty-text rejection
  - submit command validation accepts image-only metadata submits while rejecting malformed attachment metadata before runtime/provider dispatch
  - session management commands are protocol-owned ingress intents: create, rename, archive, restore, delete-as-archive, and rollback route to the session persistence owner instead of mutating WebUI local state
  - latest-active cancellation is accepted as mutation intent and routes to `reason.turn`
  - query returns snapshot truth
  - ADP query frames return the same snapshot truth without requiring WebUI DOM
  - task list/history ADP query frames use protocol-owned commands and UI-safe DTOs while runtime owner code supplies persisted task truth
  - Phase 1 TaskBoard/AgentBoard/AgentLifecycle ADP query frames use protocol-owned commands and UI-safe DTOs while runtime owner code supplies owner truth; task DTOs carry parent/observing session scope, canonical Worker session id, and task-owner `created_at`
  - task mutation ADP command frames use protocol-owned command DTOs while runtime/task owners perform create/create_agent/assign/claim/review/reject/approve/close mutation and persistence
  - Phase 1 ApplyExecutionFact/RunSchedulerTick ADP command frames use protocol-owned command DTOs while runtime/task owners perform execution-fact sync and scheduler fact emission
  - Phase 2B QueryEventInbox/RunMasterPoll ADP frames use protocol-owned DTOs
    while runtime/task owners supply event rows, cursor truth, and
    classifications
  - RunMasterPoll cursor mode is explicit: `replay_from_start=true` is allowed
    only without `after_cursor`; protocol rejects the conflicting mode before
    runtime dispatch
  - Phase 2C WorkerControl/QueryWorkerControl ADP frames use protocol-owned
    DTOs while runtime/worker-control owners supply safe-point control event
    truth and Task Center consequence evidence
  - Timer dashboard QueryTimerList/ScheduleTimer/CancelTimer ADP frames use
    protocol-owned DTOs while runtime/timer owners supply independent timer
    schedule, cancel, ledger, and UI projection truth
  - Tools dashboard QueryToolRegistry ADP frames use protocol-owned DTOs while
    runtime/tool registry owners supply the UI-safe registry rows, schema,
    examples, guidance, execution scopes, and Master/Worker exposure truth
  - task list ADP subscribe frames use protocol-owned subscription shape and receive runtime-supplied task list projections without making protocol state the task truth owner
  - error-center ADP query/subscribe frames use protocol-owned commands and UI-safe DTOs while runtime owner code supplies metadata-backed truth
  - config status ADP query frames use protocol-owned command/result DTOs for complete safe provider registry plus current primary/fallback selection while runtime owner code supplies config.core-backed truth
  - provider definition upsert ADP command frames use protocol-owned DTOs while runtime/config owners perform validation, persistence, and restart-required projection without switching active selection
  - provider selection ADP command frames use protocol-owned DTOs while runtime/config owners validate primary/fallback ids and persist only agent selection
  - model group definition and selection ADP command frames use protocol-owned DTOs while runtime/config owners validate route providers/models, load-balance weights, and active model group selection
  - Agent resource-count ADP command frames carry only non-empty `agent_name` plus `resource_count`; protocol rejects values outside `1..=5` before dispatch
  - debug query returns per-turn read-only debug snapshot truth
  - checkpoint query returns read-only runtime-owned checkpoint summary projections
  - subscribe returns an initial snapshot plus continuous incremental truth, or waits for the first matching turn when subscribing before any turn exists
  - ADP subscribe frames return an explicit accepted/waiting frame and then stream matching subscription events
  - debug subscribe returns per-turn read-only debug projections
  - shared semantic/tool/usage/terminal/error contracts can incrementally update one queryable turn projection inside protocol state
  - turn projections preserve owner-created turn time as optional
    `UiTurnProjection.created_at`; UI clients may format it, but must not
    invent persisted message time when runtime/reason did not supply it
  - provider-request-sent lifecycle projection marks a turn as `Thinking` until response/tool/usage/terminal/error projection arrives
  - completion-schema rejection retry projection marks a turn as `SchemaRetry` and carries only compact retry count plus field issue detail for UI rendering
  - tool-result continuation projection marks a turn as `ToolResultContinuation` so UI clients do not infer continuation waits from completed/failed tool cards
  - selected-session transcript refresh preserves same-turn nonterminal live-only provider/model waiting and tool activity projections already present in protocol state, while terminal refresh clears stale live waiting truth
  - session list active identity is latest-nonterminal-only: a latest terminal turn keeps its terminal status/summary but cannot keep `UiSessionSummary.active_turn_id` populated, and a later nonterminal model wait can become active again
  - tool-call projection stays lifecycle-aware: requested tool calls remain `waiting` until a matching `ReasonReq05ToolResultReentry` marks the activity `completed`, or failed terminal truth marks still-waiting activities `failed`
  - tool display projection is attached to `UiToolActivity` from the `tool.display` parser owner and is preserved in public conversation tool summaries
  - public tool summaries preserve `UiToolActivity.detail` alongside structured display fields, so failed tool results expose the actual owner-projected execution error instead of only tool parameters
  - terminal events preserve both terminal text and terminal status in UI projection
  - debug receiver drain ingests observation-only debug events into protocol state
  - source identity and stream kind stay explicit
  - slave turn presentation divergence remains protocol-safe
  - client-specific projection gating keeps slave card visible only for WebUI
  - public conversation projection preserves the user prompt while hiding reasoning, usage, debug details, raw completion schema blocks, and verbose tool term text from the main user-visible stream
  - session list and transcript queries project the cwd bound to the selected session
  - session list queries project protocol-owned session title and archived state from owner-supplied session metadata
  - cancelled terminal status remains visible to public conversation status mapping and is not projected as completed
  - `ToolPending` terminal status remains visible to public conversation as `Lifecycle` / `running` and is not projected as `Final` / completed
  - public tool summaries carry `tool_call_id` so same-tool waiting/completed updates remain one UI activity
- white-box plan:
  - command/projection mapping, status query, terminal projection, slave subscription semantics
  - command ingress acceptance and rejection mapping
  - command dispatch routing mapping
  - submit command selected-session validation mapping
  - submit command selected-cwd validation and JSON roundtrip mapping
  - submit metadata attachment validation covers image-only submit, missing base64, and unsupported/empty fields
  - session management command validation covers empty title, empty session id, empty cwd, rollback empty session id, and explicit owner-routing to `reason.persistence`
  - session transcript replacement coverage proves runtime can refresh one effective session transcript after rollback without UI-local deletion
  - explicit cancel and latest-active cancel owner-routing mapping
  - checkpoint rewind ingress validation and owner-routing mapping
  - checkpoint projection storage and query mapping
  - client-specific projection gating
  - debug-state query and subscription routing
  - protocol-owned subscription channel fanout
  - incremental turn projection updates from shared contracts
  - turn created-time projection through `TurnProjectionInput.created_at` into
    `UiTurnProjection.created_at`
  - model request waiting projection, typed phase kind, timing-key stability, and response-clear behavior, including transient provider retry/failover transport activity that never becomes a permanent conversation error or separate reasoning-flow phase after recovery
  - session transcript replacement preservation coverage for active provider transport retry/model waiting, active tool cards, and terminal refresh clearing stale live state
  - session summary projection coverage proves `active_turn_id` tracks only the latest nonterminal model/tool activity and clears for terminal latest turns instead of presenting completed work as still active
  - completion-schema mismatch waiting projection with `kind=SchemaRetry` and compact `schema polishing #N: issue` detail
  - tool activity projection from `ReasonReq04ToolCall` plus matching `ReasonReq05ToolResultReentry`
  - structured tool display projection from read/search/write/plan/shell/generic parser output
  - public tool body projection covers failed result detail when structured display is present
  - duplicate same-`tool_call_id` tool-call projection upserts one activity and one public tool card
  - debug-event ingestion and receiver-drain behavior
  - ADP frame serialization and failure-frame shape
  - task query command validation covers empty history id and command-ingress rejection for query-route misuse
  - Phase 1 board/lifecycle query commands cover runtime-route-only behavior and protocol-state mismatch rejection
  - task mutation command validation covers empty task id/title/content/goal/review summary, worker agent id/capabilities, claim execution id, review rejection reason/requirements, and owner-routing to `task.orchestration`
  - Phase 2A task command validation covers `CreateTaskAgent`, `AssignTask`, `ClaimNextTask`, `RejectTaskReview`, and `UiTaskDispatchCommand`
  - Phase 1 execution fact and scheduler tick command validation covers owner-routing to `task.orchestration` and malformed command rejection
  - Phase 2B EventInbox/MasterPoll validation covers owner-routing to
    `task.orchestration`, empty cursor rejection, JSON roundtrip, and
    protocol-state mismatch rejection
  - Phase 2B RunMasterPoll validation covers replay-from-start plus explicit
    cursor conflict rejection
  - Phase 2C WorkerControl validation covers owner-routing to
    `worker.control`, query-route misuse, unknown op rejection, and
    op-specific `question`/`constraint` required fields
  - Timer dashboard validation covers QueryTimerList route separation,
    ScheduleTimer owner-routing, CancelTimer owner-routing, mode-specific
    relative/absolute/recurring fields, repeat validation, and TimerList DTO
    JSON roundtrip without protocol-owned timer persistence
  - Tools dashboard validation covers QueryToolRegistry route separation,
    ToolRegistry DTO JSON roundtrip, protocol-state local rejection, no local
    `web_search` function row, and no protocol-owned tool execution
  - task list subscription selector and matcher cover accepted task list projections and rejection of task query/history misuse on subscribe route
  - error-center query command validation covers empty session id and command-ingress rejection for query-route misuse
  - config status query covers command-ingress rejection, runtime-owned
    protocol-state rejection, ordered multi-peer/provider-registry JSON
    roundtrip, and no-secret DTO serialization
  - provider definition upsert and legacy provider/model update cover owner routing to `config.core`, empty-field rejection, unsupported-protocol rejection, JSON roundtrip, and no credential/API-key value field in serialization
  - provider selection update covers owner routing to `config.core`, empty provider/fallback rejection, JSON roundtrip, and no credential/API-key value field in serialization
  - model group definition and selection cover owner routing to `config.core`, empty group/route rejection, zero load-balance weight rejection, JSON roundtrip, and no credential/API-key value field in serialization
  - provider web_search live-test command covers owner routing to `provider.reason-live-bridge`, empty provider id rejection, optional query validation, and command/query route separation
  - Agent resource-count update covers owner routing to `config.core`, zero/six rejection, JSON roundtrip, and safe status projection of configured count/max/shared provider
  - error-center subscription selector and matcher cover accepted error-center projections
- module black-box plan:
  - command ingress accept/reject smoke
  - selected-session submit command smoke
  - image-only SubmitUserInput metadata command smoke
  - selected-session cwd projection smoke
- session metadata projection smoke covers created empty sessions, renamed sessions, archived sessions being hidden from the active list, and restored sessions becoming visible again
- session list projection smoke covers top-level active/archived lists being metadata-only: created persisted sessions appear even when empty, turn-only sessions do not become global sessions, internal `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` sessions are absent while explicit `QuerySessionTurns` for those ids remains queryable, and `active_turn_id` is present only for the latest nonterminal live/progress turn
  - command dispatch envelope owner-routing smoke
  - latest-turn subscribe, specific-turn query, stream-kind routing through protocol boundary
  - debug-state snapshot/query by `turn_id`
  - debug-state subscription by `turn_id`
  - protocol subscription receiver gets turn/debug updates after state mutation
  - debug-core receiver to queryable protocol-state smoke
  - checkpoint summary query smoke
  - CLI hides slave card while WebUI may render it
  - public conversation projection smoke excludes internal fields and preserves visible text/terminal/tool/error summaries plus user input
  - tool activity projection smoke covers waiting-before-result, completed-after-result, failed-result detail rendering, and failed-terminal-without-result without rendering verbose tool terms into the public summary
- public tool summary smoke covers `display.kind`, semantic action title, parameter summary, target summary, result summary, and no UI-local category guessing
  - duplicate tool-call projection smoke covers one public card per `tool_call_id`
  - cancelled/failed/ToolPending terminal status projection smoke
  - blank latest-turn subscribe waits until a turn exists instead of failing early
  - ADP command/query/subscribe frame roundtrip smoke
  - ADP task list/history query smoke proves the protocol frame can carry task read models supplied by runtime
  - ADP Phase 1 board/lifecycle query smoke proves the protocol frame can carry board and lifecycle read models supplied by runtime
  - ADP task mutation command smoke proves the protocol frame can carry task create/create_agent/assign/claim/review/reject/approve/close mutation intents without protocol-owned task storage
  - ADP Phase 1 execution fact and scheduler tick command smoke proves protocol frames can carry owner-routed foundation mutation intents without protocol-owned task storage
  - ADP Phase 2B EventInbox/MasterPoll smoke proves protocol frames can carry
    owner-routed event cursor and poll projections without protocol-owned task
    event storage
  - ADP Phase 2C WorkerControl smoke proves protocol frames can carry
    owner-routed safe-point control events without protocol-owned control
    ledger storage
  - ADP Timer dashboard smoke proves protocol frames can carry TimerList
    projections plus schedule/cancel mutation intents without protocol-owned
    timer schedule or ledger storage
  - ADP Tools dashboard smoke proves protocol frames can carry ToolRegistry
    projections without protocol-owned registry storage or tool execution
  - `command_to_projection_smoke` asserts turn `created_at` survives protocol
    projection
  - ADP task list subscription smoke proves initial task list projection and subsequent runtime-published task changes use the same subscription channel
  - ADP error-center query smoke proves the protocol frame can carry metadata read models supplied by runtime
  - ADP config status query smoke proves the protocol frame can carry safe config read models supplied by runtime
  - ADP provider/model update command smoke proves the protocol frame can carry restart-required config mutation intent without exposing secrets or allowing query-route misuse
  - ADP model group command smoke proves the protocol frame can carry restart-required config mutation intent and safe group registry projection without exposing secrets or allowing query-route misuse
  - ADP error-center subscription smoke proves initial error-center projection uses the same subscription frame shape
  - ADP query-as-command negative smoke
  - ADP session management negative smoke proves invalid session metadata and rollback commands, including empty cwd/session id, fail explicitly instead of becoming local-only UI state
- project black-box impact:
  - CLI and WebUI consume one protocol truth while rendering different views
  - protocol truth can back a minimal service boundary without duplicating projection logic in apps
  - any UI remains decoupled from reason/debug truth ownership while still acting as the user input port
  - protocol state can back HTTP query and SSE subscribe adapters without app-owned projection duplication
  - protocol state can expose runtime checkpoint summaries without becoming checkpoint recovery truth
  - protocol can expose runtime task query DTOs without becoming task persistence truth
  - protocol can broadcast runtime task list projections without becoming task persistence truth
  - protocol can expose runtime error-center query DTOs without becoming metadata/error truth
  - protocol can expose runtime config status DTOs without becoming config truth or leaking key material
  - protocol can route runtime config mutation intents without becoming config persistence truth or adding a credential value DTO
  - protocol can expose timer dashboard DTOs without becoming timer schedule,
    due-fire, recurrence, or ledger truth
  - protocol can expose Tools dashboard DTOs without becoming tool registry
    truth or adding provider-hosted broad search as a local function tool
  - ADP gives WebUI, Android, CLI, and headless smoke tests one shared control/status protocol
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with protocol owner code and function map updates
- fixtures / replay inputs / runtime evidence paths:
  - UI protocol stream fixtures
  - node status snapshots
  - `~/.freehand/replays/ui`
- known gaps:
  - transport-facing app injection into a real runtime-owned dispatch port is still not landed
  - debug-state contract is minimal and per-turn only for now
- sync status between design and implementation:
  - command/query/subscribe/projection baseline landed
  - submit command optional selected-session id and selected cwd are landed and regression-locked
  - command ingress ack/rejection baseline landed
  - command dispatch envelope routing baseline landed
  - checkpoint rewind command ingress and runtime owner routing are landed
  - checkpoint summary projection/query is code-bound as read-only UI protocol state
  - protocol-owned continuous subscription channel landed
  - incremental turn projection update methods from shared contracts landed
  - typed model request waiting projection is landed and regression-locked for normal thinking, schema retry, and tool-result continuation; provider retry/failover are regression-locked as transport substate on the same model request activity
  - selected-session transcript refresh preserves active provider transport retry/model waiting and active tool activity cards through `replace_session_turn_projections`, and terminal refresh clearing stale live state is regression-locked
  - session list nonterminal-only active identity is regression-locked by `session_list_active_turn_id_tracks_only_nonterminal_turns`
  - minimal per-turn debug-state query/subscribe baseline landed
  - debug receiver-drain bridge from `debug.core` into protocol state landed
  - debug-state snapshot shape now comes from `freehand-debug`
  - client-specific projection gating remains protocol-owned
  - public turn projection is protocol-owned
  - terminal status is now preserved in `UiTurnProjection` and public conversation status mapping
  - tool activity status is now preserved in `UiTurnProjection.tool_activities` and public conversation tool summaries, including failed status for still-waiting tools when terminal truth is failed
  - tool summaries now expose `tool_call_id`, duplicate same-id tool calls are regression-locked to one public card, and completed/failed public tool bodies include tool result detail
  - tool summaries now expose `display` from the `tool.display` owner, and public bodies prefer structured result summaries over raw detail text
  - ADP request/response frames are landed and regression-locked by JSON roundtrip coverage
  - session cwd summary/transcript projection is landed and regression-locked
  - session management command/query projection is implemented for `CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`, and `RollbackLatestSessionTurn` routing through runtime to `reason.persistence`; `CreateSession.cwd` empty-string rejection and rollback empty-session rejection are regression-locked at the protocol boundary
  - task list/history query commands and DTOs are landed; runtime-backed ADP task query is regression-locked in daemon tests
  - Phase 1 TaskBoard/AgentBoard/AgentLifecycle query commands and DTOs are landed and are runtime-route-only
  - Phase 1 ApplyExecutionFact/RunSchedulerTick command DTOs are landed and route to `task.orchestration` through runtime
  - task mutation command DTOs are landed for `CreateTask`, `CreateTaskAgent`, `AssignTask`, `ClaimNextTask`, `SubmitTaskReview`, `RejectTaskReview`, `ApproveTaskReview`, and `CloseTask`; protocol validation rejects empty required fields and runtime owns mutation dispatch
  - Phase 2A task command validation and owner routing are regression-locked by `phase2a_task_commands_validate_and_route_to_task_orchestration` and `phase2a_task_commands_reject_missing_worker_execution_and_review_fields`
  - Phase 2B EventInbox/MasterPoll DTOs are landed and regression-locked by
    `phase2b_event_inbox_and_master_poll_validate_and_route_to_task_orchestration`
  - Phase 2C WorkerControl command/query DTOs are landed and regression-locked
    by `worker_control_command_validates_and_routes_to_worker_control`,
    `worker_control_command_rejects_missing_fields`, and
    `worker_control_adp_roundtrip_carries_projection`
  - task list subscription shape is landed for runtime-backed ADP task push and stays projection-only
  - error-center query/subscription commands and DTOs are landed; runtime-backed ADP error-center query is regression-locked in daemon tests
  - config status query/result DTO is landed; protocol-state rejection and secret-free serialization are regression-locked
  - provider/model update command DTO is landed; owner routing, validation rejection, and secret-free serialization are regression-locked
  - model group config and selection command DTOs are landed; owner routing, validation rejection, and secret-free serialization are regression-locked
  - provider web_search test command DTO is landed and regression-locked by `provider_web_search_test_routes_to_runtime_owner`
  - Timer dashboard command/query DTOs are landed and regression-locked by
    `cargo test -p freehand-ui-protocol timer_ -- --nocapture`
  - Tools dashboard query/result DTOs are landed and regression-locked by
    `cargo test -p freehand-ui-protocol tool_registry -- --nocapture`
