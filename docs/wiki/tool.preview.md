# Wiki: `tool.preview`

Generated from `docs/mainline-calls/tool.preview.json`. Do not edit by hand.

- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- function map: `docs/function-maps/tool.preview.md`
- generated wiki: `docs/wiki/tool.preview.md`
- test design: `docs/testing/tool.preview.md`

## Request Mainline

- runtime identifies a writable tool call that wants checkpointed live execution
- runtime asks the tool owner for preview truth before any file write
- tool owner applies the same argument validation, path lock, and transform semantics as execute
- preview computes canonical file-change truth without writing to disk
- runtime may use preview truth for checkpoint scope, debug evidence, and future UI projection

## Response Mainline

- preview returns structured file-change truth for one writable tool call
- preview truth identifies create / modify / delete semantics on locked paths
- preview truth remains semantic and replayable; unified diff or colorized display is downstream rendering
- preview/execute parity tests prove preview post-image equals execute post-image

## Error Mainline

- invalid tool arguments return explicit preview failure
- path escape or parent-directory violations return explicit preview failure
- ambiguity, anchor-not-found, or unsupported preview shape return explicit preview failure
- writable live execution without preview support must be blocked explicitly

## Shared Multi-Reference Functions

- `plan_write_file`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: calculate write-file preview truth and the exact persisted post-image used by execute
  - allowed callers: BuiltinToolRegistry::preview, execute_write_file, tests
  - related tests: write-file preview parity tests
  - why shared: preview and execute must share one create/overwrite transform path
- `plan_edit_file`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: calculate edit-file preview truth and the exact persisted post-image used by execute
  - allowed callers: BuiltinToolRegistry::preview, execute_edit_file, tests
  - related tests: edit-file preview parity tests
  - why shared: preview and execute must share one exact-match edit path
- `plan_multi_edit`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: calculate ordered multi-edit preview truth and the exact persisted post-image used by execute
  - allowed callers: BuiltinToolRegistry::preview, execute_multi_edit, tests
  - related tests: multi-edit preview parity tests
  - why shared: preview and execute must share one ordered edit path
- `parse_multi_edit_steps`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: parse and validate multi-edit step arguments before preview or execute proceeds
  - allowed callers: plan_multi_edit, tests
  - related tests: multi-edit invalid-argument rejection tests
  - why shared: edit-step parsing must not diverge between preview and execute

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `BuiltinToolRegistry::preview` | `crates/freehand-tools/src/lib.rs` | route writable tool preview requests into the single owner preview path | writable tool call | preview request dispatch | runtime checkpoint owner and tests | tool preview owner | bound |
| 02 | `plan_write_file` | `crates/freehand-tools/src/lib.rs` | compute create/overwrite preview without writing and return the exact post-image later persisted by execute | path plus content | canonical file-change truth | preview dispatch and execute | write tool preview owner | bound |
| 03 | `plan_edit_file` | `crates/freehand-tools/src/lib.rs` | compute exact-match edit preview without writing and return the exact post-image later persisted by execute | path plus old_string plus new_string | canonical file-change truth | preview dispatch and execute | edit tool preview owner | bound |
| 04 | `plan_multi_edit` | `crates/freehand-tools/src/lib.rs` | compute ordered multi-edit preview without writing and return the exact post-image later persisted by execute | path plus ordered edits | canonical file-change truth | preview dispatch and execute | multi-edit preview owner | bound |
| 05 | `pending: delete_range preview entry` | `crates/freehand-tools/src/lib.rs` | compute anchor-based delete preview without writing | path plus range anchors | canonical file-change truth | preview dispatch | delete-range preview owner | pending |

## Sync Status Against Mainline Call

- `BuiltinToolRegistry::preview` is now code-bound for `write_file`, `edit_file`, and `multi_edit`
- preview/execute parity now runs through one shared transform path for those three writable tools
- `delete_range` preview is still pending because its anchor semantics are not locked in code yet
- current live runtime path now consumes preview before writable execution and rejects previewless writable tools explicitly
- generated wiki must be regenerated from `docs/mainline-calls/tool.preview.json` when this function-map truth changes
