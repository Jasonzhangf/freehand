# Framework-Mediated Agent Operations

## Status

Design truth for how Freehand agents, tasks, executions, and runtime control
communicate through the framework.

This document clarifies a boundary that spans Task Center, Agent Lifecycle,
runtime control, and future worker queues. It distinguishes implemented Phase 1
truth from target Phase 2 behavior.

## Core Principle

Agents do not privately mutate each other, privately assign work, or privately
declare durable task truth.

All durable work and runtime coordination must enter through framework-owned
surfaces:

```text
Agent / UI / CLI / scheduler
  -> framework command or owner-scoped tool action
  -> owner validation
  -> owner ledger + snapshot
  -> board/lifecycle projection
  -> query/subscription/prompt context
```

The framework owns admission, validation, persistence, query, and recovery.
Models and UIs choose actions and render projections, but do not own truth.

## Operation Families

### Task Operations

Task operations mutate or query durable work state.

Owner:

```text
task.orchestration / Task Center
```

Model-facing direction:

```text
task(op=...)
```

Examples:

```text
task(op="create")
task(op="assign")
task(op="claim_next")
task(op="record_execution")
task(op="submit_review")
task(op="approve")
task(op="reject")
task(op="close")
task(op="history")
task(op="list_tasks")
task(op="query_board")
```

Rules:

- task creation, assignment, review, rejection, approval, close, and task
  history stay in Task Center
- worker progress/block/review facts enter as typed execution facts or task
  execution records
- invalid op, invalid args, wrong state, or permission mismatch returns a
  paired action/tool validation result
- no task mutation is allowed through Agent Lifecycle or UI-local state

### Agent Registry Operations

Agent registry operations manage worker resource registration.

Current owner:

```text
task.orchestration
```

Current direction:

```text
task(op="create_agent")
task(op="close_agent")
task(op="list_agents")
task(op="query_agent")
```

These operations register resources available to the Task Center. They are not
the same as Agent Lifecycle and do not describe what an agent is doing right
now.

### Agent Lifecycle Operations

Agent Lifecycle is not a model-facing mutation tool.

Owner:

```text
agent.lifecycle
```

Inputs:

```text
typed runtime/provider/tool/error/task events
```

Outputs:

```text
AgentLifecycleSnapshot
AgentBoardProjection
QueryAgentLifecycle(agent_id)
QueryAgentBoard
```

Rules:

- lifecycle truth is an intrinsic agent state/projection
- lifecycle reducer must not parse raw assistant prose
- model claims such as "I am blocked" are not lifecycle truth until admitted as
  typed status/action/event truth
- UI and master prompts consume AgentBoard/AgentLifecycle projection, not raw
  logs or user-visible transcript guesses

### Worker Runtime Control

Worker runtime control is for safe-point interaction with a running execution.

Planned owner:

```text
worker_control / runtime control channel
```

Planned model-facing direction:

```text
worker_control(op=...)
```

Allowed operations:

```text
query_status
ask_at_safe_point
add_constraint
request_checkpoint
request_submission_now
pause
resume
cancel
```

Forbidden operations:

```text
create task
assign task
claim next task
approve review
reject review
close task
mutate session truth
rewrite raw transcript
silently modify prompt history
```

Any task-state consequence from a control operation must flow back through Task
Center events. `worker_control` may request a pause or cancel, but Task Center
records the durable state transition.

## Agent-To-Agent Communication

Agents communicate through framework-owned queues and projections, not direct
private mutation.

Target paths:

```text
master -> task action -> Task Center -> worker task queue
worker -> execution fact -> Task Center -> master event inbox
master -> worker_control command -> worker control inbox -> safe point answer
worker -> review submission -> Task Center review queue -> master decision
```

The framework persists queue events and lifecycle consequences:

```text
task_assigned
execution_claimed
progress_reported
execution_blocked
review_ready
review_rejected
retry_started
worker_control_received
worker_control_deferred
worker_control_answered
worker_control_rejected
execution_resumed
```

## Persistence Contract

Every admitted durable operation must be reconstructable after restart.

Required truth families:

```text
task ledger and task snapshot
agent registry snapshot
execution fact ledger and execution projection
scheduler tick ledger
agent lifecycle snapshot/projection
event inbox cursor
worker control inbox events
```

Phase 1 has verified restart recovery for TaskBoard, AgentBoard,
ExecutionFact, SchedulerTick, and lifecycle projection through headless ADP/CLI
proof. Worker control inbox recovery is target behavior and not yet
implemented.

## Current Implementation Status

Implemented and verified in Phase 1:

- TaskBoard query skeleton
- AgentLifecycleSnapshot and AgentBoard projection skeleton
- ExecutionFact sync for running, recovering, blocked, and review_ready facts
- SchedulerTick facts for stale/timeout style sensing without business
  decisions
- headless `phase1-foundation-sample`
- restart same-id verify for task, review task, execution, and agent ids

Partially implemented before or during Phase 1:

- task runtime lifecycle
- task tool op surface
- agent registry skeleton
- assignment, claim, heartbeat, progress record, review, approve, reject, close
- task ledger/snapshot recovery

Not implemented yet:

- real worker task queue notification and automatic worker claim loop
- master poll loop that consumes TaskBoard + AgentBoard and chooses next action
- worker control inbox and safe-point handling
- Agent-to-Agent runtime question/answer through framework queue
- multi-worker orchestration sample
- UI task/agent dashboard projection

## Design Consequences

1. UI is a projection and command ingress only.
2. Master decides business actions, but the framework validates and persists.
3. Framework senses time and state, but does not make business decisions.
4. Worker execution is not a synchronous function call; it reports typed facts.
5. Agent Lifecycle is state, not a task mutation surface.
6. Worker Control is runtime coordination, not Task Center mutation.
7. Every new operation must map to exactly one owner before implementation.

## Relationship To Other Docs

- `task-center-truth.md` defines Task Center objects and query direction.
- `agent-lifecycle-semantics.md` defines per-agent lifecycle truth.
- `master-worker-tool-action-contract-phase1.md` defines the small tool surface.
- `master-worker-task-state-machine-phase1.md` defines the first state-machine
  lifecycle.
- `multi-task-foundation-implementation-plan.md` defines staged closure order.
