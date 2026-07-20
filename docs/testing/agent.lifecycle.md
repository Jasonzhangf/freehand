# Test Design: `agent.lifecycle`

- feature_id: `agent.lifecycle`
- owner: `crates/freehand-task` initially
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `agent.heartbeat`
- lifecycle path under test:
  - Worker process start enters the lifecycle owner as a typed
    `ProcessStarted` event with PID, process-instance identity, and start time
  - every Worker poll tick, including idle ticks, enters the same owner as a
    typed `ProcessHeartbeat` event
  - AgentBoard and AgentLifecycle queries derive `alive` only from the
    process-heartbeat timestamp and the owner TTL
  - typed runtime/provider/tool/error/task event enters lifecycle reducer
  - reducer updates one agent's lifecycle snapshot
  - AgentBoard projection exposes lifecycle truth
  - runtime/ADP/CLI query returns lifecycle truth without UI-local inference
  - restart proof re-queries the same agent id once persistence is implemented
  - lifecycle snapshot persists independently from agent resource state so
    blocked, review-ready, rejected, approved, interrupted, cancelled, failed,
    and closed task truth can release a worker while restart query still sees
    the typed event as last activity instead of current binding
  - Phase 2A worker execution and review events project lifecycle state from
    typed Task Center truth, not from raw model text

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `agent.heartbeat` | bound | `cargo test -p freehand-task agent_process -- --nocapture` covers start, same-instance heartbeat, new-instance restart count, stale/missing heartbeat, task-activity separation, and malformed instance rejection | `cargo test -p freehand-runtime production_worker_runner -- --nocapture` proves constructor start plus idle and active tick heartbeat through the Worker runner | `scripts/verify-master-three-worker-e2e-online.sh` proves three fresh Worker identities, one explicit PID stop, TTL-offline projection, same-agent restart, and restart-count increment in an isolated runtime home |

## White-Box Coverage

- `ProcessStarted` persists PID, unique process instance, start timestamp,
  heartbeat timestamp, `alive=true`, and initial `restart_count=0`
- same-instance heartbeat refreshes process health without incrementing
  `restart_count`
- a different process instance for the same agent increments `restart_count`
- fresh heartbeat projects `alive=true`; stale or missing process heartbeat
  projects `alive=false` without deleting task/execution/activity history
- task/model/tool activity never refreshes `process_heartbeat_at`
- empty process instance identity and PID zero are rejected without lifecycle
  mutation
- `model_thinking` state from typed provider/model request event
- `tool_running` state from typed tool execution event
- `recovering` state from failed-tool/schema-polishing/provider-retry typed facts
- `blocked` state from typed blocker fact
- current activity, last activity, elapsed time, task/execution/turn binding, and model/tool/error counters
- worker progress and recovering task lifecycle projections keep the current
  execution id visible while execution is active
- blocked, review_ready, review_rejected, approved, interrupted, cancelled,
  failed, and closed task lifecycle projections clear `current_task_id`,
  `current_execution_id`, and `current_turn_id`, project
  `current_activity.kind=idle`, and preserve the typed state in
  `last_activity`
- `TaskClosed` releases the former Worker resource: AgentBoard projects
  `state=idle`, clears current task/execution/turn binding, and retains one
  `last_activity.kind=closed` record naming the closed task
- boot reconciliation repairs stale persisted lifecycle snapshots whose current
  binding points at a blocked, review-ready, rejected, approved, interrupted,
  cancelled, failed, or closed task snapshot, including execution-only legacy
  snapshots where `current_task_id` is already empty but `current_execution_id`
  still points at the released task execution
- `TaskInterrupted` releases the former Worker resource: AgentBoard projects
  `state=idle`, clears current task/execution/turn binding, and retains one
  `last_activity.kind=interrupted` record naming the released task. A later
  takeover assignment must not restore the former Worker's running state
- persisted lifecycle snapshot survives reboot and keeps the same execution id
  visible for Phase 2A verification
- raw assistant prose is not accepted as lifecycle input
- unknown agent id returns explicit not-found
- implemented owner test: `agent_lifecycle_reducer_projects_model_tool_recovering_and_blocked`
- implemented owner test:
  `task_close_releases_agent_lifecycle_and_boot_repairs_stale_current_binding`
- Phase 2A owner test:
  `phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id`

## Module Black-Box Coverage

- production Worker constructor emits one typed process-start event
- idle and active `ProductionWorkerRunner::run_once` ticks refresh the same
  process heartbeat
- UI protocol projection exposes PID, process instance, start/heartbeat
  timestamps, and restart count from owner truth
- runtime can query AgentBoard truth without UI-local state
- runtime can query one AgentLifecycleSnapshot by agent id
- CLI/ADP headless sample can query lifecycle truth for a known agent id
- malformed lifecycle event returns explicit validation error and does not mutate truth
- blocked, review-ready, rejected, interrupted, cancelled, failed, approved, or
  closed task truth cannot leave the former Worker projected as running after
  its lease/execution has ended
- CLI/ADP Phase 2A sample can query lifecycle truth for the worker across
  claim, blocked, recovering, review/retry, approved, and closed phases; only
  active execution phases are current-bound

## Project Black-Box Impact

- Master consumes AgentBoard process health instead of inferring Worker
  availability from task activity or launchd state.
- UI/Android render process health and restart identity from owner projection
  instead of inferring state.
- Worker Control may read lifecycle truth for status answers, but it must not mutate task truth.

## Required Checks

```bash
cargo test -p freehand-task
cargo test -p freehand-runtime
cargo test -p freehand-ui-protocol
cargo test -p freehand-daemon worker_mode -- --nocapture
cargo test -p freehand-cli
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- D3 owner-internal skeleton is implemented in `crates/freehand-task`
- ADP/CLI lifecycle query surface is implemented for headless Phase 2A proof
- restart same-id proof is implemented headlessly and live-validated on
  S-profile `127.0.0.1:4042`
- Phase 2A restart same-id lifecycle proof is implemented by
  `master-worker-foundation-sample --verify ...`
- no standalone model-facing `agent` tool is planned for Phase 1
- launchd owns process supervision only; it does not own or synthesize
  AgentBoard health truth
