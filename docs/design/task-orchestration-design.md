# Task Orchestration Design

## Status

Initial skeleton implementation is in progress. This document is the durable design truth for task lifecycle, persistence, memory state, startup, recovery, and agent registry.

## Principles

- One built-in tool surface: `task`.
- `task` uses `op` parameters instead of many tool names.
- Status schema has no side effects; only admitted task tool actions mutate task truth.
- Task truth is append-only ledger plus rebuildable snapshot.
- Runtime memory state is a cache and scheduler surface, not truth.
- Agent and cwd are not bound. Cwd is task execution context.
- Worker agents submit review; close requires approval or explicit close action.

## Tool Surface

First implemented ops:

- `create`
- `query`
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

- `assign`
- `cancel`
- `create_agent`
- `close_agent`

## Task Lifecycle

Primary path:

```text
Created -> Assigned -> Running -> ReviewSubmitted -> Approved -> Closed
```

Branches:

```text
Created -> WaitingAgent -> Assigned
Running -> Paused -> Running
Running -> Interrupted -> Running
Running -> Blocked -> Running
Running -> Running (record_execution)
ReviewSubmitted -> Rejected -> Running
Running -> Failed -> Running | Closed
Running -> Cancelled -> Closed
Assigned -> Cancelled
```

`Draft` is intentionally absent. A model task action creates a real task.

## Task Fields

Every task has:

- `title`
- `content`
- `goal`
- `deliverables`
- `acceptance`
- `priority`
- optional `target_cwd`
- optional assignee
- parent session/turn/trace
- review state

## Persistence Layers

```text
Task Ledger       append-only truth
Task Snapshot     rebuildable cache
Task RuntimeState process memory cache and scheduler state
```

Paths:

```text
~/.freehand/ledgers/tasks/<agent_id>/<task_id>.jsonl
~/.freehand/state/tasks/<agent_id>/<task_id>.json
~/.freehand/state/tasks/<agent_id>/index.json
~/.freehand/state/agents/<agent_id>.json
~/.freehand/state/agents/index.json
~/.freehand/state/task-runtime/<agent_id>/leases.json
```

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

## Runtime Memory State

Memory state contains:

- task snapshots keyed by task id
- agent snapshots keyed by agent id
- active task leases keyed by task id

Startup rebuilds memory state from snapshots and reconciles running-task leases. Future recovery will rebuild corrupt snapshots from ledger.

## Lease And Heartbeat

`Running` is lease-backed. Entering `Running` through `resume` writes a `TaskHeartbeat` ledger event and an active lease:

```text
task_id
agent_id
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

Agent selection first version:

- `self` and `auto` pick an available agent.
- `none` creates `WaitingAgent`.
- explicit `agent` requires the agent to exist and be available.
- `assign` can bind `WaitingAgent`, `Created`, or `Interrupted` to an available agent.
- `claim_next` lets an agent claim its highest-priority assigned task into lease-backed `Running`.
- `record_execution` writes worker progress for a running task into the task ledger.
- `create_agent` creates an idle agent snapshot with declared capabilities.
- `close_agent` closes only idle agents with no current task, queued task, or running task.
- assigned tasks count as queued work; running tasks count as running work after heartbeat/resume.

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
- create with self/auto assignment or WaitingAgent
- append, pause, resume, submit_review, approve, reject, close
- assign, cancel, create_agent, close_agent
- review-before-close transition validation
- lease-backed Running state with heartbeat and boot interruption recovery
- runtime task tool bridge

Not implemented:

- real worker execution
- queue selection loop
- UI task projection
- error.center classification
