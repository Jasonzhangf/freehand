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
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `error.record_metadata`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `error`
- touched resources:
  - `metadata`
- resource operations:
  - `error.record_metadata`
- forbidden shortcuts:
  - Error-center decisions must not mutate owner truth before metadata admission.
  - Raw provider/tool/request payload text must not be stored as error metadata.

## Request Mainline

- runtime observes an owner/provider/tool/schema failure at a fixed pipeline node
- runtime constructs `ErrorCenterObservedFailure` with source owner, source pipeline node, code, message, retry index, and retry cap
- `classify_error_center_failure` maps the observed failure to one error domain, class, recovery action, public visibility, owner target, and repair fields
- runtime writes the accepted decision to `metadata.core` with writer owner `error.center`
- provider/tool/schema flow may continue, retry, repair, fail over, or fail only after the error-center metadata write succeeds
- ADP/read-only clients may query accepted error-center metadata rows through runtime-backed UI protocol queries without reading raw request or provider payloads

## Response Mainline

- schema validation failures classify as `schema` / `validation` / `repair_schema` until retry cap; schema/no-schema response mismatch is a model polishing pattern, not provider failure
- schema validation failures at retry cap classify to `stop_turn`
- provider executor failures classify as `provider` / `recoverable` / `retry_same_step` before retry cap, `failover_provider` when a configured alternate provider route is accepted, and `fail_turn` only when no provider route remains
- tool execution failures classify as `tool` / `validation` / `repair_schema`
- metadata rows carry domain, class, code, source owner, source pipeline node, recovery action, retry index, retry cap, public visibility, owner target, repair fields, and raw hash
- runtime projects only watermarked error-center fields into `UiErrorCenterEventProjection`; raw error message text remains absent from ADP query output

## Error Mainline

- unknown error sources classify as runtime/fatal/escalate_to_user
- metadata write failure returns explicit runtime metadata failure and blocks the originating state transition
- error metadata stores hashes for raw error messages and must not store provider body, tool output, user prompt, assistant text, request payload, or message content
- malformed or incomplete metadata rows are skipped by the read projection instead of being repaired into invented error-center semantics

## Shared Multi-Reference Functions

- `classify_error_center_failure`
  - owner: `crates/freehand-control/src/lib.rs`
  - purpose: single owner for framework error domain/class/recovery decisions
  - allowed callers: runtime live bridge and tests
  - why shared: prevents provider/tool/schema paths from hand-rolling retry/fail/repair policy
- `query_error_center_events_for_ui`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: project durable error-center metadata rows into UI-safe ADP query results
  - allowed callers: `RuntimeCommandDispatcher::query_runtime`
  - why shared: keeps metadata ledger reads in runtime while apps stay protocol-only

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ErrorCenterObservedFailure` | `crates/freehand-control/src/lib.rs` | capture source owner/node/code/message/retry facts before policy | observed failure | typed error-center input | runtime live bridge | error center contract | bound |
| 02 | `classify_error_center_failure` | `crates/freehand-control/src/lib.rs` | classify domain/class/recovery/public visibility | observed failure | error-center decision | runtime live bridge | error center owner | bound |
| 03 | `ErrorCenterDecision` | `crates/freehand-control/src/lib.rs` | carry classified recovery decision fields | classifier result | serializable decision | error center owner | runtime metadata writer | bound |
| 04 | `write_error_center_metadata` | `crates/freehand-runtime/src/lib.rs` | write watermarked error decision metadata and block on write failure | error-center decision | durable metadata row or explicit failure | runtime live bridge | metadata center | bound |
| 05 | `record_provider_error_metadata` | `crates/freehand-runtime/src/lib.rs` | route provider executor failure through error center before terminal failure materialization or accepted provider failover | provider executor failure plus route-switch eligibility | error-center row plus provider row or failover-provider row | runtime live bridge | error center metadata writer | bound |
| 06 | `run_live_provider_reason_turn` | `crates/freehand-runtime/src/lib.rs` | routes schema rejections and failed tool results through error center before repair/re-entry | schema/tool failure | repair/re-entry after metadata admission | runtime live bridge | error center metadata writer | bound |
| 07 | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | route `QueryErrorCenterEvents` to runtime-owned metadata projection | ADP/runtime query command | optional error-center query result | ADP query transport | runtime owner query bridge | bound |
| 08 | `query_error_center_events_for_ui` | `crates/freehand-runtime/src/lib.rs` | read session metadata ledger and filter error-center rows by trace/turn/domain | metadata ledger rows | UI-safe error-center event list | runtime query bridge | metadata center | bound |
| 09 | `project_error_center_event_for_ui` | `crates/freehand-runtime/src/lib.rs` | convert one watermarked error-center metadata envelope to protocol DTO | metadata envelope | `UiErrorCenterEventProjection` or skipped row | runtime query bridge | UI protocol DTO | bound |
| 10 | `UiCommand::QueryErrorCenterEvents` / `UiQueryResult::ErrorCenterEvents` | `crates/freehand-ui-protocol/src/lib.rs` | define protocol-owned ADP query/result shape for error-center events | query filters | UI-safe error-center projection | CLI/WebUI/daemon ADP | protocol owner | bound |
| 11 | `run_adp_error_query` / `run_adp_error_query_async` | `apps/freehand-cli/src/main.rs` | query daemon ADP for error-center event rows without WebUI | ADP URL plus session/trace/turn/domain filters | terminal-facing error-center summary | operator/agent test | daemon ADP | bound |

## Sync Status Against Code

- first implementation covers schema rejection, failed tool result, provider executor retry, and provider retry exhaustion failure
- ADP query and initial subscription projection for error-center metadata are implemented through `ui.protocol`, `runtime.ui-command-dispatch`, shared server transport, daemon, and CLI
- live push on new error-center metadata writes, task/node/UI error policy, and public WebUI error cards are pending
