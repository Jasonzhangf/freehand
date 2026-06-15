# Function Map: `foundation.workspace`

- feature_id: `foundation.workspace`
- owner crate: `xtask`
- owner module: `xtask/src/main.rs`
- owner entry symbols:
  - `run_gates_check`

## Request Mainline

- repo root invokes `xtask gates check`
- gate runner verifies required files, workspace members, and policy doc snippets

## Response Mainline

- gate returns success when required repo truth and workspace structure are present
- gate returns explicit failure with missing path or missing policy snippet

## Error Mainline

- missing file or missing required snippet surfaces as gate failure
- no fallback path exists

## Shared Multi-Reference Functions

- none at current scaffold stage

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_gates_check` | `xtask/src/main.rs` | workspace gate orchestrator | repo root state | gate result | CLI `main` | helper verifiers | bound |
| 02 | `require_files` | `xtask/src/main.rs` | required-file presence check | repo file list | pass/fail | `run_gates_check` | filesystem | bound |
| 03 | `verify_workspace_members` | `xtask/src/main.rs` | workspace member cargo check | workspace member list | pass/fail | `run_gates_check` | filesystem | bound |
| 04 | `verify_skill_rules` | `xtask/src/main.rs` | skill rule snippet check | skill text | pass/fail | `run_gates_check` | file reader | bound |
| 05 | `verify_orchestrator_policy_docs` | `xtask/src/main.rs` | policy doc snippet check | docs text | pass/fail | `run_gates_check` | file reader | bound |

## Sync Status Against Code

- aligned with current scaffold implementation
