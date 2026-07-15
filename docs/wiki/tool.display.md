# Wiki: `tool.display`

Generated from `docs/mainline-calls/tool.display.json`. Do not edit by hand.

- owner crate: `crates/freehand-blocks`
- owner module: `crates/freehand-blocks/src/tool_display.rs`
- function map: `docs/function-maps/tool.display.md`
- generated wiki: `docs/wiki/tool.display.md`
- test design: `docs/testing/tool.display.md`

## Request Mainline

- UI-visible tool display classification enters one pure parser owner
- Parser input is shared tool contracts: tool name, arguments, status, and result text
- Each display class has one independent parser function
- UI clients do not infer display category from raw tool text

## Response Mainline

- read-file tools project target path and parameter_summary plus read status without exposing file contents in the main card
- file-mutation tools project target path plus mutation kind and compact diff-oriented semantic payload when available
- search/list tools project pattern or path target plus match/list status without dumping full output into the main card
- plan tools project compact plan status and counts
- framework task tools project Task Center operation, task id/title, assignee, status, target cwd, and dispatch mode without UI-local parsing
- framework timer tools project schedule/cancel/list intent, timer id, timing, reason, and wakeup prompt without encoding timer truth as task truth
- shell tools project command intent and command-shaped read/search/list invocations through parser-owned classification
- generic tools project low-noise name, key arguments, and status

## Error Mainline

- missing or malformed arguments project explicit unknown or partial target fields
- unknown tools route to generic display projection
- parser output never rewrites model-visible tool payload or tool result truth

## Shared Multi-Reference Functions

- `project_tool_call_display`
  - owner: `crates/freehand-blocks/src/tool_display.rs`
  - purpose: produce a waiting-state structured display projection for a tool call
  - allowed callers: ui.protocol, tests
  - related tests: tool display projection tests, public conversation projection tests
  - why shared: keeps WebUI, Android, and CLI from duplicating tool display classification
- `project_tool_result_display`
  - owner: `crates/freehand-blocks/src/tool_display.rs`
  - purpose: update a structured display projection with completed or failed tool result semantics
  - allowed callers: ui.protocol, tests
  - related tests: result update projection tests
  - why shared: keeps result success/failure display semantics protocol-owned and UI-independent

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `project_tool_call_display` | `crates/freehand-blocks/src/tool_display.rs` | create structured display projection from a tool call | tool name plus arguments | display projection | ui.protocol | tool display owner |  |  |  | bound |
| 02 | `classify_tool_display_kind` | `crates/freehand-blocks/src/tool_display.rs` | map tool name and shell command shape to a display class | tool name plus arguments | display kind | display projector | classifier |  |  |  | bound |
| 03 | `parse_read_file_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse read/list target fields | tool arguments | read/list display fields | display projector | read parser |  |  |  | bound |
| 04 | `parse_file_mutation_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse write/edit target and diff-oriented fields | tool arguments | mutation display fields | display projector | mutation parser |  |  |  | bound |
| 05 | `parse_search_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse search pattern/path fields | tool arguments | search display fields | display projector | search parser |  |  |  | bound |
| 06 | `parse_plan_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse task/plan fields | tool arguments | plan display fields | display projector | plan parser |  |  |  | bound |
| 07 | `parse_task_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse framework Task Center operation, task id/title, assignee, status, cwd, and dispatch mode | task tool arguments | task display fields | display projector | task parser |  |  |  | bound |
| 08 | `parse_timer_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse framework timer operation, id, timing, reason, and wakeup prompt | timer tool arguments | timer display fields | display projector | timer parser |  |  |  | bound |
| 09 | `parse_shell_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse shell command intent without UI guessing | shell arguments | shell display fields | display projector | shell parser |  |  |  | bound |
| 10 | `parse_generic_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse unknown or miscellaneous tools into low-noise argument summary | tool arguments | generic display fields | display projector | generic parser |  |  |  | bound |
| 11 | `project_tool_result_display` | `crates/freehand-blocks/src/tool_display.rs` | update display projection with success/failure result state | previous display plus tool result | updated display projection | ui.protocol | result projector |  |  |  | bound |

## Sync Status Against Mainline Call

- implementation is bound in crates/freehand-blocks/src/tool_display.rs
- ui.protocol consumes project_tool_call_display and project_tool_result_display when projecting UiToolActivity.display
- WebUI consumes protocol display projection including parameter_summary and does not classify tools locally
- framework task and timer tools are first-class display kinds in tool.display rather than generic UI labels
