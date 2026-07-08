# Task Orchestration Design

## Status

Initial skeleton implementation is in progress. This document is the durable design truth for task lifecycle, persistence, memory state, startup, recovery, and agent registry.

The next foundation direction is global multi-task management. See
`task-center-truth.md`, `agent-lifecycle-semantics.md`, and
`master-worker-task-state-machine-phase1.md`. Those documents define the target
Task Center, Agent Lifecycle, and Phase 1 master/worker task loop before those
capabilities are attached to real agents.

## Principles

- One built-in tool surface: `task`.
- `task` uses `op` parameters instead of many tool names.
- Status schema has no side effects; only admitted task tool actions mutate task truth.
- Task truth is append-only ledger plus rebuildable snapshot.
- Runtime memory state is a cache and scheduler surface, not truth.
- A task is a workspace-scoped work item, not a worker and not a session.
- Workspace owns cwd and session truth. Task truth references workspace/session but does not own session history.
- Worker is a schedulable resource. Execution is a worker runtime activity bound to one task, workspace, and session.
- Worker agents submit review; close requires approval or explicit close action.

## Tool Surface

First implemented ops:

- `create`
- `query`
- `list_tasks`
- `history`
- `list_agents`
- `query_agent`
- `append`
- `pause`
- `resume`
- `heartbeat`
- `assign`
- `claim_next`
- `record_execution`
- `cancel`
- `submit_review`
- `approve`
- `reject`
- `close`

Planned ops:

- real worker process/channel dispatch
- workspace selection action integration
- execution creation/query projection
- task/workspace UI projection

## Task Lifecycle

Primary path:

```text
Created -> WaitingWorker -> Assigned -> Running -> ReviewSubmitted -> Approved -> Closed
```

Branches:

```text
Created -> Cancelled
Created -> WaitingWorker -> Assigned
WaitingWorker -> Assigned
Assigned -> Cancelled
Running -> Paused -> Running
Running -> Interrupted -> WaitingWorker | Assigned | Running
Running -> Blocked -> Running | Cancelled
Running -> Running (record_execution)
Running -> Failed -> WaitingWorker | Assigned | Closed
ReviewSubmitted -> Rejected -> WaitingWorker | Assigned | Running
ReviewSubmitted -> Approved -> Closed
```

`Draft` is intentionally absent. A model task action creates a real task.

## Dispatch Flow

Task dispatch is driven by admitted `task` tool actions. The framework must not
create or assign tasks from raw user text, paths, assistant prose, UI state, or
tool output.

The master should be prompted to actively choose dispatch actions when condition
rules match. The condition matrix and prompt contract live in
`docs/design/multi-agent-dispatch-alignment.md`; this document owns the durable
task state after those actions are admitted.

Dispatch is a three-layer flow:

```text
master conversation coordination
  -> durable task lifecycle
  -> worker execution lifecycle
```

The layers must not be collapsed. A provider/schema/tool repair loop inside one
execution is not the same thing as task failure. A rejected review is not worker
resource failure. A worker crash interrupts execution and may move the task back
to a schedulable state.

Flow:

```text
master user turn
  -> model status schema (intent only, no side effects)
  -> workspace action selects or creates workspace/session when needed
  -> task(op="create")
  -> task owner validates workspace/session/task contract
  -> task snapshot enters Created or WaitingWorker
  -> task(op="assign") or task(op="claim_next")
  -> execution starts with one context profile
  -> task(op="record_execution") records progress
  -> task(op="submit_review") submits deliverables/evidence
  -> task(op="approve" | "reject")
  -> task(op="close") after accepted review or explicit close policy
```

Dispatch policy fields:

- `kind`: `search`, `code_edit`, `review`, `test`, `docs`, or `generic`
- `requested_capabilities`
- `target_workspace_id` or `target_cwd`
- `parallelism`
- `model_tier`: `main`, `small`, or `default`
- `context_profile`: `clean_search`, `workspace_inherited`, or `debug_direct`

Context profiles:

- `clean_search`: use only task goal, target scope, allowed tools, and output schema
- `workspace_inherited`: use workspace session summary plus task-specific context
- `debug_direct`: debug-only mode with explicit transcript access

Dispatch policy validation:

- `parallelism` is explicit and bounded
- `requested_capabilities` must match worker resource declarations
- `target_workspace_id` or `target_cwd` must already resolve through workspace
  owner validation
- `context_profile=debug_direct` is not allowed for ordinary user-facing task
  execution
- invalid policy is returned as a task tool result and must not mutate task
  truth

Search dispatch:

- broad search should use `clean_search`
- worker returns one typed conclusion with evidence summary
- master/main model performs analysis and next-task decision
- raw worker search transcript stays out of parent context
- failed search attempts are repair-visible only until a later successful search
  supersedes them; durable debug/task ledgers keep the raw evidence

Waiting behavior:

- if no suitable worker is available, task remains `WaitingWorker`
- waiting is an owner-backed task state, not a UI-local spinner
- later `assign` or `claim_next` may move the task forward

Execution behavior:

- claim or assignment creates an execution identity bound to one task, workspace,
  session, and worker
- execution progress is recorded with `record_execution`
- tool errors inside execution are paired results returned to the model for
  repair and do not automatically fail the task
- provider/network errors follow provider retry policy before terminal failure
- execution interruption leaves durable task truth recoverable through lease and
  heartbeat reconciliation

Dispatch error taxonomy:

- `schema_mismatch`: response polishing; no task mutation
- `workspace_validation_error`: workspace action result; returned to model
- `task_validation_error`: task action result; returned to model
- `worker_unavailable`: task remains `WaitingWorker`
- `execution_tool_error`: paired tool result inside execution
- `provider_error`: retry policy, then terminal execution/turn failure if
  exhausted
- `review_rejected`: task lifecycle event; next work requires admitted action

## Task Fields

Every task has:

- `workspace_id`
- `session_id`
- `title`
- `content`
- `goal`
- `deliverables`
- `acceptance`
- `priority`
- `target_cwd`
- requested capabilities
- visibility
- source master session/turn/trace
- optional assignee
- optional active execution id
- optional parent task id and child task ids
- review state

Task and execution split:

- task owns durable work-item truth
- execution owns in-flight worker runtime activity
- one task may have multiple historical executions
- one execution must reference one task, one workspace, one session, and one worker
- worker progress is recorded against execution and admitted into task history by the task owner

## Persistence Layers

```text
Task Ledger       append-only truth
Task Snapshot     rebuildable cache
Task RuntimeState process memory cache and scheduler state
```

Paths:

```text
~/.freehand/state/workspaces/<workspace_key>/tasks/<task_id>.json
~/.freehand/state/workspaces/<workspace_key>/tasks/<task_id>.jsonl
~/.freehand/state/workspaces/<workspace_key>/tasks/index.json
~/.freehand/state/workspaces/<workspace_key>/executions/<execution_id>.json
~/.freehand/state/agents/<agent_id>.json
~/.freehand/state/agents/index.json
~/.freehand/state/workspaces/<workspace_key>/task-runtime/leases.json
```

Existing implementation paths may be migrated toward this workspace-scoped shape.
New task/workspace work should not deepen worker-scoped task persistence.

Mutation order:

```text
validate action
append ledger event
apply reducer to snapshot
atomic write snapshot
update index
update runtime memory state
```

If ledger append fails, memory must not change. If snapshot write fails, the mutation is not reported as complete.

`history` reads the append-only task ledger and returns ordered lifecycle events. UI task timelines and worker debug projection must use this owner API instead of reading ledger files directly.

`list_tasks` reads in-memory snapshot truth rebuilt from persistence and returns task snapshot projections filtered by status and assignee. Queue/debug/UI surfaces should use this owner API instead of scanning task snapshot files directly.

## Runtime Memory State

Memory state contains:

- task snapshots keyed by task id
- execution snapshots keyed by execution id
- agent snapshots keyed by agent id
- active task leases keyed by task id

Startup rebuilds memory state from snapshots and reconciles running-task leases. Future recovery will rebuild corrupt snapshots from ledger.

## Lease And Heartbeat

`Running` is lease-backed. Entering `Running` through `resume` writes a `TaskHeartbeat` ledger event and an active lease:

```text
task_id
execution_id
agent_id
workspace_id
lease_id
acquired_at
heartbeat_at
expires_at
```

Workers refresh the lease with `task(op="heartbeat", task_id, ttl_seconds)`. Heartbeat is accepted only for the assigned agent of a `Running` task. Heartbeat for `Assigned`, `Paused`, `Closed`, or unassigned tasks is rejected and must not write a lease.

When a task leaves `Running`, its lease is removed. Recovery must not infer task completion from lease state.

## Agent Registry

First skeleton always registers the owner agent as a self agent:

```text
status=Available
capabilities=code_edit,test_run,docs
```

Agent states:

```text
Available
Busy
Paused
Offline
Closing
Closed
Failed
```

Worker resource selection first version:

- `self` and `auto` pick an available agent.
- `none` creates `WaitingWorker` in the design model.
- explicit worker/agent id requires the resource to exist and be available.
- `assign` can bind `WaitingWorker`, `Created`, or `Interrupted` to an available worker resource.
- `claim_next` lets an agent claim its highest-priority assigned task into lease-backed `Running`.
- `record_execution` writes worker progress for a running task into the task ledger.
- `create_agent` creates an idle agent snapshot with declared capabilities.
- `close_agent` closes only idle agents with no current task, queued task, or running task.
- assigned tasks count as queued work; running tasks count as running work after heartbeat/resume.

Implementation naming note:

- current code still uses `TaskStatus::WaitingAgent` and `TaskWaitingAgent`
  ledger event names
- target design language is `WaitingWorker`
- renaming runtime status/event symbols requires a separate migration plan,
  compatibility handling, and ledger replay tests; this design update does not
  claim that migration is implemented

## Startup And Recovery

Startup sequence:

```text
load task snapshots
load task leases
load or create self agent snapshot
interrupt running tasks with missing, mismatched, inactive, or expired leases
rebuild runtime memory maps
return runtime ready
```

Recovery requirements:

- corrupted/missing snapshot replays ledger
- running task requires valid lease and live agent heartbeat
- expired or missing lease becomes `Interrupted`
- recovery never promotes `Running` to completed

## Review Closure

Workers cannot directly close tasks. They submit review with deliverables and evidence. Reviewer/master/user approves or rejects. Approval may close. Rejection returns the task to execution with required changes.

## Current Implementation Scope

Implemented:

- task persistence crate
- create/query/list_agents/query_agent
- self-agent registry skeleton
- create with self/auto assignment or current legacy `WaitingAgent` implementation status
- append, pause, resume, submit_review, approve, reject, close
- assign, cancel, create_agent, close_agent
- review-before-close transition validation
- lease-backed Running state with heartbeat and boot interruption recovery
- runtime task tool bridge

Not implemented:

- real worker execution
- global Task Center query/sync truth
- Agent Lifecycle truth and AgentBoard projection
- scheduler tick timer events
- master poll loop over TaskBoard and AgentBoard
- runtime control channel to running workers
- queue selection loop
- UI task projection
- full task/node/UI error.center classification beyond the first schema/tool/provider skeleton
- workspace owner integration and workspace-scoped task persistence migration
- execution query/subscribe projection
