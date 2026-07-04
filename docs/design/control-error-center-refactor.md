# Control and Error Center Refactor

## Status

Planning truth for the next refactor. This document records the required semantic shape and the first landed skeleton.

Landed slice:

- `control.center` owner exists at `crates/freehand-control`
- fixed hook metadata writes exist for the basic status stopHook path
- `<<<freehand_status>>>` schema v1 parsing and validation exists for simple stop, task-complete stop, blocked, user options, and next-step rhythm decisions
- UI public projection strips hidden status blocks
- compact `task` action tool and task lifecycle persistence exist as `task.orchestration`
- `error.center` first skeleton classifies schema, tool, and provider failures and writes watermarked metadata before runtime repair/re-entry/failure paths

Still pending:

- full schema repair loop for rejected status
- selectable user-option projection
- task/node/UI error-center routing
- ADP/UI query and render surfaces for error-center decisions

## Problem

Freehand is an agent framework. Framework control must be passive:

- the framework defines fields, prompts, tags, schemas, validators, ledgers, and state executors
- the model returns explicit control fields
- the framework validates those fields and drives flow transitions
- the framework must not infer task/control intent from user text, assistant text, file paths, tool output, or UI state

Current code has partial pieces:

- `reason.turn` parses `<freehand_completion>` and retries invalid terminal schema
- `metadata.core` stores provenance metadata with writer owner and write node
- `ErrorErr01RuntimeClassified` exists as a shared error contract
- runtime dispatch can cancel, submit, and route simple UI commands
- node runtime has pairing, progress, direct message, and slave turn projection

The gap is that control and error handling are still distributed across owners. Completion schema parsing, runtime retry decisions, provider/tool failures, task lifecycle actions, cancellation, and node delegation do not pass through one central control/error admission path with durable watermark provenance.

## Non-Negotiable Rules

1. Control is not data.
2. Error policy is not local glue.
3. Model status feedback enters through hard tagged status schema blocks.
4. Model side effects enter only through built-in tools.
5. Public UI strips status/control schema blocks before rendering.
6. Runtime/task/node/reason owners consume accepted control-center truth, not raw assistant text.
7. Every status/action/error write must carry a watermark: writer, pipeline node, source model/agent/session/turn, trace id, timestamp, schema version, status/action/error code, validation status, retry index, and raw/control hashes.
8. Bad or incomplete model schema triggers repair/retry. The framework may normalize compatible syntax, but must not invent missing semantics.
9. Error classification and recovery route through one error center before flow state changes.

## Target Owners

### `control.center`

Recommended owner crate: `crates/freehand-control` after the refactor starts.

Until the crate exists, document it as a planned owner rather than overloading runtime.

Responsibilities:

- parse hard status schema blocks from model output
- validate schema shape and state-specific required fields
- admit built-in framework action tool calls after tool-registry validation
- normalize compatible syntax without inventing missing intent
- write accepted/rejected status and action records to `metadata.core`
- decide whether a status is sufficient for reasoning rhythm or needs schema repair
- decide whether an action tool call is ready for owner execution or must fail as a tool result
- emit control-center events consumed by reason/runtime/task/node owners

Non-responsibilities:

- no task execution
- no provider wire behavior
- no UI rendering
- no session/turn truth writing
- no direct task state mutation before action metadata admission

### `error.center`

Recommended owner crate: same `crates/freehand-control` module family or a sibling module inside it.

Responsibilities:

- classify runtime, provider, tool, task, node, schema, metadata, persistence, and UI-dispatch errors
- assign recovery class and retry/repair policy
- write error-watermark metadata before owner state changes
- decide whether the next flow action is repair prompt, retry, stop, fail, block, cancel, or escalate
- emit error-center decisions consumed by reason/runtime/task/node owners

Non-responsibilities:

- no swallowing errors
- no fallback-to-success
- no local owner mutation without a classified decision

## Status Schema and Built-In Action Tools

Freehand has two model feedback channels:

1. Status schema: no side effects. It tells the framework what interaction state the model believes the current turn is in.
2. Built-in action tools: side effects. They create, dispatch, append, stop, close, or query framework tasks.

Status schema must not execute actions. Action tools must not rely on status text as their authority.

## Fixed Control Hook Points

The first refactor target is a small skeleton placed at four existing flow points. The existing reasoning/provider/tool loop remains the mainline. Do not turn the whole reasoning loop into a giant hook chain.

Only these hook points are fixed:

```text
ControlHook01AfterLocalToolResult
  position: immediately after this client/runtime finishes local tool execution and records the tool result
```

```text
ControlHook02BeforeModelRequest
  position: after local tool-result re-entry/control feedback preparation, immediately before sending the next request to the model/provider
```

```text
ControlHook03AfterModelResponse
  position: immediately after the model/provider response is received and captured
```

```text
ControlHook04BeforeClientReturn
  position: after response inspection/control processing, immediately before returning/projection to the client
```

Fixed responsibilities:

- `ControlHook01AfterLocalToolResult` observes local tool result completion, admits tool-result-related action/error metadata, and prepares any control feedback needed for the next model request.
- `ControlHook01AfterLocalToolResult` must not send a provider request and must not perform unrelated owner side effects.
- `ControlHook02BeforeModelRequest` may inject or refresh prompt-contract material, status-schema instructions, repair feedback, compact action-tool specs, and tool-result feedback for the next provider request.
- `ControlHook02BeforeModelRequest` must run after local tool result handling and before the exact outbound model request is rendered/sent.
- `ControlHook03AfterModelResponse` captures the raw provider/model response boundary, extracts hidden status blocks, validates status schema, admits status metadata, and classifies schema/provider response errors before any client-visible return.
- `ControlHook04BeforeClientReturn` decides final rhythm/projection behavior, strips hidden status blocks, and ensures client-visible output contains only public projection plus explicit task/status/error projections.
- `ControlHook04BeforeClientReturn` must run before client-visible projection is returned so schema/status/control blocks cannot leak into UI text.
- owner actions still execute in their existing standard flow. The hook records/adjudicates control and error truth at the fixed boundary; it does not replace the whole flow.

The hook locations are stable contracts. Individual status schemas and action processors can vary behind these four hook points, but new implementations must not insert ad hoc parsing, action admission, or error classification elsewhere in the request/response flow.

Mounting standard:

1. If a check needs the most precise raw request-side data available after local tool execution, mount it on `ControlHook01AfterLocalToolResult`.
2. If a control step must affect what is sent to the model/provider, mount it on `ControlHook02BeforeModelRequest`.
3. If a step needs raw model/provider response data immediately after receive, mount it on `ControlHook03AfterModelResponse`.
4. If a step must run after all processing is complete and just before returning to the client, mount it on `ControlHook04BeforeClientReturn`.

Examples:

- tool-result error classification that needs exact local tool output -> hook 01
- injecting schema repair feedback or compact task tool specs into outbound request -> hook 02
- extracting `<<<freehand_status>>>` from raw model text -> hook 03
- stripping hidden schema blocks and producing final public projection -> hook 04

## Status Schema Block

The future status block is separate from the existing terminal completion block.

```text
<<<freehand_status>>>
{
  "schema_version": 1,
  "status": {
    "kind": "needs_task",
    "reason": "target_workspace_not_current",
    "target_cwd": "/Volumes/extension/code/zterm",
    "next_expected_tool": "task",
    "simple_request": false,
    "task_complete": false,
    "blocked": false,
    "needs_user_involvement": false
  }
}
<</freehand_status>>>
```

Rules:

- the raw block may be hashed and audited, but must not become request/task/user-visible data
- the body text outside the block remains public assistant text after projection filters
- status schema may include interaction facts, readiness facts, next-step facts, and user-involvement facts
- status schema must not create, dispatch, stop, close, or mutate any task
- model-visible repair feedback references field names and constraints, not hidden runtime internals

## Status-Driven Reasoning Rhythm

Status schema controls whether the framework may naturally stop, continue, ask for repair, ask the user, or expect a tool call.

Terminal decision examples:

- `finish_reason=stop` plus `status.simple_request=true` allows natural terminal completion for simple requests.
- If `simple_request` is absent or false, the framework checks `status.task_complete`.
- If `task_complete=true`, the framework requires `evidence`. If evidence exists, terminal completion may be accepted.
- After accepted task completion, the framework checks `learned` / `needs_record`. If present, it records to `note.md` or the owned memory path and tells the user that the record was made.
- If `task_complete=false` and `blocked=true`, the framework requires `blocked_reason`. Valid blocked status may stop in blocked state.
- If `task_complete=false` and `next_step` exists, the framework uses `next_step` as the next reasoning instruction and continues the turn.
- If `needs_user_involvement=true`, the framework requires an `options` array. Valid options allow the turn to stop and render selectable choices.
- If required fields are missing, the framework writes rejected status metadata and feeds the missing fields into the next repair prompt.

The framework may read status schema to control reasoning rhythm. It may not execute side effects from status schema.

## Built-In Action Tools

Use one or two framework tools, three maximum. The first implementation should prefer one general task tool plus optionally one query tool:

```text
task
```

Tool arguments carry the concrete operation:

```json
{
  "op": "create",
  "target_cwd": "/Volumes/extension/code/zterm",
  "title": "Review zterm architecture",
  "input_ref": "current_user_request"
}
```

```json
{
  "op": "dispatch",
  "task_id": "task-...",
  "dispatch_policy": {
    "kind": "workspace_match",
    "target_cwd": "/Volumes/extension/code/zterm"
  }
}
```

Supported `task.op` values:

- `create`
- `dispatch`
- `append`
- `stop`
- `close`
- `query`

If a separate query surface is required later, the maximum surface is:

```text
task
task-query
```

Do not create six separate tools such as `task-create`, `task-dispatch`, `task-stop`, and so on unless the tool registry proves one general tool is unsafe or untestable.

## Status Admission Chain

```text
Provider semantic text
  -> ControlStatus01TaggedRaw
  -> ControlStatus02ParsedBlock
  -> ControlStatus03ValidatedState
  -> ControlStatus04MetadataWatermarked
  -> ControlStatus05RhythmDecision | ControlStatusErr05Rejected
  -> Reasoning rhythm consumes accepted status
```

Only adjacent conversions are allowed.

### `ControlStatus01TaggedRaw`

Input:

- model output text
- session id
- turn id
- trace id
- model/provider identity when available

Output:

- zero or more tagged raw blocks
- raw block hash
- block source offsets for debug only

Failure:

- missing required status block when the current prompt contract demands one
- multiple blocks when the status contract requires exactly one

### `ControlStatus02ParsedBlock`

Input:

- raw tagged block

Output:

- parsed JSON object
- parse-normalization notes when compatible syntax was repaired
- parsed block hash

Failure:

- invalid JSON that cannot be normalized
- unsupported top-level shape

### `ControlStatus03ValidatedState`

Input:

- parsed JSON object

Output:

- typed interaction state
- schema version
- status kind enum
- status-specific fields
- validation result

Failure:

- missing status kind
- unsupported status kind
- missing required status fields
- unknown fields if the schema version forbids them
- incompatible field types

### `ControlStatus04MetadataWatermarked`

Input:

- typed status or typed rejection

Output:

- metadata-center record with watermark provenance

Required metadata entries:

- `control.kind=status`
- `control.schema_version`
- `control.status_kind`
- `control.validation_status`
- `control.retry_index`
- `control.raw_block_hash`
- `control.normalized_block_hash`
- `control.state_hash`
- `control.source_model`
- `control.source_provider`
- `control.source_agent_id`
- `control.source_session_id`
- `control.source_turn_id`
- `control.timestamp_ms`
- `control.repair_required`
- `control.error_code` when rejected
- `control.error_fields` when rejected

### `ControlStatus05RhythmDecision`

Input:

- validated status plus metadata admission success

Output:

- one reasoning rhythm decision:
  - `allow_terminal_simple`
  - `allow_terminal_completed`
  - `continue_with_next_step`
  - `wait_for_task_tool`
  - `stop_for_user_options`
  - `stop_blocked`
  - `request_status_repair`

Failure:

- metadata write failure blocks the rhythm decision
- missing required terminal/option/next-step fields becomes an error-center input

## Action Admission Chain

```text
Built-in tool call
  -> ControlAction01ToolCallRaw
  -> ControlAction02ToolArgsValidated
  -> ControlAction03MetadataWatermarked
  -> ControlAction04AcceptedDecision | ControlActionErr04Rejected
  -> Owner executor consumes accepted action
```

### `ControlAction01ToolCallRaw`

Input:

- tool call id
- tool name
- raw tool arguments
- source agent/session/turn/trace ids

Failure:

- unknown task tool
- tool call id missing
- unsupported tool operation

### `ControlAction02ToolArgsValidated`

Input:

- raw tool arguments

Output:

- typed task action
- operation enum
- operation-specific fields

Failure:

- missing `op`
- unsupported `op`
- missing operation-specific fields
- invalid task id or target cwd shape

### `ControlAction03MetadataWatermarked`

Required metadata entries:

- `control.kind=action`
- `control.tool_name`
- `control.tool_call_id`
- `control.action_op`
- `control.validation_status`
- `control.retry_index`
- `control.arguments_hash`
- `control.action_hash`
- `control.source_agent_id`
- `control.source_session_id`
- `control.source_turn_id`
- `control.timestamp_ms`
- `control.error_code` when rejected
- `control.error_fields` when rejected

### `ControlAction04AcceptedDecision`

Output:

- execution decision routed to one owner:
  - `task.orchestration`
  - `node.master-slave`
  - `runtime.ui-command-dispatch`
  - `runtime.checkpoint-rewind`

Failure:

- metadata write failure blocks the action
- owner target missing or unsupported becomes an error-center input

## Error Admission Chain

```text
Owner/runtime/provider/tool failure
  -> ErrorIn01ObservedFailure
  -> ErrorIn02Classified
  -> ErrorIn03RecoveryDecision
  -> ErrorIn04MetadataWatermarked
  -> ErrorIn05OwnerAction
```

### `ErrorIn01ObservedFailure`

Input:

- source owner
- source pipeline node
- trace/session/turn/task ids
- error code/message
- optional provider/tool/status details

This input is not yet policy. It is only an observed failure.

### `ErrorIn02Classified`

Output:

- error domain:
  - `schema`
  - `provider`
  - `tool`
  - `task`
  - `node`
  - `runtime`
  - `persistence`
  - `metadata`
  - `ui_protocol`
- error class:
  - `validation`
  - `recoverable`
  - `periodic_recoverable`
  - `blocked`
  - `cancelled`
  - `fatal`
- public visibility:
  - `hidden_control`
  - `status_line`
  - `task_card`
  - `terminal_summary`

### `ErrorIn03RecoveryDecision`

Output:

- one next action:
  - `repair_schema`
  - `retry_same_step`
  - `wait_until`
  - `stop_turn`
  - `fail_turn`
  - `block_task`
  - `cancel_task`
  - `escalate_to_user`
- retry count and cap
- repair prompt fields if applicable
- owner target for the next action

No owner should hand-roll this decision after the refactor.

### `ErrorIn04MetadataWatermarked`

Required entries:

- `error.domain`
- `error.class`
- `error.code`
- `error.source_owner`
- `error.source_pipeline_node`
- `error.recovery_action`
- `error.retry_index`
- `error.retry_cap`
- `error.public_visibility`
- `error.source_trace_id`
- `error.source_session_id`
- `error.source_turn_id`
- `error.source_task_id`
- `error.timestamp_ms`
- `error.raw_hash` when applicable

### `ErrorIn05OwnerAction`

Owner action examples:

- schema error -> reason/runtime sends repair request to model
- provider rate limit -> runtime waits or blocks according to policy
- tool validation failure -> tool result or task failure depending on stage
- metadata write failure -> block the originating mutation
- worker unavailable -> task blocked
- cancellation -> terminal cancelled / task stopped

## Task Control Semantics

Task orchestration must be control-center driven.

The framework must not decide from a raw path that it should create a task. Status schema may say the model believes a task is needed, but the side effect starts only when the model calls the built-in task tool.

1. prompt contract tells the model when to emit status schema and when to call the task tool
2. model emits status such as `needs_task`
3. model calls `task` with `op=create`, `op=dispatch`, `op=append`, `op=stop`, `op=close`, or `op=query`
4. control center validates and watermarks the task tool action
5. task owner consumes accepted action decision

Planned task states:

```text
Proposed
Created
Queued
Dispatched
Accepted
Running
WaitingInput
Stopping
Stopped
Succeeded
Failed
Blocked
Closed
```

Task state changes must include `action_metadata_id` and `tool_call_id` so the transition can be traced back to the accepted action decision. Task state may also reference `status_metadata_id` as context, but status metadata alone cannot authorize mutation.

## Prompt Contract

The system/developer prompt must contain:

- the hard status tag format
- schema version
- status kind enum
- field definitions
- examples for simple requests, task completion, blocked, next-step continuation, and user-option involvement
- examples for task tool calls using `task.op`
- invalid examples
- repair behavior
- instruction that status blocks are not user-visible prose
- instruction that side effects must use built-in tools, not status schema
- instruction that missing semantics must be expressed as `need_more_information` or `blocked`, not guessed

Prompt additions are not themselves control truth. They only instruct the model to emit status schema and call action tools.

## Public Projection

UI and public conversation projection must:

- strip `<<<freehand_status>>>...<</freehand_status>>>`
- strip existing `<freehand_completion>...</freehand_completion>` blocks
- render accepted status/rhythm decisions as status cards from protocol projections
- render accepted task action decisions as task cards from protocol projections
- render rejected status schema as concise status/error cards only when public visibility allows it
- never show raw control metadata unless debug details are enabled

## Gap List

| Gap | Current state | Required change |
| --- | --- | --- |
| Control owner | no central owner; completion schema parser exists in blocks/reason path only | add `control.center` owner and docs/function map/test design before implementation |
| Error owner | first skeleton classifies schema/tool/provider failures; task/node/UI error policy is still local or pending | route remaining owner failures through `error.center` and add ADP/UI query surfaces |
| Metadata admission | control and error skeletons write metadata rows, but cross-index/query surfaces are still missing | add query indexes by trace/session/turn/action/error code |
| Task control | `task.orchestration` owns task ledger/snapshot/lifecycle and ADP task query/subscribe; real worker execution is pending | add worker process/channel dispatch and task recovery |
| Model status block | `<<<freehand_status>>>` v1 parser and projection stripping exist | add full status repair loop and selectable user option projection |
| Built-in task action | one compact `task` tool exists with op arguments | route task action admission through control metadata before every task transition |
| Schema repair | completion schema repair exists; status schema repair does not | add status schema repair loop with retry cap |
| Runtime rhythm | runtime live loop makes some retry/failure decisions locally | move retry/repair/stop/block decisions to error center decisions |
| UI projection | UI strips completion block, not future status block | add public projection tests for status-block stripping |
| Recovery audit | metadata by trace exists, but no control/error cross-index | add query paths by trace/session/turn/task/action/error code |

## Implementation Phases

1. Documentation and maps
   - add `control.center`, `error.center`, and `task.orchestration` to feature routing
   - write function maps and test designs
   - migrate this design into generated mainline manifests when code begins

2. Skeleton hook contracts
   - land no-op `ControlHook01AfterLocalToolResult` immediately after local tool execution/result recording
   - land no-op `ControlHook02BeforeModelRequest` immediately before model/provider request send
   - land no-op `ControlHook03AfterModelResponse` immediately after model/provider response capture
   - land no-op `ControlHook04BeforeClientReturn` immediately before client-visible return/projection
   - lock the mounting standard for raw request checks, outbound controls, raw response processing, and final client return processing
   - add tests proving the four hook positions and no bypass for status parsing/action admission/error classification

3. Contracts and blocks
   - add status/action/error IDs and typed DTOs to `freehand-contracts`
   - add pure parsers/validators/projectors to `freehand-blocks`
   - add public projection stripping tests

4. Metadata center extension
   - add metadata kind or typed helpers for control/error records
   - add watermark validation helpers
   - add query indexes by trace/session/turn/action/error code

5. Control center implementation
   - parse/validate/normalize/repair status blocks
   - admit/validate task tool action calls
   - write accepted/rejected status and action decisions to metadata center
   - expose accepted status rhythm decisions and accepted action decisions

6. Error center implementation
   - classify observed failures
   - decide retry/repair/stop/fail/block/cancel
   - write decisions to metadata center

7. Runtime/reason integration
   - route completion/status schema errors through error center
   - remove local retry policy duplication from runtime live loop
   - require accepted action decisions before task/node transitions

8. Task and multi-agent integration
   - implement task state truth
   - upgrade delegated task assignment
   - add worker dispatch and task recovery

9. UI/ADP integration
   - query/subscribe control decisions, errors, tasks, and recovery status
   - render task cards and worker agent views

## Verification Requirements

Positive tests:

- `ControlHook01AfterLocalToolResult` runs immediately after local tool execution/result recording
- `ControlHook02BeforeModelRequest` runs immediately before provider request send
- `ControlHook03AfterModelResponse` runs immediately after provider response capture
- `ControlHook04BeforeClientReturn` runs immediately before client-visible projection
- hook mounting examples route to the correct phase: precise request/tool-result checks to hook 01, outbound control to hook 02, raw response parsing to hook 03, final public projection to hook 04
- valid status block writes watermarked accepted status metadata
- simple request status plus provider stop permits natural terminal completion
- completed task status requires evidence before terminal acceptance
- incomplete task status with `next_step` continues the reasoning loop
- needs-user-involvement status with options stops and renders options
- valid task tool action creates a task transition linked to `action_metadata_id` and `tool_call_id`
- valid schema repair response continues the same status lifecycle
- error center classifies provider/tool/schema failures and writes watermarked decisions
- UI public projection strips status blocks

Negative tests:

- a path that parses status, admits actions, or classifies errors outside the four fixed hook points fails a skeleton gate/test
- missing status tag when required causes rejected status metadata and repair prompt
- invalid JSON causes rejected status metadata and repair prompt
- missing status fields are not guessed
- status schema that says `needs_task` does not create a task without a task tool call
- `needs_user_involvement=true` without options is rejected and fed into schema repair
- task tool call with missing operation fields fails as an action-tool error
- metadata write failure blocks the status/action decision
- owner state mutation without accepted action metadata fails a gate/test
- local runtime retry decision bypassing error center fails a gate/test

Online proof:

- run symlink profile on `127.0.0.1:4042`
- submit a model task that emits `needs_task` status and then calls the `task` tool with `op=create`
- prove ADP can query status metadata, action metadata, task status, and worker projection
- restart daemon and prove task/control/error truth recovers from ledgers
