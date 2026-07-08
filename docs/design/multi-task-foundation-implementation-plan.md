# Multi-Task Foundation Implementation Plan

## Status

Planning document for implementing the multi-task management foundation after
the single-agent lifecycle closeout.

This plan intentionally starts with foundation design and owner maps before
runtime implementation. The goal is to avoid attaching multi-task behavior to
agents before Task Center and Agent Lifecycle truth exist.

## Current Baseline

Already completed:

- single-agent reasoning lifecycle
- multi-round tool failure repair
- schema polishing
- provider retry distinction
- same-session continuation
- WebUI/ADP lifecycle proof
- deterministic single-agent task lifecycle commands
- task runtime skeleton with task/agent registry, assignment, claim, heartbeat,
  review, close, and recovery
- repaired-failure prompt context economy
- Phase 1 headless foundation: TaskBoard, AgentBoard, AgentLifecycle snapshot,
  ExecutionFact sync, SchedulerTick facts, S-profile sample, and restart
  same-id proof

Phase 1 closed the minimum queryable/recoverable foundation. The current gap is
no longer "does framework truth exist"; it is "do real master/worker execution
loops communicate through that truth".

Still missing after Phase 1:

- worker task queue notification and automatic or explicit worker claim loop
- master poll loop over TaskBoard and AgentBoard
- runtime control channel to running workers
- EventInbox cursor and subscription surface beyond board snapshots
- real lifecycle event coverage from live worker execution
- Phase 2 master/worker prompt contract wiring
- ADP/CLI samples for master/worker execution closure
- UI task/agent projection after the headless loop is real

## Existing Design Chapters

### `task-orchestration-design.md`

Current role:

- durable task lifecycle skeleton
- task tool op surface
- task persistence and lease heartbeat
- agent registry skeleton
- review-before-close rule

Gap:

- not yet a global Task Center
- no execution query/subscription truth
- no worker lifecycle synchronization
- no scheduler tick/timer event truth

### `workspace-session-execution-taxonomy.md`

Current role:

- canonical vocabulary
- session belongs to workspace, not worker
- worker is resource
- execution is worker runtime activity
- task is workspace-scoped work item

Gap:

- does not yet define global Task Center query/sync semantics
- does not yet define Agent Lifecycle truth

### `multi-agent-dispatch-alignment.md`

Current role:

- Codex/Reasonix comparison
- model-triggered dispatch direction
- active dispatch prompt and condition matrix
- worker subscription and context-admission direction

Gap:

- broader than Phase 1
- includes future parallel/multi-task ideas that should not be implemented before
  Task Center and Agent Lifecycle foundations

### `node-master-slave-design.md`

Current role:

- local master/slave topology
- pairing permission and direct message baseline
- delegated task and slave turn subscription direction

Gap:

- transport-oriented and topology-oriented
- not the Task Center truth owner

### `reason-context-planner-design.md`

Current role:

- typed context admission
- subagent conclusion admission
- successful-history preference after failed repair

Gap:

- depends on Task Center accepted summary/conclusion events for future
  multi-task context admission

### `reason-rewrite-policy-design.md`

Current role:

- compaction/rollback/resume-rebuild/prune policy
- superseded failed-attempt pruning rule

Gap:

- needs Task Center and Agent Lifecycle events to decide future multi-task
  context pruning and accepted-summary admission

## New Foundation Design Docs

### `task-center-truth.md`

Defines:

- global Task Center
- BigTask/SubTask/Execution/Review/EventInbox/SchedulerTick truth
- task registration, dispatch, sync, query, restart recovery
- state synchronization from agents into Task Center

### `agent-lifecycle-semantics.md`

Defines:

- per-agent live state
- current/last activity
- runtime stats
- model/tool/error counters
- lifecycle reducer from runtime events
- runtime control channel and safe points
- AgentBoard query/subscription direction

### `master-worker-task-state-machine-phase1.md`

Defines:

- Phase 1 scope: one BigTask, multiple SubTasks
- MasterPollLoop
- WorkerExecutionLoop
- FrameworkSchedulerTick
- timeout/stale/block/review/retry semantics
- acceptance tests and headless sample direction

### `master-worker-prompt-contract-phase1.md`

Defines:

- master prompt state handling table
- worker prompt state handling table
- control channel rules
- context admission rules
- Phase 1 prompt/tool categories

### `master-worker-tool-action-contract-phase1.md`

Defines:

- small owner-scoped exposed tool surface
- typed `op` parameter contract
- semantic-action-to-op mapping
- Task Center, Agent Lifecycle, and worker-control owner boundaries
- action validation and paired error feedback rules
- Agent Lifecycle as intrinsic agent state rather than a standalone
  model-facing tool

### `framework-mediated-agent-operations.md`

Defines:

- all Agent and Task operations must enter through framework-owned surfaces
- Task Center owns durable task truth
- Agent registry is resource registration, not lifecycle
- Agent Lifecycle is intrinsic state/projection, not a mutation tool
- worker control is a future safe-point control queue, not Task Center mutation
- target Agent-to-Agent communication paths through Task Center/EventInbox and
  worker-control inbox
- current Phase 1 implemented status versus Phase 2 gaps

## Implementation Sequence

### P0: Owner Map And Contracts

Goal:

Create owner entries and contract boundaries before code implementation.

Deliverables:

- update feature map if new feature ids are needed
- decide whether `task.orchestration` owns Task Center directly or whether a new
  feature id such as `task.center` is needed
- decide whether `agent.lifecycle` belongs in `freehand-task`,
  `freehand-node`, `freehand-runtime`, or a new crate
- decide the actual exposed tool names and op enums; default to the existing
  `task(op=...)` style and add a separate tool name only when owner boundaries
  require it
- treat Agent Lifecycle as agent state/projection. Do not add a model-facing
  `agent` tool for lifecycle query unless owner-map analysis proves a separate
  action surface is required
- define `worker_control` only for safe-point control of running worker
  executions, and keep task mutation in Task Center
- map every semantic action category to one owner-scoped tool/op pair
- add function-map and test-design pending entries
- define serializable contract DTOs in the correct owner

Validation:

- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`

### P1: Task Center Snapshot And Query Skeleton

Goal:

Expose owner-backed Task Center query truth without real worker execution.

Deliverables:

- BigTask/SubTask/Execution snapshot structs
- EventInbox cursor model
- TaskBoard projection
- `task(op="query_board")`, `task(op="query")`, and execution-query op direction
- restart restore tests from snapshots/ledger

Validation:

- white-box task center reducer tests
- module black-box runtime query tests
- no UI-local task state

### P2: Agent Lifecycle Reducer

Goal:

Reduce runtime/provider/tool/error events into per-agent lifecycle state.

Deliverables:

- AgentLifecycleSnapshot
- current_activity and last_activity
- runtime stats counters
- error profile counters
- AgentBoard projection
- AgentBoard/lifecycle query projection direction; lifecycle is intrinsic agent
  state, not a default model-facing `agent` tool

Validation:

- model_thinking state from provider request start
- tool_running state from tool event
- recovering state from paired tool failure/schema polishing/provider retry
- blocked state from typed blocker report
- no raw prose parsing

### P3: Task Center Sync From Lifecycle

Goal:

Make execution facts update Task Center state.

Deliverables:

- ExecutionFact event contract
- running/recovering/blocked/review_ready transitions
- Task Center event inbox update
- agent-to-task binding index
- restart recovery of bindings and cursor

Validation:

- worker recovering does not fail task
- worker blocked emits master-visible event
- review_ready enters review queue
- same agent query returns active task and task list

### P4: Scheduler Tick And Timers

Goal:

Framework owns elapsed time, stale, soft timeout, hard timeout, and next check.

Deliverables:

- scheduler tick event
- timer fields on execution/subtask
- stale/timeout detection
- recommendations without business decision
- master wake event

Validation:

- soft timeout does not fail task
- stale event requires no progress/heartbeat beyond threshold
- hard timeout requires master decision rather than silent failure
- scheduler events are durable and replayable

### P5: Runtime Control Channel

Goal:

Master can query or ask running workers without stopping their execution.

Deliverables:

- control inbox
- framework-answerable status query
- worker-model-answerable runtime question
- safe point handling
- command accepted/deferred/answered/rejected events

Validation:

- status query returns lifecycle truth without interrupting worker
- runtime question is answered at safe point and execution resumes
- cancel/pause writes explicit events

### P6: Phase 1 Master/Worker Loop Samples

Goal:

Prove one BigTask subtask loop end to end through ADP/CLI.

Samples:

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

Validation:

- S profile fixed `127.0.0.1:4042`
- query Task Center and Agent Lifecycle truth
- restart and query same ids
- no model prose as proof

### P7: UI Projection

Goal:

Expose task and agent state visually without UI guessing.

Deliverables:

- TaskBoard projection
- AgentBoard projection
- execution status rows
- blocked/recovering/review_ready/stale rendering
- debug detail optional raw transcript

Validation:

- browser evidence + ADP truth
- historical states do not animate
- UI does not show raw ids by default
- UI renders lifecycle truth only

## First Implementation Goal

The first coding goal after the initial design was Phase 1. It is now closed:

```text
Task Center query skeleton
Agent Lifecycle reducer skeleton
Task Center sync from lifecycle facts
Scheduler tick timer events
Headless query samples
```

The next coding goal should attach these capabilities to a minimal real
master/worker execution loop, still without UI:

```text
worker queue / claim loop
execution id binding
progress / blocked / recovering / review_ready facts
reject -> retry -> review_ready -> approve -> close
restart same-id proof
```

Use `docs/goals/multi-task-foundation-phase2-gap-plan.md` as the current Phase
2 gap and execution plan.

## Non-Goals Until Later

- multi BigTask context switching
- cross-session task switching
- worker pool autoscaling
- cross-machine workers
- full UI task dashboard
- automatic business decisions by framework
- raw worker transcript admission into parent context
