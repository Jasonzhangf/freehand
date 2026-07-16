# Test Design: `worker.control`

- feature_id: `worker.control`
- owner: `crates/freehand-task`
- lifecycle path under test:
  - worker execution is already created through Task Center assignment and claim
  - worker-control command targets one `task_id`, `execution_id`, and `agent_id`
  - owner validates target task, assignee, active execution id, agent existence, terminal status, and op-specific payloads before writing truth
  - owner writes append-only worker-control ledger events and latest execution control snapshot
  - framework-answerable `query_status` returns Task Center, AgentSnapshot, and AgentLifecycle status truth
  - worker-model-answerable safe-point ops queue pending/deferred events without mutating task status or prompt history
  - pause/resume/cancel route task lifecycle consequences through existing Task Center APIs before persisting the `applied` control event
  - restart same-id verification reads persisted control events for the same task/execution/agent/control ids

## White-Box Coverage

- `worker_control_query_status_persists_and_recovers`
  - creates a worker, assigns and claims a task, runs `query_status`, reboots `TaskRuntime`, and verifies the same control event is still readable
- `worker_control_safe_point_events_queue_without_task_mutation`
  - runs `ask_at_safe_point`, `add_constraint`, `request_checkpoint`, and `request_submission_now`
  - verifies each event is persisted as queued/deferred
  - verifies task status stays `Running`
- `worker_control_pause_resume_cancel_write_task_consequences`
  - runs `pause`, verifies task becomes `Paused`
  - runs `resume`, verifies task becomes `Running` with the same execution id
  - runs `cancel`, verifies task becomes `Cancelled`
  - verifies control events and Task Center ledger events are both present
  - verifies `applied` control events are persisted only after the matching Task Center consequence succeeds
- `worker_control_rejects_wrong_execution_without_mutation`
  - sends a valid task/agent but wrong execution id
  - verifies explicit error, no worker-control event, and no task ledger mutation
- `worker_control_rejects_terminal_task_without_event`
  - closes or cancels a task, then sends worker control
  - verifies terminal task rejection and no control event
- `worker_control_rejects_missing_question_and_constraint`
  - verifies `ask_at_safe_point` without `question` and `add_constraint` without `constraint` fail before persistence

## Module Black-Box Coverage

- `freehand-ui-protocol`
  - `worker_control_command_validates_and_routes_to_worker_control`
  - `worker_control_command_rejects_missing_fields`
  - `worker_control_adp_roundtrip_carries_projection`
- `freehand-runtime`
  - `runtime_dispatches_worker_control_to_task_owner`
  - `runtime_worker_control_invalid_target_returns_explicit_failure`
- `freehand-cli`
  - `worker-control-foundation-sample` mock ADP test creates a worker execution, sends query/safe-point/pause/resume/cancel commands, and verifies explicit output
  - verify mode mock test checks same task/execution/agent/control ids after restart

## Project Black-Box Impact

- no WebUI/Android dashboard claim in Phase 2C
- no model prompt dependency for worker-control closure
- no private Agent-to-Agent mutation path; all commands enter ADP/protocol/runtime owner routing
- no fallback to empty success on owner error
- S-profile `127.0.0.1:4042` proof must run:
  - `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
  - `freehand-cliS worker-control-foundation-sample --url ws://127.0.0.1:4042/adp`
  - restart `com.freehand.daemonS`
  - verify same ids with `freehand-cliS worker-control-foundation-sample --url ws://127.0.0.1:4042/adp --verify-task ... --execution ... --agent ... --control ...`

## Required Checks

```bash
cargo test -p freehand-task worker_control -- --nocapture
cargo test -p freehand-ui-protocol worker_control -- --nocapture
cargo test -p freehand-runtime worker_control -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo fmt --check
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

## Known Gaps

- WebUI/Android task dashboard and rich worker-control rendering are Phase 2D.
- The production Worker runner does not yet acknowledge an in-flight pause at a
  safe point, stop provider/tool progress, or deterministically re-enter
  reasoning after `resume`; Task Center pause/resume mutation alone is not a
  closed execution lifecycle.
- Required paired red tests are
  `production_worker_runner_pause_stops_before_submission`,
  `production_worker_runner_paused_execution_cannot_publish_stale_success`,
  `production_worker_runner_resume_reenters_reasoning_and_submits_review`, and
  `production_worker_runner_paused_without_resume_stays_idle`.
- Worker execution does not yet consume safe-point queued questions,
  constraints, checkpoint requests, or submission requests in a real model
  loop; Phase 2C only persists and exposes the queue.
- Cross-machine worker control is out of scope until node transport is expanded.
