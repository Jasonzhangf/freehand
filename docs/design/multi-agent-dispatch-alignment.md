# Multi-Agent Dispatch Alignment

## Status

Design truth for aligning Freehand task dispatch with the useful parts of Codex
thread-spawn agents and DeepSeek-Reasonix task subagents.

This document is not an implementation claim. Current Freehand implementation
still has task lifecycle, agent registry, leases, claim, history, and progress
recording skeletons; real worker process/channel execution and worker turn
subscription are pending.

Important scope update:

- Freehand is now moving from single-agent lifecycle closeout into multi-task
  management foundations.
- The immediate Phase 1 design is not full multi-agent scheduling. It is one
  active BigTask, multiple SubTasks, Task Center truth, Agent Lifecycle truth,
  scheduler timers, and master/worker prompt contracts.
- See `task-center-truth.md`, `agent-lifecycle-semantics.md`,
  `master-worker-task-state-machine-phase1.md`,
  `master-worker-prompt-contract-phase1.md`, and
  `multi-task-foundation-implementation-plan.md`.
- Broader dispatch and parallel/multi-BigTask ideas in this document are future
  direction unless the Phase 1 documents explicitly include them.

## Reference Findings

### Codex Direction

Codex models spawned agents as separate threads with parent/child topology.

Useful patterns:

- `agent-graph-store` persists parent/child thread-spawn edges.
- spawned thread edges have lifecycle status such as open/closed.
- `AgentRegistry` enforces max spawned threads and reserves nickname/path before
  spawn commit.
- spawned agents inherit selected shell/exec policy from the parent when allowed.
- app-server protocol exposes collaboration tool calls:
  `SpawnAgent`, `SendInput`, `ResumeAgent`, `Wait`, and `CloseAgent`.
- collaboration tool call items carry sender thread, receiver threads, prompt,
  model, reasoning effort, and target agent states.
- thread history upserts begin/end events into the same visible item, so spawn
  and wait state update in place instead of creating noisy duplicate rows.
- client can discover new spawned threads through thread-created notifications
  and subscribe/drain them separately.

Freehand takeaways:

- persist master/worker relationship as topology truth, not UI state
- represent collaboration as first-class protocol items
- use begin/end upsert semantics for UI lifecycle rows
- cap worker/thread usage through owner-backed resource accounting
- allow subagent turn subscription, but keep it separate from parent prompt
  context admission

### Reasonix Direction

Reasonix models subagent dispatch as an explicit `task` tool call.

Useful patterns:

- the model triggers subagent work by calling the `task` tool with prompt,
  description, tool whitelist, model/effort, max steps, and optional background
  mode
- subagent runs in its own session with a focused system prompt
- parent model receives only the final answer, not the subagent raw transcript
- subagent tool activity is forwarded into the parent event stream by setting
  `ParentID`, so UI can nest subagent tool activity under the parent task call
- subagent meta-tools are excluded from subagent registries to prevent recursive
  uncontrolled delegation
- subagent transcripts persist with metadata: ref, status, kind, parent session,
  parent tool call id, workspace root, tool scope, tool schema hash, model, and
  effort
- subagent can continue/fork only when identity, prompt persona, tools, model,
  effort, workspace, and parent ownership match
- planner/executor split keeps sessions separate to preserve cache-stable
  prefixes
- event stream is typed and frontend-neutral; frontends render `TurnStarted`,
  `Phase`, `ToolDispatch`, `ToolResult`, `ToolProgress`, `Retrying`, and
  `TurnDone`

Freehand takeaways:

- dispatch must be model-triggered through admitted tools, not framework path
  sniffing
- worker context should be isolated and bounded
- parent context should ingest typed conclusions, not raw worker transcripts
- UI can show nested worker tool/progress events without admitting them into
  parent session history
- continuation/fork requires strict identity and workspace/session matching

## Freehand Decision

Freehand should combine both patterns:

```text
Reasonix trigger model
  explicit task/workspace action tool triggers dispatch

Codex topology model
  master/worker/session relationships are persisted and subscribable

Freehand truth model
  workspace owns session truth
  task owns durable work lifecycle
  execution owns in-flight worker activity
  worker is resource, not session owner
```

The word "active" means the master model can proactively choose collaboration
actions from an explicit tool/protocol surface. It does not mean the framework
autonomously scans text and creates work. The framework remains passive:
validate, persist, schedule, subscribe, and project.

## 1. When To Dispatch And How It Is Triggered

Dispatch is allowed only after the model emits an explicit admitted action.

Allowed trigger path:

```text
UserTurn(master session)
  -> ModelStatus(intent only, no side effects)
  -> WorkspaceAction(select/create, if needed)
  -> TaskAction(create/update dispatch plan)
  -> TaskAction(assign or claim_next)
  -> ExecutionStart(worker resource attached)
```

Forbidden trigger path:

```text
user mentions a path
  -> framework sniffs path
  -> framework creates workspace/task/worker silently
```

Status schema answers "what the model intends". It never mutates truth.

Action tools mutate truth after owner validation:

- `workspace.select_or_create`
- `task.create`
- `task.dispatch`
- `task.assign`
- `task.claim_next`
- `task.record_execution`
- `task.submit_review`
- `task.approve`
- `task.reject`
- `task.close`

First implementation may keep the single `task` tool surface with `op` values,
but the semantic actions above must remain distinct in docs, function maps,
tests, and protocol projection.

Dispatch decision cases:

- simple answer: no task dispatch
- current workspace work: create task in current workspace session if work needs
  independent execution or durable tracking
- other cwd work: model must request workspace resolution first
- broad search: create search task with `context_profile=clean_search`
- multi-step work: create parent task and child tasks through admitted actions
- blocked: ask user for missing cwd, scope, permission, or priority

Dispatch policy required fields:

```text
task_id
workspace_id
session_id
kind = search | code_edit | review | test | docs | generic
requested_capabilities
parallelism
model_tier = main | small | default
context_profile = clean_search | workspace_inherited | debug_direct
visibility = user_visible | compact | debug
```

Validation:

- no `workspace_id` means no cross-workspace dispatch
- no suitable worker means task enters `WaitingWorker`
- invalid dispatch policy returns an action-tool error to the model
- tool/action validation errors are paired results, not provider failures

## 1.1 Active Dispatch Trigger Surface

Freehand should expose a small collaboration action surface to the master model,
similar in spirit to Codex collaboration tools and Reasonix `task`.

Minimum actions:

```text
spawn_worker_task
  create a worker execution for a focused task

send_worker_input
  continue or steer an existing worker execution/session

wait_worker
  wait for one or more worker executions to reach a useful state

resume_worker_task
  resume interrupted work from durable task/execution/session truth

close_worker_task
  close or release a worker execution after accepted result
```

These may be implemented as `task(op=...)` in the first version, but they must
project as collaboration semantics instead of generic tool text.

Trigger conditions the model should learn:

- spawn when the request needs isolated exploration, another cwd, parallel work,
  long-running work, or context-budget protection
- send input when an existing worker has the right workspace/session and needs a
  follow-up instruction
- wait when useful progress is expected from running worker executions
- resume when a task has durable unfinished state and the user/master wants to
  continue it
- close when the worker result is accepted and no longer needs live resources

Required prompt/tool guidance:

- tell the master that workers are resources, not owners of session truth
- tell the master to dispatch search/exploration with `clean_search`
- tell the master to dispatch edit/test/review with `workspace_inherited`
- tell the master it should not directly work outside its allowed workspace
- tell the master that worker raw transcript is debug detail; it should ask for
  typed conclusions/evidence

## 1.1.1 Master Prompt Contract

The master prompt should make dispatch an ordinary model choice, not a hidden
framework decision. The prompt must expose collaboration actions and teach the
model when to use them.

Prompt block:

```text
You are the master agent. You own the user conversation and coordinate work.

You may answer directly only when the request is conversational, explanatory, or
small enough to complete inside your current allowed workspace without losing
important context.

Use worker tasks when work should be isolated, parallelized, resumable, done in
another workspace, or kept out of your main context. Workers are resource slots.
They do not own sessions. Workspace sessions own durable context.

Never create tasks by mentioning paths in prose. If work targets a cwd, call the
workspace action first. If work requires a worker, call the task dispatch action.

For broad search, use clean_search and ask the worker for a typed conclusion:
answer, evidence_summary, confidence, open_questions, recommended_next_action.

For code edit, test, review, or docs work, use workspace_inherited and require
deliverables, evidence, changed resources, risks, and suggested next action.

Do not ask the user to manually trigger workers. If the conditions match, call
the collaboration action yourself. If information is missing, ask one concise
question or create a blocked task only when the missing item is task-scoped.

Worker raw transcripts are debug detail. In your future reasoning, rely on typed
worker conclusions and accepted summaries, not raw worker turns.
```

The prompt must not say the framework will infer tasks automatically. The model
must learn that dispatch happens only by explicit action.

## 1.1.2 Dispatch Condition Matrix

The master should evaluate these conditions before deciding direct answer versus
worker dispatch.

| condition | direct answer | spawn worker task | required context profile |
| --- | --- | --- | --- |
| small explanation or status question | yes | no | none |
| current workspace, small single-step edit | maybe | optional if durable tracking needed | `workspace_inherited` |
| current workspace, multi-file search | no | yes | `clean_search` |
| current workspace, code edit plus tests | no | yes | `workspace_inherited` |
| another cwd/workspace | no direct work | yes after workspace action | `workspace_inherited` |
| broad exploration with uncertain files | no | yes | `clean_search` |
| parallel independent subtasks | no | yes, one child task per shard | depends on shard |
| long-running waitable work | no | yes with background execution | depends on work |
| user asks to review worker result | maybe | use existing task/execution | `workspace_inherited` or debug |
| missing cwd/permission/scope | ask user or blocked task | no execution until resolved | none |

Decision rules:

- if the answer requires reading many files, dispatch search
- if the action may modify files, create a task before execution
- if target cwd is not the master's allowed workspace, resolve workspace first
- if the work can run independently while master continues, dispatch background
- if two subtasks do not share write targets, dispatch parallel search/review
- if subtasks may race on writes, serialize or assign one editor worker
- if the model only needs facts, use `clean_search`
- if the model needs project history or accepted workspace context, use
  `workspace_inherited`
- if the user explicitly asks for low-level debug transcript, use debug
  subscription, not parent prompt admission

## 1.1.3 Dispatch Action Schema Shape

The action schema should force the model to state why it is dispatching.

```text
spawn_worker_task {
  title
  reason
  target_cwd
  workspace_id optional
  parent_task_id optional
  kind
  goal
  deliverables
  acceptance
  requested_capabilities
  tools
  context_profile
  model_tier
  parallelism_group optional
  background boolean
  visibility
}
```

Validation conditions:

- `reason` must match one dispatch condition such as search, cross-workspace,
  parallelizable, long-running, context-isolation, code-edit, test, review
- `target_cwd` must be present unless `workspace_id` is already resolved
- `goal`, `deliverables`, and `acceptance` are required
- `context_profile=clean_search` cannot request writer tools
- `kind=code_edit` requires writable capability and review/evidence acceptance
- `parallelism_group` requires non-overlapping write scope or read-only work
- `background=true` requires a user-visible progress projection

Rejected dispatch action is returned to the model as a tool result with the
missing/invalid field list, not as a system failure.

## 1.1.4 Wait And Follow-Up Prompt Contract

The master must actively manage already spawned work.

Prompt block:

```text
Before spawning a new worker, check whether an existing task or worker execution
already matches the workspace, goal, and tool scope.

If a worker is already running and the user asks about progress, call wait_worker
or query task status. Do not summarize from stale memory.

If a worker produced a conclusion, decide whether to approve, reject, ask the
worker for more work, spawn a follow-up task, or report to the user.

If a worker failed because of tool/schema/action validation, send the paired
error back as follow-up input unless the error requires user permission or scope
clarification.
```

Follow-up decision rules:

- existing running execution plus same goal: wait or send input
- existing completed execution plus insufficient evidence: send follow-up
- existing failed execution plus repairable tool/action error: send follow-up
- existing failed execution plus provider retry exhausted: report failure or
  spawn a new execution only after policy allows
- accepted conclusion: approve/close and admit summary
- rejected conclusion: reject task with required changes and keep task open

## 1.2 Task Management Model

Freehand should manage task work as a graph, not a flat list of worker runs.

```text
MasterSession
  -> Workspace
    -> WorkspaceSession
    -> Task
      -> ChildTask*
      -> Execution*
        -> WorkerResource
        -> WorkerTurn*
```

Task management rules:

- task belongs to workspace
- execution belongs to task
- worker is assigned to execution
- worker turn belongs to execution debug/runtime truth
- accepted result summary can be admitted into workspace session
- parent task can wait on child tasks
- parent task closes only after required child task acceptance

Codex-aligned graph truth:

- persist parent task to child task edges
- persist task to execution edges
- persist master session to spawned worker/task edges
- mark edges open/closed/interrupted
- list direct children and descendants with stable ordering

Reasonix-aligned context truth:

- worker runs in isolated context
- worker output to parent is final/typed conclusion
- worker tool/progress events may be observed live
- raw worker transcript does not enter parent prompt

## 1.3 Collaboration Flow

Collaboration is a protocol-level lifecycle, not just task status.

```text
Spawn
  master requests focused worker task
  scheduler reserves worker resource
  task/execution edge becomes open

Send
  master sends a follow-up instruction to an existing worker execution
  worker appends a new turn inside the same workspace-bound context

Wait
  master waits for one or more workers
  UI shows elapsed wait and target worker states

Review
  worker submits typed conclusion/deliverables/evidence
  master accepts, rejects, or asks for more work

Close
  accepted execution releases worker resource
  task/session summaries are updated by owners
```

Collaboration item projection should update in place:

```text
WorkerDispatchCall(id=call-1, status=in_progress, tool=spawn_worker_task)
WorkerDispatchCall(id=call-1, status=completed, receiver_worker_ids=[Atlas])
```

This follows Codex begin/end upsert behavior and avoids noisy duplicate UI rows.

## 1.4 Active Dispatch Examples

Search:

```text
User: find every place config is loaded
Master: spawn_worker_task(kind=search, context_profile=clean_search, tools=[search,read])
Worker: returns typed conclusion with evidence summary
Master: decides next action
```

Cross-workspace edit:

```text
User: fix /repo/a build
Master: workspace.select_or_create(cwd=/repo/a)
Master: spawn_worker_task(kind=code_edit, context_profile=workspace_inherited)
Worker: edits/tests/submits review
Master: approves/rejects and reports user-facing result
```

Parallel exploration:

```text
User: audit webui and android gaps
Master: creates parent task
Master: creates child task webui search
Master: creates child task android search
Master: wait_worker([webui, android])
Workers: return typed conclusions
Master: synthesizes one plan
```

Long-running wait:

```text
Master: spawn_worker_task(run_in_background=true)
UI: shows worker row as running with timer
Master next turn: wait_worker(worker_id)
Worker: returns terminal conclusion or still-running state
```

## 2. How To Track And Display Status

Tracking has three separate truth layers.

### Task Truth

Task truth is durable:

```text
Created
  -> WaitingWorker
  -> Assigned
  -> Running
  -> ReviewSubmitted
  -> Approved
  -> Closed
```

Task UI shows:

- title
- workspace
- assignee
- state
- current execution summary
- review/result summary
- known blocker if any

### Execution Truth

Execution truth is in-flight runtime activity:

```text
ExecutionCreated
  -> ContextBuilt
  -> ModelWaiting
  -> ToolRunning
  -> ToolResultReturnedToModel
  -> ModelWaiting
  -> ReviewReady
  -> Completed
```

Branches:

```text
ModelWaiting -> ProviderRetrying -> ModelWaiting | Failed
ToolRunning -> ToolFailed -> ToolResultReturnedToModel
Any non-terminal -> Interrupted
ReviewReady -> ReviewRejected -> Running
```

Execution UI shows compact lifecycle rows:

- "Atlas searching files · 18s"
- "Beacon reading `src/lib.rs` · done"
- "Comet waiting for model · retry 2/5"
- "Delta submitted review · 3 files changed"

It must not show raw internal ids by default.

### Worker Resource Truth

Worker truth is scheduling state:

```text
Available
Queued
Running
Paused
Offline
Failed
Closed
```

Worker UI shows resource/status only, not full transcript by default.

## 3. Can We Subscribe To Subagent Turns And Extract Status?

Yes, but subscription and context admission are different.

Allowed:

- UI subscribes to worker turn/execution events for live status.
- master subscribes to worker task conclusions and lifecycle events.
- debug surfaces can open full worker turn transcript.
- compact public projection can derive status from worker events.

Forbidden:

- parent prompt context directly ingests raw worker transcript.
- UI infers task/session/worker ownership from raw turn ids.
- worker scratchpad becomes workspace session truth without owner admission.
- failed exploration transcripts remain in future prompt context after a later
  successful conclusion supersedes them.

Subscription model:

```text
SubscribeTask(task_id)
  -> task lifecycle events
  -> active execution ids

SubscribeExecution(execution_id)
  -> execution phase events
  -> compact tool semantic events
  -> model wait/retry events
  -> terminal/review-ready event

SubscribeWorkerTurn(worker_turn_id)
  -> debug/detail stream only
  -> not admitted to parent context
```

Projection rule:

```text
worker raw event
  -> execution projector
  -> compact status row
  -> task timeline
  -> UI protocol
```

Context admission rule:

```text
worker raw transcript
  -> debug/replay truth only

worker typed conclusion
  -> master model context
  -> workspace accepted summary only after review/approval
```

## UI Projection Contract

Default UI should show one master conversation plus task/execution side status.

In the conversation:

- show that the master dispatched work
- show compact worker progress lines
- update one row per execution phase rather than appending duplicate system rows
- show final accepted summary at the end

In task drawer/detail:

- show task tree
- show worker assignments
- show execution timeline
- allow opening debug transcript explicitly

In debug view:

- show raw worker turns
- show provider retry/error details
- show task ledger and execution event ids

## Proposed Protocol Items

Freehand should add protocol-owned items equivalent to Codex collaboration items,
but using Freehand vocabulary:

```text
WorkerDispatchCall
  id
  status = in_progress | completed | failed
  tool = select_workspace | create_task | dispatch_worker | send_input | wait | close
  sender_session_id
  receiver_worker_ids
  task_id
  execution_id
  workspace_id
  prompt_summary
  model_tier
  context_profile
  worker_states
```

```text
ExecutionStatusItem
  execution_id
  task_id
  worker_id
  phase
  status
  elapsed_ms
  semantic_summary
  public_tool_events
  retry_count
  terminal_reason
```

These are projection contracts. They should be generated from owner truth, not
assembled in WebUI or Android.

## Context Economy Policy

Search/exploration dispatch:

- use `clean_search`
- use small/default worker model unless master explicitly needs main model
- provide only goal, scope, allowed tools, output schema, and workspace id
- return typed conclusion
- do not inject raw transcript into parent

Code edit/review/test dispatch:

- use `workspace_inherited`
- include workspace session summary and task-specific context
- require deliverables/evidence/review result
- admit accepted summary after review

Failure pruning:

- failed tool/execution attempts stay visible for immediate repair
- after later success, future prompt history prefers successful result and
  concise lesson
- debug/replay/error/task ledgers keep raw failed attempts

## Implementation Gap List

Current Freehand gaps before this design is real:

- workspace owner feature and workspace action tool are missing
- task status naming still uses legacy `WaitingAgent` in code
- worker pool config compilation is not yet the scheduler truth
- execution runtime identity/query/subscribe is missing
- real worker process/channel dispatch is missing
- worker turn subscription and compact execution projector are missing
- UI protocol lacks task/execution/worker subscription items
- WebUI/Android task drawer and debug transcript surfaces are missing
- function map and test design need pending entries before implementation

## Validation Direction

Required red/green coverage when implementing:

- model status alone cannot create a task
- raw path text cannot create a workspace/task
- admitted workspace action can select/create a workspace
- invalid workspace action returns paired action error
- task dispatch with no worker becomes `WaitingWorker`
- tool failure inside worker execution returns to model and does not fail task
- provider failure retries by provider policy before terminal failure
- UI projection updates the same execution row from begin to end
- parent context receives typed conclusion, not raw worker transcript
- debug subscription can read worker turn transcript without admitting it to
  parent prompt context
