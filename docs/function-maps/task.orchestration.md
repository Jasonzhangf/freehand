# Function Map: `task.orchestration`

- feature_id: `task.orchestration`
- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- owner entry symbols:
  - `TaskRuntime::boot`
  - `TaskRuntime::create_task`
  - `TaskRuntime::query_task`
  - `TaskRuntime::list_agents`
  - `TaskRuntime::query_agent`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `task.orchestration` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots and self-agent snapshot into memory
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- task tool result returns semantic task ids, status, event counts, or JSON snapshots

## Error Mainline

- missing task fields return explicit task errors
- unknown task id returns explicit task-not-found
- unknown agent id returns explicit agent-not-found
- persistence failures return explicit task persistence errors
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

## Sync Status Against Code

- first implementation supports `create`, `query`, `list_agents`, and `query_agent`
- real worker execution, lease heartbeat, review actions, UI task projection, and multi-agent dispatch are pending
