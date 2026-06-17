# Function Map: `tool.preview`

- feature_id: `tool.preview`
- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- mainline call source: `docs/mainline-calls/tool.preview.json`
- generated wiki: `docs/wiki/tool.preview.md`
- owner entry symbols:
  - `pending: writable tool preview dispatch in freehand-tools`
  - `pending: preview-capable registry surface in freehand-tools`

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

- `pending: exact transform helper shared by preview and execute`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: guarantee one semantic transform path for writable tools
  - allowed callers: writable preview paths, writable execute paths
  - related tests: preview/execute parity tests
  - why shared: preview and execute must not diverge into duplicated semantics
- `pending: preview change renderer`
  - owner: `crates/freehand-tools/src/lib.rs` or `crates/freehand-blocks`
  - purpose: project canonical preview truth into unified diff or compact text without changing semantics
  - allowed callers: runtime debug projection, future UI projections, tests
  - related tests: preview projection smoke
  - why shared: rendering should stay downstream from canonical preview truth

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `pending: BuiltinToolRegistry preview dispatch` | `crates/freehand-tools/src/lib.rs` | route writable tool preview requests into the single owner preview path | writable tool call | preview request dispatch | runtime checkpoint owner | tool preview owner | pending |
| 02 | `pending: write_file preview entry` | `crates/freehand-tools/src/lib.rs` | compute create/overwrite preview without writing | `path` plus `content` | canonical file-change truth | preview dispatch | write tool preview owner | pending |
| 03 | `pending: edit_file preview entry` | `crates/freehand-tools/src/lib.rs` | compute exact-match edit preview without writing | `path` plus `old_string` plus `new_string` | canonical file-change truth | preview dispatch | edit tool preview owner | pending |
| 04 | `pending: multi_edit preview entry` | `crates/freehand-tools/src/lib.rs` | compute ordered multi-edit preview without writing | `path` plus ordered edits | canonical file-change truth | preview dispatch | multi-edit preview owner | pending |
| 05 | `pending: delete_range preview entry` | `crates/freehand-tools/src/lib.rs` | compute anchor-based delete preview without writing | `path` plus range anchors | canonical file-change truth | preview dispatch | delete-range preview owner | pending |

## Metadata / Request Isolation Notes

- preview truth is filesystem-change metadata, not provider request content
- preview diagnostics and diff renderings must not be re-injected into request text except through a deliberate request-builder owner path
- preview contract additions that become shared cross-module types must land in `crates/freehand-contracts`

## Sync Status Against Code

- design truth is locked
- current code has writable execute paths but no code-bound preview owner path yet
- current live writable tool path therefore lacks checkpoint-ready preview semantics
- generated wiki must be regenerated from `docs/mainline-calls/tool.preview.json` when this function-map truth changes
