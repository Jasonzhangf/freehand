# Agent Lifecycle Semantics

## Status

Design truth for per-agent runtime lifecycle semantics.

This document applies to every agent, whether the agent is acting as master or
worker. It is not a UI design, not a task design, and not a model-facing tool
surface. It defines how an agent's reasoning process is reduced into structured
lifecycle truth.

## Purpose

Every agent is a long-running reasoning execution body. During reasoning, it
must continuously maintain a live state that answers:

- what is it doing
- how long has it been doing it
- which task/execution/turn is it bound to
- what model is it using
- how many model calls has it made
- how many tools has it called
- what errors happened
- what is the current activity
- what happened in the previous activity
- whether it is recovering, blocked, waiting review, or failed

UI, master, scheduler, and debug surfaces should not infer this from raw logs or
raw terminal text. They should consume lifecycle truth.

## Core Decision

Agent Lifecycle is the semantic reducer from runtime events into agent state:

```text
runtime/term/provider/tool/error events
  -> AgentLifecycleReducer
  -> AgentLifecycleTruth
  -> AgentBoardProjection
  -> UI / Master / Scheduler / Debug
```

Raw events remain available for debug/replay, but normal consumers use typed
lifecycle truth.

Agent Lifecycle is an intrinsic agent property. It should be queryable through
framework projections such as AgentBoard, ADP/debug query surfaces, scheduler
inputs, and master context summaries. It should not become a separate
model-facing mutation tool unless a later owner map proves a distinct action
surface is required.

## Truth Model

### Agent Lifecycle Snapshot

Minimum fields:

```text
agent_id
role = master | worker
alive
state
state_entered_at
elapsed_ms
current_task_id optional
current_subtask_id optional
current_execution_id optional
current_turn_id optional
current_activity
last_activity
runtime_stats
error_profile
last_seen_at
next_check_at
```

### Agent State

Shared state family:

```text
idle
assigned
planning
dispatching
model_thinking
streaming
tool_ready
tool_running
tool_result_returned
recovering
schema_polishing
provider_retrying
blocked
waiting_review
reviewing
paused
cancelling
completed
failed
offline
```

Master and worker share this vocabulary, but not every state is equally common
for both roles.

### Current Activity

Current activity is the compact semantic answer to "what is the agent doing
right now".

Examples:

```text
planning subtasks
waiting for model response
reading crates/freehand-runtime/src/lib.rs
running cargo test -p freehand-runtime
recovering from failed read_file
waiting for provider retry 3/5
blocked by missing permission
reviewing worker submission
```

Activity fields:

```text
kind
semantic_summary
target optional
started_at
elapsed_ms
tool_name optional
model optional
retry_count optional
visibility = public | compact | debug
```

### Runtime Stats

Runtime stats track the agent's live and historical behavior.

Fields:

```text
started_at
uptime_ms
turn_count
model_request_count
model_retry_count
tool_call_count
tool_failure_count
schema_polish_count
provider_error_count
blocked_count
successful_submission_count
rejected_submission_count
current_model
error_rate
```

Error rate is a projection metric, not an owner decision. It should be
calculated from typed error counters.

### Error Profile

Error profile gives master and UI a decision-ready summary:

```text
tool_failures
provider_errors
schema_mismatches
permission_errors
persistence_errors
system_errors
recovering
blocked
last_error_kind
last_error_code
last_recovery_action
```

## Runtime Event Inputs

Lifecycle truth is reduced from typed events, including:

```text
provider request started
provider response chunk
provider response completed
provider retry scheduled
provider retry exhausted
tool call received
tool execution started
tool result returned
tool failed
schema mismatch
schema polished
turn terminal
heartbeat
progress report
blocked report
runtime command received
runtime command answered
submission ready
review approved
review rejected
agent disconnected
agent reconnected
```

The reducer must not parse raw assistant prose as lifecycle truth.

## Derived Facts

### Model Thinking

Represents waiting for model/provider output.

```text
state = model_thinking
activity = waiting for model response
fields = provider, model, turn_id, started_at, elapsed_ms
```

### Tool Running

Represents tool execution.

```text
state = tool_running
activity = semantic tool action
fields = tool_name, target, started_at, elapsed_ms
```

Tool display semantics must reuse protocol/tool display owner logic. UI must
not reclassify raw tool names.

### Recovering

Represents a non-terminal local repair loop.

Examples:

```text
tool failure paired back to model
schema mismatch returned as polishing feedback
provider retry scheduled but not exhausted
```

Recovering means the worker can likely continue without master intervention.

### Blocked

Represents action-needed state.

Examples:

```text
missing permission
missing user input
missing dependency
unavailable workspace precondition
```

Blocked should emit an event to Task Center and Master inbox.

### Review Ready

Represents worker submission ready for master/reviewer decision.

```text
state = waiting_review
activity = submitted deliverables and evidence
```

## Runtime Control Channel

Master may send runtime commands to a running worker without treating the worker
as a synchronous function call.

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

### Framework-Answerable Commands

These can be answered from lifecycle/task truth without interrupting the worker
model:

```text
query_status
query_elapsed_time
query_current_tool
query_last_progress
query_error_profile
query_retry_count
```

### Worker-Model-Answerable Commands

These require worker model involvement at a safe point:

```text
ask why it chose a strategy
ask for current findings
ask for partial conclusion
ask whether a blocker can be bypassed
inject new constraint into next reasoning step
```

### Safe Points

Worker execution checks control inbox at safe points:

```text
after provider response
before next provider request
after tool result
before tool execution
after schema mismatch feedback
before retry/backoff sleep
before submission
```

Unsafe points:

```text
while committing a critical file mutation
while writing terminal truth
while holding checkpoint/rewind critical section
while persisting ledger/snapshot mutation
```

All command acceptance, deferral, rejection, answer, and resume events must be
written as execution events.

## Agent Board Projection

Agent Board summarizes every agent:

```text
agent_id
role
alive
state
current_task_id
current_execution_id
current_activity
elapsed_ms
model
model_request_count
tool_call_count
tool_failure_count
error_rate
needs_master_action
```

Example:

```text
master: planning subtasks · 12s · MiniMax-M3 · 3 model calls
worker-atlas: reading runtime source · 3m03s · 8 tools · 1 recovered error
worker-beacon: blocked · missing permission · 5m41s · needs master action
```

## Query And Subscription Direction

Minimum protocol surface:

```text
QueryAgentLifecycle(agent_id)
QueryAgentBoard
SubscribeAgentLifecycle(agent_id)
SubscribeAgentBoard
```

These are target capabilities. Implementation must route through owner-backed
truth, not UI-local state.

## UI Contract

UI renders lifecycle truth:

- model thinking with elapsed time
- current tool semantic and target
- provider retry count and next retry time
- recovering versus blocked versus failed
- current task/execution binding
- current and last activity
- model/tool/error counters

UI must not infer agent state from raw ids, logs, or text.

## Master Prompt Contract

Master receives Agent Board summary as context. The prompt should direct:

- do not interrupt agents that are recovering and still making progress
- inspect blockers and stale executions before dispatching new work
- review `waiting_review` agents first when their result can unblock the task
- ask runtime questions only when the lifecycle summary is insufficient
- query status through lifecycle tools, not by reading raw logs

## Relationship To Task Center

Agent Lifecycle does not own tasks. It reports agent state and current
execution binding. Task Center owns task/execution state and uses lifecycle facts
to update task board.

```text
AgentLifecycleTruth
  -> ExecutionFact
  -> TaskCenterTruth
```

## Phase 1 Scope

In scope:

- master lifecycle: planning, dispatching, waiting, reviewing
- worker lifecycle: model_thinking, tool_running, recovering, blocked,
  waiting_review
- elapsed time and current/last activity
- model/tool/error counters
- ADP query/subscription direction
- Task Center synchronization contract

Out of scope:

- full UI dashboard
- multi BigTask context switching
- cross-machine lifecycle transport
- advanced health scoring
- model-quality benchmarking
