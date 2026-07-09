# Function Map: `worker.control`

- feature_id: `worker.control`
- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- mainline call source: `docs/mainline-calls/worker.control.json`
- generated wiki: `docs/wiki/worker.control.md`
- owner entry symbols:
  - `TaskRuntime::apply_worker_control`
  - `TaskRuntime::query_worker_control_events`
  - `WorkerControlOp`
  - `WorkerControlRequest`
  - `WorkerControlEvent`
  - `WorkerControlProjection`

## Request Mainline

- master or framework command enters as `worker_control(op=...)` semantics against an already-running worker execution
- UI/CLI protocol wraps that request as `UiCommand::WorkerControl` and routes it to `worker.control`
- runtime dispatch converts protocol DTOs into `WorkerControlRequest`
- `TaskRuntime::apply_worker_control` validates non-empty `task_id`, `execution_id`, `agent_id`, and operation
- task snapshot must exist, must be non-terminal for control purposes, must be assigned to the target agent, and must have an `active_execution_id` equal to the request execution id
- agent snapshot must exist and must match the task assignee
- op-specific validation happens before any write: `ask_at_safe_point` requires `question`, `add_constraint` requires `constraint`
- framework-answerable `query_status` reads Task Center, AgentSnapshot, and AgentLifecycle truth, then writes an auditable control event
- safe-point requests `ask_at_safe_point`, `add_constraint`, `request_checkpoint`, and `request_submission_now` write pending/deferred control events and do not mutate task status
- task-state controls `pause`, `resume`, and `cancel` route consequences through existing Task Center APIs first, then write an `applied` control event only after the Task Center consequence succeeds

## Response Mainline

- `WorkerControlProjection` returns the created control event plus current task, agent, and lifecycle status truth
- `query_status` returns a status event whose payload summarizes framework-derived task status, active execution id, agent status, lifecycle state, elapsed time, and current activity
- safe-point operations return `queued` status and a compact summary of what the worker should answer or respect at the next safe point
- `pause` returns a control event plus the Task Center event that moved the task to `Paused`
- `resume` returns a control event plus the Task Center event that moved the task to `Running` and refreshed lease state
- `cancel` returns a control event plus the Task Center event that moved the task to `Cancelled` and released the agent
- `TaskRuntime::query_worker_control_events` returns persisted control events for restart same-id verification

## Error Mainline

- missing required ids or op-specific payloads return explicit `MissingField` and write no control event
- unknown task id returns explicit task-not-found and writes no control event
- unknown agent id returns explicit agent-not-found and writes no control event
- task assignee mismatch returns explicit invalid transition and writes no control event
- active execution mismatch returns explicit invalid transition and writes no control event
- terminal task states for worker-control purposes are `Approved`, `Closed`, `Cancelled`, and `Failed`; they reject all worker-control operations and write no control event
- `Paused` and `Blocked` are non-terminal and may be queried or targeted by transitions where Task Center allows them
- Task Center consequence failures from pause/resume/cancel surface as explicit owner errors and must not be converted into queued safe-point success or persisted as `applied` worker-control events
- persistence failure for the worker-control ledger or snapshot fails the request explicitly and must not pretend the control was queued
- worker control must not create, assign, claim, approve, reject, close, rewrite transcript, mutate session/workspace truth, or directly edit prompt history

## Shared Multi-Reference Functions

- `TaskRuntime::apply_worker_control`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: validate and persist safe-point worker control events, and route allowed task-state consequences through Task Center
  - allowed callers: runtime dispatch, CLI/ADP headless samples, tests
  - related tests:
    - `worker_control_query_status_persists_and_recovers`
    - `worker_control_safe_point_events_queue_without_task_mutation`
    - `worker_control_pause_resume_cancel_write_task_consequences`
    - `worker_control_rejects_wrong_execution_without_mutation`
    - `worker_control_rejects_terminal_task_without_event`
  - why shared: keeps runtime control truth in one owner instead of UI/runtime-local queues
- `TaskRuntime::query_worker_control_events`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: read persisted worker-control events for same-id restart verification and UI/debug projection
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests:
    - `worker_control_query_status_persists_and_recovers`
  - why shared: restart proof must read owner truth, not command receipts

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `UiWorkerControlCommand` | `crates/freehand-ui-protocol/src/lib.rs` | define protocol DTO for worker-control op, target ids, and typed payload | UI/CLI worker-control command | validated DTO | ADP command transport | protocol boundary | bound |
| 02 | `UiCommand::WorkerControl` | `crates/freehand-ui-protocol/src/lib.rs` | route worker-control mutation-intent commands to `worker.control` | protocol command | dispatch envelope target | protocol boundary | runtime dispatch | bound |
| 03 | `RuntimeCommandDispatcher::dispatch_worker_control` | `crates/freehand-runtime/src/lib.rs` | convert protocol DTO to owner request and project result | dispatch envelope | worker-control query result / receipt | runtime dispatch | task owner | bound |
| 04 | `TaskRuntime::apply_worker_control` | `crates/freehand-task/src/lib.rs` | validate target task/execution/agent, route allowed Task Center consequences, and persist accepted control truth | worker control request | worker control projection | runtime dispatch / tests | worker-control owner | bound |
| 05 | `TaskStore::append_worker_control_event` / `TaskStore::write_worker_control_snapshot` | `crates/freehand-task/src/lib.rs` | write append-only worker-control ledger rows and latest execution snapshot only after owner validation and any required Task Center consequence succeeds | worker control event | durable ledger/snapshot truth | worker-control owner | task store | bound |
| 06 | `TaskRuntime::query_worker_control_events` | `crates/freehand-task/src/lib.rs` | read persisted control events for same-id restart verification | task id + execution id | ordered control events | runtime query / CLI verify | worker-control owner | bound |
| 07 | `project_worker_control_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert owner projection to UI-safe protocol result | worker-control projection | `UiWorkerControlProjection` | runtime dispatch | ui.protocol DTO | bound |
| 08 | `run_worker_control_foundation_sample` / `run_worker_control_foundation_sample_async` / `verify_worker_control_foundation_truth` | `apps/freehand-cli/src/main.rs` | drive no-UI Phase 2C sample and restart same-id verification through ADP | ADP URL plus optional verify ids | terminal-facing evidence | CLI dispatcher | daemon `/adp` | bound |

## Sync Status Against Code

- `worker.control` owner is code-bound for Phase 2C safe-point runtime control.
- CLI mock ADP coverage is implemented for create+control sample mode and verify-only mode.
- S-profile online same-id restart proof is required before declaring Phase 2C fully closed.
- Phase 2C excludes WebUI/Android dashboard projection, rich safe-point UI, worker autoscaling, multi BigTask context switching, and cross-machine workers.
