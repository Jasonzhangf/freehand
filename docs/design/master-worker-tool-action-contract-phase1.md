# Master Worker Tool Action Contract Phase 1

## Status

Design truth for the tool/action surface used by the first multi-task
foundation slice.

This document resolves a naming risk in the prompt and state-machine docs:
semantic actions are many, but exposed tool names must stay few.

## Core Rule

Use a small number of logical tools with typed `op` parameters.

A tool name defines the owner surface. `op` defines the operation. Typed
parameters define the payload.

Adding a new task-management behavior does not automatically add a new tool
name.

## Why

Do not expose dozens of single-action tools such as:

```text
query_task_board
query_agent_board
query_blocked_tasks
dispatch_subtask
approve_submission
reject_submission
wait_with_next_check
```

Those names are useful as semantic action categories in docs, prompts, tests,
and UI projections. They are not the default runtime tool surface.

Too many exposed tools create avoidable problems:

- tool selection becomes noisier for the model
- owner boundaries are harder to audit
- equivalent actions get duplicated under different names
- UI and lifecycle projection start guessing from raw tool names
- validation and error feedback scatter across many executors

## Exposed Tool Surface

Phase 1 should converge on this small owner-scoped surface:

| tool | owner surface | purpose |
| --- | --- | --- |
| `task` | Task Center / task orchestration | create, query, assign, claim, heartbeat, progress, review, close, and task-board operations |
| `agent` | Agent Lifecycle / agent board | query lifecycle, query worker state, list available workers, report heartbeat/status when not represented by a task op |
| `worker_control` | runtime control channel | ask or control a running worker at safe points |

The existing `task(op=...)` direction remains the baseline. `agent` and
`worker_control` are Phase 1 candidates only if the owner map proves they should
not be sub-ops of `task`.

No implementation should add a new exposed tool name until the function map and
test design prove a separate owner surface is necessary.

## Operation Contract

Every exposed tool call must have:

- `op`: required enum owned by the tool owner
- `args`: typed payload for that op
- `trace`: framework-supplied or runtime-bound provenance, not model-invented
- `idempotency_key` when the op mutates durable truth

The model may choose an `op`, but the framework validates the operation before
any mutation.

Unknown `op`, invalid args, forbidden transition, wrong state, missing required
field, duplicate idempotency key, or permission mismatch returns a paired
tool/action error to the model. It is not a provider failure and must not be
silently normalized into a successful mutation.

## Semantic Action Mapping

Semantic action categories remain useful for prompting and tests. They map to
ops under the small tool surface.

| semantic category | exposed tool/op direction |
| --- | --- |
| read TaskBoard | `task(op="query_board")` |
| read one task | `task(op="query")` |
| read task history | `task(op="history")` |
| read blocked tasks | `task(op="query_board", filter.status="blocked")` |
| read review queue | `task(op="query_board", filter.status="review_ready")` |
| read stale executions | `task(op="query_board", filter.execution="stale")` |
| create subtask | `task(op="create")` with parent task id |
| dispatch or assign | `task(op="assign")` or `task(op="claim_next")` |
| record worker progress | `task(op="record_execution")` |
| report blocker | `task(op="record_execution", status="blocked")` or a future typed blocker op if owner map requires it |
| submit review | `task(op="submit_review")` |
| approve or reject review | `task(op="approve")` / `task(op="reject")` |
| close BigTask/SubTask | `task(op="close")` |
| wait with next check | `task(op="wait")` or scheduler tick event, depending on owner-map decision |
| query AgentBoard | `agent(op="query_board")` or `task(op="query_agents")` if Task Center owns the projection |
| query agent lifecycle | `agent(op="query_lifecycle")` |
| ask runtime status | `worker_control(op="query_status")` |
| ask worker model question | `worker_control(op="ask_at_safe_point")` |
| inject constraint | `worker_control(op="add_constraint")` |
| pause, resume, cancel | `worker_control(op="pause"|"resume"|"cancel")` plus Task Center state event |

This table is a contract direction, not an implementation claim. P0 must decide
the final op names and owner placement before code lands.

## Owner Boundaries

Task Center owns durable work truth:

- task creation and mutation
- execution identity and progress admission
- task board query
- review lifecycle
- restart recovery

Agent Lifecycle owns live agent truth:

- current activity
- last activity
- model/tool/error counters
- elapsed state
- availability
- AgentBoard projection

Runtime control channel owns running-worker control:

- safe-point delivery
- accepted/deferred/rejected control events
- framework-answerable status replies
- worker-model-answerable runtime questions

UI owns none of these semantics. UI renders projections and submits typed
commands. It must not infer status from raw tool names, raw args, or result
strings.

## Validation And Error Feedback

Tool/action validation must be paired back to the model using the same
tool/action result path that produced the invalid action.

Required branches:

- valid op with valid args mutates or queries the owner truth
- unknown op returns an action validation error
- valid op with invalid args returns the missing/invalid fields
- valid args in the wrong lifecycle state returns a forbidden-transition error
- owner write failure returns an owner/system error and does not pretend success
- query ops are read-only and cannot mutate truth

Schema mismatch in a model response is response polishing. Tool/action mismatch
is action validation. Provider/network failure is provider error. These three
must remain separate.

## Prompt Wording Rule

Prompts may list semantic actions to teach behavior, but must also state:

```text
Use the small owner-scoped tools. Select the correct op and typed args. Do not
invent new tool names for semantic actions.
```

## Non-Goals

Phase 1 does not define:

- dozens of user-facing tool labels
- UI rendering from raw tool names
- automatic task intent inference from prose
- fallback from invalid op to a guessed nearby op
- runtime scanning of arbitrary tool names as action intent
