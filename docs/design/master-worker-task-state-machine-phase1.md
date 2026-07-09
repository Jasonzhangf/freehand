# Master Worker Task State Machine Phase 1

## Status

Design truth for the first multi-task foundation slice.

This phase extends Freehand from a single-agent lifecycle into task management
foundations. It does not implement full multi-agent scheduling or multi-big-task
context switching.

## Scope

Phase 1 focuses on one active BigTask with multiple SubTasks.

In scope:

- Task Center truth for one BigTask and its SubTasks
- Agent Lifecycle truth for master and worker agents
- worker execution progress/block/review/retry lifecycle
- framework timers, scheduler ticks, stale/timeout detection
- master poll loop over TaskBoard and AgentBoard
- runtime control commands to running workers at safe points
- restart recovery of task/execution/event cursor/binding truth

Out of scope:

- multiple independent BigTasks
- user task context switching
- cross-session master context switching
- complex global worker-pool optimization
- worker autoscaling
- cross-machine worker transport
- large task-management UI

## Three-Party Responsibility Model

```text
Master Agent
  task manager and decision maker
  decomposes, dispatches, reviews, adjusts, reports

Worker Agent
  executor
  claims/executes work, reports progress/blockers, submits results, retries

Framework Task Runtime
  task truth, lifecycle truth, timers, events, projections, recovery
```

The framework senses state and time. The model decides business action.

## Master Poll Loop

Master is not a synchronous waiter. It runs a loop over board truth.

```text
Tick or Event Wake
  -> Load TaskBoard + AgentBoard + EventInbox cursor
  -> Classify current board state
  -> Ask master model for next action
  -> Validate and apply admitted actions
  -> Persist board/cursor/timers
  -> Schedule next tick or wait
```

Board classification includes:

```text
has user input
has review_ready submission
has blocked execution
has stale execution
has soft_timeout or hard_timeout
has idle worker
has ready subtask
has all required subtasks closed
```

Master semantic actions:

```text
create_subtask
dispatch_subtask
query_agent_lifecycle
query_task_board
ask_runtime_question
inject_constraint
approve_submission
reject_submission
split_unblock_task
reassign_execution
pause_or_cancel_execution
wait_with_next_check
close_big_task
report_to_user
```

These are semantic action categories, not exposed runtime tool names. Runtime
execution must use the small owner-scoped tool surface and typed `op` parameters
defined in `master-worker-tool-action-contract-phase1.md`.

## Worker Execution Loop

Worker execution is not a one-shot result. It updates lifecycle and task state
throughout reasoning.

```text
Assigned
  -> Claimed
  -> ExecutionStarted
  -> ModelThinking
  -> ToolRunning
  -> ProgressReported
  -> Recovering
  -> Blocked
  -> ReviewReady
  -> AwaitReview
  -> RejectedRetrying
  -> Approved
  -> Released
```

Worker requirements:

- heartbeat/progress must be typed and recorded
- blocker must be reported explicitly
- tool/schema/provider errors must be classified
- local recoverable errors should not become task failure
- submission waits for review
- rejection must link feedback to retry
- approved execution releases the worker resource

## Single-Agent Task Space State Machine

Long-running single-agent work uses a framework-owned task space. The task
space is persisted context, similar to a scoped skill/memory surface, and is
injected into later model requests.

The model must return a structured candidate state machine at stop points. The
framework compares only explicit standard fields, not natural language meaning.

Standard machine-checkable fields:

```text
schema_version
simple_question
phase
current_step
next_step
completed
blocked
needs_user_input
target_alignment
progress_percent
retry_count
```

Rules:

- `simple_question=true` means the previous user input was a simple question or
  answer request. If the provider also naturally stops, the framework may allow
  the stop instead of forcing long-task continuation.
- `simple_question` is a standard boolean field, not a prose hint.
- task target text is not machine-corrected by the framework. The framework
  stores the original user target and the model's current target understanding,
  injects both into later prompts, and asks the model to review drift.
- framework validation checks enumerations, booleans, required fields, allowed
  phase transitions, completion consistency, and missing next-step/blocker
  fields.
- framework validation must not compare two natural-language target strings and
  decide they are semantically equivalent or wrong.
- when structured fields are missing or inconsistent, the response is a schema
  mismatch/polishing retry, not provider failure.

## Framework Scheduler Tick

The framework owns time. Models do not.

Scheduler tick:

```text
scan executions
scan heartbeat/progress timestamps
scan error-center and lifecycle facts
scan review queue
scan task dependencies
detect stale/timeout/block/review events
append scheduler events
update TaskBoard projection
wake master when needed
```

Timer fields:

```text
entered_at
elapsed_ms
last_progress_at
last_heartbeat_at
next_check_at
soft_timeout_at
hard_timeout_at
blocked_since
review_since
retry_count
```

Timeout meanings:

```text
soft_timeout:
  short-term no completion; master can continue waiting or switch to another
  ready subtask inside the same BigTask

stale:
  no progress/heartbeat beyond threshold; master should inspect worker

hard_timeout:
  execution exceeded policy; master must decide cancel/reassign/fail/block
```

## State Flow

```text
User request
  -> Master creates BigTask
  -> Master decomposes SubTasks
  -> Task Center registers SubTasks
  -> Master dispatches one or more SubTasks
  -> Workers execute independently
  -> Framework tracks timers and lifecycle facts
  -> Master reads board snapshots on wake/tick
  -> Master reviews/blocker-handles/retries/reassigns
  -> Required SubTasks close
  -> BigTask closes with accepted summary
```

## Error And Recovery Semantics

### Recovering

Worker can continue without master intervention.

Examples:

- failed tool result has been paired back to worker model
- schema mismatch is being polished
- provider retry has not exhausted retry budget

Master should not duplicate dispatch or mark task failed.

### Blocked

Worker cannot continue without outside action.

Examples:

- permission missing
- user input missing
- dependency missing
- workspace precondition missing

Master should inspect blocker and decide whether to ask user, create unblock
subtask, reassign, or pause.

### Failed

Execution cannot continue.

Examples:

- provider retry exhausted
- worker process crashed
- hard timeout policy requires termination
- persistence/system error blocks owner mutation

Master decides retry/reassign/fail/report.

## Runtime Control During Worker Reasoning

Master can interact with a running worker through control channel, not by
mutating worker prompt history directly.

Commands:

```text
query_status
ask_runtime_question
inject_constraint
pause_execution
cancel_execution
request_checkpoint
request_submission_now
```

Framework-answerable queries return lifecycle facts immediately.

Worker-model-answerable questions enter worker control inbox and are handled at
safe points.

All control command lifecycle events are persisted:

```text
command_received
command_deferred
command_answered
command_rejected
execution_resumed
```

## Phase 1 Acceptance

The first implementation should prove:

1. Master creates one BigTask.
2. Master decomposes it into SubTasks.
3. Task Center registers BigTask/SubTask/Execution.
4. Master dispatches a SubTask to a worker and does not block synchronously.
5. Worker lifecycle projects model_thinking/tool_running/recovering states.
6. Worker progress/heartbeat updates Task Center.
7. Framework detects soft timeout/stale from timers and notifies master.
8. Worker reports blocked; Task Center projects blocker event.
9. Master reads board snapshot and chooses unblock/reassign/ask-user action.
10. Worker submits result; Task Center emits review_ready.
11. Master approves or rejects.
12. Rejection causes linked retry with feedback.
13. Approval closes SubTask.
14. Required SubTasks closed means BigTask closes.
15. Restart restores BigTask/SubTask/Execution/Agent binding/Event cursor.

## Headless Samples Direction

Target samples:

```text
master-single-task-loop-sample
subtask-timeout-notify-sample
subtask-blocker-adjust-sample
worker-review-reject-retry-sample
worker-observation-sample
worker-runtime-status-query-sample
worker-runtime-question-resume-sample
task-center-restart-sample
```

Samples must query Task Center and Agent Lifecycle truth. Model prose is not
proof.

## Relationship To Other Docs

- `task-center-truth.md` defines global task truth.
- `agent-lifecycle-semantics.md` defines agent runtime state.
- `master-worker-prompt-contract-phase1.md` defines model-facing behavior.
- `task-orchestration-design.md` defines the current implemented task runtime
  skeleton.
- `multi-agent-dispatch-alignment.md` contains broader future dispatch
  alignment and remains superseded by this document for Phase 1 scope.
