# Function Map: `control.center`

- feature_id: `control.center`
- owner crate: `crates/freehand-control`
- owner module: `crates/freehand-control/src/lib.rs`
- owner entry symbols:
  - `parse_control_status_block`
  - `control_status_rhythm_decision`
  - `strip_control_status_block`

## Request Mainline

- runtime keeps the existing reason/provider/tool loop as the mainline
- `ControlHook01AfterLocalToolResult` runs after local tool execution and records tool-result control metadata through `metadata.core`
- `ControlHook02BeforeModelRequest` runs immediately before provider request send and records that status schema guidance is present
- control status guidance is provided as a stable developer context segment; it does not mutate task state
- task side effects remain out of status schema and must later enter through built-in action tools

## Response Mainline

- `ControlHook03AfterModelResponse` parses hidden `<<<freehand_status>>>` blocks from model text after provider response capture
- `parse_control_status_block` validates schema version, field types, and required fields for simple stop, task completion, blocked, user options, or next step
- accepted status writes watermarked metadata with writer owner `control.center`, hook node, status schema version, validation state, decision, and raw/control hashes
- rejected status writes watermarked metadata with validation failure and field-level issue summary
- `ControlHook04BeforeClientReturn` strips hidden control blocks from public projection and records the stripping decision
- basic stopHook allows terminal stop for `simple_request=true` or `task_complete=true` with `evidence`, while legacy `<freehand_completion>` remains supported when no status block is present

## Error Mainline

- malformed status JSON is rejected with field-level feedback
- missing required fields are rejected explicitly; runtime must not infer the missing semantic intent
- metadata write failure remains an explicit runtime failure
- status schema never executes task mutations

## Shared Multi-Reference Functions

- `strip_control_status_block`
  - owner: `crates/freehand-control/src/lib.rs`
  - purpose: remove hidden control blocks before public projection
  - allowed callers: runtime and UI protocol projection
  - why shared: prevents each projection surface from inventing its own hidden-block parser

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `control_status_contract_segment` | `crates/freehand-runtime/src/lib.rs` | provide model-visible status schema guidance before provider request | live round context | developer context segment | runtime live bridge | control schema owner | bound |
| 02 | `write_control_hook_metadata` | `crates/freehand-runtime/src/lib.rs` | write hook metadata with `control.center` owner watermark | hook node + trace/session/turn + accepted entries | durable metadata record | runtime live bridge | metadata center | bound |
| 03 | `parse_control_status_block` | `crates/freehand-control/src/lib.rs` | parse and validate hidden status schema block | provider text | typed status submission or rejection | runtime live bridge | control center | bound |
| 04 | `control_status_rhythm_decision` | `crates/freehand-control/src/lib.rs` | convert validated status into passive rhythm decision | status submission | stop/continue/block/options decision | runtime live bridge | control center | bound |
| 05 | `strip_control_status_block` | `crates/freehand-control/src/lib.rs` | remove hidden status block before projection | assistant/terminal text | public text | runtime and UI protocol | control center | bound |

## Sync Status Against Code

- first implementation is a skeleton plus basic stopHook
- task action tool execution and task lifecycle orchestration are implemented under `task.orchestration`, not inside `control.center`
- status-driven user option rendering is represented as a rhythm decision but does not yet have a selectable UI projection
