# Function Map: `agent.lifecycle`

- feature_id: `agent.lifecycle`
- owner crate: `crates/freehand-task` initially
- owner module: `crates/freehand-task/src/lib.rs` initially
- owner entry symbols:
  - `AgentLifecycleSnapshot`
  - `AgentBoardProjection`
  - `AgentLifecycleEvent`
  - `TaskRuntime::apply_agent_lifecycle_event`
  - `TaskRuntime::query_agent_board`
  - `TaskRuntime::query_agent_lifecycle`
- mainline call source: `docs/mainline-calls/agent.lifecycle.json`
- generated wiki: `docs/wiki/agent.lifecycle.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `agent.heartbeat`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `agent`
- touched resources:
  - `agent`
- resource operations:
  - `agent.heartbeat`
- forbidden shortcuts:
  - UI, daemon hosts, and launchd scripts must not infer AgentBoard process
    health from PID inspection, task activity, or service status.
  - Task lifecycle activity must not refresh process-heartbeat truth.

## Request Mainline

- Worker process start and every poll tick emit typed process lifecycle events
- lifecycle owner validates PID and process-instance identity before mutation
- process heartbeat persists independently from task/model/tool activity
- runtime/provider/tool/error/task owners emit typed lifecycle events
- lifecycle reducer accepts only typed lifecycle events
- lifecycle reducer updates per-agent lifecycle state
- lifecycle snapshots are persisted independently from resource AgentSnapshot so
  worker resource release can return to available while lifecycle query still
  reports the last typed task state for restart proof
- Task Center execution binding supplies current task/execution/turn ids when available
- Phase 2A task execution events project worker running, progress, blocked,
  recovering, review_ready, retrying, approved, and closed semantics without
  parsing raw assistant prose
- `TaskInterrupted` releases the former Worker resource, clears its current
  task/execution/turn binding, projects idle current activity, and keeps the
  interruption as typed last activity for audit and UI display
- runtime or ADP query surface requests AgentBoard or one AgentLifecycleSnapshot

## Response Mainline

- AgentLifecycleSnapshot returns one agent's intrinsic state plus process PID,
  process-instance identity, start/heartbeat timestamps, and restart count
- AgentBoardProjection derives `alive` from the owner heartbeat TTL. Current
  task/execution binding exists only while lifecycle truth still binds that
  Worker; interruption clears current binding while retaining typed last
  activity
- scheduler and master prompt context consume AgentBoard summaries, not raw logs
- UI and Android render lifecycle projections and do not infer state from raw text

## Error Mainline

- raw assistant prose is rejected as lifecycle input
- unknown agent id returns explicit agent-not-found
- malformed typed lifecycle event returns explicit validation error and does not mutate lifecycle truth
- missing, empty, or zero-valued process identity fields return explicit
  validation errors and do not mutate lifecycle truth
- lifecycle query without initialized lifecycle truth returns explicit not-ready or empty-board truth, not fallback state
- missing or stale process heartbeat projects `alive=false`; task activity and
  persisted AgentSnapshot status are not health fallbacks
- missing execution id on execution-bound lifecycle events is rejected by task owner before lifecycle projection is accepted
- interrupted task truth must not leave its former Worker projected as running
  or bound to the ended task/execution
- persisted lifecycle snapshot parse/write failures surface as task persistence
  errors; query must not rebuild a false idle lifecycle when persisted typed
  truth exists

## Shared Multi-Reference Functions

- `TaskRuntime::apply_agent_lifecycle_event`
  - owner: `crates/freehand-task/src/lib.rs` initially
  - purpose: reduce typed runtime/provider/tool/error/task events into per-agent lifecycle state
  - allowed callers: runtime live bridge, task runtime, tests
  - related tests:
    `agent_process_lifecycle_projects_fresh_stale_and_restart_truth`,
    `agent_lifecycle_reducer_projects_model_tool_recovering_and_blocked`
  - why shared: keeps lifecycle semantics single-sourced instead of duplicated in UI/runtime/node code
- `AgentBoardProjection`
  - owner: `crates/freehand-task/src/lib.rs` initially
  - purpose: expose compact lifecycle truth for master, scheduler, UI, and headless ADP/CLI queries
  - allowed callers: runtime query dispatch, scheduler tick, tests
  - related tests: `agent_lifecycle_reducer_projects_model_tool_recovering_and_blocked`
  - why shared: keeps "what each agent is doing" as owner truth, not app-local inference
- `TaskStore::write_agent_lifecycle_snapshot`
  - owner: `crates/freehand-task/src/lib.rs` initially
  - purpose: persist latest typed lifecycle projection for restart same-id query
  - allowed callers: task event projection, lifecycle reducer
  - related tests:
    `phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id`
  - why shared: keeps lifecycle truth durable without coupling it to releasable worker resource state

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TaskRuntime::apply_agent_lifecycle_event` | `crates/freehand-task/src/lib.rs` | reduce typed lifecycle events into per-agent state | typed lifecycle event | updated lifecycle state | runtime/task owner | lifecycle owner | bound |
| 02 | `AgentLifecycleSnapshot` | `crates/freehand-task/src/lib.rs` | represent one agent's intrinsic lifecycle truth | agent state | serializable lifecycle snapshot | lifecycle owner | query/projection surfaces | bound |
| 03 | `AgentBoardProjection` | `crates/freehand-task/src/lib.rs` | project all agent lifecycle snapshots for master/scheduler/UI/headless query | lifecycle state map | AgentBoard projection | lifecycle owner | runtime query dispatch | bound |
| 04 | `TaskRuntime::query_agent_lifecycle` | `crates/freehand-task/src/lib.rs` | query one agent lifecycle snapshot | agent id | lifecycle snapshot or explicit not-found | runtime query dispatch | lifecycle owner | bound |
| 05 | `TaskRuntime::query_agent_board` | `crates/freehand-task/src/lib.rs` | query AgentBoard projection | optional filters | AgentBoard projection | runtime query dispatch | lifecycle owner | bound |
| 06 | `TaskRuntime::apply_execution_fact` / `TaskRuntime::reject_review` / `TaskRuntime::approve_review` / `TaskRuntime::close_task` | `crates/freehand-task/src/lib.rs` | derive Worker lifecycle state from typed task execution and review events, including releasing current Worker binding on interruption | execution/review task events with execution id | AgentLifecycleSnapshot and AgentBoard truth | task.orchestration | agent.lifecycle reducer | bound |
| 07 | `TaskStore::write_agent_lifecycle_snapshot` / `TaskStore::load_agent_lifecycle_snapshots` | `crates/freehand-task/src/lib.rs` | persist and reload latest lifecycle snapshot for restart same-id query | lifecycle snapshot | durable lifecycle projection | task event projection / boot | lifecycle owner storage | bound |
| 08 | `TaskRuntime::apply_agent_lifecycle_event` | `crates/freehand-task/src/lib.rs` | validate and persist process start/heartbeat truth, derive restart count, and project TTL-backed alive state | typed Worker process lifecycle event | durable process identity and queryable health | production Worker runner | agent.lifecycle owner | bound |

## Sync Status Against Code

- `agent.lifecycle` has the first in-owner lifecycle skeleton implemented in
  `crates/freehand-task`.
- No model-facing `agent` tool is implemented or allowed by default.
- Agent Lifecycle must remain an intrinsic agent state/projection.
- Phase 2A ADP/CLI same-id proof is implemented and live-validated on the
  S-profile with restart same-id verification.
- Worker process health is owner-projected from typed heartbeat truth; launchd
  remains a supervisor and is not an AgentBoard truth source.
