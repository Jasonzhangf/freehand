# Function Map: `error.center`

- feature_id: `error.center`
- owner crate: `crates/freehand-control`
- owner module: `crates/freehand-control/src/lib.rs`
- owner entry symbols:
  - `classify_error_center_failure`
  - `ErrorCenterObservedFailure`
  - `ErrorCenterDecision`
- mainline call source: `docs/mainline-calls/error.center.json`
- generated wiki: `docs/wiki/error.center.md`

## Request Mainline

- runtime observes an owner/provider/tool/schema failure at a fixed pipeline node
- runtime constructs `ErrorCenterObservedFailure` with source owner, source pipeline node, code, message, retry index, and retry cap
- `classify_error_center_failure` maps the observed failure to one error domain, class, recovery action, public visibility, owner target, and repair fields
- runtime writes the accepted decision to `metadata.core` with writer owner `error.center`
- provider/tool/schema flow may continue, repair, or fail only after the error-center metadata write succeeds

## Response Mainline

- schema validation failures classify as `schema` / `validation` / `repair_schema` until retry cap
- schema validation failures at retry cap classify to `stop_turn`
- provider executor failures classify as `provider` / `recoverable` / `fail_turn`
- tool execution failures classify as `tool` / `validation` / `repair_schema`
- metadata rows carry domain, class, code, source owner, source pipeline node, recovery action, retry index, retry cap, public visibility, owner target, repair fields, and raw hash

## Error Mainline

- unknown error sources classify as runtime/fatal/escalate_to_user
- metadata write failure returns explicit runtime metadata failure and blocks the originating state transition
- error metadata stores hashes for raw error messages and must not store provider body, tool output, user prompt, assistant text, request payload, or message content

## Shared Multi-Reference Functions

- `classify_error_center_failure`
  - owner: `crates/freehand-control/src/lib.rs`
  - purpose: single owner for framework error domain/class/recovery decisions
  - allowed callers: runtime live bridge and tests
  - why shared: prevents provider/tool/schema paths from hand-rolling retry/fail/repair policy

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ErrorCenterObservedFailure` | `crates/freehand-control/src/lib.rs` | capture source owner/node/code/message/retry facts before policy | observed failure | typed error-center input | runtime live bridge | error center contract | bound |
| 02 | `classify_error_center_failure` | `crates/freehand-control/src/lib.rs` | classify domain/class/recovery/public visibility | observed failure | error-center decision | runtime live bridge | error center owner | bound |
| 03 | `ErrorCenterDecision` | `crates/freehand-control/src/lib.rs` | carry classified recovery decision fields | classifier result | serializable decision | error center owner | runtime metadata writer | bound |
| 04 | `write_error_center_metadata` | `crates/freehand-runtime/src/lib.rs` | write watermarked error decision metadata and block on write failure | error-center decision | durable metadata row or explicit failure | runtime live bridge | metadata center | bound |
| 05 | `record_provider_error_metadata` | `crates/freehand-runtime/src/lib.rs` | route provider executor failure through error center before terminal failure materialization | provider executor failure | error-center row plus provider row | runtime live bridge | error center metadata writer | bound |
| 06 | `run_live_anthropic_reason_turn` | `crates/freehand-runtime/src/lib.rs` | routes schema rejections and failed tool results through error center before repair/re-entry | schema/tool failure | repair/re-entry after metadata admission | runtime live bridge | error center metadata writer | bound |

## Sync Status Against Code

- first implementation covers schema rejection, failed tool result, and provider executor failure
- task/node/UI error policy, ADP query surfaces, and public error cards are pending
