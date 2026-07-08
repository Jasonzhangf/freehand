# Task Center Truth

## Status

Design truth for the multi-task management foundation.

This document defines the global Task Center. It is not an implementation claim.
The current implementation has a task lifecycle skeleton in `freehand-task`;
Task Center expansion is the next foundation layer after the single-agent
lifecycle closeout.

## Purpose

Freehand now has the base ability for one agent to reason, call tools, repair
tool/schema/provider failures, persist turns, project UI state, and recover after
restart. The next foundation is multi-task management.

The Task Center is the global task truth for all agents:

- every master-created task registers here
- every worker execution binds here
- task dispatch, review, retry, close, and recovery flow through this owner
- agent-local task state synchronizes into this center through typed events
- UI, master prompts, headless tests, and scheduler ticks query this center

Task Center is not a UI list and not a model memory. It is an owner-backed
runtime truth with append-only events and rebuildable snapshots.

## Core Decision

Task Center owns "what work exists and what state it is in".

Agent Lifecycle owns "what each agent is doing right now".

Execution Binding joins them:

```text
TaskCenterTruth
  BigTask / SubTask / Execution / Review / EventInbox / SchedulerTick

AgentLifecycleTruth
  AgentState / CurrentActivity / RuntimeStats / ErrorProfile

ExecutionBindingTruth
  agent_id <-> task_id <-> subtask_id <-> execution_id
```

This split lets the system answer both questions:

- from a task: who is executing it, what is the current execution status, and
  what is blocking it
- from an agent: what task is it executing, what is in its task list, and what
  is it currently doing

## Responsibilities

### Task Registration

The Task Center registers:

- one or more `BigTask` objects
- child `SubTask` objects
- execution attempts by agents
- review submissions and review decisions
- task dependency edges
- task events and scheduler events

Phase 1 uses one active `BigTask` with multiple subtasks. Multiple independent
big tasks and cross-session context switching are future work.

### Dispatch And Assignment

Task dispatch must enter through admitted actions. The framework must not create
tasks by sniffing paths or assistant prose.

Allowed path:

```text
master model action
  -> task action validation
  -> Task Center mutation
  -> execution binding
  -> projection/query/subscription update
```

Forbidden path:

```text
user text or raw path
  -> framework silently creates task/worker/session
```

### State Synchronization

Agent-local execution state synchronizes into Task Center through typed events:

```text
Agent runtime event
  -> AgentLifecycle reducer
  -> ExecutionFact
  -> Task Center transition
  -> TaskBoard projection
```

Examples:

- worker enters model thinking -> execution remains running with
  `current_activity=model_thinking`
- worker tool fails but result is paired back to the model -> execution becomes
  `recovering`, not `failed`
- worker reports permission missing -> execution becomes `blocked` and emits a
  blocker event
- worker submits deliverables -> execution becomes `review_ready` and creates a
  review event

### State Query

Task Center must support query by task, agent, worker, status, blocker, review,
and stale/timeout state.

Minimum query surface:

```text
QueryTaskCenter
QueryTask(task_id)
QueryTaskList(status?, agent_id?, parent_task_id?)
QueryExecution(execution_id)
QueryExecutions(agent_id?, task_id?, status?)
QueryAgentTasks(agent_id)
QueryTaskBoard
QueryReviewQueue
QueryBlockedTasks
QueryStaleExecutions
SubscribeTaskCenter
SubscribeAgentTasks(agent_id)
```

Master commonly uses:

```text
QueryTaskBoard
QueryAgentBoard
QueryBlockedTasks
QueryReviewQueue
QueryStaleExecutions
QueryAgentTasks(worker_id)
```

UI commonly uses:

```text
QueryTaskBoard
QueryAgentBoard
SubscribeTaskCenter
SubscribeAgentLifecycle
```

Worker commonly uses:

```text
RegisterExecution
UpdateExecutionProgress
ReportBlocked
SubmitExecutionResult
ReceiveReviewFeedback
MarkRetryStarted
```

## Truth Objects

### BigTask

Represents the user-facing task scope that the master is managing.

Fields:

```text
big_task_id
goal
status
completion_policy
required_subtask_ids
accepted_summary
created_by_agent
created_at
updated_at
event_cursor
```

Phase 1 has one active BigTask per master task-management loop.

### SubTask

Represents one work unit inside a BigTask.

Fields:

```text
task_id
parent_task_id
workspace_id
session_id
goal
deliverables
acceptance
priority
status
depends_on
blocks
assigned_agent
active_execution_id
created_at
updated_at
next_check_at
soft_timeout_at
hard_timeout_at
blocker
review_state
```

### Execution

Represents one agent's attempt to execute one subtask.

Fields:

```text
execution_id
task_id
agent_id
workspace_id
session_id
attempt
status
current_activity
started_at
last_heartbeat_at
last_progress_at
next_check_at
soft_timeout_at
hard_timeout_at
retry_count
error_profile
submission_id
```

Execution status family:

```text
created
running
model_thinking
tool_running
recovering
schema_polishing
provider_retrying
blocked
review_ready
rejected_retrying
approved
closed
failed
cancelled
interrupted
```

### Worker Task Index

Lets the system answer "what is this agent doing".

Fields:

```text
agent_id
active_execution_id
active_task_id
task_list
status
capacity
last_seen_at
```

### Event Inbox

Task Center owns an event inbox for master-visible task-management events.

Events include:

```text
task_created
task_assigned
execution_started
progress_reported
execution_blocked
execution_stale
execution_soft_timeout
execution_hard_timeout
review_ready
review_approved
review_rejected
retry_started
task_closed
agent_offline
```

Events must carry a cursor so master can process events once:

```text
event_id
event_cursor
kind
task_id
execution_id optional
agent_id optional
created_at
payload
processed_by_master_cursor optional
```

### Scheduler Tick

The framework owns time and state transitions. Scheduler ticks are explicit
truth, not hidden timers.

Tick responsibilities:

```text
scan running executions
scan worker heartbeat
scan task dependencies
scan review queue
scan error-center summaries
detect soft_timeout / hard_timeout / stale / blocker / review_ready
append scheduler events
produce board snapshot
wake master when needed
```

Task Center may recommend next actions. It must not make business decisions for
the master model.

## State Transition Principles

### Running Does Not Mean Unknown

Running execution should always have current activity and timer facts:

```text
running + model_thinking elapsed=42s
running + tool_running target=...
running + recovering retry=2
```

### Recovering Is Not Failed

Tool failure, schema mismatch, and provider retry are distinct:

```text
tool failure paired to worker model -> recovering
schema mismatch before retry cap -> schema_polishing
provider retry before exhausted -> provider_retrying
provider exhausted -> failed/system_error
```

### Blocked Requires Master Attention

Blocked means the worker cannot proceed without master/user/environment action.

Examples:

```text
missing permission
missing user input
missing dependency
workspace precondition not satisfied
```

### Timeout Is Not Automatically Failure

Timeouts must be typed:

```text
soft_timeout:
  not failed; master can continue waiting or switch to another subtask

stale:
  no progress/heartbeat beyond threshold; master should query/check/reassign

hard_timeout:
  execution exceeded policy; master must decide cancel/reassign/fail/block
```

## Persistence Direction

Task Center truth should remain append-only ledger plus rebuildable snapshots.

Target paths:

```text
~/.freehand/state/task-center/tasks/<task_id>.json
~/.freehand/state/task-center/tasks/<task_id>.jsonl
~/.freehand/state/task-center/executions/<execution_id>.json
~/.freehand/state/task-center/agents/<agent_id>.json
~/.freehand/state/task-center/event-inbox.jsonl
~/.freehand/state/task-center/scheduler-ticks.jsonl
```

Existing `freehand-task` paths may migrate toward this shape through a separate
implementation plan. This design does not claim migration is implemented.

## Phase 1 Scope

In scope:

- one active BigTask
- multiple SubTasks inside that BigTask
- Task Center registration for BigTask/SubTask/Execution
- agent task index
- worker lifecycle synchronization into execution truth
- blocker/review/stale/timeout event projection
- master can query by agent id and task board
- restart restores Task Center, execution bindings, and event cursor

Out of scope:

- multiple independent BigTasks
- user task context switching
- cross-session master context switching
- complex resource optimization
- worker pool autoscaling
- cross-machine worker transport
- large UI management console

## Relationship To Existing Docs

- `task-orchestration-design.md` owns current task runtime lifecycle skeleton.
- This document defines the target global Task Center foundation.
- `agent-lifecycle-semantics.md` defines per-agent runtime state.
- `master-worker-task-state-machine-phase1.md` defines the first state-machine
  slice that combines Task Center and Agent Lifecycle.
