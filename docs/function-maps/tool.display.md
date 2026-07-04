# Function Map: `tool.display`

- feature_id: `tool.display`
- owner crate: `crates/freehand-blocks`
- owner module: `crates/freehand-blocks/src/tool_display.rs`
- mainline call source: `docs/mainline-calls/tool.display.json`
- generated wiki: `docs/wiki/tool.display.md`
- owner entry symbols:
  - `project_tool_call_display`
  - `project_tool_result_display`
  - `classify_tool_display_kind`
  - `parse_read_file_tool_display`
  - `parse_file_mutation_tool_display`
  - `parse_search_tool_display`
  - `parse_plan_tool_display`
  - `parse_shell_tool_display`
  - `parse_generic_tool_display`

## Request Mainline

- UI-visible tool display classification enters one pure parser owner.
- Parser input is shared tool contracts: tool name, arguments, status, and result text.
- Tool display classification is not owned by WebUI, Android, CLI, runtime, provider, or reason orchestration.
- Each display class has one independent parser function.
- Parser output is structured semantic display projection, including `parameter_summary`, not raw term text.

## Response Mainline

- read-file tools project target path plus read status without exposing file contents in the main card.
- read-file tools must expose the requested file path in `parameter_summary` so UI can show the target without parsing raw arguments.
- file-mutation tools project target path plus mutation kind and, when available from arguments, a compact diff-oriented semantic payload.
- search/list tools project pattern or path target plus match/list status without dumping full output into the main card.
- plan tools project compact plan status and counts.
- shell tools project command intent; command-shaped read/search/list invocations are classified semantically by the parser owner, not by UI code.
- ordinary shell tools expose a structured `command` display field for UI truncation/display; the special `pwd` projection continues to hide raw `command=pwd` and renders as current-workspace reading.
- generic tools project low-noise name, key arguments, and status.
- result failure remains a tool execution result and does not become a system terminal failure.

## Error Mainline

- missing or malformed arguments project explicit unknown/partial target fields rather than silently guessing in UI.
- parser failure must not block UI protocol projection; unknown tools route to generic display projection.
- parser output never rewrites the model-visible tool payload or tool result truth.

## Shared Multi-Reference Functions

- `project_tool_call_display`
  - owner: `crates/freehand-blocks/src/tool_display.rs`
  - purpose: produce a waiting-state structured display projection for a tool call
  - allowed callers: `ui.protocol`, tests
  - related tests: tool display projection tests, public conversation projection tests
  - why shared: keeps WebUI, Android, and CLI from duplicating tool display classification
- `project_tool_result_display`
  - owner: `crates/freehand-blocks/src/tool_display.rs`
  - purpose: update a structured display projection with completed or failed tool result semantics
  - allowed callers: `ui.protocol`, tests
  - related tests: result update projection tests
  - why shared: keeps result success/failure display semantics protocol-owned and UI-independent

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `project_tool_call_display` | `crates/freehand-blocks/src/tool_display.rs` | create structured display projection from a tool call | tool name plus arguments | display projection | ui.protocol | tool display owner | bound |
| 02 | `classify_tool_display_kind` | `crates/freehand-blocks/src/tool_display.rs` | map tool name and shell command shape to a display class | tool name plus arguments | display kind | display projector | classifier | bound |
| 03 | `parse_read_file_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse read/list target fields | tool arguments | read/list display fields | display projector | read parser | bound |
| 04 | `parse_file_mutation_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse write/edit target and diff-oriented fields | tool arguments | mutation display fields | display projector | mutation parser | bound |
| 05 | `parse_search_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse search pattern/path fields | tool arguments | search display fields | display projector | search parser | bound |
| 06 | `parse_plan_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse task/plan fields | tool arguments | plan display fields | display projector | plan parser | bound |
| 07 | `parse_shell_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse shell command intent without UI guessing | shell arguments | shell display fields | display projector | shell parser | bound |
| 08 | `parse_generic_tool_display` | `crates/freehand-blocks/src/tool_display.rs` | parse unknown or miscellaneous tools into low-noise argument summary | tool arguments | generic display fields | display projector | generic parser | bound |
| 09 | `project_tool_result_display` | `crates/freehand-blocks/src/tool_display.rs` | update display projection with success/failure result state | previous display plus tool result | updated display projection | ui.protocol | result projector | bound |

## Sync Status Against Code

- implementation is bound in `crates/freehand-blocks/src/tool_display.rs`
- `ui.protocol` consumes `project_tool_call_display` and `project_tool_result_display` when projecting `UiToolActivity.display`
- WebUI consumes the protocol `display` projection, including `parameter_summary`, and does not classify tools locally
- WebUI consumes ordinary shell `command` display fields for truncated command display while keeping shell classification in `tool.display`
- generated wiki must be regenerated from `docs/mainline-calls/tool.display.json` when this function-map truth changes
