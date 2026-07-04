# Wiki: `task.orchestration`

Generated from `docs/mainline-calls/task.orchestration.json`. Do not edit by hand.

- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- function map: `docs/function-maps/task.orchestration.md`
- generated wiki: `docs/wiki/task.orchestration.md`
- test design: `docs/testing/task.orchestration.md`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `execute_task_tool` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots, task leases, and self-agent snapshot into memory
- `TaskRuntime::boot` interrupts running tasks whose lease is missing, mismatched, inactive, or expired
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`
- `TaskRuntime::assign_task` binds waiting, created, or interrupted tasks to an available agent
- `TaskRuntime::cancel_task` moves non-terminal tasks to Cancelled and releases assignee state
- `TaskRuntime::create_agent` and `TaskRuntime::close_agent` manage persisted worker agent snapshots
- lifecycle actions use explicit task mutation requests and validate state transitions before writing truth
- `TaskRuntime::resume_task` enters `Running` and creates a lease-backed heartbeat record
- `TaskRuntime::heartbeat_task` refreshes the lease for the assigned running agent

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- append, pause, resume, heartbeat, assign, cancel, submit_review, approve, reject, and close return event-backed mutation results
- create_agent and close_agent return persisted agent snapshot summaries
- task tool result returns semantic task ids, status, event names, sequence numbers, or JSON snapshots

## Error Mainline

- missing task fields return explicit task errors
- unknown task id returns explicit task-not-found
- unknown agent id returns explicit agent-not-found
- invalid lifecycle transitions return explicit invalid-transition errors
- heartbeat for non-running or unassigned tasks returns invalid-transition and writes no lease
- assigning to unavailable agents and closing busy agents return explicit errors without mutating task or agent truth
- persistence failures return explicit task persistence errors
- task failures become failed tool results and can be sent back to the model

## Shared Multi-Reference Functions

- `TaskRuntime::boot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: rebuild memory state from persisted task and agent snapshots
  - allowed callers: runtime task tool bridge, future daemon bootstrap
  - related tests: create_task_writes_ledger_snapshot_and_recovers_on_boot, boot_interrupts_running_task_with_expired_lease, task_tool_create_persists_and_queries_task
  - why shared: keeps startup recovery in task owner, not UI/runtime glue
- `TaskRuntime::mutate_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: validate lifecycle transitions, write ledger and snapshot, and update memory state
  - allowed callers: TaskRuntime lifecycle methods
  - related tests: review_reject_resume_submit_approve_close_lifecycle_persists, close_before_review_approval_is_rejected
  - why shared: keeps lifecycle mutation sequencing single-sourced
- `TaskRuntime::heartbeat_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: refresh active running task lease and persist heartbeat event
  - allowed callers: runtime task tool bridge
  - related tests: resume_creates_lease_and_heartbeat_extends_it, heartbeat_for_assigned_task_is_rejected_without_lease_write, task_tool_resume_and_heartbeat_persist_running_lease
  - why shared: keeps task execution liveness truth in task owner
- `TaskRuntime::assign_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: bind waiting, created, or interrupted tasks to an available agent and persist assignment truth
  - allowed callers: runtime task tool bridge
  - related tests: waiting_task_assigns_to_available_agent_and_recovers, task_tool_agent_assign_cancel_close_lifecycle
  - why shared: keeps agent selection mutation in task owner
- `TaskRuntime::create_agent`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: create persisted idle worker agent snapshots with declared capabilities
  - allowed callers: runtime task tool bridge
  - related tests: create_agent_persists_recovers_and_closes_when_idle, task_tool_agent_assign_cancel_close_lifecycle
  - why shared: keeps agent registry mutation in task owner

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | expose one `task` tool schema with op-dispatched arguments | static registry truth | provider tool definition | runtime live bridge | tool registry | bound |
| 02 | `execute_task_tool` | `crates/freehand-runtime/src/lib.rs` | route task tool calls into task owner with runtime home, session, turn, and trace context | task tool call | tool result text | runtime live bridge | task runtime | bound |
| 03 | `TaskRuntime::boot` | `crates/freehand-task/src/lib.rs` | load task and agent snapshots into memory | runtime home and owner agent | ready task runtime | runtime task bridge | task owner | bound |
| 04 | `TaskRuntime::create_task` | `crates/freehand-task/src/lib.rs` | validate, persist, assign/wait, and update memory state | task create request | task snapshot plus ledger events | runtime task bridge | task owner | bound |
| 05 | `TaskRuntime::query_task` | `crates/freehand-task/src/lib.rs` | return one task snapshot truth | task id | task snapshot | runtime task bridge | task owner | bound |
| 06 | `TaskRuntime::submit_review` | `crates/freehand-task/src/lib.rs` | record review submission with deliverables and evidence | task review submission | review-submitted task snapshot and event | runtime task bridge | task owner | bound |
| 07 | `TaskRuntime::approve_review` | `crates/freehand-task/src/lib.rs` | approve submitted review before close | task mutation request | approved task snapshot and event | runtime task bridge | task owner | bound |
| 08 | `TaskRuntime::close_task` | `crates/freehand-task/src/lib.rs` | close only approved or otherwise closeable tasks and release assignee state | task mutation request | closed task snapshot and event | runtime task bridge | task owner | bound |
| 09 | `TaskRuntime::heartbeat_task` | `crates/freehand-task/src/lib.rs` | refresh the lease for an assigned running task | task heartbeat request | running task snapshot plus active lease | runtime task bridge | task owner | bound |
| 10 | `reconcile_running_leases` | `crates/freehand-task/src/lib.rs` | interrupt running tasks with missing, mismatched, inactive, or expired leases during boot | persisted task snapshots plus lease snapshot | recovered runtime state | task boot | task owner | bound |
| 11 | `TaskRuntime::assign_task` | `crates/freehand-task/src/lib.rs` | assign waiting, created, or interrupted tasks to an available agent | task assignment request | assigned task snapshot plus queued agent state | runtime task bridge | task owner | bound |
| 12 | `TaskRuntime::cancel_task` | `crates/freehand-task/src/lib.rs` | cancel non-terminal tasks and release assignee state | task mutation request | cancelled task snapshot plus released agent state | runtime task bridge | task owner | bound |
| 13 | `TaskRuntime::create_agent` | `crates/freehand-task/src/lib.rs` | create persisted idle worker agents | agent create request | available agent snapshot | runtime task bridge | task owner | bound |
| 14 | `TaskRuntime::close_agent` | `crates/freehand-task/src/lib.rs` | close only idle agents | agent mutation request | closed agent snapshot | runtime task bridge | task owner | bound |

## Sync Status Against Mainline Call

- first implementation supports `create`, `query`, `list_agents`, and `query_agent`
- current implementation supports `append`, `pause`, `resume`, `heartbeat`, `assign`, `cancel`, `submit_review`, `approve`, `reject`, `close`, `create_agent`, and `close_agent`
- review-before-close is locked by positive and negative tests
- lease-backed Running recovery is locked by positive and negative tests
- agent registry lifecycle is locked by positive and negative tests
- real worker execution, UI task projection, and multi-agent dispatch are pending
