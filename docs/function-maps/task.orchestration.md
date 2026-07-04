# Function Map: `task.orchestration`

- feature_id: `task.orchestration`
- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- owner entry symbols:
  - `TaskRuntime::boot`
  - `TaskRuntime::create_task`
  - `TaskRuntime::append_task`
  - `TaskRuntime::pause_task`
  - `TaskRuntime::resume_task`
  - `TaskRuntime::heartbeat_task`
  - `TaskRuntime::assign_task`
  - `TaskRuntime::cancel_task`
  - `TaskRuntime::create_agent`
  - `TaskRuntime::close_agent`
  - `TaskRuntime::submit_review`
  - `TaskRuntime::approve_review`
  - `TaskRuntime::reject_review`
  - `TaskRuntime::close_task`
  - `TaskRuntime::query_task`
  - `TaskRuntime::list_agents`
  - `TaskRuntime::query_agent`
- mainline call source: `docs/mainline-calls/task.orchestration.json`
- generated wiki: `docs/wiki/task.orchestration.md`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `task.orchestration` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots and self-agent snapshot into memory
- `TaskRuntime::boot` loads task leases and interrupts running tasks whose lease is missing or expired
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`
- `assign_task` binds waiting/created/interrupted tasks to an available agent
- `create_agent` and `close_agent` manage persisted worker agent snapshots
- `cancel_task` moves a non-terminal task to `Cancelled` and releases assignee state
- lifecycle actions use explicit mutation request types and validate allowed transitions before writing ledger/snapshot truth
- `resume_task` enters `Running` and creates a lease-backed heartbeat record
- `heartbeat_task` refreshes the lease for the assigned running agent

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- task tool result returns semantic task ids, status, event counts, or JSON snapshots
- review lifecycle actions return event-backed mutation summaries
- heartbeat returns event-backed running-state mutation summary
- agent create/close returns persisted agent snapshot summaries

## Error Mainline

- missing task fields return explicit task errors
- unknown task id returns explicit task-not-found
- unknown agent id returns explicit agent-not-found
- persistence failures return explicit task persistence errors
- invalid lifecycle transitions return explicit `InvalidTransition` errors and do not write ledger/snapshot truth
- heartbeat for non-running or unassigned tasks returns explicit invalid transition and writes no lease
- assigning to unavailable agents and closing busy agents return explicit errors without mutating task/agent truth
- task failures become failed tool results and can be sent back to the model

## Shared Multi-Reference Functions

- `TaskRuntime::boot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: rebuild memory state from persisted task/agent snapshots
  - allowed callers: runtime task tool bridge, future daemon bootstrap
  - why shared: keeps startup recovery in task owner, not UI/runtime glue

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | expose one `task` tool schema with op-dispatched arguments | static registry truth | provider tool definition | runtime live bridge | tool registry | bound |
| 02 | `execute_task_tool` | `crates/freehand-runtime/src/lib.rs` | route task tool calls into task owner with runtime home/session/turn context | task tool call | tool result text | runtime live bridge | task runtime | bound |
| 03 | `TaskRuntime::boot` | `crates/freehand-task/src/lib.rs` | load task and agent snapshots into memory | runtime home + owner agent | ready task runtime | runtime task bridge | task owner | bound |
| 04 | `TaskRuntime::create_task` | `crates/freehand-task/src/lib.rs` | validate, persist, assign/wait, and update memory state | task create request | task snapshot + ledger events | runtime task bridge | task owner | bound |
| 05 | `TaskRuntime::query_task` | `crates/freehand-task/src/lib.rs` | return one task snapshot truth | task id | task snapshot | runtime task bridge | task owner | bound |
| 06 | `TaskRuntime::list_agents` / `TaskRuntime::query_agent` | `crates/freehand-task/src/lib.rs` | return agent registry truth | agent query | agent snapshots | runtime task bridge | task owner | bound |
| 07 | `TaskRuntime::append_task` / `pause_task` / `resume_task` | `crates/freehand-task/src/lib.rs` | mutate non-review lifecycle states through one transition validator | task mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 08 | `TaskRuntime::submit_review` / `approve_review` / `reject_review` / `close_task` | `crates/freehand-task/src/lib.rs` | enforce review-before-close lifecycle and persist each transition | review mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 09 | `TaskRuntime::heartbeat_task` | `crates/freehand-task/src/lib.rs` | refresh the lease for an assigned running task and persist a heartbeat event | task heartbeat request | running task snapshot + lease | runtime task bridge | task owner | bound |
| 10 | `reconcile_running_leases` | `crates/freehand-task/src/lib.rs` | interrupt running tasks with missing, mismatched, inactive, or expired leases during boot | persisted task snapshots + lease snapshot | recovered runtime state | task boot | task owner | bound |
| 11 | `TaskRuntime::assign_task` | `crates/freehand-task/src/lib.rs` | assign waiting/created/interrupted task to an available agent | task assignment request | assigned task snapshot + agent queued state | runtime task bridge | task owner | bound |
| 12 | `TaskRuntime::cancel_task` | `crates/freehand-task/src/lib.rs` | cancel non-terminal task and release assignee state | task mutation request | cancelled task snapshot + released agent | runtime task bridge | task owner | bound |
| 13 | `TaskRuntime::create_agent` / `close_agent` | `crates/freehand-task/src/lib.rs` | create persisted idle worker agents and close only idle agents | agent mutation request | agent snapshot | runtime task bridge | task owner | bound |

## Sync Status Against Code

- first implementation supports `create`, `query`, `list_agents`, and `query_agent`
- current implementation also supports `append`, `pause`, `resume`, `heartbeat`, `assign`, `cancel`, `submit_review`, `approve`, `reject`, `close`, `create_agent`, and `close_agent`
- real worker execution, UI task projection, and multi-agent dispatch are pending
