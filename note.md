# note.md

# 2026-07-04 error.center first skeleton
  - user requirement: implement the first `error.center` skeleton with feature/function/test/mainline/wiki truth, classify schema/tool/provider errors, write watermarked metadata decisions, and prevent runtime-local bypass for those paths.
  - owner: `error.center`.
  - implementation:
    - added `ErrorCenterObservedFailure`, `ErrorCenterDecision`, error domain/class/recovery/visibility enums, and `classify_error_center_failure` in `crates/freehand-control`.
    - runtime now writes `error.center` metadata for completion schema rejection, failed tool result, and provider executor failure before repair/re-entry/failure materialization continues.
    - error-center metadata uses writer owner `error.center`, write-node provenance, retry fields, public visibility, owner target, repair fields, and raw hash; raw error text is not written into `error.center` rows.
    - added `docs/function-maps/error.center.md`, `docs/testing/error.center.md`, `docs/mainline-calls/error.center.json`, and generated `docs/wiki/error.center.md`.
    - updated feature map, control/error design truth, and adjacent control/task docs.
  - verified:
    - `cargo test -p freehand-control -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_records_error_center_metadata_for_schema_repair -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing -- --nocapture`
    - `cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p xtask -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`
  - remaining gaps:
    - task/node/UI error policy is not routed through error center yet.
    - ADP query/subscribe projection for error-center metadata is not implemented.
    - WebUI error-center cards are not implemented.
    - full status schema repair loop and selectable user option projection remain pending.

# 2026-07-04 control.center basic status stopHook
  - user requirement: implement the basic status stopHook on the fixed four-hook skeleton, while keeping task dispatch built-in tool lifecycle as a separate review topic.
  - owner: `control.center`.
  - implementation:
    - added `crates/freehand-control` with `parse_control_status_block`, `control_status_rhythm_decision`, and `strip_control_status_block`.
    - runtime live bridge includes status contract guidance before model request, writes `control.center` metadata at the four fixed hook points, and accepts `simple_request=true` status stop without requiring legacy `<freehand_completion>`.
    - UI protocol public projection strips hidden status blocks from assistant and terminal text.
    - docs updated: feature map, function map, test design, control/error design doc, architecture gap registry.
  - current non-goals:
    - no compact `task` action tool yet.
    - no task lifecycle persistence/dispatch yet.
    - no centralized `error.center` yet.
    - no selectable user-option UI projection yet.

# 2026-07-04 task.orchestration persistence skeleton
  - user requirement: land task persistence/lifecycle/memory/startup/recovery design, then start implementation.
  - owner: `task.orchestration`.
  - implementation:
    - added `docs/design/task-orchestration-design.md`, `docs/function-maps/task.orchestration.md`, and `docs/testing/task.orchestration.md`.
    - added `crates/freehand-task` with task ids, task statuses, agent statuses, task snapshots, agent snapshots, append-only ledger events, self-agent bootstrap, create/query/list_agents/query_agent, and runtime memory rebuild on boot.
    - added one built-in `task` tool schema to `freehand-tools`; runtime handles `task` tool calls via `execute_task_tool` and routes to `freehand-task`.
    - first scope supports create with self/auto assignment or WaitingAgent; no real worker execution yet.
  - verified:
    - `cargo fmt --check`
    - `cargo test -p freehand-task`
    - `cargo test -p freehand-tools`
    - `cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-07-04 task.orchestration lifecycle and mainline migration
  - user requirement: each implementation round must include white-box/black-box tests plus mainline caller and function map updates.
  - implementation:
    - added lifecycle methods: append, pause, resume, submit_review, approve, reject, close.
    - added transition validation so close before approval fails.
    - runtime `task` tool supports the lifecycle ops.
    - migrated task.orchestration to machine-readable mainline call source and generated wiki.
    - updated xtask required-file gate list, function map, test design, and design doc.
  - verified:
    - `cargo fmt --check`
    - `cargo test -p freehand-task -- --nocapture`
    - `cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture`
    - `cargo test -p freehand-runtime task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval -- --nocapture`
    - `cargo test -p xtask -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `cargo test --workspace`
    - `cargo clippy --workspace --all-targets -- -D warnings`

# 2026-07-04 task.orchestration lease heartbeat recovery
  - user requirement: continue implementation in rounds with white-box and black-box tests, plus mainline caller and function map updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `TaskLease` persisted under `~/.freehand/state/task-runtime/<agent_id>/leases.json`.
    - `resume_task` now enters `Running` and creates an active lease-backed heartbeat record.
    - added `task(op="heartbeat")` runtime/tool schema bridge.
    - `TaskRuntime::boot` loads leases and conservatively changes `Running` tasks with missing, mismatched, inactive, or expired lease to `Interrupted`.
    - leaving `Running` removes the active lease; heartbeat for non-running tasks is rejected and writes no lease.
    - updated design doc, test design, function map, feature map, machine mainline caller, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 8 passed; covers resume lease creation, heartbeat refresh, expired lease recovery to `Interrupted`, and non-running heartbeat rejection.
    - module black-box: `cargo test -p freehand-runtime task_tool_resume_and_heartbeat_persist_running_lease -- --nocapture` -> 1 passed.
    - existing runtime task black-box: create/query and review lifecycle tests passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - mainline/gate: `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check` passed.
    - full regression: `cargo test --workspace` -> 398 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues.
  - remaining gaps:
    - no real worker execution loop yet.
    - no UI task projection yet.
    - no multi-agent dispatch/agent create-close operations yet.

# 2026-07-04 task.orchestration agent registry lifecycle
  - user requirement: continue execution in implementation rounds with white-box/black-box testing and mainline/function map updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `assign`, `cancel`, `create_agent`, and `close_agent` to the single `task` tool op surface.
    - `TaskRuntime::boot` now loads all persisted agent snapshots, not only the self agent.
    - `create_agent` persists available worker snapshots with declared capabilities.
    - `assign_task` moves `WaitingAgent`/`Created`/`Interrupted` tasks to `Assigned` only when the target agent is available.
    - assigned tasks count as queued work; resume/heartbeat moves work to running count; cancel/review/terminal release assignee state.
    - `close_agent` closes only idle agents and rejects busy/queued/running agents.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 12 passed; covers agent create/recover/close, waiting assign, cancel release/reject resume, and busy-agent close rejection.
    - module black-box: `cargo test -p freehand-runtime task_tool_agent_assign_cancel_close_lifecycle -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for create/query, review lifecycle, and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 403 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution loop yet.
    - no UI task projection yet.
    - multi-agent dispatch still has no real worker process/channel; agent registry lifecycle is only persisted skeleton truth.

# 2026-07-04 task.orchestration priority claim skeleton
  - user requirement: continue task lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `claim_next` to the single `task` tool op surface.
    - added `TaskRuntime::claim_next_task`, which lets an agent claim its highest-priority assigned task into lease-backed `Running`.
    - adjusted assign semantics so an agent can hold multiple queued assigned tasks; assigned work increments queued count, claim/resume moves one queued task into running count.
    - empty queue claim returns an explicit no-task result without mutating task/agent/lease truth.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 14 passed; covers highest-priority claim, running lease creation, queue count, and empty queue no-mutation.
    - module black-box: `cargo test -p freehand-runtime task_tool_claim_next_runs_highest_priority_task -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for agent lifecycle and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 406 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration worker execution record skeleton
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `record_execution` to the single `task` tool op surface.
    - added `TaskRuntime::record_execution`, which writes semantic worker progress only for `Running` tasks.
    - execution records write `TaskExecutionRecorded` events into the task ledger and keep task status `Running`.
    - non-running tasks reject `record_execution` with invalid transition and do not advance event sequence.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 16 passed; covers running progress write/recovery and non-running rejection/no sequence advance.
    - module black-box: `cargo test -p freehand-runtime task_tool_record_execution_requires_running_task -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for claim_next and heartbeat lifecycle passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 409 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration ledger history query
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `history` to the single `task` tool op surface.
    - added `TaskRuntime::task_history`, which reads the append-only task ledger and returns ordered lifecycle events.
    - history for unknown task returns explicit `TaskNotFound`.
    - runtime `task(op="history")` returns task timeline JSON.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 18 passed; covers ordered ledger event history and unknown-task failure.
    - module black-box: `cargo test -p freehand-runtime task_tool_history_returns_ordered_execution_timeline -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for record_execution and claim_next passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 412 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 task.orchestration task list query
  - user requirement: continue task/multi-agent lifecycle implementation in tested rounds with function map and mainline caller updates.
  - owner: `task.orchestration`.
  - implementation:
    - added `list_tasks` to the single `task` tool op surface.
    - added `TaskRuntime::list_tasks`, which returns task snapshots filtered by status and assignee and sorted by priority.
    - runtime `task(op="list_tasks", status, agent_id)` returns queue/UI projection JSON without mutating task truth.
    - updated task design, test design, function map, feature map, mainline caller JSON, and generated wiki.
  - verified:
    - white-box: `cargo test -p freehand-task -- --nocapture` -> 19 passed; covers status/assignee filtering and priority order.
    - module black-box: `cargo test -p freehand-runtime task_tool_list_tasks_filters_queue_projection -- --nocapture` -> 1 passed.
    - existing runtime task black-box tests for history and record_execution passed.
    - tool schema: `cargo test -p freehand-tools -- --nocapture` -> 27 passed.
    - `cargo test -p xtask -- --nocapture` -> 18 passed.
    - `cargo run -p xtask -- mainlines generate`, `mainlines check`, and `gates check` passed.
    - full regression: `cargo test --workspace` -> 414 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no UI task projection yet.
    - no worker debug stream/turn update projection yet.

# 2026-07-04 runtime-backed ADP task query
  - user requirement: continue closing task/multi-agent gaps with real implementation, white-box and black-box verification, function map and mainline caller updates.
  - owners: `ui.protocol`, `runtime.ui-command-dispatch`, app transport callers.
  - implementation:
    - added protocol-owned `QueryTaskList` and `QueryTaskHistory` commands plus UI-safe task list/history DTOs.
    - added `UiRuntimeQueryPort` so app transports stay protocol-only while daemon/runtime can answer owner-backed read-only queries.
    - added `RuntimeCommandDispatcher::query_runtime`, routing task list/history through `TaskRuntime::list_tasks` and `TaskRuntime::task_history` without duplicating task filtering or ledger ordering.
    - wired WebUI/daemon ADP query handling to ask the runtime query port first, then protocol state only when no runtime owner handles the query.
    - added `freehand-cli adp-task-query` for no-UI task list/history verification.
    - updated feature map, function maps, test designs, mainline JSON, and generated wiki for touched features.
  - verified:
    - white-box: `cargo test -p freehand-runtime runtime_query_reads_task_truth_from_task_runtime -- --nocapture`.
    - module black-box: `cargo test -p freehand-daemon daemon_adp_queries_runtime_task_truth -- --nocapture`.
    - target packages: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-server`, `cargo test -p freehand-cli`, `cargo test -p freehand-task`, `cargo test -p freehand-runtime`, `cargo test -p freehand-daemon`.
    - mainline/gates: `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`, `cargo test -p xtask`.
    - online S daemon: `scripts/install-launchd.sh installS`, 4042 health ok, `freehand-cliS adp-smoke`, `freehand-cliS adp-task-query --status waiting_agent` returned `count=0`, and missing history returned `command_dispatch_target_not_found`.
    - full regression: `cargo test --workspace` -> 416 passed; `cargo clippy --workspace --all-targets -- -D warnings` -> no issues; `cargo fmt --check` passed.
  - remaining gaps:
    - no real worker execution process/channel yet.
    - no push subscription for task truth yet; current task visibility is ADP query only.
    - no WebUI visual task management panel yet.

# 2026-07-04 runtime-backed ADP task list subscription
  - user requirement: continue closing task/multi-agent gaps with real implementation and validation.
  - owners: `ui.protocol`, `runtime.ui-command-dispatch`, `app.webui-smoke`, `app.runtime-daemon`, `app.cli-runtime-smoke`.
  - implementation:
    - added protocol-owned `SubscribeTaskList { status, agent_id }`, `UiStreamKind::TaskList`, and `UiProjection::TaskList`.
    - added `UiProtocolState::publish_task_list_projection` so runtime can publish task projections without making UI protocol the task truth owner.
    - ADP subscribe initial snapshot now asks injected `UiRuntimeQueryPort` for task list truth before subscribing.
    - runtime live task tool bridge publishes task list projection after successful task truth mutation ops.
    - added `freehand-cli adp-task-subscribe` for no-UI live task subscription verification.
    - updated function maps, test designs, mainline JSON, and generated wiki for touched features.
  - verified:
    - white-box: `cargo test -p freehand-ui-protocol task_list_subscription_matches_runtime_projection_only -- --nocapture`.
    - runtime white-box: `cargo test -p freehand-runtime runtime_task_tool_mutation_publishes_task_list_projection -- --nocapture`.
    - daemon black-box: `cargo test -p freehand-daemon daemon_adp_subscribes_runtime_task_truth -- --nocapture`.
    - package/full gates: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-runtime`, `cargo test -p freehand-server`, `cargo test -p freehand-daemon`, `cargo test -p freehand-cli`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo run -p xtask -- mainlines generate`, `mainlines check`, `gates check`.
    - online S daemon: `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-task-query --status waiting_agent`, and `freehand-cliS adp-task-subscribe --status waiting_agent` passed.
  - remaining gaps:
    - no WebUI task management panel yet.
    - no real worker execution process/channel yet.
    - task history remains query-only; worker debug stream remains separate future scope.

# 2026-07-04 freehand-framework-loop initialization
  - user requirement: initialize project loop governance according to loop-governance, ask for missing decisions only if needed.
  - owner: `foundation.workspace`.
  - implementation:
    - added `docs/loops/freehand-framework-loop/` with `LOOP.md`, `STATE.md`, `loop-constraints.md`, `loop-budget.md`, `loop-run-log.md`, and `README.md`.
    - loop starts as `L1 report-only`, manual trigger only, with L2/L3 disabled until explicit approval.
    - bound loop governance to feature map, function map, test design, mainline JSON, and generated wiki.
  - verification:
    - `cargo test -p xtask -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - finding:
    - gate rejected a docs-only call-table row as a fake source binding; kept loop docs in mainline prose instead of call-table symbol binding.

# 2026-07-04 development symlink launchd profile
  - user requirement: development validation must not repeatedly reinstall/replace the global release binary or trigger the same macOS permission path; global release mode and development symlink mode must coexist with S-suffixed names.
  - owner: `foundation.workspace`.
  - implementation:
    - added `scripts/install-symlink.sh`, which builds debug host binaries and exposes `freehand-cliS`, `freehand-serverS`, and `freehand-daemonS` as symlinks to `target/debug/*`.
    - `freehand-daemon-launchdS` is installed as a prefix-local wrapper copy instead of a symlink because launchd refused to execute a symlink wrapper with `Operation not permitted`.
    - `scripts/install-launchd.sh installS/restartS` manages `com.freehand.daemonS`, `~/.freehand/daemonS.env`, `127.0.0.1:4042`, and `daemonS.*.log`.
    - existing `install/restart` still manage global `com.freehand.daemon`, `~/.freehand/daemon.env`, and `127.0.0.1:4041`.
    - Makefile adds `install-symlink`, `install-launchdS`, `restart-launchdS`, `uninstall-launchdS`, `launchd-statusS`, and `launchd-logsS`.
  - verification:
    - `scripts/install-launchd.sh installS` created S commands and started `com.freehand.daemonS`.
    - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
    - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> `adp_smoke_ok`.
    - `scripts/install-launchd.sh restartS` restarted only `com.freehand.daemonS`.
    - `curl -4fsS http://127.0.0.1:4041/health` and `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` still passed, proving global service stayed available.
    - `make ci` -> exit 0.
  - durable workflow: normal development online validation should use S mode on `127.0.0.1:4042`; global `127.0.0.1:4041` is for release/promotion closeout.
  - follow-up repair during WebUI verification:
    - launchd became unreliable when executing the S wrapper or the `freehand-daemonS` symlink after repeated debug rebuilds; symptoms were `Operation not permitted` / `getcwd` stderr and a process that printed listening but did not accept 4042 connections.
    - current S profile keeps `freehand-cliS` and `freehand-serverS` as symlinks, but uses `~/.local/bin/freehand-daemonS-bin` as a local debug daemon copy for launchd.
    - plist now starts through `/bin/bash -lc 'cd <repo> && exec <daemonS-bin> serve ...'`, with S env values projected into launchd environment.
    - verified: `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-smoke`, and served/workspace `webui.js` hash match.

# 2026-07-04 WebUI chat bubble / SSE display repair
  - user requirement: render as chat conversation; user right-aligned and visually distinct, assistant left-aligned, tool activity embedded inside assistant card, reasoning italic, normal assistant text regular, SSE refresh supported, semantic tool display, shell command shown/truncated, lifecycle colors blue/green/red.
  - implementation:
    - WebUI render path now emits chat bubbles from `RenderConversation` rows; assistant rows and tool rows share one assistant bubble role surface.
    - `ensureSseTurnSubscription()` consumes latest-turn SSE as a display-refresh mirror and routes events through `setTurnProjection()`.
    - `tool.display` projects ordinary shell command fields and keeps `pwd` semantic instead of exposing raw `command=pwd`.
    - WebUI source arrays and DOM fragments dedupe same `turn_id` + visible card text to prevent live latest-turn/session transcript races from duplicating r2 assistant/final cards.
    - S profile repair was required before browser verification because launchd symlink execution on the external workspace became unreliable.
  - verified:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-blocks -- --nocapture` -> 42 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-launchd.sh installS` with 4042 health, ADP smoke, and served/workspace JS hash match
    - browser evidence captured completed/success and failed selected sessions after click, with selected session pinned, user right alignment, assistant left alignment, semantic shell tool embedded inside assistant, green success, red failure, SSE, no raw completion schema leak, and no duplicate r2 assistant/final card: `artifacts/webui-online/20260704-chat-bubble-sse-4042-click-proof-1783141455456/summary.json`.
    - browser evidence captured current-code running blue tool block plus italic reasoning and SSE: `artifacts/webui-online/20260704-chat-bubble-sse-4042-running-proof-1783141498366/summary.json`.
  - remaining verification gap:
    - freeform provider/tool behavior can still time out independently of WebUI rendering; use ADP sample sessions or already-terminal sessions for deterministic terminal visual proofs.

# 2026-07-01 WebUI submitted input/history disappearance trace
  - user live feedback: after submitting a request, the composer text disappeared and the conversation area showed no user-visible history while the top status still showed live model/tool-result state.
  - correction: clearing the composer after send is acceptable only if the submitted input is immediately preserved in the conversation transcript or pending render projection. Already-observed history must never be removed or hidden by later live status transitions.
  - investigation target: compare raw ADP session transcript with WebUI `RenderConversation` output for `webui-session-20260701131739-e31eb6cf` / `runtime-turn-65`.
  - follow-up user feedback: two real consecutive requests still disappeared. Prior proof only covered a single immediate pending render and did not cover repeated submit / dispatch failure / later refresh lifecycle.
  - live follow-up: screenshot showed visible ADP failure `reason ledger sequence is invalid: expected 338, got 337`. Online 4041 verification found corrupted reason ledger sequence in `~/.freehand/ledgers/reason/master/webui-session-20260701131739-e31eb6cf.jsonl` line 338 and `runtime-session-master.jsonl` line 380. This is runtime persistence truth failure, not a WebUI display-only bug.
  - root cause in code: `ReasonPersistence::persist_row` computed `next_seq` from authoritative cursor before acquiring any session-wide lock, while `append_row_only` only locked the file append. Concurrent same-session writers could both allocate the same next seq, append duplicate/regressed rows, and later block projection/recovery.
  - implementation: added session-scoped reason persistence lock around cursor read -> seq allocation -> ledger append -> snapshot/sidecar refresh; added `concurrent_same_session_writes_allocate_monotonic_sequences` regression.
  - online verification gap: installing this fix caused 4041 bootstrap to fail because the already-corrupted production ledger still blocks restore. Data repair/quarantine of `~/.freehand/ledgers/reason/master/webui-session-20260701131739-e31eb6cf.jsonl` requires explicit authorization.

# 2026-07-01 WebUI selected-session render source trace
  - user issue: continuing a previous conversation and submitting new input left the WebUI visually stale.
  - diagnosis:
    - `renderMessages()` used `state.sessionTurns.length > 0` as a hard switch and ignored `state.turn` whenever a selected session transcript existed.
    - ADP subscription/query updates can deliver the latest same-session turn before the selected transcript is refreshed, so stale `sessionTurns` can hide the new in-flight/completed turn.
    - render state was split across transcript truth and latest-turn truth without one view selector.
  - implementation:
    - added `conversationTurnsForRender()` in `apps/freehand-server/assets/webui.js`.
    - render now merges selected-session transcript with the latest same-session turn before drawing chronological cards.
    - draft empty state remains clean only when there is no latest turn.
  - pending verification:
    - completed
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh` -> full release/install completed
    - `scripts/install-launchd.sh restart`
    - fixed `127.0.0.1:4041` health returned `ok`
    - served WebUI JS hash matched workspace hash `95b46401c605d0adaf78a4a3f85d765f99ce7ebceb92b6623c05c9acaf2fa07a`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - controlled same-session ADP continuation: `webui-render-e2e-1782893413637890000` first query count 1, second query count 2, new turn `runtime-turn-67`
    - real headless Chrome/CDP WebUI continuation on the same session:
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn.png` shows immediate third-turn pending card and cleared input
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn-final.png` shows third turn in `THINKING... 17S`
      - screenshot `artifacts/webui-selected-session-render/20260701-continue/headless-old-session-third-turn-terminal.png` shows final `runtime-turn-68-r9` terminal card in the selected session

# 2026-07-01 WebUI render architecture closeout plan
  - user request: produce a complete implementation plan, land it to docs, then provide a `/goal` prompt.
  - review correction:
    - WebUI main control/status path is ADP WebSocket `/adp`; SSE is compatibility only and should not be treated as the primary next step.
    - the main bug class is render/state/lifecycle coupling, not merely "missing SSE".
    - historical turns must not keep animating after a later live turn appears.
  - plan landed:
    - `docs/goals/webui-render-architecture-closeout-plan.md`
    - plan now explicitly calls for projection -> render model -> view separation, turn-scoped and tool-scoped lifecycle clocks, ADP as the unified path, and real browser + ADP transcript verification.
  - implementation:
    - `apps/freehand-server/assets/webui.js` now builds `RenderConversation` / `RenderTurn` / `RenderRow` before DOM rendering.
    - model wait clocks use `lifecycleClocks` keyed by session/turn/phase/detail; removed the old global `modelRequestStartedAt` state.
    - tool timings are keyed by turn/tool identity; completed/failed tool timings freeze at terminal observation instead of continuing to count in historical cards.
    - `conversationTurnsForRender()` preserves transcript order and appends latest same-session turn instead of sorting by `runtime-turn-*` ordinal, because runtime turn ordinals can reset after restart.
    - current prompt-first submit state remains live `dispatching` until model/tool projection arrives, so the conversation area no longer goes blank while the status strip says dispatching/thinking.
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh` -> completed release/install and installed host binaries to `~/.local/bin`
    - `scripts/install-launchd.sh restart`
    - fixed `127.0.0.1:4041` health returned `ok`
    - served JS hash matched workspace hash `4b999956af46174a99ecc83c6d40307187121b6a7c2a24b91057acec32b52e41`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - Playwright WebUI evidence under `artifacts/webui-render-architecture-closeout/20260701-live/`:
      - `03-current-live-old-static.json/png`: `blockCount=4`, `liveCount=1`, `nonLastLiveCount=0`; only bottom `runtime-turn-65` is live dispatching, historical turns are static.
      - `04-terminal-no-stale-animation.json/png`: `blockCount=5`, `liveCount=0`, `nonLastLiveCount=0`; terminal state has no stale animation, slow bash tool row is completed with frozen elapsed.

# 2026-06-30 ADP multi-round sample closeout
  - user correction: one-round success is not valid evidence; failure sample must complete a continuous multi-round tool loop before reporting success.
  - implementation:
    - `freehand-cli adp-turn-sample --sample failure` now creates an isolated sample session instead of using the shared runtime session.
    - after matching the final projection, the CLI queries `QuerySessionTurns` and requires transcript evidence; failure sample requires `rounds>=2`, at least one unique tool execution, and at least one unique failed tool result.
    - CLI de-duplicates transcript tool counts by `tool_call_id` because final round projections can aggregate earlier tool activity.
    - CLI now fails immediately when the target sample session reaches a system/provider failed terminal instead of waiting for timeout.
    - provider executor failures now materialize `provider_executor_failure` error truth and failed terminal truth before dispatch failure returns, preventing silent active-turn hangs.
    - WebUI inactive text-only/restored turns now render neutral waiting/active state rather than fake streaming animation.
  - live verification:
    - `target/debug/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `adp_turn_sample_ok ... session=cli-adp-sample-failure-1782833766680278000 turn=runtime-turn-69-r2 rounds=2 tool_executions=1 failed_tools=1 ... command_receipt ... reason_live_turn_completed rounds=2 schema_rejections=0 tool_executions=1 ... EXIT:0`
    - ADP/latest-turn for prior run `runtime-turn-68-r4` showed full multi-round completion after schema retry: failed `read_file` tool activity plus final `terminal_status=Success`.
    - metadata ledger for `runtime-trace-66` showed round 1 provider request, failed `read_file`, `ReasonReq05ToolResultReentry`, round 2 provider request with `tool.exchange_count=1`, and `RuntimeLive04TurnClosed` with `bridge.rounds=2`, `bridge.tool_executions=1`, `terminal.status=Success`.
    - provider/system failure negative path observed on `runtime-turn-65-r4`: after schema retries, provider HTTP send failure surfaced as `terminal_status=Failed` with explicit error instead of hanging.
  - verification commands:
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cargo fmt --check` -> passed

# 2026-06-30 WebUI session CRUD and tool card follow-up
  - user correction: WebUI still could not multi-select/delete sessions, so CRUD was not usable despite ADP/session protocol support.
  - user correction: tool/result "merge" must be semantic, not just visual style; successful tool results should update the same execution card status instead of becoming a separate mechanical content item.
  - implementation:
    - sidebar adds visible `session-bulk-toolbar` with selected count, Clear, and Delete
    - session rows now use checkboxes for multi-select and a separate session button for navigation
    - Delete sends ADP `DeleteSession` for every selected session, clears local selection, refreshes `QuerySessionList`/selected transcript
    - server smoke asserts HTML/JS include multi-select/delete controls and ADP `DeleteSession`
  - tool display follow-up:
    - removed old live wait helper path that could render extra waiting cards
    - tool card status bar now owns waiting/completed timing; card body only shows semantic target/result
    - `tool.display` projects `bash pwd` as `Read current working directory` without `command=pwd`
    - `ui.protocol` public tool body now prefers semantic target/diff display for waiting/completed/failed tools; success/failure result text is status/outcome, not primary body content
    - WebUI execution cards now render one execution-cycle card and no longer append `display.result_summary` as another success-result line
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 39 passed
    - `target/debug/xtask mainlines generate/check` and `target/debug/xtask gates check` -> ok
    - `cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon` -> ok, host binaries installed to `~/.local/bin`
    - `scripts/install-launchd.sh restart` restarted fixed `127.0.0.1:4041`
    - fixed-port `/assets/webui.js` contains `turnExecutionCard` / `pendingExecutionCard` and no longer contains `display.result_summary`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - CDP-operated Chrome screenshots:
      - `artifacts/webui-semantic-merge/20260630-page/13-cdp-after-send-immediate.png` shows submit immediately renders one execution card with `dispatching...0s`
      - `artifacts/webui-semantic-merge/20260630-page/14-cdp-after-send-update.png` shows explicit ADP timeout failure and retained composer input for retry
    - full `scripts/install-global.sh` did not complete reliably under this Codex tool session; stale install processes were terminated by exact PID only, and no install/release/cargo residual process remained after cleanup

# 2026-06-30 WebUI final summary and sample-label cleanup
  - user correction:
    - Final card should not render Evidence, Learned, or Completion reason by default; those are debug-only details
    - success card border should be green, failure card border red and smaller
    - bottom demo/sample wording should be removed
  - implementation:
    - WebUI adds `terminalBodyForDisplay`, `terminalSummaryLine`, and `stripDebugTerminalLines`
    - default Final rendering extracts only `Summary:` content; full terminal text is restored only when `Debug details` is enabled
    - topbar exposes `Debug details` toggle with `Debug off/on` state
    - execution card success/failed border colors now follow terminal status
    - visible buttons changed from `Success sample`/`Failure sample` to `Success`/`Failure`, and sample prompt visible prefixes were removed
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo run -p xtask -- mainlines generate/check`
    - `cargo run -p xtask -- gates check`
    - `cargo build --release -p freehand-cli -p freehand-server -p freehand-daemon`
    - installed host binaries to `~/.local/bin`, restarted launchd fixed `127.0.0.1:4041`
    - fixed-port HTML has `Debug details`, `Success`, `Failure`, and no `Success sample`/`Failure sample`
    - fixed-port JS has `terminalBodyForDisplay`, `stripDebugTerminalLines`, `scenario loaded`, `Debug off`, and no `ADP success sample`/`ADP failure sample`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - screenshot `artifacts/webui-semantic-merge/20260630-page/15-final-filtered-current-chrome.png` shows Final only has summary, no Evidence/Learned/Completion reason, and bottom buttons read `Success`/`Failure`
    - `scripts/install-global.sh` completed full Rust/Android release regression and installed matching `~/.local/bin/freehand-daemon`
    - `scripts/install-launchd.sh restart` restarted fixed `127.0.0.1:4041`
    - fixed-port HTML contains `session-bulk-count`, `session-clear-selection-button`, `session-delete-selected-button`
    - fixed-port JS contains `selectedSessionIds`, `deleteSelectedSessions`, `DeleteSession`, `session-selector`, `renderSessionBulkToolbar`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed

# 2026-06-30 fixed daemon port verification
  - occupied launchd service `com.freehand.daemon` stopped first with `launchctl stop gui/$(id -u)/com.freehand.daemon`
  - exact PID `49108` was terminated only after stop did not exit it
  - `scripts/install-launchd.sh install` rebuilt/reinstalled and relaunched the daemon
  - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
  - `~/.local/bin/freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action create --session webui-fixed-4041-check --title 'Fixed 4041 Check' --cwd /Volumes/extension/code/freehand` -> success
  - empty cwd create via `adp-session-manage` failed explicitly as `empty_session_cwd`

# 2026-06-30 WebUI session bulk select-all and typography follow-up
  - user request:
    - add a `Select all` action to the session bulk toolbar
    - small text should not use heavy bold weight
  - user correction after screenshot:
    - `Select all` showed `9 selected` while several visible sessions were still unchecked
    - root cause was `isDraftSessionId(sessionId)` using `sessionId.startsWith("webui-session-")`
    - persisted real sessions also use `webui-session-*`, so bulk select skipped them as if they were draft
  - implementation:
    - page shell now renders `session-select-all-button`
    - WebUI JS selects all non-draft session ids into the existing multi-select set
    - draft status is now only `state.draftSessionId === sessionId`; id prefix is never used
    - bulk toolbar is split into a summary row and a wrapping action row
    - small bulk count/button text is normal weight (`font-weight: 500`) instead of the previous heavy look
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - fixed-port HTML at `127.0.0.1:4041` contains `session-select-all-button`, `session-clear-selection-button`, `session-delete-selected-button`, and `session-bulk-count`
    - fixed-port JS at `127.0.0.1:4041` contains `selectAllSessions`, `sessionSelectAllButton`, `selectedSessionIds`, `draftSessionId: null`, and `state.draftSessionId === sessionId`; it no longer contains `startsWith("webui-session-")`
    - fixed-port CSS at `127.0.0.1:4041` contains `session-bulk-summary`, `session-bulk-actions`, `session-bulk-button.select-all`, and `font-weight: 500`
    - Chrome AppleScript DOM click verification was blocked by Chrome's "Allow JavaScript from Apple Events" setting, so no visual DOM count proof was captured in this slice

# 2026-06-30 WebUI new conversation state and chat visual follow-up
  - user correction:
    - after `New conversation`, sidebar showed both `no sessions` and a draft item, which is an incorrect state projection
    - conversation area still looked like old card UI rather than a chat surface
  - implementation:
    - when `state.sessions.length === 0` and `state.draftSessionId` exists, sidebar renders only the draft row
    - empty transcript now renders a dedicated `chat-empty-state` with title/copy instead of plain text or system-card feel
    - dialogue cards are narrowed to fit-content bubbles, borders/headers are reduced, and title weights are reduced to avoid heavy card UI
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture`
    - full `scripts/install-global.sh` completed workspace tests, mainlines/gates, Android JVM, Rust release binaries, and Android release APK
    - `scripts/install-launchd.sh restart`
    - fixed-port `127.0.0.1:4041` CSS contains `chat-empty-title`, `width: fit-content`, and `border-bottom: 0`
    - fixed-port `127.0.0.1:4041` JS contains `if (state.draftSessionId)` and `Send a message to start this session.`
    - ADP smoke on `ws://127.0.0.1:4041/adp` passed
    - screenshot evidence saved to `artifacts/webui-session-ui-fix/20260630-new-session-chat/02-chrome-after-refresh.png`, but Chrome AppleScript JS permissions still block automated click/DOM-count verification

# 2026-06-29 tool display semantic owner
  - user requirement:
    - tool classification must have a standard independent file and locked owner
    - every parser must be an independent function in an independent module
    - UI must not guess categories; UI only consumes parsed projection
  - implementation:
    - added `tool.display` owner in `crates/freehand-blocks/src/tool_display.rs`
    - added structured `ToolDisplayProjection` with kind/outcome/action/target/parameter_summary/summary/result_summary/fields/diff
    - added independent parser functions for read/list, file mutation, search, plan, shell, and generic tools
    - `ui.protocol` now attaches `UiToolActivity.display` during tool call projection and updates it on tool result projection
    - public tool summaries now prefer structured display action/summary/parameter/result over raw detail
    - WebUI `toolSummaryBody` consumes `display` fields and renders tool parameters/results as secondary grey lines instead of classifying raw tool text
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-blocks`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-cli -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live WebUI verification:
    - global install + launchd fixed-port restart completed
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - real WebUI page operation loaded `Failure sample`, submitted it, and captured screenshots under `artifacts/webui-tool-display-e2e/20260629-parameters/`
    - `03-tool-parameter-visible.png` captured in-flight read_file tool card with `path=definitely-missing-freehand-file.txt`
    - `05-reloaded-final-parameter.png` captured terminal state with `turn completed`, no waiting text, `Read file` card showing parameter and failed result, plus shell command card showing `command=... · timeout=60`
    - ADP truth for `webui-session-20260629044814-5eb78029` showed `runtime-turn-43-r4` Success with `display.parameter_summary=path=definitely-missing-freehand-file.txt`

# 2026-06-29 session CRUD protocol support
  - user requirement:
    - before UI presentation, session CRUD must be supported through shared protocol/runtime truth
    - WebUI must not invent local-only session management state
  - implementation:
    - `ui.protocol` added session management commands: `CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`
    - `UiSessionSummary` and `UiSessionTranscriptProjection` now expose `title` and `archived`
    - `UiProtocolState` can project metadata-only sessions and archived-session list separately
    - `reason.persistence` owns `PersistedSessionMetadataEntry` in `~/.freehand/state/ui/<agent>/session-metadata.json`
    - `delete` is currently non-destructive delete-as-archive because physical deletion of turn truth needs explicit destructive lifecycle approval
    - `runtime.ui-command-dispatch` routes CRUD commands into `ReasonPersistence` and refreshes shared UI projection
    - `freehand-cli adp-session-manage` provides no-UI ADP CRUD control
  - verification:
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 38 passed
    - `cargo test -p freehand-reason -- --nocapture` -> 56 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 51 passed
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live ADP verification:
    - existing fixed daemon `127.0.0.1:4041` health and ADP smoke passed
    - current workspace daemon on `127.0.0.1:4092` health passed
    - `target/debug/freehand-cli adp-session-manage --action create --session adp-crud-session-20260629 --title 'ADP CRUD Session' --cwd /tmp` returned `target=reason.persistence status=session_metadata_updated`
    - rename returned `session_metadata_updated`
    - active query showed `adp-crud-session-20260629:0:empty`
    - archive hid it from active session list
    - restore made it visible again
    - delete hid it again through non-destructive archive semantics
    - unknown archive returned explicit `command_dispatch_target_not_found`
  - remaining risk:
    - WebUI session context menu/UI presentation is not implemented in this slice
    - archived-session list is protocol-supported but CLI query helper currently prints only active list unless extended further
    - 4092 debug daemon health still responded after validation, but process PID was not discoverable through allowed exact-PID lookup; no broad kill was attempted

# 2026-06-29 session cwd owner wiring
  - user requirement:
    - session must have a working directory and WebUI must allow choosing it
    - `/new` should not render system feedback as a chat message
  - implementation:
    - `SubmitUserInput.cwd` is protocol-owned and empty cwd is rejected as `empty_session_cwd`
    - `UiTurnProjection`, `UiSessionSummary`, and `UiSessionTranscriptProjection` expose cwd
    - runtime canonicalizes requested cwd, binds it to the selected session, persists it on `TurnRecord.cwd`, restores it after bootstrap, and inherits it on later same-session submits
    - tool execution uses `freehand-tools::with_workspace_root` so session cwd is passed as an explicit per-call workspace root instead of mutating process-global cwd/env
    - WebUI adds a cwd input, forwards `SubmitUserInput.cwd`, shows cwd in topbar/session metadata, and `/new` now renders a clean empty state instead of the old selected-session/no-turns chat card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-tools -- --nocapture`
    - `cargo test -p freehand-reason -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
  - live WebUI verification:
    - local WebUI smoke server on `127.0.0.1:4088` returned `/health=ok`
    - Playwright real page operation clicked `New session`, set cwd `/Volumes/extension/code/freehand`, verified no `selected session:` system card, submitted a prompt, and observed command receipt/status
    - screenshots:
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/01-initial-cwd-control.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/02-new-session-cwd-clean.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/03-submit-cwd-status.png`

# 2026-06-29 WebUI provider-request wait state repair
  - gap found:
    - submit/dispatch waiting was local WebUI state and did not prove provider request had been built/sent
    - runtime already emitted `RuntimeLive02ProviderRequestBuilt` debug truth, but ADP turn projection did not expose request-sent/model-response-waiting lifecycle state
  - fix:
    - `UiTurnProjection.model_request` carries protocol-owned request-sent/model-response-waiting state
    - `RuntimeLive02ProviderRequestBuilt` is mapped into `UiProtocolState::apply_model_request_waiting`
    - WebUI renders an animated "waiting for model response" card with elapsed time from protocol projection
    - model request wait clears on semantic response, tool call, tool result, usage, terminal, or error projection
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI tool-result and model-wait progress repair
  - gap found:
    - `ui.protocol` intentionally kept public tool summary body status-only, so WebUI could not show tool execution output
    - WebUI only timed submit/dispatch and tool execution waiting; after tool completion while waiting for model continuation, there was no dedicated timed lifecycle card
  - fix:
    - `UiToolActivity.detail` now carries completed/failed tool result detail from `ToolResultContract.output`
    - public tool summaries expose the protocol-projected result detail
    - WebUI tool cards render result detail together with status and elapsed execution time
    - WebUI renders a timed animated "waiting for model" card after completed/failed tool activity until terminal/update arrives
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI lifecycle progress timing repair
  - gap found: tool waiting had animation/timer, but submit/dispatch waiting before first turn projection only had static pending/status text
  - fix:
    - WebUI records `submitStartedAt`
    - pending submit card renders animated running state and elapsed dispatch wait time
    - lifecycle status refreshes once per second for submit/dispatch and tool waiting
    - tool waiting status includes elapsed time in the main status strip as well as the tool card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI assistant-card collapse repair
  - review found a remaining regression after multiround transcript merge: same logical turn still rendered multiple assistant cards because `derivePublicConversation` emitted one `AssistantText` per text chunk
  - fixed by collapsing assistant text inside each turn into one visible card while preserving tool summaries and terminal/error cards
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI multiround restore closeout
  - verified live fixed-port daemon at `http://127.0.0.1:4041/` and `ws://127.0.0.1:4041/adp`
  - real browser evidence now shows one logical transcript item per execution cycle, with `runtime-turn-N` and `runtime-turn-N-rM` merged for display only
  - assistant cards now strip raw `<freehand_completion>...</freehand_completion>` blocks, while the Final card keeps user-facing completion content
  - restart restore now rebuilds UI projections from reason-ledger per-turn snapshots so earlier-round tool activity survives daemon restart
  - verification evidence:
    - `artifacts/webui-visual-session/20260629-multiround-slowtool-success/`
    - `artifacts/webui-visual-session/20260629-final-restore-merged-clean/`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-runtime live_bootstrap_restores_multiround_tool_activity_into_ui_state`

# 2026-06-28 Minimonth config and WebUI alignment goal
  - config check:
    - requested source config `/Volumes/extension/.rcc/provider/minimonth/config.v2.toml` contains provider id `minimonth`, type `anthropic`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and a present API key
    - current runtime config `~/.freehand/config.toml` uses provider id `minimonth`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and the active `master` and `worker` agents both point to `minimonth`
    - Freehand config schema requires explicit `protocol`; RCC `transportBackend` is not a Freehand runtime config field
  - goal doc:
    - added `docs/goals/webui-session-transcript-alignment-plan.md`
    - goal locks WebUI rendering to persisted session truth plus latest ADP overlay, Codex-style low-noise conversation/tool display, and Reasonix-style session restore/history rebuild
  - 2026-06-28 progress:
    - `~/.freehand/config.toml` updated to runtime provider `minimonth` with base URL `https://api.53hk.cn` and model `MiniMax-M2.7`; secret copied from RCC source without printing it
    - `freehand-cli --agent master` verified active provider `minimonth`, protocol `messages`, model `MiniMax-M2.7`, and Minimonth base URL
    - fixed session transcript ordering in `ui.protocol` and WebUI local overlay path so numeric turn ids such as `runtime-turn-10` do not sort before `runtime-turn-2`
    - added `freehand-cli adp-session-query --url ... [--session <id>]` for no-UI session list/transcript validation over ADP
    - WebUI input layer now has shortcut and slash-command affordances routed through existing ADP/query/cancel/sample helpers: `/help`, `/sessions`, `/reload`, `/success`, `/failure`, `/cancel`, `/clear`; shortcuts include Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2
    - targeted verification passed: `node --check apps/freehand-server/assets/webui.js`, `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-cli`, `cargo test -p freehand-server`, `cargo test -p freehand-runtime`
  - 2026-06-29 final smoke:
    - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`
    - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success` returned `adp_turn_sample_ok` with `turn=runtime-turn-41` and `rounds=1`

# 2026-06-28 WebUI conversation/session product repair
  - user correction:
    - current WebUI is not acceptable as a chat product because it behaves like a dashboard/slide surface
    - primary missing product contract: persistent session concept, session management, and refresh recovery
    - status/permission/tool failures must be rendered inside the conversation lifecycle, not as disconnected panels or silent failures
  - implementation direction:
    - add protocol-owned session list and session-turn query truth
    - WebUI restores selected session from localStorage and queries persisted/protocol state after refresh
    - page shell becomes normal chat layout: session list + conversation transcript + composer
    - permissions preflight/failure state will be attached to the same visible turn/status chain after session UI is stabilized
  - DeepSeek-Reasonix reference findings:
    - actual relevant implementation is `/Volumes/extension/code/DeepSeek-Reasonix/desktop`, not `~/code/reasonix`
    - Reasonix restores tabs/session paths on startup, persists session files only on turn completion, and front-end event subscription is live-only
    - front-end rebuilds visible transcript from session history first (`historyMessagesToItems`), then applies live events as updates (`turn_started`, `text`, `tool_dispatch`, `tool_result`, `turn_done`)
    - blocking prompts are explicitly replayed after subscription reconnect (`ReplayPendingPrompts`) so UI never waits silently without a visible pending action
    - Freehand equivalent must be: session/index/transcript is restart truth; ADP is latest live signal; WebUI/Android/CLI render session truth plus ADP deltas, never ADP-only history


# 2026-06-28 live tool failure UI projection repair
  - new real-session failure found after fixed-port/sample validation:
    - WebUI screenshot error was `dispatch port failure: failed to project live error turn from persistence: reason ledger sequence is invalid: expected 380, got 379`
    - real broken path is historical session `~/.freehand/ledgers/reason/master/runtime-session-master.jsonl`, not fresh ADP sample turns
    - current file inspection shows no internal blank lines and no extra trailing bytes beyond newline; `wc -l` > final `seq` came from counting the terminal newline, not an extra JSON row
    - likely real failure mode is restore racing a partially appended final ledger row during live error projection; current `load_reason_ledger` has no explicit "last line incomplete" recovery rule and parses whole file snapshot at once
    - WebUI also renders `adpFailure` before persisted conversation, so transport failure can visually preempt the user message / turn history
  - deeper runtime evidence after launchd restart:
    - historical `runtime-session-master` reason ledger contains old pre-`ToolResultContract.status` rows such as line 6 `tool_result={tool_call_id, output}` with no outer status field
    - historical metadata ledger `~/.freehand/ledgers/metadata/master/runtime-session-master.jsonl` line 495 contains two JSON objects concatenated on one physical line
    - metadata loader assumed one JSON object per line and metadata append had no file lock, so launchd bootstrap could fail on `trailing characters`

# 2026-06-28 live tool failure UI projection repair
  - root cause:
    - live bridge tool execution failure used to return `RuntimeLiveBridgeError::ToolExecutionFailed` before materializing failed turn truth, so protocol truth stayed active/non-terminal and WebUI could only show waiting
    - first repair wrote failed truth to persistence but runtime dispatch `Err` branch still skipped UI projection; fixed by refreshing `UiProtocolState` from authoritative persistence before returning dispatch failure
    - second live validation exposed a history-pollution bug: dispatch failure projection was aggregating all restored session turns; fixed by projecting only the current runtime-turn ordinal
    - failed terminal projection still left waiting tool activities as waiting; fixed `ui.protocol` to mark still-waiting tool activities `Failed`
  - locked by tests:
    - `live_bridge_fails_explicitly_on_unknown_tool_name`
    - `live_bridge_fails_explicitly_on_registered_unimplemented_tool_name`
    - `live_dispatch_projects_failed_tool_turn_into_ui_state` now covers consecutive failures without historical tool leakage
    - `failed_terminal_marks_waiting_tool_activity_failed`
  - validation passed:
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-runtime`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - fixed-port daemon `http://127.0.0.1:4041/health` -> 200 `ok`
    - real command ingress with `ls path=~/code/codex` -> HTTP 500 explicit `command_dispatch_port_failure`
    - latest-turn query for `runtime-turn-17` -> `terminal_status=Failed`, one current `tool_activities[0].status=Failed`, terminal/error public cards failed
    - latest-turn SSE emitted same failed turn projection

# 2026-06-28 low-noise tool card rendering
  - UI truth gap:
    - protocol projected tool cards still carried verbose generic wording like `Tool call requested` / `Tool result returned`
    - user only needs core tool semantics, blocking state, elapsed waiting time, and success/failure outcome
  - direction:
    - keep semantic tool identity in the shared protocol projection
    - render tool cards as a single updating card per `tool_call_id`
    - let WebUI add local elapsed-time animation for waiting cards instead of exposing raw term/detail in the main stream

# 2026-06-28 launchd fixed-port daemon bootstrap root cause
  - root cause:
    - `freehand-daemon serve --agent master` uses `RuntimeCommandDispatcher::from_default_config()`
    - that path requires `HOME` to resolve `~/.freehand/config.toml`
    - launchd environment did not provide `HOME`, so daemon bootstrap failed before bind even though the process itself remained alive briefly
  - fix direction:
    - launchd install must inject explicit `HOME` into both `daemon.env` and plist environment

# 2026-06-28 launchd restart readiness closeout
  - observed failure mode:
    - `scripts/install-launchd.sh restart` returned before the daemon was actually ready for `GET /health`
    - immediate curl after restart could fail even though launchd had already kicked the service
  - root cause:
    - startup window race, not a fixed-port or ADP protocol regression
  - fix:
    - `scripts/install-launchd.sh install` and `scripts/install-launchd.sh restart` now wait for `/health` readiness before reporting success
  - validation:
    - `scripts/install-launchd.sh restart`
    - bounded poll reached `health_ready_after=2`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok ... subscription_accepted ... query_result ... ingress_command_kind_mismatch`

# 2026-06-28 ADP success/failure sample closeout
  - added CLI/headless sample command:
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success`
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure`
  - WebUI composer now has `Success sample` and `Failure sample` buttons that load the same prompts; actual submit still uses normal ADP command path
  - verification:
    - `cargo test -p freehand-cli` -> 11 passed
    - `cargo test -p freehand-server` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci` -> EXIT 0
    - `scripts/install-global.sh` -> EXIT 0
    - `scripts/install-launchd.sh restart`
    - installed fixed-port success sample -> `runtime-turn-21`, `terminal_status=Success`
    - installed fixed-port failure sample -> `runtime-turn-22`, `terminal_status=Failed`

# 2026-06-28 WebUI tool card/status repair
  - root cause: whole-turn projection could duplicate same `tool_call_id` as separate waiting activities, and WebUI rendered tool summaries as static cards without stable tool identity
  - fix:
    - `ui.protocol` now upserts duplicate tool calls by `tool_call_id`
    - public tool summaries carry `tool_call_id`
    - WebUI normalizes same-tool cards by `tool_call_id`, adds waiting animation, clears composer immediately on submit, and routes command status through one renderer
  - validation passed:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
    - live local smoke: `freehand-server webui-serve-smoke --bind 127.0.0.1:4062`, `curl /` returned WebUI shell containing composer/status elements

# 2026-06-28 review/launchd/ui projection closeout
  - verified after fixes:
    - runtime final multi-round projection now aggregates only cross-round `tool_calls` and `tool_results`
    - final visible text / usage / errors / terminal status come from the final round
    - WebUI debug 404 now renders as `debug pending`; SSE transport errors now render as `debug stream reconnecting`
    - launchd wrapper requires explicit `FREEHAND_DAEMON_BIN` and fails on prefix mismatch instead of silently running an old binary
  - validation passed:
    - `cargo test -p freehand-runtime`
    - `cargo test -p freehand-server`
    - `cargo test -p xtask`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`

- 2026-06-27 launchd global daemon install closeout
  - added service scripts:
    - `scripts/freehand-daemon-launchd.sh`
    - `scripts/install-launchd.sh`
    - `scripts/uninstall-launchd.sh`
  - real install executed: `scripts/install-launchd.sh` exit 0
  - installed real commands:
    - `~/.local/bin/freehand-cli`
    - `~/.local/bin/freehand-server`
    - `~/.local/bin/freehand-daemon`
    - `~/.local/bin/freehand-daemon-launchd`
  - LaunchAgent installed:
    - label `com.freehand.daemon`
    - plist `~/Library/LaunchAgents/com.freehand.daemon.plist`
    - env `~/.freehand/daemon.env` mode 0600
    - logs `~/.freehand/logs/daemon.stdout.log` and `~/.freehand/logs/daemon.stderr.log`
    - fixed WebUI `http://127.0.0.1:4041/`
    - `RunAtLoad=true`, `KeepAlive=true`
  - verified active daemon:
    - launchctl showed `pid = 55614`, then exact PID killed to verify KeepAlive
    - launchd restarted it as `pid = 65923`, `runs = 3`
    - `curl /health` -> 200 `ok`
    - `curl /` -> 200, 5040-byte Freehand WebUI HTML
  - stdout log contains `freehand-daemon listening on http://127.0.0.1:4041`
  - permission note: localhost bind needs no macOS Accessibility/Full Disk permission; changing bind to LAN/Tailscale may trigger one-time firewall prompt
  - reinstall behavior note: `scripts/install-launchd.sh install` intentionally re-copies host binaries via `scripts/install-global.sh`, so repeated reinstall can make macOS re-evaluate the daemon binary; ordinary restarts must use `scripts/install-launchd.sh restart` and should not rewrite install state

- 2026-06-27 release/global-install/daemon startup closeout
  - added release truth: `scripts/release.sh` runs `make ci`, Android JVM tests, Rust release binaries, Android release APK, and artifact staging
  - added global install truth: `scripts/install-global.sh` installs `freehand-cli`, `freehand-server`, `freehand-daemon` to `${FREEHAND_PREFIX:-$HOME/.local}/bin`
  - Android `assembleRelease` repeatedly hung/failed in Lint Vital (`lintVitalAnalyzeRelease`, `lintVitalRelease` missing intermediate files); fixed at config owner by `lint { checkReleaseBuilds = false }` in `apps/freehand-android/app/build.gradle.kts`
  - release script uses Gradle `--no-daemon` to avoid persistent release-script child process leakage
  - verified release script exit 0 with staged artifacts: `freehand-cli`, `freehand-server`, `freehand-daemon`, `freehand-android-release-unsigned.apk`
  - verified install-global exit 0 with temp `FREEHAND_PREFIX`; installed binaries executable
  - verified installed daemon startup with temp `~/.freehand/config.toml`: `freehand-daemon serve --agent master --bind 127.0.0.1:4059`, `/health` 200 `ok`, `/` 200 with WebUI HTML
  - config smoke lesson: first local topology requires paired agents to resolve the same pair token value; separate env names with different values fail bootstrap explicitly

- 2026-06-27 android-client doc alignment pass
  - current truth: Android scaffold already exists under `apps/freehand-android`
  - live render host: `apps/freehand-android/app/src/main/assets/bridge.html`
  - design preview: `apps/freehand-server/assets/mocks/android/mobile-mock.html`
  - plan update: align design / execution / testing docs to the real native shell + protocol-only bridge split

- 2026-06-24T08:00+08:00 android-client execution plan locked
  - reviewed: `apps/freehand-android/` (existing scaffold) vs `apps/freehand-server/assets/mocks/android/mobile-mock.html` (locked design)
  - gap: WebView loads crude `mobile-shell.html`; no SSE; `TimelineProjector` only handles a tiny subset
  - plan doc: `docs/design/android-client-v1-android-shell.md`
  - execution order:
    1. bundle mobile-mock.html+css into Android assets; flip WebView loadUrl
    2. add SSE subscribe to ProtocolClient
    3. expand TimelineProjector to full ui.protocol mapping
    4. JS bridge: snapshot from projector -> window.__freehand.applySnapshot
    5. wire native controllers to projector state
    6. command ingress + cancel via existing CommandIngress
    7. theme: dark/light via Android night mode
    8. local.properties for SDK path
    9. compile + adb install
    10. run integration smoke against running freehand-daemon
  - hard constraints unchanged: no direct reason/provider/node imports; only ui.protocol consumer + command ingress

## 2026-06-24T11:35:57.426Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T193523692-397984-479
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: Android 客户端 milestone 已完成：APK 编译通过，SSE 协议客户端+WebView 渲染壳+native 控制器全部就位，freehand-daemon SSE 路由验证通过。剩余 applySnapshot JS 函数需在 HTML 中补齐（下一步）。
- evidence: 1. assembleDebug BUILD SUCCESSFUL, app-debug.apk 6.4M 2. curl http://127.0.0.1:4040/ui/subscribe/turn/latest → 200 text/event-stream 3. SseEventStream.kt, TimelineProjector.kt, MainActivity.kt, HostConfig.kt 全部代码已就位

OkHttp 4.12 SSE 需要 okhttp-sse 单独 artifact；Android buildFeatures 没有 webView flag；HostConfig URL 必须与 freehand-server 真实路由对齐（latest-active-turn 不是 turn/latest）；emulator 在 exec_command PTY 退出后会被杀，无法在当前环境维持长跑

## 2026-06-24T13:04:05.537Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T210335356-398243-738
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB device 100.104.163.65:5555 connection refused; pairing code expired. Need new pairing code from device.
- evidence: adb connect 100.104.163.65:5555 -> Connection refused; adb pair -> protocol fault

Gradle 9.6 incompatible with AGP 8.2.2; must pin Gradle 8.7 via wrapper. ADB pairing codes expire quickly.

## 2026-06-24T13:40:03.410Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T213913294-398413-908
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB 100.104.163.65:5555 connection refused, device wireless debug likely off. Need user to re-enable. Meanwhile fixing Gradle build issue (Gradle 9.6 incompatible with AGP 8.2.2, reverting to Gradle 8.7). Also need to sync all code changes, rebuild APK, then install once device is online.
- evidence: nc -zv 100.104.163.65 5555 -> Connection refused. adb connect 100.104.163.65:5555 -> failed. Device PLZ110 was previously connected via adb pair but connection lost after adb server restart.

ADB wireless debug ports expire; always need fresh connect. Gradle 9.6 incompatible with AGP 8.2.2.

## 2026-06-25T00:05:00+08:00 android-client tailscale-first closeout

- Android app no longer ships demo/mobile-shell assets as runtime truth.
- removed dead assets:
  - `apps/freehand-android/app/src/main/assets/mobile-mock.html`
  - `apps/freehand-android/app/src/main/assets/mobile-shell.html`
  - `apps/freehand-android/app/src/main/res/layout/activity_main.xml`
- runtime UI narrowed:
  - native side keeps the single input bar
  - drawer keeps only connection settings
  - fake session / agent quick-switch actions removed
- connection truth changed to tailscale-first:
  - bundled config host = `100.66.1.82`
  - bundled profile = `tailscale-main`
  - upgrade URLs switched to `100.66.1.82:4040`
  - `autoLanScan = false`
  - `HostStore.DEFAULT_HOST = 100.66.1.82`
  - old persisted localhost / `192.168.*` values are overridden by `MainActivity.selectPreferredHost()`
- build truth:
  - stale gradle wrapper lock at `~/.gradle/wrapper/dists/gradle-8.7-bin/.../gradle-8.7-bin.zip.lck`
  - removed stale lock, downloaded gradle 8.7 successfully
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- install truth:
  - `adb -s 100.104.163.65:5555 install -r .../app-debug.apk` -> Success
- packaged config truth:
  - `unzip -p apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk assets/config/client.json`
  - confirms host `100.66.1.82`, profile `tailscale-main`, `autoLanScan=false`
- remaining runtime evidence gap:
  - device screenshots are still covered by lockscreen / black overlay
  - `dumpsys window` shows `com.freehand.android/.ui.MainActivity` as focused app/task, but screenshot evidence is not user-visible UI yet
  - this blocks final visual acceptance, not code/build/install truth

## 2026-06-24T23:59:36.327Z stopless learned

- requestId: openai-responses-orangeai.key1-glm-5.2-20260625T075914215-399715-2210
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: 闪退根因已修，app 已不闪退。但截图仍然是黑屏（设备锁屏层覆盖），且 daemon 未运行在 Tailscale 地址上，Android 连不上 SSE。下一步：启动 daemon 绑 0.0.0.0:4040，再截图验证 bridge.html 渲染。
- evidence: 1. SecurityException crash stacktrace: DaemonDiscovery.getWifiIp -> WifiManager.getConnectionInfo -> ACCESS_WIFI_STATE permission missing. 2. 新 APK 已重装并验证不闪退: pid=26255 alive, focusedApp=com.freehand.android. 3. APK 内 config 确认 host=100.66.1.82 profile=tailscale-main. 4. daemon 未运行在 0.0.0.0:4040，设备 SSE 连接返回空. 5. 截图 18KB 黑屏是设备锁屏层覆盖不是 app 问题.

DaemonDiscovery 的 scanLan 路径在 autoLanScan=false 时仍然可以通过 health check 失败 fallback 触发。必须在 discover() 入口就拦截，不能依赖 config flag。另外 Android 新安装的 APK 需要重新 install，不能假设 build 产物已部署。

## 2026-06-25T13:08:00+08:00 android-client protocol and daemon truth closed

- Android bundled daemon truth is now unified to `100.66.1.82:4041`.
- Removed dead Android discovery owner:
  - deleted `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonDiscovery.kt`
- Fixed Android command ingress protocol shape:
  - old wrong payload: `{"type":"SubmitUserInput","text":"..."}`
  - canonical payload now matches `UiCommand` serde external-tag form:
    - `{"SubmitUserInput":{"text":"..."}}`
    - `{"CancelLatestActiveTurn":{}}`
- Fixed old persisted-host override gap:
  - same Tailscale host + legacy port `4040` now upgrades to bundled `4041`
- Runtime truth verified on real daemon process:
  - `env FREEHAND_PAIR_TOKEN_SHARED=devpair target/debug/freehand-daemon serve --agent master --bind 127.0.0.1:4041`
  - `curl http://127.0.0.1:4041/health` -> `200 ok`
  - `curl http://127.0.0.1:4041/ui/query/latest-active-turn` after submit returns submitted turn projection
  - `curl -sN http://127.0.0.1:4041/ui/subscribe/turn/latest` emits canonical `event: turn`
  - submitted prompt `reply with one short sentence and valid freehand completion schema` completed with `terminal_status=Success`
- Android build truth reverified:
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- Remaining device-side blocker:
  - `adb connect 100.104.163.65:5555` -> `failed to authenticate`
  - TCP `5555` is reachable, but host cannot currently reinstall APK or capture fresh runtime logs until device re-authorizes ADB.

Current real root cause split:
- earlier `connected + daemon unreachable` was app-side premature connected-state mutation plus wrong port collision (`4040` hitting `fin`)
- current Android command failure root cause was protocol payload mismatch, now fixed

## 2026-06-26 数据/控制 分离审计 + MetadataKind 死变体清理

### 审计结论（见上）

### 死变体清理 - 已完成 commit 5eae53e

### provider adapter error 接入 metadata 中心化 - 已完成 commit e4542f7

用户指令：Gap 2 closure path 第 1-2 步。

设计决策：
- provider adapter crates 保持 protocol-only，不加 freehand-metadata 依赖
- metadata 写入发生在 runtime bridge 的 executor 错误返回路径（单次 + 流式）
- 新 pipeline node: `RuntimeLive05ProviderError`（MetadataKind::Provider）
- 新 helper: `record_provider_error_metadata` + `emit_provider_error_debug`
- 白盒测试: HTTP 500 → metadata ledger 写入 RuntimeLive05ProviderError

当前 gap 状态：
- provider error ✓（RuntimeLive05ProviderError）
- 请求构造成功路径 — 依赖 RuntimeLive02ProviderRequestBuilt（已有）
- 响应解析成功路径 — 依赖 raw capture callback（已有可观测性）
- OpenAI executor — 当前无 executor，未来接入时复用 RuntimeLive05ProviderError 模式

用户指令：物理删除 `MetadataKind::Control` + `MetadataKind::DebugLink`（两个变体生产代码 0 次使用）。

用户要求：审计当前"推理与请求响应生命周期"中数据链 vs metadata 控制流的隔离状态。
范围：只读 audit。无代码改动。

### A. 核心结构已就位

- `crates/freehand-metadata/src/lib.rs` 是 metadata 唯一 owner
  - `MetadataCenter` (in-memory) + `MetadataLedger` (durable JSONL)
  - `MetadataWriteOwner` / `MetadataWriteNode` / `MetadataSubject` / `MetadataEntry` / `MetadataEnvelope` / `MetadataKind`
  - `validate_metadata_envelope` 强制 owner/node/subject + 拒绝 request-like key (`request.*`/`payload.*`/`prompt.*`/`input.*`/`content`/`text`/`messages` 等)
  - `is_reserved_request_key` 在 rust 字符串层做白名单，是元数据与请求数据硬隔离的第一道闸
- `crates/freehand-debug/src/lib.rs` 是 debug 唯一 owner
  - `DebugHub` + 3 类 sink (Memory / Stdout / File JSONL / Replay)
  - 独立 `DebugObservationFailure` 流 (`DebugHub::subscribe_failures`)
  - 观测-only，禁止承载请求内容
- `crates/freehand-contracts/src/lib.rs` 持有请求节点类型 (`ReasonReq01..05`, `ReasonResp01..03`, `ErrorErr01`)
- 静态 gate: `xtask/src/main.rs::verify_data_control_boundaries`
  - 拒绝 `ReasonReq*` 携带 metadata/debug/control 字段或类型
  - 拒绝 metadata owner struct 携带 request payload 字段
  - 拒绝 metadata owner struct 携带 control execution payload (cancel token / retry / checkpoint / route policy / gate decision)
  - 拒绝 `Metadata*` 类型出现在 `crates/freehand-metadata` 之外
  - 红测在 `cargo test -p xtask`

### B. 中心化元数据写入路径（已落实）

- 单例 ledger 路径: `~/.freehand/ledgers/metadata/<agent_id>/<session_id>.jsonl`
  - 由 `crates/freehand-runtime/src/lib.rs::metadata_ledger_path` 唯一生成
  - 没有第二处拼路径的代码（`metadata_ledger_path` 仅在 `freehand-runtime` 内出现）
- 唯一写入 helper: `write_live_bridge_metadata` (in `freehand-runtime`)
  - 构造 `MetadataWriteOwner`（`feature_id="provider.reason-live-bridge"`, `crate_name="freehand-runtime"`, `symbol_path` 由 spec 传入）
  - 构造 `MetadataWriteNode`（`pipeline_node` 显式标注：`RuntimeLive01RestoreResolved` / `RuntimeLive02ProviderRequestBuilt` / `RuntimeLive03ToolExecuted` / `RuntimeLive04TurnClosed`）
  - 入参是 `RuntimeMetadataWriteSpec`，不是裸 string/JSON
- 已经接入 metadata 中心化的 producer（按写入次数统计）：
  - `freehand-reason` (`ReasonTurnEngine::write_metadata`): 2 处 (`start_turn` + `apply_provider_output`)
  - `freehand-runtime` (`write_live_bridge_metadata`): 5 个 pipeline_node 节点
  - `freehand-node` (`LocalNodeRuntime::write_metadata`): 6 个节点
- debug producer 与 metadata 中心化 producer 数量级相同：
  - `freehand-reason` (`emit_debug`): 14 处生命周期点
  - `freehand-runtime` (`emit_live_bridge_debug`): 5 个 pipeline_node 节点
  - `freehand-node` (`emit_debug`): 6 个节点
- metadata write failure 在所有三个 producer 都是显式错误（`MetadataWriteFailed` / `NodeRuntimeError::MetadataWriteFailed` / `RuntimeLiveBridgeError::MetadataFailed`），无 fallback 吞错

### C. 隔离现状 — 已锁住

- `MetadataKind` 在生产代码只使用 4 个变体：
  - `RuntimeState` 10 次
  - `Routing` 5 次
  - `Cache` 4 次
  - `Provider` 2 次
  - `Control` 0 次
  - `DebugLink` 0 次（声明了但无生产 producer，参见 F）
- `metadata_ledger_path` 是 metadata 持久化的唯一真源路径生成点
- `MetadataCenter::by_trace` 是当前唯一的 metadata 查询接口（按 trace_id 反查）
- 测试覆盖：metadata ledger append/reload、corrupt ledger reject、validation-failed ledger reject、metadata write failure 不污染 turn truth（reason/node 两边都锁）

### D. 已记录的 gap（`docs/architecture/architecture-gaps.md`）

- Gap 2 明确：`metadata.core` 的 provider/debug producer 未全覆盖
  - 未接 producer：`freehand-provider-anthropic` / `freehand-provider-openai` / `freehand-debug`
  - 状态：非违规，gate 不会拦，closure path 已写在 gap 文件里

### E. 用户原则对照

1. "数据链与 metadata 控制流要分离" — 已落实：metadata 中心独立 crate + 静态 gate + 类型级禁止（`is_reserved_request_key` + `xtask::is_forbidden_request_field_*`）
2. "metadata 需要统一中心管理，不能零散写" — 已落实：唯一 ledger 路径 = `metadata_ledger_path`；唯一 helper = `write_live_bridge_metadata`；3 个 producer 都使用同一中心；没有第二份 metadata owner struct
3. "需要写入记录" — 已落实：ledger append-only JSONL 持久化 + `load_records` 回放 + `by_trace` 查询 + 静态 metadata/request gate 把"想散写"的尝试拦在编译期

### F. 待办（更新于死变体删除 + Gap 1/2 closure 后）

1. ~~**`MetadataKind::DebugLink` 与 `Control` 是死变体** — 已物理删除（commit 5eae53e）~~
2. **provider 成功路径 metadata 写入**（Gap 2 剩余项）— RuntimeLive02ProviderRequestBuilt 前补充请求构造验证 metadata / 响应解析成功路径。当前已覆盖 error 路径，成功路径有 raw capture 兜底。优先级低。
3. **`MetadataCenter` 查询接口单一**（`by_trace` 之外）— 没有 `by_owner` / `by_kind` / `by_node` 维度。当前审计只能 grep `MetadataKind::`，多 producer 写入的可观测性受限于 trace_id 单一维度
4. **MetadataCenter 是 `Mutex<MetadataCenter>` 形式持有** — 写入串行化。`freehand-runtime` 多处持有同一个 `Arc<Mutex<MetadataCenter>>`，并发 producer 写入需要锁
5. **`verify_data_control_boundaries` 静态扫描只覆盖 `freehand-contracts` 的 `ReasonReq*`** — 不扫描 `ReasonReq*` 之外的请求节点（如 `ReasonReq04ToolCall`/`ReasonReq05ToolResultReentry`/`ReasonResp01..03`/`ErrorErr01`），不扫 `freehand-ui-protocol` 里的 `UiCommand` 是否携带 metadata/debug 字段。已知受限范围（gate 文件注释里没写）

## 2026-06-26T04:49:25.671Z stopless learned

- requestId: openai-responses-minimax.key1-MiniMax-M3-20260626T124912992-402594-347
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: MetadataKind 死变体 Control + DebugLink 物理删除并验证完成
- evidence: git commit 5eae53e；make ci EXIT 0；enum 验证 grep -A 5 只剩 4 个变体 (Routing/Provider/Cache/RuntimeState)

死变体必须按 hard rule 10 物理删除并同步 docs+gate+tests+memory，不能靠注释保留；本次 make ci 一次通过验证 enum + gate + docs sync 是闭环的

## 2026-06-26T05:18:48.331Z stopless learned

- requestId: openai-responses-mimo.key2-mimo-v2.5-20260626T131837059-403051-804
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: 两轮 commit 均完成，make ci EXIT 0。第一轮物理删除 MetadataKind 死变体 Control+DebugLink；第二轮 provider adapter error 接入 metadata 中心化（RuntimeLive05ProviderError）。docs/function-map/mainline-test/gap 同步更新。
- evidence: git log: 5eae53e (delete dead variants) + e4542f7 (provider error metadata)。make ci EXIT 0。cargo test: 324 passed。白盒测试 live_bridge_writes_provider_error_metadata_on_executor_failure 验证 HTTP 500 → ledger 写入 RuntimeLive05ProviderError

1. metadata 与 debug 保持物理隔离比通过 DebugLink 变体交叉引用更干净 2. provider adapter 保持 protocol-only 不加 metadata 依赖，metadata 写入在 runtime bridge 错误返回路径 3. RuntimeLive05ProviderError 作为 provider error 的唯一 metadata 入口可复用于未来 OpenAI executor

## 2026-06-27 UI status/tool SSE repair

- Root cause: WebUI treated `/ui/query/debug/{turn}` 404 as command failure while turn SSE can arrive before debug snapshot; debug subscribe also returned 404, so late debug could not arrive over SSE. Existing tests only covered debug-present query/SSE, not turn-before-debug race.
- Fix path: `app.webui-smoke` keeps debug HTTP query snapshot-only but makes debug SSE wait for late snapshots; WebUI renders missing debug as pending instead of command failure.
- Tool lifecycle gap: `UiTurnProjection.tool_calls` only carried names, so WebUI could only guess running. Added `UiToolActivity` plus `apply_tool_result`; `reason.turn` broadcasts `ReasonBroadcastEvent::ToolResult`; runtime maps it into UI state so latest-turn SSE carries waiting -> completed updates.
- Locked by tests: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-server`, `cargo test -p freehand-reason`, `cargo test -p freehand-runtime`; mainline docs regenerated.

## 2026-06-28 ADP daemon control/status

- User direction: WebUI, Android, CLI, and headless automation should share ADP for status/query/control, so failures can be inspected through ADP instead of UI-specific guessing.
- Implemented `UiAdpRequest`/`UiAdpResponse` in `ui.protocol` and `/adp` WebSocket in shared server transport; daemon exposes it on fixed launchd port through the existing injected runtime dispatcher and shared `UiProtocolState`.
- Debug finding: ADP subscription must return explicit `SubscriptionAccepted`, otherwise clients cannot distinguish waiting from a dead connection; command dispatch also must not block the connection loop, or subscription status events can starve behind long provider work.
- Verified: `daemon_adp_websocket_controls_command_query_and_subscription`, `daemon_adp_rejects_query_sent_as_command_frame`, full `make ci`, global install, launchd restart, and real Node WebSocket smoke against `ws://127.0.0.1:4041/adp`.
2026-06-28: 生成 ADP unified UI closeout 计划文档 `docs/goals/adp-unified-ui-closeout-plan.md`，目标是把 WebUI / Android / CLI-headless 收口到 daemon `/adp` 统一控制面，HTTP/SSE 仅保留兼容路径，并同步补齐固定端口、后台守护、自动化验证与 docs/mainline/wiki 真源。

2026-06-28: WebUI default ADP slice landed locally. `page.rs` adds `data-adp-endpoint="/adp"`; `webui.js` now opens one ADP WebSocket and routes query/subscribe/command through `UiAdpResponse` frames. Removed default `fetch`/`EventSource` usage from WebUI live path; HTTP/SSE/POST routes remain compatibility surfaces. Verified `node --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`, local page/JS smoke on `127.0.0.1:4073`, and local WebSocket smoke on `ws://127.0.0.1:4074/adp` with accepted/event/query/failure frames.

2026-06-28: Android default ADP slice landed locally. `MainActivity` now wires `AdpEventStream` as the default live shell transport; `HostConfig.adpUrl` / `ClientConfig.adpPath` make `/adp` explicit; `TimelineProjector.applyAdp` consumes ADP query/subscription/failure frames and projects failure visibly to `bridge.html`. `ProtocolClient` and `SseEventStream` remain compatibility classes, not default shell path. Verified Android JVM tests, `xtask mainlines generate/check`, `xtask gates check`, and `cargo test -p freehand-server android_mock_route_returns_design_preview`.

2026-06-28: CLI/headless ADP smoke landed locally. `freehand-cli adp-smoke --url ws://.../adp` uses typed `UiAdpRequest/UiAdpResponse`, sends subscribe/query/query-as-command frames, and requires accepted/event/query plus explicit `ingress_command_kind_mismatch`. Verified `cargo test -p freehand-cli` with local mock WebSocket server and a real local `freehand-server webui-serve-smoke` `/adp` smoke.

## 2026-06-28 WebUI shortcuts slash closeout

- Live failure ADP sample re-run passed on fixed daemon: runtime-turn-32-r2, rounds=2, tool_executions=1, terminal_status=Success, read_file tool_activity status=Failed; proves tool execution failure is model-visible result, not system dispatch failure.
- WebUI JS contains shortcutHelp, keydown handlers for Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2; slash commands /help /sessions /reload /success /failure /cancel /clear are present and server asset smoke locks them.

- After reinstall/restart, fixed-port served JS hash matched workspace hash 8b8df0fa84b37ec7c7802ca8ce5d7c88a2859ab2c7370e3655f68230f5195379.
- Full install-global passed, launchd pid 27507 healthy on 127.0.0.1:4041.
- Live ADP smoke passed. Sequential failure sample passed as runtime-turn-33-r2 with rounds=2/tool_executions=1. Sequential success sample passed as runtime-turn-35 with terminal_status=Success.
- Verification caution: running success/failure samples concurrently can produce command_dispatch_port_failure because runtime dispatch has a single active turn boundary; do not treat parallel sample verification as valid positive evidence.
- Found and fixed WebUI slash UX bug: liveTurnStatus always overrode local commandStatus, so /help and /sessions looked inert on completed turns; slash inputs also remained in composer. Added sticky command status and slash input consumption.

## 2026-06-28 goal completion audit continuation

- Objective file re-read: `/Users/fanzhang/.codex/attachments/098e982a-74ac-494e-8e66-6ebb387506f0/pasted-text-1.txt`.
- Minimax config current evidence:
  - RCC source `/Volumes/extension/.rcc/provider/minimax/config.v2.toml` declares providerId/id `minimax`, `type=anthropic`, `baseURL=https://api.minimaxi.com/anthropic`, `defaultModel=MiniMax-M3`.
  - Runtime truth `~/.freehand/config.toml` matches `provider=minimax`, `protocol=messages`, `defaultModel=MiniMax-M3`, and master/worker agents use `provider = "minimax"`.
  - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli --agent master` and source `cargo run -p freehand-cli -- --agent master` both printed `provider=minimax provider_protocol=messages default_model=MiniMax-M3 base_url=https://api.minimaxi.com/anthropic`.
- Fixed daemon evidence:
  - `launchctl print gui/$(id -u)/com.freehand.daemon` showed `state = running`, pid `27507`, `keepalive | runatload`.
  - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`.
- Missing screenshot evidence added under `artifacts/webui-session-alignment/20260628-continued/`:
  - `12-before-daemon-restart-history.png`: pre-restart selected session/history state.
  - `13-after-daemon-restart-session-restored.png`: service-scoped launchd restart + reload restored `runtime-session-master` and prior latest turn `runtime-turn-36-r2`.
  - `14-webui-bash-submit-cleared-pending.png`: WebUI submit cleared composer and showed pending/dispatching state.
  - `15-webui-tool-waiting-animation.png`: WebUI showed tool waiting/running state during `bash sleep 8`.
  - `16-webui-tool-completed-after-wait.png`: WebUI showed completed tool turn after wait sample.
- Explicit gate evidence:
  - `make ci` rerun with log `/tmp/freehand-make-ci.log`, tail showed `xtask mainlines check: ok`, `xtask gates check: ok`, and `MAKE_CI_EXIT=0`.
- Current post-restart headless ADP evidence:
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed with `subscription_accepted`, `subscription_event`, `query_result`, and `ingress_command_kind_mismatch`.
  - Sequential success sample passed: `runtime-turn-38`, `rounds=1`, `tool_executions=0`.
  - Sequential failure sample passed: `runtime-turn-39-r2`, `rounds=2`, `tool_executions=1`.
  - `adp-session-query --session runtime-session-master` returned 36 ordered turns through `runtime-turn-39-r2`.
- Latest projection for `runtime-turn-39-r2` has `terminal_status=Success` and one `read_file` tool activity with `status=Failed`.

2026-06-29: Current `/new` session issue is a render-state bug, not an ADP transport bug. The draft session can be created, but empty `QuerySessionTurns` results must not clear `draftSessionId`, and the main transcript should prefer the selected session over the global latest turn. Also avoid timestamp-only draft IDs because repeated `/new` can collide inside the same second.

2026-06-29: WebUI layering contract clarified. ADP / `ui.protocol` stays stable; UI expands only in control and presentation layers. Session attachments must be session-scoped, history must use placeholders only, and draft attachments must clear on success while surviving failure for retry. New durable docs: `docs/design/webui-layered-controls-and-attachments.md` and `docs/goals/webui-layered-controls-attachments-plan.md`.

2026-06-29: WebUI layered controls first implementation landed. Controls: attach file/image/video, preview, selected-session refresh, read-only model selector. Attachment draft metadata is session-scoped in localStorage, current page `File` handles are kept for retry, restored metadata is marked metadata-only, and submitted text gets placeholder lines only. Success clears draft after ADP command receipt; ADP timeout/dispatch failure restores composer text and retains attachment draft. Evidence: `node --check`, `cargo test -p freehand-server`, `cargo test -p freehand-ui-protocol`, `xtask mainlines generate/check`, `xtask gates check`, `make ci MAKE_CI_EXIT=0`, ADP smoke on `4079`, screenshots under `artifacts/webui-layered-controls-e2e/20260629-layered-attachments-success-v2/` and `...failure-v2/`.

2026-06-29: WebUI selected-session reload evidence added under `artifacts/webui-layered-controls-e2e/20260629-layered-session-reload-v2/`; before and after reload both showed `strip-session=session-webui-smoke` and `conversation=turn-webui-smoke`.

2026-06-29: WebUI tool rendering收口继续压缩。当前 live 结论：历史 completed turn 不再闪，tool 终态只用 compact color dot 表达成功/失败；正文只保留一条核心语义线，参数或 diff 已经足够时不再重复 result summary。真实 daemon `127.0.0.1:4102` 验证成功，failure sample 截图见 `artifacts/webui-state-render-fix/20260629-compact-tool-display-live-v2/01-live-failure-sample.png`，计数为 `wait_model_continue_count=0`, `compact_tool_state_count=1`, `tool_block_count=1`, `running_state_count=0`.

2026-06-30: WebUI 新 session 工作目录选择修复。根因：cwd 协议链已存在，但 WebUI 只有 composer 底部手输 cwd，`/new`/New session 没有明确的新 session workspace 选择动作，并且无 cwd 时会静默创建 draft。修复：session rail 新增 `Workspace directory` + `Use for new session`，`/new` 和 New session 都要求先选择 cwd，draft submit 无 cwd 会显式阻断。验证：`node --check`, `cargo fmt --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`；真实页面 `127.0.0.1:4103` 截图在 `artifacts/webui-cwd-session-e2e/20260630-new-session-workspace/`，负向 `no_cwd_status=new session requires a workspace directory`，正向 `strip_cwd=/Volumes/extension/code/freehand`，提交后 `strip_turn=runtime-turn-43` 且 cwd 保持绑定。

2026-06-30: 用户纠正 session/workspace 设计分层。正确方向：拆成基础 Agent 层和应用执行框架层。基础 Agent 负责 cwd 持久化、session 目录、全局 workspace 命名、权限、启动/生命周期、global/worker 工作模式、中断恢复；session 目录形如 `~/.freehand/sessions/local/<absolute-path-slash-to-minus>/<uuid>/`。应用层负责 WebUI/master agent 接收用户命令、理解任务、派发 worker、汇总结果；正常流程用户只对 master/global agent 对话，worker 由 master 派遣，WebUI 直连 worker 仅作为 debug 模式。此前 `afb046e` 强制 WebUI new session 必填 cwd 是应用层误修，应后续 forward-fix 删除/替换。

2026-06-30: WebUI forward-fix direction now locked: sidebar must show `New conversation` and `New task` as separate actions. `New conversation`/`/new` is global conversation and must not require cwd. `New task`/`/task` requires a visible task target cwd and uses existing ADP `CreateSession` metadata path for cwd-bound task session creation. Do not claim master-worker task dispatch is complete from this UI slice alone.

2026-06-30: Completion schema rejection feedback was too weak for non-string fields. Root cause: completion parsing treated arrays/objects in optional text fields as missing, so `evidence: []` or `{}` surfaced as `is required` instead of telling the model to emit a plain string. Fix: `crates/freehand-blocks/src/lib.rs` now reports explicit type errors for non-string completion fields, and guidance now says required text fields must be plain strings. Verified by `cargo test -p freehand-blocks -- --nocapture` and `cargo test -p freehand-reason -- --nocapture`.

2026-06-30: Completion schema retry transparency repair. Root cause: schema rejection/retry was only runtime/ledger truth, so WebUI showed generic waiting rather than schema retry state. Fix: runtime publishes `CompletionSchemaRejected`, `ui.protocol` projects it through `apply_completion_schema_retry_waiting`, and WebUI renders compact `schema retry #N: <field issue>` detail in the same turn card with elapsed timing. Verification: `cargo test -p freehand-blocks`, `cargo test -p freehand-reason`, `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-runtime`, `cargo test -p freehand-server`, `node --check apps/freehand-server/assets/webui.js`, `xtask mainlines generate/check`, `xtask gates check`, fixed-port `/health`, ADP smoke, and live ADP subscription captured schema retry detail.

2026-06-30: Tool-result continuation wait and composer history repair. Root cause: WebUI inferred waiting-for-model from completed/failed tool cards, so the animation was fake and disappeared when projections changed; dispatch failure also refilled composer text. Fix: runtime publishes `ModelContinuationWaiting` after tool results are paired for the next provider request, WebUI renders only protocol `model_request` waits, the fake `turnIsWaitingForModel` path is removed, composer stays cleared after submit/failure, and Up/Down recalls local input history.

# 2026-07-01 live reasoning state/UI round rendering repair
  - user live feedback: schema retry state sticks and can override later tool/model phases; timers should start from submit/client dispatch and every real phase must animate/time.
  - user live feedback: runtime appears to schema-reject during tool-use/incomplete-tool phases; schema retry must only run when provider normalized finish reason is stop/end_turn, and consecutive stop/end_turn rejections count only across terminal candidates.
  - user UI correction: WebUI must not merge the whole user request, all rounds, all tools, and final summary into one card. Each provider round/tool execution lifecycle should render as its own chronological card that grows downward; final summary belongs at the end, not visually above execution history.
  - implementation:
    - runtime now selects latest unexecuted tool calls per id and returns incomplete tool_use as a failed tool result re-entry instead of schema retry
    - completion schema parse/retry is gated by terminal-candidate finish reason (`stop` / `end_turn` style)
    - consecutive schema retry counter resets on tool execution / non-schema continuation
    - `UiModelRequestActivity.kind` distinguishes `Thinking`, `SchemaRetry`, and `ToolResultContinuation`
    - WebUI model wait timing is keyed by turn + typed phase + detail, so schema retry cannot stick after phase changes
    - WebUI renders chronological per-round cards; later/superseded rounds hide duplicate user prompt and show `continued`; final summary stays in the final row at the bottom
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 41 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 52 passed
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate/check`
    - `cargo run -p xtask -- gates check`
    - fixed-port install/restart: release build, install to `~/.local/bin`, `scripts/install-launchd.sh restart`, `curl http://127.0.0.1:4041/health` -> ok
    - live ADP sample: `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `rounds=2 tool_executions=1 failed_tools=1 schema_rejections=0`
    - screenshot: `artifacts/webui-reasoning-state/20260701-round-cards/04-fixed-4041-round-sequence-tall.png`

2026-07-01 latest WebUI/live regression trace:
- Latest user session `webui-session-20260701041619-de431b82` did not actually terminal-fail through schema rejection; reason ledger showed `runtime-turn-66-r10` ended with provider `finish_reason=max_tokens`, `schema_rejections=0`, and runtime incorrectly closed it as failed with `Provider ended without a completion-schema candidate: max_tokens`.
- Last-card merge root cause is runtime projection, not CSS: `project_runtime_turn_history` aggregated all same-ordinal round tool calls/results into the final round projection, and restore grouped runtime rounds by ordinal before applying one UI projection. This violates one-round/one-card.
- Forward fix direction: schema retry exhaustion must not be `Failed`; use non-failed terminal truth (`Blocked`). Provider interruption/non-candidate such as `max_tokens` must be `Interrupted`. Runtime/UI projection must keep each `runtime-turn-N[-rM]` as its own chronological card and remove WebUI `logicalExecutionKey` / `__supersededRound` grouping.
- Follow-up lock: schema repair must close both sides, not just status labels. Runtime now tests that invalid completion schema feedback is sent back to the model in the next provider request with concrete missing fields (`completion_reason`, `evidence`, `learned`), and runtime dispatch UI-state tests prove clients can query `SchemaRetry` with retry index plus missing field summaries before the repair round completes.

# 2026-07-03 WebUI online validation after Android bridge review
  - user direction: skip Android tests; verify WebUI first.
  - fixed-port service was already healthy on `127.0.0.1:4041`; workspace WebUI asset hash initially matched served asset.
  - browser automation against real WebUI found runtime JS error after submit: `modelRequestPhase is not defined`. `node --check apps/freehand-server/assets/webui.js` did not catch this because the symbol was syntactically valid but undefined at runtime.
  - implementation: added `modelRequestPhase(turn)` beside `modelRequestKind` / `modelRequestLabel`, mapping typed `model_request.kind` to `thinking`, `schema_retry`, or `tool_result_continuation`; added server asset smoke assertions for the helper definition and call site.
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo build --release -p freehand-server -p freehand-daemon -p freehand-cli`
    - installed release binaries to `~/.local/bin`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - served JS hash matched workspace hash `faab159c8376736ea66fd64c9041298aca9ff0a11e13c5cda4c948ea2135b00f`
    - Playwright real WebUI submit evidence under `artifacts/webui-online/20260703-webui-after-model-phase-fix/`
    - DOM after submit had two completed execution blocks, zero live/running blocks, composer cleared, submitted prompt visible, and no `modelRequestPhase` error; remaining console error was favicon 404 only
    - `~/.local/bin/freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp --session cli-adp-sample-success-1782953474447457000` -> `turns=2`, `turn_ids=runtime-turn-1,runtime-turn-6`, session status success
  - reusable validation rule: WebUI lifecycle/helper edits need browser console capture in addition to `node --check`; syntax check alone cannot prove runtime helper binding.

# 2026-07-04 dirty-tree closeout verification
  - resumed from dirty tree containing Android bridge multi-turn projection, runtime/reason terminal status and persistence sequence-lock repairs, CLI ADP transcript evidence repair, WebUI `modelRequestPhase` helper, docs/mainline/test-design updates, skill and memory updates.
  - mapped owners touched: `app.android-client`, `app.webui-smoke`, `provider.reason-live-bridge`, `reason.persistence`, `runtime.ui-command-dispatch`, `ui.protocol`, `reason.turn`.
  - local verification passed:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-reason -- --nocapture` -> 57 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 56 passed
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 41 passed
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cd apps/freehand-android && ./gradlew testDebugUnitTest` -> build successful
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
  - fixed-port install/online verification passed:
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp`
    - `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success` -> `session=cli-adp-sample-success-1783100519808659000`, `turn=runtime-turn-1`, `rounds=1`, `tool_executions=0`, `failed_tools=0`
    - `~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` -> `session=cli-adp-sample-failure-1783100523482624000`, `turn=runtime-turn-2-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`
  - browser evidence:
    - live submit path saved `artifacts/webui-online/20260704-closeout-live-submit/01-before-submit.png`, `02-after-submit.png`, `03-after-wait.png`, `04-terminal-after-wait.png`, `browser-state.json`, `terminal-browser-state.json`
    - live browser submit proved prompt `webui closeout browser proof 1783101772921` became visible, composer cleared, and no page errors occurred; favicon 404 was the only console error.
    - same live browser prompt remained `waiting_model` after later ADP query, so it is not terminal proof and must not be used as completion evidence.
    - terminal sample proof saved `artifacts/webui-online/20260704-closeout-terminal-samples/cli-adp-sample-success-1783100519808659000.png`, `cli-adp-sample-failure-1783043566126808000.png`, and `terminal-samples-browser-state.json`
    - terminal sample browser states showed both selected sessions visible, composer cleared, `liveCount=0`, and completed final cards.
    - ADP query confirmed terminal sample sessions: success `cli-adp-sample-success-1783100519808659000` had `turns=1`, `turn_ids=runtime-turn-1`; failure `cli-adp-sample-failure-1783043566126808000` had `turns=1`, `turn_ids=runtime-turn-1-r2`.
  - remaining risk:
    - live freeform browser prompt `cli-adp-sample-failure-1783100523482624000` stayed `waiting_model`; not part of the passed terminal sample proof.
    - Android live APK/WebView was unit/release-built but not device-installed in this closeout.

# 2026-07-04 new-session lifecycle E2E test
  - user acceptance focus: input history recall + append, full transcript, client/provider/tool lifecycle timing and animation, semantic tool result projection, one turn card per lifecycle, no merge, and color semantics where success is green, failed is red, running is blue.
  - route/owner surface: `app.webui-smoke`, `runtime.ui-command-dispatch`, `provider.reason-live-bridge`, `reason.persistence`, `ui.protocol`, `tool.display`.
  - setup:
    - service-scoped restart via `scripts/install-launchd.sh restart`
    - fixed health `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - browser artifacts: `artifacts/webui-online/20260704-new-session-lifecycle-e2e-1783130843077/`
  - new WebUI session: `webui-session-20260704020727-d1b9081f`
  - marker: `fh-e2e-1783130848405`
  - flow evidence:
    - `01-new-conversation-clicked`: new draft session visible.
    - `03-first-after-submit`: first prompt immediately visible, composer cleared, `runtime-turn-1`, `liveCount=1`, dispatching timer visible.
    - `05-first-terminal`: first turn completed, marker visible, `liveCount=0`.
    - `06-second-recalled-and-appended`: ArrowUp recalled first input and composer appended `SECOND_LAYER_CONTEXT`.
    - `07-second-after-submit`: second prompt visible as a separate turn, first turn still visible, `liveCount=1`.
    - `09-second-terminal`: second turn completed, both marker and appended phrase visible.
    - `11-tool-after-submit` / `12-tool-running-00`: tool turn submitted and running state captured.
    - `13-tool-terminal`: bash/pwd tool result projected semantically as `Read current working directory` and `current workspace`, not raw JSON.
    - `15-failure-after-submit` / `16-failure-tool-state-00` / `17-failure-continuation-01`: missing-file read tool failure projected semantically with path, then `thinking after tool result... 0s` continuation captured.
    - `18-failure-terminal`: final continuation turn completed successfully; tool failure did not become command failure.
    - `19-after-refresh`: browser refresh preserved 6 cards and full history.
    - `20-after-daemon-restart-restore`: daemon restart + browser restore preserved `cardCount=6`, `liveCount=0`, marker, second-layer text, and missing path.
  - ADP evidence:
    - `~/.local/bin/freehand-cli adp-session-query --url ws://127.0.0.1:4041/adp --session webui-session-20260704020727-d1b9081f`
    - returned `selected_session=webui-session-20260704020727-d1b9081f`, `turns=6`, `turn_ids=runtime-turn-1,runtime-turn-2,runtime-turn-3,runtime-turn-3-r2,runtime-turn-4,runtime-turn-4-r2`, session status `success`.
  - passed:
    - new session was independent.
    - input history recall + append worked.
    - first and second user inputs stayed visible in order.
    - refresh and daemon restart restored same 6-turn transcript.
    - tool result projection was semantic for pwd and missing read_file.
    - tool failure returned to model and final state succeeded.
    - no browser `pageerror`; only favicon 404 console error observed.
  - failed acceptance:
    - running card color is not blue. Captured running cards have `className=dialog-block execution-block running-state` but `borderLeftColor=rgba(31, 108, 88, 0.44)` and pulse CSS is amber, not blue.
    - completed success card computed `borderLeftColor=rgb(31, 33, 30)`, not a green frame in computed CSS.
    - tool-execution precursor turns (`runtime-turn-3`, `runtime-turn-4`) restore as `pending-state` / `WAITING` after continuation terminal, leaving 2 pending cards in a fully terminal transcript (`20-after-daemon-restart-restore`: `successCards=4`, `pendingCards=2`, `failedCards=0`, `runningCards=0`).
    - no red turn card was produced for tool-failure precursor card; the failed tool row was visible, but the owning turn card stayed pending/waiting and final continuation card was success.
  - conclusion: lifecycle/content/history proof is mostly closed, but color semantics and non-terminal precursor-card lifecycle projection do not meet the user's acceptance.

# 2026-07-04 lifecycle color + restart continuation repair
  - implementation:
    - WebUI inactive tool precursor cards now derive lifecycle from protocol tool status: completed/success tools render as success cards, failed tools render as failed cards.
    - WebUI execution cards now have explicit state borders: running blue `rgb(47, 111, 237)`, success green `rgb(23, 107, 85)`, failed red `rgb(178, 72, 62)`.
    - Runtime live bootstrap now initializes `next_turn_ordinal` from the maximum persisted `runtime-turn-N` ordinal across all sessions, not only the default runtime session.
  - regression lock:
    - `live_restore_resumes_turn_ordinal_from_selected_non_default_session` creates a non-default WebUI-style session, restarts runtime dispatch, submits again to that session, and requires `runtime-turn-1`, `runtime-turn-1-r2`, `runtime-turn-2`, `runtime-turn-2-r2` without ID reuse.
  - validation:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-runtime live_restore_resumes_turn_ordinal_from_selected_non_default_session -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
  - online evidence:
    - initial color restore proof: `artifacts/webui-online/20260704-lifecycle-color-fix-1783132581/summary.json`
    - full clean-session proof: `artifacts/webui-online/20260704-full-fix-e2e-1783133697/summary.json`
    - full proof session: `webui-session-20260704025459-e136d862`, marker `fh-fix-1783133697681`
    - refresh after 4 logical requests: `cardCount=6`, `successCards=5`, `failedCards=1`, `pendingCards=0`, `liveCount=0`, marker/SECOND_LAYER_CONTEXT/missing path/pwd semantic all present.
    - daemon restart restore: same `cardCount=6`, `successCards=5`, `failedCards=1`, `pendingCards=0`, `liveCount=0`; failed `read_file` precursor card red `rgb(178, 72, 62)`, completed `pwd` precursor card green `rgb(23, 107, 85)`.
    - running captures for second, pwd, and failed-tool submits all blue `rgb(47, 111, 237)` with live animation.
    - post-restart continuation plus second restart: final restore `cardCount=7`, `successCards=6`, `failedCards=1`, `pendingCards=0`, `liveCount=0`, latest `runtime-turn-11`.
    - ADP truth: `webui-session-20260704025459-e136d862:7:success`, `turn_ids=runtime-turn-7,runtime-turn-8,runtime-turn-9,runtime-turn-9-r2,runtime-turn-10,runtime-turn-10-r2,runtime-turn-11`.
# 2026-07-04 WebUI duplicate/sample-button follow-up

- owner: `app.webui-smoke`
- user issue: selected-session chat view showed repeated assistant/final-looking cards and persistent `Success` / `Failure` composer buttons.
- implementation:
  - removed persistent sample buttons from `render_webui_smoke`, DOM bindings, click handlers, and CSS.
  - kept success/failure diagnostics only through `/success`, `/failure`, and keyboard shortcuts.
  - hardened `uniqueChatFragments()` with adjacent assistant visible-text de-duplication so duplicate assistant/final fragments from transcript/latest-turn refresh races do not render twice.
  - updated `app.webui-smoke` function map and test design to lock "no persistent sample buttons".
- verification:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `scripts/install-launchd.sh installS`, `curl -4fsS http://127.0.0.1:4042/health`, `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
  - served/workspace JS hash matched `fc95d9906aa760d16735c23711735498a6a710ba6e138e13a95b60a07b805ef5`
  - Playwright artifact `artifacts/webui-online/20260704-duplicate-buttons-followup-1783144225304/summary.json`: no `success-sample-button`/`failure-sample-button`, no visible `Success`/`Failure` buttons, no adjacent duplicate assistant cards in the reported cross-workspace session.
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- follow-up gap for user item 3:
  - current framework has session cwd binding and `SendDirectMessageToSlave`, plus `LocalNodeRuntime::delegate_task` status projection, but no protocol-owned "create task and dispatch to subagent for outside-workspace target" command/lifecycle.
  - current cross-workspace path failure therefore reaches provider/tool blocked/error semantics instead of a framework routing decision.

# 2026-07-04 agent-framework task-control correction

- user correction: Freehand is an agent framework, so the framework must remain passive. It defines status fields, prompt instructions, schemas, tags, retry policy, built-in action tools, and state transitions; the model returns explicit status fields and calls explicit tools; the framework starts/stops/switches flows only from accepted status/action truth, not from guessed request semantics.
- design correction:
  - cross-workspace/task/subagent routing must be model-status-plus-tool driven, not runtime NLP/path guessing.
  - status schema should live inside a hard invisible block such as `<<<freehand_status>>>...<<<\/freehand_status>>>`.
  - UI/public projection strips the status block from display.
  - incomplete/bad model schema triggers repair/retry through schema feedback; framework may structurally normalize compatible formats but must not infer missing task semantics.
  - task side effects use compact built-in tools, preferably one `task` tool with operation arguments; framework executes validated action-tool calls and reports status back through protocol truth.
- follow-up correction: task/subagent status and action signals are metadata/control truth, not data payload. Parsed status blocks and task tool calls must write to `metadata.core` / control center with explicit watermark provenance before any task lifecycle mutation. Control signals must not be passed through request/user-visible data chains or embedded into task input. Every write needs writer owner, pipeline node, schema version, status/action, source model/agent/turn, timestamp, validation status, and error/retry trace so bad control decisions can be audited and replayed.
- follow-up correction: flow rhythm and errors need explicit centers. New design doc `docs/design/control-error-center-refactor.md` and architecture gap entry define planned `control.center`, `error.center`, and `task.orchestration`; local runtime/provider/tool retry/fail/block decisions must move behind a centralized metadata-watermarked error policy before the task/subagent refactor.
- follow-up correction: model feedback has two channels. Status schema is no-side-effect interaction state used for reasoning rhythm and UI status; side effects must use compact built-in action tools, preferably one `task` tool with `op=create|dispatch|append|stop|close|query`, maximum three framework tools total. Status schema can allow simple stop, task-complete-with-evidence terminal, next-step continuation, blocked terminal, user-option stop, and schema repair feedback when required fields are missing.
- follow-up correction: first implementation must be a four-point hook skeleton, not a full-flow hook chain. Raw request-side checks needing the most precise local tool/result data mount after local tool result; outbound controls mount before model request send; raw response processing mounts immediately after model response receive; final client-return processing mounts after all processing immediately before returning to the client. No schema/action implementation may bypass these hook points.

# 2026-07-04 error-center ADP read surface closeout

- owner slice: `error.center`, `ui.protocol`, `runtime.ui-command-dispatch`, `app.runtime-daemon`, `app.cli-runtime-smoke`, `app.webui-smoke`, plus `foundation.workspace` for S profile restart repair.
- implementation already present on resume:
  - `UiCommand::QueryErrorCenterEvents` / `SubscribeErrorCenterEvents`.
  - `UiQueryResult::ErrorCenterEvents` / `UiProjection::ErrorCenterEvents`.
  - runtime `RuntimeCommandDispatcher::query_runtime` route to `query_error_center_events_for_ui`.
  - server ADP query and initial subscription projection support.
  - daemon black-box test and CLI `adp-error-query`.
- documentation closeout:
  - synced `error.center`, `runtime.ui-command-dispatch`, `ui.protocol`, `app.cli-runtime-smoke`, `app.runtime-daemon`, `app.webui-smoke`, and `foundation.workspace` function maps/test designs/mainline JSONs.
  - regenerated generated wiki from mainline JSON truth.
- S profile runtime gap found:
  - `restartS` previously only kickstarted launchd; because launchd runs copied `freehand-daemonS-bin`, daemon could keep stale code while `freehand-cliS` symlink used current code.
  - fixed `scripts/install-launchd.sh restartS` to run `scripts/install-symlink.sh` before `launchctl kickstart`, refreshing debug binaries and the launchd daemon copy without touching global service.
  - updated `.agents/skills/freehand-dev/SKILL.md` to preserve the stale-code diagnostic rule.
- verification:
  - `cargo fmt --check`
  - `cargo test -p freehand-control -- --nocapture` -> 5 passed
  - `cargo test -p freehand-runtime runtime_query_reads_error_center_metadata_without_raw_text -- --nocapture`
  - `cargo test -p freehand-daemon daemon_adp_queries_runtime_error_center_truth -- --nocapture`
  - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 43 passed
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `cargo test --workspace` -> 423 passed
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `bash -n scripts/install-launchd.sh scripts/install-symlink.sh scripts/freehand-daemon-launchd.sh`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo test -p xtask` -> 18 passed
  - `make ci` -> exit 0
- online S-profile proof:
  - `scripts/install-launchd.sh installS`, health `ok`, `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`.
  - before `restartS` fix, `adp-error-query` timed out because daemon copy was stale; after `installS` refresh and script fix, query passed.
  - real failure sample: `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample failure` -> session `cli-adp-sample-failure-1783177452885366000`, turn `runtime-turn-37-r2`, `rounds=2`, `tool_executions=1`, `failed_tools=1`.
  - query proof: `freehand-cliS adp-error-query --url ws://127.0.0.1:4042/adp --session cli-adp-sample-failure-1783177452885366000` -> `count=1`, event `tool:validation:repair_schema:tool_result_failed:451bc61e1a05e812`.
  - filter proof: `--domain tool` -> `count=1`; `--domain provider` -> `count=0`.
  - raw ADP subscribe proof with correct externally tagged `UiCommand`: accepted plus initial event `tool:validation:repair_schema:tool_result_failed:451bc61e1a05e812`.
  - post-fix `scripts/install-launchd.sh restartS` rebuilt S binaries, restarted `com.freehand.daemonS`, health `ok`, ADP smoke passed, and persisted error-center query still returned `count=1`.
- remaining gaps:
  - live push when new error-center metadata is written after subscription is still pending.
  - WebUI visible error-center cards are still pending.
  - task/node/UI error policy integration remains future scope.

# 2026-07-05 session CRUD + double-Esc rollback S-profile closeout

- owner slice: `ui.protocol`, `reason.persistence`, `runtime.ui-command-dispatch`, `app.webui-smoke`, `app.runtime-daemon`, `app.cli-runtime-smoke`.
- local gates passed before online proof:
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo test -p freehand-ui-protocol -- --nocapture` -> 44 passed
  - `cargo test -p freehand-reason -- --nocapture --test-threads=1` -> 60 passed
  - `cargo test -p freehand-runtime -- --nocapture --test-threads=1` -> 70 passed
  - `cargo test -p freehand-server -- --nocapture` -> 11 passed
  - `cargo test -p freehand-daemon -- --nocapture` -> 17 passed
  - `cargo test -p freehand-cli -- --nocapture` -> 13 passed
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines generate`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo test --workspace` -> 430 passed
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `make ci` -> exit 0
- S profile proof:
  - `scripts/install-launchd.sh restartS` refreshed S symlink/debug daemon copy and restarted `com.freehand.daemonS`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> subscription/query/failure smoke passed.
  - real browser/CDP evidence saved under `artifacts/webui-online/20260705-session-crud-rollback-4042-1783188706/`.
  - session: `webui-session-20260704182300-fe1fceed`.
  - marker: `fh-crud-rollback-1783189377071`.
  - flow: new conversation -> first terminal `runtime-turn-38` -> WebUI rename -> refresh title persisted -> WebUI archive -> active list excluded and archived list included titled session -> WebUI restore -> transcript intact -> second terminal `runtime-turn-39` -> double Esc rollback -> composer restored second prompt and visible transcript hid `SECOND_TURN_FOR_ROLLBACK` -> edited replacement terminal `runtime-turn-40` -> restartS + browser reload restored title and effective transcript.
  - ADP before restart: `turns=2`, `turn_ids=runtime-turn-38,runtime-turn-40`, status `success`.
  - ADP after restart: same `turns=2`, `turn_ids=runtime-turn-38,runtime-turn-40`, status `success`.
  - browser states reported `pageErrors=0`, console errors none.
- validation note:
  - For CDP online proof in this environment, spawn headless Chrome inside the automation process and shut down only that explicit PID. Starting Chrome as a background child of a short-lived shell can leave the DevTools port unavailable because Chrome exits when the parent shell closes.

# 2026-07-05 instruction capability loader index slice

- user request: align Freehand AGENTS.md and skills design with `~/code/codex`, support local skills/local AGENTS.md, and index global AGENTS.md from `~/.freehand/AGENTS.md`.
- owner: new `instruction.capability-loader` feature in `crates/freehand-instructions`.
- implementation:
  - added `InstructionCapabilityCompileInput`, `InstructionCapabilityManifest`, `compile_instruction_capability_manifest`, and `write_instruction_capability_manifest`.
  - compiler indexes global `~/.freehand/AGENTS.md`, global `~/.freehand/skills/**/SKILL.md`, local `AGENTS.md` from project root to cwd, and local `.agents/skills/**/SKILL.md` from project root to cwd.
  - manifest entries include scope, precedence, normalized path/root, byte count, and content hash; skill entries also include parsed `name` and `description`.
  - malformed skill frontmatter becomes explicit manifest error records while valid entries remain indexed.
  - current slice is index-only; runtime/context-planner consumption remains pending and must use the compiled manifest rather than loose directory scanning.
- docs/gates:
  - added feature-map seed, function map, test design, design doc, mainline-call manifest, generated wiki, design index entry, and xtask required-file/workspace gates.
  - updated `freehand-dev` skill to lock owner boundary and forbid runtime/UI/provider direct directory scanning.
- validation:
  - `cargo test -p freehand-instructions -- --nocapture` -> 3 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `cargo fmt --check` -> ok.
  - `cargo test -p xtask -- --nocapture` -> 18 passed.
  - `cargo test --workspace` -> 433 passed.
  - `cargo clippy --workspace --all-targets -- -D warnings` -> no issues.

# 2026-07-05 Anthropic max_tokens default 8192

- user correction: default provider output budget must be 8192, not current 512.
- owner slice: `provider.anthropic-adapter` plus `provider.reason-live-bridge` caller wiring in runtime.
- implementation:
  - added `DEFAULT_ANTHROPIC_MAX_TOKENS: u64 = 8192` in `crates/freehand-provider-anthropic`.
  - runtime live Anthropic executor now uses that constant instead of hardcoded `512`.
  - Anthropic adapter tests now assert rendered request `max_tokens=8192`.
  - provider Anthropic function map, test design, mainline JSON, and generated wiki synchronized.
- validation:
  - `cargo fmt --check` -> ok.
  - `cargo test -p freehand-provider-anthropic -- --nocapture` -> 15 passed.
  - `cargo test -p freehand-runtime live_bridge_interrupts_non_candidate_max_tokens_without_failed_status -- --nocapture` -> 1 passed.
  - `cargo run -p xtask -- mainlines generate` -> ok.
  - `cargo run -p xtask -- mainlines check` -> ok.
  - `cargo run -p xtask -- gates check` -> ok.
  - `scripts/install-launchd.sh restartS` rebuilt/restarted S profile on `127.0.0.1:4042`.
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`.
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok.
  - `freehand-cliS adp-turn-sample --url ws://127.0.0.1:4042/adp --sample success` -> session `cli-adp-sample-success-1783219259178878000`, turn `runtime-turn-42`, `rounds=1`, terminal command receipt.
  - `cargo clippy -p freehand-provider-anthropic -p freehand-runtime --all-targets -- -D warnings` -> no issues.

# 2026-07-05 WebUI online gate profile correction

- user correction: dev/S profile uses fixed `4042`; release/global profile uses fixed `4041`.
- implementation:
  - `make verify-webui-online` now defaults to S profile `http://127.0.0.1:4042/`, `ws://127.0.0.1:4042/adp`, `freehand-cliS`, and `FREEHAND_WEBUI_PROFILE=4042`.
  - `make verify-webui-release-online` is the explicit release wrapper for `4041` and `freehand-cli`.
  - browser verifier renamed from `scripts/webui_verify_4041.mjs` to `scripts/webui_verify_online.mjs` and parameterized by environment.
  - `xtask` gate now locks default/release URL, health URL, ADP URL, CLI, and profile snippets; xtask fixtures include both wrapper scripts.
  - docs/function map/test design/mainline JSON/generated wiki/release doc/MEMORY corrected so alpha closeout no longer points at 4041.
- validation:
  - `cargo run -p xtask -- mainlines generate`
  - `bash -n scripts/verify-webui-online.sh scripts/verify-webui-release-online.sh`
  - `node --check scripts/webui_verify_online.mjs`
  - `cargo test -p xtask -- --nocapture` -> 18 passed
  - `cargo fmt --check`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `scripts/install-launchd.sh installS` started `com.freehand.daemonS` on `127.0.0.1:4042`
  - `curl -4fsS http://127.0.0.1:4042/health` -> `ok`
  - `~/.local/bin/freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp` -> ok
  - `make verify-webui-online` -> run `20260705-verify-4042-1783248727371`, all checks true, ADP session query exit 0
  - `make ci` -> exit 0
- evidence:
  - `artifacts/webui-online/20260705-verify-4042-1783248727371/summary.json`
  - session `webui-session-20260705105207-a9295d35`
  - turns `runtime-turn-53,runtime-turn-54,runtime-turn-54-r2`
- exclusion:
  - old `20260705-verify-4041-*` artifacts are wrong-profile intermediate evidence and are intentionally not staged.
