# Function Map: `agent.lifecycle`

- feature_id: `agent.lifecycle`
- owner crate: `crates/freehand-task` initially
- owner module: `crates/freehand-task/src/lib.rs` initially
- owner entry symbols:
  - pending: `AgentLifecycleReducer`
  - pending: `AgentLifecycleSnapshot`
  - pending: `AgentBoardProjection`
  - pending: `TaskRuntime::query_agent_board`
  - pending: `TaskRuntime::query_agent_lifecycle`
- mainline call source: `docs/mainline-calls/agent.lifecycle.json`
- generated wiki: `docs/wiki/agent.lifecycle.md`

## Request Mainline

- runtime/provider/tool/error/task owners emit typed lifecycle events
- lifecycle reducer accepts only typed lifecycle events
- lifecycle reducer updates per-agent lifecycle state
- Task Center execution binding supplies current task/execution/turn ids when available
- runtime or ADP query surface requests AgentBoard or one AgentLifecycleSnapshot

## Response Mainline

- AgentLifecycleSnapshot returns one agent's intrinsic state
- AgentBoardProjection returns agent availability, current activity, elapsed time, task/execution/turn binding, and model/tool/error counters
- scheduler and master prompt context consume AgentBoard summaries, not raw logs
- UI and Android render lifecycle projections and do not infer state from raw text

## Error Mainline

- raw assistant prose is rejected as lifecycle input
- unknown agent id returns explicit agent-not-found
- malformed typed lifecycle event returns explicit validation error and does not mutate lifecycle truth
- lifecycle query without initialized lifecycle truth returns explicit not-ready or empty-board truth, not fallback state

## Shared Multi-Reference Functions

- pending: `AgentLifecycleReducer`
  - owner: `crates/freehand-task/src/lib.rs` initially
  - purpose: reduce typed runtime/provider/tool/error/task events into per-agent lifecycle state
  - allowed callers: runtime live bridge, task runtime, tests
  - related tests: model_thinking/tool_running/recovering/blocked reducer tests
  - why shared: keeps lifecycle semantics single-sourced instead of duplicated in UI/runtime/node code
- pending: `AgentBoardProjection`
  - owner: `crates/freehand-task/src/lib.rs` initially
  - purpose: expose compact lifecycle truth for master, scheduler, UI, and headless ADP/CLI queries
  - allowed callers: runtime query dispatch, scheduler tick, tests
  - related tests: AgentBoard query and restart proof tests
  - why shared: keeps "what each agent is doing" as owner truth, not app-local inference

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `AgentLifecycleReducer` | `crates/freehand-task/src/lib.rs` | reduce typed lifecycle events into per-agent state | typed lifecycle event | updated lifecycle state | runtime/task owner | lifecycle owner | pending |
| 02 | `AgentLifecycleSnapshot` | `crates/freehand-task/src/lib.rs` | represent one agent's intrinsic lifecycle truth | agent state | serializable lifecycle snapshot | lifecycle owner | query/projection surfaces | pending |
| 03 | `AgentBoardProjection` | `crates/freehand-task/src/lib.rs` | project all agent lifecycle snapshots for master/scheduler/UI/headless query | lifecycle state map | AgentBoard projection | lifecycle owner | runtime query dispatch | pending |
| 04 | `TaskRuntime::query_agent_lifecycle` | `crates/freehand-task/src/lib.rs` | query one agent lifecycle snapshot | agent id | lifecycle snapshot or explicit not-found | runtime query dispatch | lifecycle owner | pending |
| 05 | `TaskRuntime::query_agent_board` | `crates/freehand-task/src/lib.rs` | query AgentBoard projection | optional filters | AgentBoard projection | runtime query dispatch | lifecycle owner | pending |

## Sync Status Against Code

- `agent.lifecycle` is a Phase 1 owner-map entry before implementation.
- No model-facing `agent` tool is implemented or allowed by default.
- Agent Lifecycle must remain an intrinsic agent state/projection.
- First implementation must add symbols and tests before claiming D3 complete.
