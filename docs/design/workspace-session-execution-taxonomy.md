# Workspace, Session, Execution, And Worker Taxonomy

## Status

Canonical design truth for the current multi-agent discussion.

This document fixes the product/domain vocabulary before implementation. If another
document uses conflicting meanings for `workspace`, `session`, `worker`, or
`execution`, this document wins until it is explicitly replaced.

For the next implementation foundation, this vocabulary is consumed by:

- `task-center-truth.md`: global Task Center and task/execution query truth
- `agent-lifecycle-semantics.md`: per-agent live state and current activity
- `master-worker-task-state-machine-phase1.md`: one BigTask multi-subtask loop

Phase 1 deliberately avoids multiple independent BigTasks and cross-session
context switching. Those remain future work.

## Core Decision

`session` belongs to `workspace`, not to `worker`.

Workers are schedulable resources. A worker can attach to a workspace, inherit the
workspace session truth, execute work, and write admitted results back to that
workspace session. A worker does not carry one long-lived project session across
unrelated workspaces.

```text
Master
  manages workers and workspaces

Workspace
  owns cwd
  owns session truth
  owns shared assets and summaries
  owns tasks

Worker
  resource slot
  attaches to workspace
  runs an execution

Execution
  worker runtime instance inside one workspace
  consumes and updates workspace session truth through owner APIs
```

## Object Meanings

### Master

The master is the only user-facing manager agent.

Responsibilities:

- hold the user's primary conversation
- understand user goals
- create or select workspaces
- allocate workers from the resource pool
- monitor worker progress
- summarize accepted results back to the user

The master does not represent one ordinary worker slot. It should not be modeled
as a worker session owner.

### Worker

A worker is a resource, not a session.

Responsibilities:

- declare capabilities
- expose availability and current load
- run assigned executions
- report heartbeat, progress, and result

A worker may work on different workspaces over time. Workspace context must come
from the workspace session being attached, not from the worker's prior execution
history in another workspace.

### Workspace

A workspace is a cwd-bound work pool.

Responsibilities:

- own one cwd
- own the durable session truth for that cwd-bound work context
- hold workspace-level shared assets
- hold accepted summaries
- contain task state for work performed in this cwd-bound context
- admit multiple worker executions over time

The same cwd may host multiple historical workspace sessions only if product
semantics explicitly create separate workspaces. The default rule is one active
workspace context per cwd-scoped work pool.

### Session

A session is workspace-owned durable context.

Responsibilities:

- persist the conversation and execution history admitted into the workspace
- preserve workspace context across worker changes
- provide inherited context for later executions
- hold attachments and summary references that belong to the workspace

Session truth is not owned by UI, master, worker, node pairing, or an execution
slot. Those layers may query or append through their owner APIs only.

### Execution

An execution is worker-owned runtime activity inside a workspace.

Responsibilities:

- bind one worker to one workspace for one running activity
- maintain in-flight status, heartbeat, scratchpad, and temporary observations
- emit progress and result candidates
- write accepted progress/results back to task/session truth through owner APIs

Execution state is independent. Session truth is shared by the workspace.

### Task

A task is a workspace-scoped work item.

Responsibilities:

- express one unit of work to be done inside a workspace
- reference the workspace session that provides inherited context
- declare goal, deliverables, acceptance, priority, and target cwd
- coordinate one or more executions over time
- collect worker progress, review evidence, and accepted results

A task is not a worker. A task is not a session. A task may outlive any single
worker execution and may be resumed by another worker resource.

Task truth is append-only ledger plus rebuildable snapshot. Task truth can refer
to workspace session context, but task truth must not own the workspace session
history itself.

## Relationship Model

```text
User
  -> Master
    -> Workspace A (cwd=/repo/a)
      -> Session A (workspace-owned truth)
      -> Task A1
        -> Execution 1 (worker alpha attached)
        -> Execution 2 (worker beta attached)
    -> Workspace B (cwd=/repo/b)
      -> Session B (separate workspace-owned truth)
      -> Task B1
        -> Execution 3 (worker alpha attached later)
```

Worker `alpha` may participate in both workspaces over time, but it must not move
Workspace A context into Workspace B. Each execution inherits the target
workspace session.

## Worker Resource Pool Startup

Master startup materializes a deterministic worker resource pool from config.

Rules:

- master count is exactly one
- worker count comes from configured worker resources or configured worker-pool
  templates
- configured workers are resources only; startup must not create workspace
  sessions for them
- worker ids must be stable after config compilation
- display names may come from the default worker name pool
- when the name pool is exhausted, generated workers use a deterministic
  sequence suffix

Default worker display-name pool:

```text
Atlas
Beacon
Comet
Delta
Echo
Falcon
Harbor
Ion
Jasper
Kepler
Lumen
Meridian
Nova
Orion
Pioneer
Quartz
Ranger
Solace
Talon
Vector
```

Name assignment is cosmetic. Runtime identity must use stable worker ids, not
display names.

Example compiled pool:

```text
worker-001 display=Atlas
worker-002 display=Beacon
...
worker-020 display=Vector
worker-021 display=worker-021
```

## Master Request Handling Contract

The master is a global coordinator. It must not directly perform work outside
its allowed workspace boundary.

If a user asks the master to work on a target cwd, the model should inspect the
path and decide whether the work belongs to the master's current allowed
workspace. If the target cwd is outside that boundary, the model should request
workspace/task/agent creation through the built-in action-tool schema.

Framework behavior is passive:

- the framework provides prompt contracts, schemas, validators, built-in tools,
  and owner APIs
- the model returns explicit status schema and tool calls
- the framework validates model feedback and executes only accepted tool calls
- the framework must not infer task creation from user text, raw file paths,
  assistant prose, UI state, or tool output
- status schema has no side effects
- built-in action tools perform side effects after owner validation

Required trigger shape:

```text
User asks for work in target cwd
  -> model may emit status schema such as needs_workspace_task
  -> model must call the built-in task/workspace action tool for mutation
  -> framework validates schema/tool args
  -> owner creates or selects workspace/session/task/execution
  -> UI receives protocol projection
```

Forbidden trigger shape:

```text
User mentions /some/path
  -> framework creates task or agent by path sniffing
```

This aligns with the passive control rule: schema can explain intent, but only
admitted tools mutate truth.

## Task Dispatch Flow

Task dispatch is a passive framework response to model-emitted schema and
action-tool calls. The framework provides contracts and owner APIs; the model
chooses when to request workspace, task, and execution changes.

The core invariant is:

```text
user intent -> model intent -> admitted action -> owner mutation -> projection
```

No step may skip the admitted action or owner mutation boundary. The framework
may validate, reject, persist, schedule, and project. It must not invent task
intent or workspace targets from raw text.

### Phase 1: Intake

The master receives the user request in the master conversation.

Allowed framework actions:

- add the user turn to the master conversation truth
- expose available prompt contracts, status schema, and built-in task/workspace
  tools
- expose read-only workspace and worker pool projections

Forbidden framework actions:

- create a task from path sniffing
- assign a worker from user text alone
- mutate workspace/session truth before an admitted action tool

### Phase 2: Model Intent

The model decides whether the request is:

- a simple master answer
- work inside the master's current allowed workspace
- work inside another cwd-bound workspace
- broad search/exploration
- a multi-step task requiring workers
- blocked and needing user choice

Status schema may describe this intent. Status schema has no side effects.

Minimum status shape:

```text
intent_kind = answer | workspace_work | cross_workspace_work | search | multi_step | blocked
target_cwd optional
needs_workspace_action boolean
needs_task_action boolean
reason_for_user optional
missing_user_choice optional
```

Validation rules:

- `intent_kind=answer` must not include task mutation intent
- `cross_workspace_work` requires explicit `target_cwd`
- `blocked` requires a user-visible missing choice
- invalid status is returned to the model as schema polishing, not as task
  failure

### Phase 3: Workspace Resolution

If work targets a cwd, the model must request workspace selection or creation
through an admitted action tool.

Workspace resolution validates:

- target cwd is explicit
- target cwd canonicalizes to one workspace key
- master has authority to request work for that cwd
- workspace session exists or can be created by the workspace owner

Failure becomes an explicit tool/action error and is returned to the model for
repair. The framework does not silently switch cwd or create hidden workspaces.

Workspace action result shape:

```text
workspace_id
workspace_key
target_cwd
session_id
created | selected
authority_status
```

Workspace errors are normal tool results when the action was admitted but failed
validation. They are not provider failures and they do not terminalize the
master turn by themselves.

### Phase 4: Task Creation

The model creates a task through the built-in task action tool.

Task creation must provide:

- workspace id or target cwd
- session id when already known
- title
- content
- goal
- deliverables
- acceptance
- priority
- requested capabilities
- visibility

Task owner validates and writes task ledger/snapshot truth. The created task may
start as `WaitingWorker` or may be assigned if a valid dispatch policy is also
provided.

Task creation result shape:

```text
task_id
workspace_id
session_id
status
created_event_id
dispatch_policy_status = absent | accepted | rejected
user_visible_summary
```

Task creation may fail validation. That failure is returned as a paired tool
result to the model so it can correct missing fields, reduce scope, ask the
user, or choose a different workspace.

### Phase 5: Dispatch Planning

The model may request dispatch through task action fields. Dispatch policy should
be explicit:

```text
kind = search | code_edit | review | test | docs | generic
target_workspace_id
requested_capabilities
parallelism
model_tier = main | small | default
context_profile = clean_search | workspace_inherited | debug_direct
```

The framework validates policy against the worker resource pool. If no worker is
available, the task remains `WaitingWorker` and the UI receives an owner-backed
waiting projection.

Dispatch policy validation:

- `parallelism` must be a positive bounded integer
- `requested_capabilities` must map to declared worker capabilities
- `model_tier=small` is allowed for search/extraction tasks, not final
  user-facing synthesis unless explicitly requested by the master
- `context_profile=debug_direct` requires explicit debug visibility
- a cross-workspace task must already have a resolved workspace id

Worker selection is resource scheduling, not session ownership. Selection input:

```text
task_id
workspace_id
requested_capabilities
priority
parallelism
worker_availability
worker_load
worker_authority
```

Selection output:

```text
assigned_worker_ids
queue_state = assigned | waiting
reason
```

### Phase 6: Worker Claim And Execution

An available worker claims or is assigned the task. This creates an execution.

Execution creation binds:

```text
execution_id
task_id
workspace_id
session_id
worker_id
context_profile
```

The execution inherits context according to `context_profile`:

- `clean_search`: goal, scope, allowed tools, and output schema only
- `workspace_inherited`: workspace session summary plus task-specific context
- `debug_direct`: explicit debug-only transcript access

Execution writes progress to execution state first. Accepted progress is then
admitted into task history and projected through UI protocol.

Execution lifecycle:

```text
ExecutionCreated
  -> ContextBuilt
  -> ProviderWaiting
  -> ToolRunning
  -> ProgressRecorded
  -> ReviewReady
  -> Completed
```

Branches:

```text
ProviderWaiting -> ProviderRetrying -> ProviderWaiting | ExecutionFailed
ToolRunning -> ToolResultFailed -> ProviderWaiting
ToolRunning -> ToolResultSucceeded -> ProviderWaiting | ReviewReady
ContextBuilt -> Blocked
Any non-terminal -> Interrupted
```

Execution event contract:

```text
execution_id
task_id
workspace_id
session_id
worker_id
phase
started_at
updated_at
elapsed_ms
summary
visibility = public | compact | debug
```

Tool execution failures are paired results inside the execution and are returned
to the model for repair. They are not task failure unless the model later emits
an admitted task failure/blocked/review action or retry policy is exhausted.

### Phase 7: Search Then Decide

For broad search/exploration, dispatch should prefer `clean_search`.

Search worker output must be one typed conclusion:

```text
task_id
execution_id
scope
answer
evidence_summary
confidence
open_questions
recommended_next_action
```

The master/main model receives this conclusion, not raw search transcript. The
main model then decides whether to create another task, dispatch a worker, ask
the user, or submit/close.

Search task constraints:

- worker context starts clean
- context contains only task goal, target scope, allowed tools, output schema,
  and minimal workspace identifiers
- raw search transcript remains execution/debug truth
- parent context receives only the typed conclusion and accepted evidence
  summary
- if search fails, the failure remains repair-visible until a later successful
  search supersedes it

### Phase 8: Review

Workers do not close tasks unilaterally.

Worker review submission includes:

- deliverables
- evidence
- changed files or affected resources when relevant
- known risks
- suggested next action

Master or accepted reviewer approves, rejects, or requests more work. Rejection
returns the task to `WaitingWorker`, `Assigned`, or `Running` depending on the
next admitted action.

Review result shape:

```text
task_id
execution_id
review_state = submitted | approved | rejected
accepted_summary optional
required_changes optional
evidence_refs
```

Review rejection is not a worker resource failure. It is a task lifecycle event
that gives the model a bounded next-action surface.

### Phase 9: Close And Session Update

Task close requires accepted review or explicit close policy.

On close:

- task truth records final status and accepted summary
- workspace session may receive a concise accepted summary
- superseded failed attempts may be pruned from future prompt history only
  through rewrite/prune policy
- debug/replay/error/task ledger truth remains durable

Session update rules:

- only accepted task summaries enter future workspace session prompt context by
  default
- unresolved blockers and unrepaired failures stay visible until repaired or
  explicitly closed
- superseded failed attempts are pruned only from future prompt history, never
  from task/error/debug/replay ledgers
- worker-private scratchpad is never admitted into workspace session truth
  without an owner-reviewed summary

### Dispatch State Summary

```text
MasterTurn
  -> ModelStatus(no side effects)
  -> WorkspaceAction(select/create)
  -> TaskAction(create)
  -> TaskAction(assign/claim_next)
  -> Execution(run/heartbeat/record_execution)
  -> WorkerReview(submit_review)
  -> MasterDecision(approve/reject/next task)
  -> TaskAction(close)
  -> WorkspaceSessionSummary(update through owner)
```

## Task Dispatch State Machine

The task-level state machine is durable owner truth:

```text
Created
  -> WaitingWorker
  -> Assigned
  -> Running
  -> ReviewSubmitted
  -> Approved
  -> Closed
```

The execution-level state machine is runtime activity truth:

```text
ExecutionCreated
  -> ContextBuilt
  -> ProviderWaiting
  -> ToolRunning
  -> ProgressRecorded
  -> ReviewReady
  -> Completed
```

The master-level state machine is conversation coordination truth:

```text
UserTurnAccepted
  -> ModelThinking
  -> ActionRequested
  -> ActionResultReturnedToModel
  -> MasterDecision
  -> UserVisibleUpdate
```

These machines are related but not interchangeable:

- a schema mismatch belongs to `ModelThinking` / response polishing
- a tool failure belongs to execution action result and model repair
- a worker crash belongs to execution interruption and task recovery
- a rejected review belongs to task lifecycle
- provider/network retry belongs to provider waiting/retry policy

## Dispatch Error Taxonomy

Dispatch errors must remain typed so the model and UI can recover correctly.

```text
schema_mismatch
  response formatting problem
  returned to model for polishing

workspace_validation_error
  invalid cwd, unauthorized cwd, missing workspace, or canonicalization problem
  returned as action result

task_validation_error
  missing goal/deliverables/acceptance/capabilities/visibility
  returned as action result

worker_unavailable
  no suitable worker resource
  task remains WaitingWorker

execution_tool_error
  tool failed inside execution
  paired result returned to model

provider_error
  model/provider/network failure
  retry by provider policy, then terminal execution/turn failure if exhausted

review_rejected
  deliverables not accepted
  task returns to more work through admitted action
```

Only `provider_error` after retry exhaustion or explicit admitted terminal task
failure should become a terminal failed turn/task. Schema mismatch and tool
execution error are expected repair loops.

## Task Model

Tasks are created only by admitted action tools. A status schema may say the
model believes a task is needed, but status alone cannot create or mutate a task.

Minimum task identity:

```text
task_id
workspace_id
session_id
source_master_session_id
source_turn_id
target_cwd
```

Minimum task contract:

```text
title
content
goal
deliverables
acceptance
priority
requested_capabilities
visibility
```

Minimum runtime bindings:

```text
assigned_worker_id optional
active_execution_id optional
parent_task_id optional
child_task_ids
review_state
```

Task lifecycle:

```text
Created
  -> WaitingWorker
  -> Assigned
  -> Running
  -> ReviewSubmitted
  -> Approved
  -> Closed
```

Branches:

```text
Created -> Cancelled
WaitingWorker -> Assigned
Assigned -> Cancelled
Running -> Paused -> Running
Running -> Interrupted -> WaitingWorker | Assigned | Running
Running -> Blocked -> Running | Cancelled
Running -> Failed -> WaitingWorker | Assigned | Closed
ReviewSubmitted -> Rejected -> WaitingWorker | Assigned | Running
ReviewSubmitted -> Approved -> Closed
```

Execution relationship:

- one task can have zero active executions
- one task can have at most one active execution per assigned worker unless a
  later scheduling design explicitly allows parallel shards
- one task may have multiple historical executions
- an execution must reference exactly one task, one workspace, one session, and
  one worker
- worker progress is first recorded against execution, then admitted into task
  history and workspace session summary through owner APIs

Subtasks:

- a task may create child tasks through the same admitted action-tool path
- child tasks inherit workspace/session by default
- child tasks may target a different workspace only through explicit
  workspace-selection fields
- parent task closes only after required child tasks satisfy review/acceptance
  policy

Review and close:

- workers submit review with deliverables and evidence
- master or an accepted reviewer approves, rejects, or asks for more work
- workers do not unilaterally close tasks
- closing a task writes task truth and may append accepted summary into the
  workspace session through the session owner

Visibility:

- default user view shows task title, status, assignee, progress, and accepted
  result summary
- execution transcript is expandable/debug detail, not the primary conversation
- hidden/internal prompts and schema blocks must not be projected as user task
  content

Context economy:

- failed executions may be visible to the model while they are needed for repair
- after a later execution succeeds for the same task purpose, future workspace
  session context should prefer the successful result and accepted summary
- raw failed execution details stay in debug/replay/error/task ledgers, not in
  the default cache-hit prompt path
- broad search tasks should use clean small worker contexts with only goal,
  scope, allowed tools, and output schema
- search workers return typed conclusions; master/main model handles analysis,
  synthesis, and next-task decisions
- parent/master context should not ingest raw worker search transcripts

## Persistence Direction

Workspace-owned session data should live under a workspace-scoped runtime path,
not under a worker-scoped path.

Canonical shape:

```text
~/.freehand/state/workspaces/<workspace-key>/
  workspace.json
  session/
    history.jsonl
    snapshots/
    summaries/
    attachments/
  tasks/
    <task-id>.json
    <task-id>.jsonl
  executions/
    <execution-id>.json
```

The exact file layout may evolve, but the ownership rule must not:

- workspace owns session persistence
- task owner owns task ledger/snapshot truth
- execution owns in-flight runtime activity
- worker owns resource state

## Sharing Rules

Allowed sharing:

- workspace shared assets
- workspace accepted summaries
- master-approved handoff content
- task ledger events projected through owner APIs

Forbidden sharing:

- worker-private scratchpad as default workspace truth
- direct transcript copy from one worker execution into another without owner
  admission
- UI-local merge of worker histories
- worker carrying one workspace session into another workspace

## UI Projection Direction

Default user view:

- show the master conversation as the primary surface
- show workspace/task progress as structured status, not raw worker transcript
- show workers as resource/status rows
- show executions as expandable details under a workspace or task
- keep raw worker execution transcript debug-only unless the user explicitly
  opens it

The UI must consume protocol truth. It must not infer workspace/session/execution
relationships from raw turn ids, tool names, or local browser state.

## Owner Mapping

Current owner direction:

- `reason.session-history`: single-session history mechanics
- `reason.persistence`: authoritative session persistence mechanics
- `task.orchestration`: task lifecycle, task history, worker agent registry,
  worker progress events
- `node.master-slave`: node pairing, authorization, direct messages, slave turn
  publication
- `runtime.ui-command-dispatch`: runtime command/query routing into owners
- `ui.protocol`: projected query/subscribe/command contracts for UIs

Needed follow-up design:

- workspace owner feature and function map binding
- workspace-scoped session persistence path
- execution runtime state object
- worker pool config compilation and generated-name tests
- task/workspace action-tool schema for model-triggered workspace selection and
  execution creation
- ADP query/subscribe projections for workspace, worker, task, and execution
- WebUI/mobile information architecture for master conversation plus
  workspace/task/worker side panels

## Non-Goals

- This document does not define final scheduling policy.
- This document does not define real worker execution transport.
- This document does not change current runtime files by itself.
- This document does not make UI task management complete.

## Update Triggers

Update this document before implementation if any of these change:

- session owner changes away from workspace
- worker becomes a persistent context owner
- workspace cwd binding changes
- execution state becomes durable session truth
- UI exposes worker transcript as default primary conversation truth
- task orchestration starts owning workspace session history directly
