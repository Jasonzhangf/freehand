# Control and Error Center Refactor

## Status

Planning truth for the next refactor. This document records the required semantic shape before implementation.

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

The gap is that control and error handling are still distributed across owners. Completion schema parsing, runtime retry decisions, provider/tool failures, task intent, cancellation, and node delegation do not pass through one central control/error admission path with durable watermark provenance.

## Non-Negotiable Rules

1. Control is not data.
2. Error policy is not local glue.
3. Model control feedback enters only through hard tagged schema blocks.
4. Public UI strips control blocks before rendering.
5. Runtime/task/node/reason owners consume accepted control-center truth, not raw assistant text.
6. Every control/error write must carry a watermark: writer, pipeline node, source model/agent/session/turn, trace id, timestamp, schema version, action/error code, validation status, retry index, and raw/control hashes.
7. Bad or incomplete model schema triggers repair/retry. The framework may normalize compatible syntax, but must not invent missing semantics.
8. Error classification and recovery route through one error center before flow state changes.

## Target Owners

### `control.center`

Recommended owner crate: `crates/freehand-control` after the refactor starts.

Until the crate exists, document it as a planned owner rather than overloading runtime.

Responsibilities:

- parse hard control blocks from model output
- validate schema shape and action-specific required fields
- normalize compatible syntax without inventing missing intent
- write accepted/rejected control records to `metadata.core`
- decide whether a control action is ready for execution or needs schema repair
- emit control-center events consumed by reason/runtime/task/node owners

Non-responsibilities:

- no task execution
- no provider wire behavior
- no UI rendering
- no session/turn truth writing
- no direct task state mutation before metadata admission

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

## Control Block

The future control block is separate from the existing terminal completion block.

```text
<<<freehand>>>
{
  "schema_version": 1,
  "control": {
    "action": "create_task",
    "reason": "target_workspace_not_current",
    "target_cwd": "/Volumes/extension/code/zterm",
    "task_title": "Review zterm architecture",
    "task_input_ref": "current_user_request",
    "dispatch": {
      "mode": "worker_agent",
      "target_agent": null
    }
  }
}
<</freehand>>>
```

Rules:

- the raw block may be hashed and audited, but must not become request/task/user-visible data
- the body text outside the block remains public assistant text after projection filters
- `task_input_ref` points at already-owned data truth; the control block should not duplicate full request content
- action-specific data that is control-only stays in the control record
- model-visible repair feedback references field names and constraints, not hidden runtime internals

## Control Admission Chain

```text
Provider semantic text
  -> ControlIn01TaggedRaw
  -> ControlIn02ParsedBlock
  -> ControlIn03ValidatedIntent
  -> ControlIn04MetadataWatermarked
  -> ControlIn05AcceptedDecision | ControlErr05RejectedDecision
  -> Owner executor consumes accepted decision
```

Only adjacent conversions are allowed.

### `ControlIn01TaggedRaw`

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

- missing required control block when the current prompt contract demands one
- multiple blocks when the action requires exactly one

### `ControlIn02ParsedBlock`

Input:

- raw tagged block

Output:

- parsed JSON object
- parse-normalization notes when compatible syntax was repaired
- parsed block hash

Failure:

- invalid JSON that cannot be normalized
- unsupported top-level shape

### `ControlIn03ValidatedIntent`

Input:

- parsed JSON object

Output:

- typed control intent
- schema version
- action enum
- action-specific fields
- validation result

Failure:

- missing action
- unsupported action
- missing required action fields
- unknown fields if the schema version forbids them
- incompatible field types

### `ControlIn04MetadataWatermarked`

Input:

- typed intent or typed rejection

Output:

- metadata-center record with watermark provenance

Required metadata entries:

- `control.schema_version`
- `control.action`
- `control.validation_status`
- `control.retry_index`
- `control.raw_block_hash`
- `control.normalized_block_hash`
- `control.intent_hash`
- `control.source_model`
- `control.source_provider`
- `control.source_agent_id`
- `control.source_session_id`
- `control.source_turn_id`
- `control.timestamp_ms`
- `control.repair_required`
- `control.error_code` when rejected
- `control.error_fields` when rejected

### `ControlIn05AcceptedDecision`

Input:

- validated intent plus metadata admission success

Output:

- execution decision routed to one owner:
  - `reason.turn`
  - `runtime.ui-command-dispatch`
  - `task.orchestration`
  - `node.master-slave`
  - `runtime.checkpoint-rewind`

Failure:

- metadata write failure blocks the decision
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

The framework must not decide from a raw path that it should create a task. Instead:

1. prompt contract tells the model when to emit task control fields
2. model emits `create_task`, `dispatch_task`, `append_task_input`, `stop_task`, `close_task`, or `query_task`
3. control center validates and watermarks the intent
4. task owner consumes accepted decision

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

Task state changes must include `control_metadata_id` so the transition can be traced back to the accepted control decision.

## Prompt Contract

The system/developer prompt must contain:

- the hard tag format
- schema version
- action enum
- field definitions
- examples for create/dispatch/stop/close/query
- invalid examples
- repair behavior
- instruction that control blocks are not user-visible prose
- instruction that missing semantics must be expressed as `need_more_information` or `blocked`, not guessed

Prompt additions are not themselves control truth. They only instruct the model to emit control schema.

## Public Projection

UI and public conversation projection must:

- strip `<<<freehand>>>...<</freehand>>>`
- strip existing `<freehand_completion>...</freehand_completion>` blocks
- render accepted control decisions as task/status cards from protocol projections
- render rejected control schema as concise status/error cards only when public visibility allows it
- never show raw control metadata unless debug details are enabled

## Gap List

| Gap | Current state | Required change |
| --- | --- | --- |
| Control owner | no central owner; completion schema parser exists in blocks/reason path only | add `control.center` owner and docs/function map/test design before implementation |
| Error owner | shared error contracts exist, but classification/recovery is local in runtime/provider/tool paths | add `error.center` policy and route owner failures through it |
| Metadata admission | metadata center exists but control/error records are not first-class schema-watermarked records | add control/error watermark entry schema and validation/gates |
| Task control | no task owner; node delegated task is progress text, WebUI task is cwd-bound session only | add `task.orchestration` state machine after control center |
| Model control block | only `<freehand_completion>` terminal block exists | add `<<<freehand>>>` control block parser and projection stripping |
| Schema repair | completion schema repair exists; task/control schema repair does not | add control schema repair loop with retry cap |
| Runtime rhythm | runtime live loop makes some retry/failure decisions locally | move retry/repair/stop/block decisions to error center decisions |
| UI projection | UI strips completion block, not future control block | add public projection tests for control-block stripping |
| Recovery audit | metadata by trace exists, but no control/error cross-index | add query paths by trace/session/turn/task/action/error code |

## Implementation Phases

1. Documentation and maps
   - add `control.center`, `error.center`, and `task.orchestration` to feature routing
   - write function maps and test designs
   - migrate this design into generated mainline manifests when code begins

2. Contracts and blocks
   - add control/error IDs and typed DTOs to `freehand-contracts`
   - add pure parsers/validators/projectors to `freehand-blocks`
   - add public projection stripping tests

3. Metadata center extension
   - add metadata kind or typed helpers for control/error records
   - add watermark validation helpers
   - add query indexes by trace/session/turn/action/error code

4. Control center implementation
   - parse/validate/normalize/repair control blocks
   - write accepted/rejected decisions to metadata center
   - expose accepted decision events

5. Error center implementation
   - classify observed failures
   - decide retry/repair/stop/fail/block/cancel
   - write decisions to metadata center

6. Runtime/reason integration
   - route completion/control schema errors through error center
   - remove local retry policy duplication from runtime live loop
   - require accepted control decisions before task/node transitions

7. Task and multi-agent integration
   - implement task state truth
   - upgrade delegated task assignment
   - add worker dispatch and task recovery

8. UI/ADP integration
   - query/subscribe control decisions, errors, tasks, and recovery status
   - render task cards and worker agent views

## Verification Requirements

Positive tests:

- valid control block writes watermarked accepted control metadata
- valid task control creates a task transition linked to `control_metadata_id`
- valid schema repair response continues the same control lifecycle
- error center classifies provider/tool/schema failures and writes watermarked decisions
- UI public projection strips control blocks

Negative tests:

- missing control tag when required causes rejected control metadata and repair prompt
- invalid JSON causes rejected control metadata and repair prompt
- missing action-specific fields are not guessed
- metadata write failure blocks the control decision
- owner state mutation without accepted control metadata fails a gate/test
- local runtime retry decision bypassing error center fails a gate/test

Online proof:

- run symlink profile on `127.0.0.1:4042`
- submit a model task that emits `create_task`
- prove ADP can query control metadata, task status, and worker projection
- restart daemon and prove task/control/error truth recovers from ledgers

