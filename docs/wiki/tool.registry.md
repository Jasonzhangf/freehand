# Wiki: `tool.registry`

Generated from `docs/mainline-calls/tool.registry.json`. Do not edit by hand.

- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- function map: `docs/function-maps/tool.registry.md`
- generated wiki: `docs/wiki/tool.registry.md`
- test design: `docs/testing/tool.registry.md`

## Resource Operation Backlinks

- tool_call.execute_workspace_path

## Request Mainline

- runtime asks the tool owner for a per-run registry
- registry exports provider-neutral tool definitions without importing provider adapter DTOs
- registry can export generic and master-safe stable implemented-tool schema fingerprints for planner/cache diagnostics without leaking provider DTOs into reason owners
- registry keeps Reasonix-aligned tool names, schemas, and `read_only` metadata in one owner
- registry exposes `timer` as a standard internal framework tool for durable wakeups and keeps timer semantics out of task lifecycle operations
- registry classifies registered tools as framework, workspace, shell, or network execution scope
- the master-safe export excludes unrestricted shell scope while retaining framework and workspace-scoped tools
- relative path-based tools resolve from one owner-supplied current workspace root
- `glob` accepts relative patterns and absolute patterns only when they remain under the locked workspace root after canonical/symlink resolution; it rejects `~`, `..`, and external absolute patterns
- read-only path tools may inspect readable external absolute or parent paths
- file-mutation tools remain locked to the current workspace root
- writable live exposure additionally depends on `tool.preview` and `runtime.checkpoint-rewind`
- provider adapters render schemas; they do not own tool registry truth

## Response Mainline

- completed provider tool calls enter `BuiltinToolRegistry::execute`
- first real foreground command execution set is: `bash`
- first real read-only execution set is: `read_file`, `glob`, `grep`, `ls`; `ls` can list directories or report one file entry for existence checks
- first real file-mutation execution set is: `write_file`, `edit_file`, `multi_edit`
- first internal timer execution surface is `timer(op="schedule"|"cancel"|"list")` with relative, absolute, local-time recurring, and local-time cron schedule fields plus a persisted prompt and examples
- implemented tools return user/model-visible tool result text
- foreground `bash` remains generically executable for non-master owners and tests but is absent from the master-safe export
- runtime may bind one explicit per-call workspace root through `with_workspace_root` without mutating process-global cwd or environment
- unsupported or unimplemented tools fail explicitly and do not become successful tool-result truth

## Error Mainline

- unknown tool names return `ToolRegistryError::UnknownTool`
- registered but not implemented tools return `ToolRegistryError::UnimplementedTool`
- invalid tool arguments return `ToolRegistryError::InvalidArguments`
- writable path escape returns typed `ToolRegistryError::WorkspaceBoundaryViolation`
- foreground `bash` timeout and non-zero exit return `ToolRegistryError::ExecutionFailed`
- runtime and filesystem failures return `ToolRegistryError::ExecutionFailed`

## Shared Multi-Reference Functions

- `locked_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: derive the canonical current workspace root for relative path resolution and writable path locking, respecting the explicit per-call workspace context installed by `with_workspace_root`
  - allowed callers: execute_read_file, execute_glob, execute_grep, execute_ls
  - related tests: read-file external-read test, runtime live tool loop test
  - why shared: keeps current-cwd truth in one owner helper instead of per-tool duplication
- `with_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: install an explicit thread-local workspace root for one registry tool execution
  - allowed callers: runtime live bridge
  - related tests: runtime live tool execution with requested session cwd
  - why shared: keeps session workspace execution in the tool owner instead of process-global env switching in runtime
- `resolve_read_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve read-only path arguments from current cwd for relative paths while allowing readable absolute or parent paths
  - allowed callers: execute_read_file, execute_grep, execute_ls
  - related tests: read-file, grep, and ls external-read tests
  - why shared: keeps read path resolution single-sourced without confusing read access with write permission
- `resolve_glob_pattern`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve a workspace-scoped glob pattern, accepting relative patterns and absolute patterns only under the locked workspace root after canonical/symlink resolution
  - allowed callers: execute_glob
  - related tests: glob in-workspace absolute pattern test, glob external absolute and tilde rejection tests, path-tool symlink-alias positive test
  - why shared: keeps glob boundary semantics single-sourced instead of scattering path checks across schema guidance and execution
- `resolve_locked_write_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve writable path targets inside the locked workspace root even when the target file does not yet exist
  - allowed callers: execute_write_file
  - related tests: write-file create/escape tests
  - why shared: keeps writable path-boundary enforcement single-sourced
- `write_text_atomic`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: persist file-mutation tool output through one owner-controlled temp-file-and-rename path
  - allowed callers: execute_write_file, execute_edit_file, execute_multi_edit
  - related tests: write-file overwrite test, edit-file test, multi-edit test
  - why shared: keeps mutation write semantics centralized instead of per-tool duplication
- `replace_exactly_once`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: enforce exact single-match replacement semantics for text mutation tools
  - allowed callers: execute_edit_file, execute_multi_edit
  - related tests: edit-file single-match test, edit-file multi-match rejection test
  - why shared: keeps exact-match editing semantics centralized
- `render_tool_arguments_json`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: render structured tool arguments without duplicating JSON conversion in runtime/tool owner code
  - allowed callers: provider adapters, tool registry diagnostics, tests
  - related tests: tool argument JSON render tests, tool registry execution tests
  - why shared: keeps tool argument conversion as a shared block instead of per-crate helpers
- `BuiltinToolRegistry::implemented_schema_fingerprint`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose a deterministic implemented-tool schema fingerprint for planner/cache diagnostics consumers
  - allowed callers: runtime live bridge, tests
  - related tests: registry fingerprint stability tests, runtime live bridge planner diagnostics tests
  - why shared: keeps tool-schema truth and canonicalization in the tool owner instead of runtime/reason duplication
- `BuiltinToolRegistry::master_implemented_definitions / BuiltinToolRegistry::master_implemented_schema_fingerprint`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: export the master-safe schema surface and matching deterministic fingerprint without unrestricted shell scope
  - allowed callers: runtime master live bridge, tests
  - related tests: master tool-surface exclusion test, runtime planner diagnostics tests
  - why shared: keeps master exposure policy in the registry owner instead of runtime-local filtering
- `BuiltinToolRegistry::execution_scope`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: classify registered tools for execution-time role and workspace policy
  - allowed callers: runtime live bridge, tests
  - related tests: master tool-surface exclusion test, cross-workspace boundary test
  - why shared: keeps tool category truth out of runtime string lists
- `reasonix_aligned_builtin_specs`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: declare the independent timer tool schema alongside other built-in tool contracts
  - allowed callers: BuiltinToolRegistry::reasonix_aligned, tests
  - related tests: timer tool schema export test, master and worker tool-surface tests
  - why shared: keeps timer schema truth in the registry owner instead of task or runtime prompt-only definitions

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `BuiltinToolRegistry::reasonix_aligned` | `crates/freehand-tools/src/lib.rs` | create per-run built-in registry aligned with Reasonix names and schemas | none | registry | runtime live bridge/tests | tool owner |  |  |  | bound |
| 02 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | declare built-in tool metadata, schema, read-only state, and implementation state | static registry truth | tool specs | registry constructor/tests | tool owner |  |  |  | bound |
| 03 | `BuiltinToolRegistry::master_implemented_definitions` | `crates/freehand-tools/src/lib.rs` | export master-safe provider-neutral tool schemas without unrestricted shell scope | registry | master provider tool definitions | runtime live bridge | tool owner |  |  |  | bound |
| 04 | `BuiltinToolRegistry::master_implemented_schema_fingerprint` | `crates/freehand-tools/src/lib.rs` | export deterministic master-safe tool schema fingerprint for planner/cache diagnostics | registry | stable master tool-schema fingerprint string | runtime live bridge | tool owner |  |  |  | bound |
| 05 | `BuiltinToolRegistry::execute` | `crates/freehand-tools/src/lib.rs` | dispatch completed tool calls into the single owner implementation set | ReasonReq04ToolCall | tool execution output | runtime live bridge | tool owner | tool_call | workspace_path | tool_call.execute_workspace_path | bound |
| 06 | `execute_bash` | `crates/freehand-tools/src/lib.rs` | run one foreground shell command from the locked workspace root with timeout and explicit failure reporting | command plus optional timeout_seconds | combined stdout/stderr text | registry execute | command tool owner |  |  |  | bound |
| 07 | `execute_read_file` | `crates/freehand-tools/src/lib.rs` | read UTF-8 text from one readable file, resolving relative paths from cwd and permitting external readable paths | path plus optional offset plus optional limit | numbered text window | registry execute | read-only file tool owner |  |  |  | bound |
| 08 | `execute_write_file` | `crates/freehand-tools/src/lib.rs` | create or overwrite one UTF-8 text file inside the locked root | path plus content | write summary | registry execute | file-mutation tool owner |  |  |  | bound |
| 09 | `execute_edit_file` | `crates/freehand-tools/src/lib.rs` | replace one exact text occurrence in one locked in-root file | path plus old_string plus new_string | edit summary | registry execute | file-mutation tool owner |  |  |  | bound |
| 10 | `execute_multi_edit` | `crates/freehand-tools/src/lib.rs` | apply ordered exact text edits and write once at the end | path plus ordered edits | edit summary | registry execute | file-mutation tool owner |  |  |  | bound |
| 11 | `execute_glob` | `crates/freehand-tools/src/lib.rs` | match locked-workspace files by relative or in-workspace absolute glob pattern with recursive filename fallback | pattern | newline-separated match list | registry execute | read-only search tool owner |  |  |  | bound |
| 12 | `execute_grep` | `crates/freehand-tools/src/lib.rs` | search readable UTF-8 text files by regex, resolving relative paths from cwd and permitting external readable paths | pattern plus optional path | path:line:text matches | registry execute | read-only search tool owner |  |  |  | bound |
| 13 | `execute_ls` | `crates/freehand-tools/src/lib.rs` | list readable directory entries/recursive tree or report one file entry, resolving relative paths from cwd and permitting external readable paths | optional path plus optional recursive | newline-separated directory listing or one file entry | registry execute | read-only file tool owner |  |  |  | bound |
| 14 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | declare timer as an independent framework tool with schedule, cancel, list, relative, absolute, local-time recurring, local-time cron, weekday, skip-weekend, max-runs, reason, prompt, and example schema fields | static timer registry truth | provider-neutral timer tool definition | registry constructor/tests | tool owner |  |  |  | bound |

## Sync Status Against Mainline Call

- Reasonix-aligned built-in names and schemas are bound in `freehand-tools`
- current implemented tool set is: `bash`, `read_file`, `write_file`, `edit_file`, `multi_edit`, `glob`, `grep`, `ls`, `todo_write`, `complete_step`, `timer`
- generic and master-safe implemented tool schema fingerprints are bound in `freehand-tools`; the runtime master bridge consumes only the master-safe fingerprint
- tool execution scopes are bound in the registry owner and master exposure excludes shell scope
- read-only path tools use the owner-supplied workspace root only as current cwd for relative paths and may read/query external readable paths
- file-mutation tools are locked to the owner-supplied workspace root and return typed workspace-boundary violations on write escape
- first-version `bash` is foreground-only, starts in the locked workspace root, defaults to a 900-second timeout, and does not claim filesystem/network sandboxing
- first-version file-mutation tools are text-only, workspace-locked, require existing parent directories, and write through one atomic owner path
- checkpointed live writable execution now depends on the code-bound `tool.preview` and `runtime.checkpoint-rewind` owner paths instead of runtime-local mutation shortcuts
- `timer` is implemented as a framework tool: `freehand-tools` owns schema exposure while `freehand-runtime` owns durable schedule execution and Master wakeup routing
- timer daily, weekly, and cron fields use local-time semantics; cron is a strict 5-field expression: minute hour day-of-month month weekday
- `bg_jobs`, `kill_shell`, `wait_job`, web, notebook, and symbol-aware mutation tools remain registered but explicitly unimplemented until their lifecycle/gates are designed
