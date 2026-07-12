# Function Map: `tool.registry`

- feature_id: `tool.registry`
- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- mainline call source: `docs/mainline-calls/tool.registry.json`
- generated wiki: `docs/wiki/tool.registry.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `tool_call.execute_workspace_path`
- owner entry symbols:
  - `BuiltinToolRegistry::reasonix_aligned`
  - `BuiltinToolRegistry::definitions`
  - `BuiltinToolRegistry::implemented_definitions`
  - `BuiltinToolRegistry::implemented_schema_fingerprint`
  - `BuiltinToolRegistry::master_implemented_definitions`
  - `BuiltinToolRegistry::master_implemented_schema_fingerprint`
  - `BuiltinToolRegistry::execution_scope`
  - `BuiltinToolRegistry::execute`
  - `with_workspace_root`
  - `reasonix_aligned_builtin_specs`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `tool_call`
  - `workspace_path`
- touched resources:
  - `checkpoint`
- resource operations:
  - `tool_call.execute_workspace_path`
- forbidden shortcuts:
  - Runtime command restore paths must not mutate workspace paths without checkpoint owner admission.
  - Master framework tool surface must not expose Worker workspace file/search/write tools directly.
  - `execute_bash`
  - `execute_write_file`
  - `execute_edit_file`
  - `execute_multi_edit`

## Request Mainline

- runtime asks the tool owner for a per-run registry
- registry exports provider-neutral tool definitions without importing provider adapter DTOs
- registry can export one stable implemented-tool schema fingerprint for planner/cache diagnostics without leaking provider DTOs into reason owners
- registry keeps Reasonix-aligned tool names, schemas, and `read_only` metadata in one owner
- registry keeps task-management semantic action categories out of exposed tool
  names; task-management behavior enters through `task` with typed `op`
- registry schema guidance must be concise but complete enough to prevent
  model trial calls: `glob` declares workspace-scoped patterns, prefers
  relative patterns, accepts absolute patterns only inside the locked workspace,
  and rejects `~`, `..`, or external absolute discovery attempts; known paths
  should use `ls` for existence/type checks and `read_file` for files
- `task` schema points Master to the injected `TaskSpaceSnapshot` before
  exploratory query/list/history calls, says `status` is omitted for all
  visible tasks instead of `status="all"`, requires every task call to include
  top-level `op`, and documents the production create/assign pattern with
  `{"op":"create",...,"dispatch":{"mode":"none"}}` plus the configured Worker
- `task` schema tells the Master to expand `~/...` into an absolute existing
  repository/workspace path before writing `target_cwd`; `target_cwd` must not
  be `~`, a glob, or a not-yet-created output directory
- registry exposes `timer` as a standard internal framework tool for durable
  wakeups; timer semantics must not be encoded as task lifecycle operations
- timer schema tells the Master to schedule instead of dead-waiting when the
  next useful wait exceeds 3 minutes, then continue other ready Master-side
  work
- timer schema tells the Master not to claim a timer was scheduled unless the
  current turn has a successful `Timer scheduled` tool result; if no other work
  is ready after scheduling, the completion evidence should cite that timer
  result
- the generic registry retains all implemented definitions for non-master owners and owner tests
- the master-safe export excludes unrestricted shell scope and is now
  framework-only, exposing exactly `task` plus `timer`; Master live turns do not
  receive file/search/write, shell, `todo_write`, or `complete_step` schemas
- the master-safe schema fingerprint is derived from exactly the same safe export
- runtime classifies every registered tool as framework, workspace, shell, or network before execution policy is applied
- relative path-based tools resolve from one owner-supplied current workspace root
- `glob` is workspace-scoped: it accepts relative patterns and absolute
  patterns only when they remain under the locked workspace root, including
  symlink aliases that canonicalize back into that root
- read-only path tools may inspect readable external absolute or parent paths
- file-mutation tools remain locked to the current workspace root
- writable live exposure additionally depends on `tool.preview` and `runtime.checkpoint-rewind`
- provider adapters render schemas; they do not own tool registry truth

## Response Mainline

- completed provider tool calls enter `BuiltinToolRegistry::execute`
- task-management tool definitions expose `task(op=...)` rather than one
  standalone tool name per semantic action
- task-management tool field descriptions carry valid status filters,
  `record_execution` state names, dispatch restrictions, and configured-Worker
  routing examples so the model does not need failed calls to infer framework
  behavior
- `glob` tool definitions carry the workspace-scoped path contract and examples
  of valid relative or in-workspace absolute patterns, plus invalid `~`, `..`,
  and external absolute patterns
- path tools canonicalize symlink aliases before boundary decisions so
  user-facing paths such as `~/github/repo` can resolve to canonical task cwd
  paths such as `/Users/name/Documents/github/repo` without false denial
- `ls` reports directory entries or one file entry, so existence checks on
  generated files do not require a failing `glob` call
- `read_file` guidance tells the model to use `ls` first when the target may be
  a directory and not to read guessed or not-yet-created files
- timer tool definitions expose `timer(op="schedule"|"cancel"|"list")` for
  relative, absolute, local-time recurring, and local-time cron internal wakeups
  with a persisted prompt and examples
- timer prompts must say what current truth to inspect, what waited condition
  to revisit, and what decision to make when the wakeup fires
- first real foreground command execution set is:
  - `bash`
- first real read-only execution set is:
  - `read_file`
  - `glob`
  - `grep`
  - `ls`
- first real file-mutation execution set is:
  - `write_file`
  - `edit_file`
  - `multi_edit`
- implemented tools return user/model-visible tool result text
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
  - allowed callers: `read_file`, `glob`, `grep`, `ls`
  - related tests: read-file external-read test, runtime live tool loop test
  - why shared: keeps current-cwd truth in one owner helper instead of per-tool duplication
- `with_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: install an explicit thread-local workspace root for one tool execution so session cwd does not mutate process-global cwd or environment
  - allowed callers: runtime live bridge tool execution
  - related tests: runtime live tool execution with requested session cwd
  - why shared: keeps session workspace execution in the tool owner instead of process-global env switching in runtime
- `resolve_read_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve read-only path arguments from current cwd for relative paths while allowing readable absolute or parent paths
  - allowed callers: `read_file`, `grep`, `ls`
  - related tests: read-file, grep, and ls external-read tests
  - why shared: keeps read path resolution single-sourced without confusing read access with write permission
- `resolve_locked_write_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve writable path targets inside the locked workspace root even when the target file does not yet exist
  - allowed callers: `write_file`
  - related tests: write-file create/escape tests
  - why shared: keeps writable path-boundary enforcement single-sourced
- `write_text_atomic`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: persist file-mutation tool output through one owner-controlled temp-file-and-rename path
  - allowed callers: `write_file`, `edit_file`, `multi_edit`
  - related tests: write-file overwrite test, edit-file test, multi-edit test
  - why shared: keeps mutation write semantics centralized instead of per-tool duplication
- `replace_exactly_once`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: enforce exact single-match replacement semantics for text mutation tools
  - allowed callers: `edit_file`, `multi_edit`
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
- `BuiltinToolRegistry::master_implemented_definitions` / `BuiltinToolRegistry::master_implemented_schema_fingerprint`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: export one framework-only Master schema surface (`task`, `timer`) and matching deterministic fingerprint
  - allowed callers: runtime master live bridge, tests
  - related tests: master tool-surface exclusion test, runtime planner diagnostics tests
  - why shared: keeps master exposure policy in the registry owner instead of filtering schemas in runtime
- `BuiltinToolRegistry::execution_scope`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: classify registered tools for execution-time role/workspace policy
  - allowed callers: runtime live bridge, tests
  - related tests: master tool-surface exclusion and cross-workspace boundary tests
  - why shared: keeps tool category truth out of runtime string lists

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `BuiltinToolRegistry::reasonix_aligned` | `crates/freehand-tools/src/lib.rs` | create per-run built-in registry aligned with Reasonix names and schemas | none | registry | runtime live bridge/tests | tool owner | bound |
| 02 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | declare built-in tool metadata, schema, read-only state, and implementation state | static registry truth | tool specs | registry constructor/tests | tool owner | bound |
| 03 | `BuiltinToolRegistry::master_implemented_definitions` | `crates/freehand-tools/src/lib.rs` | export master-safe provider-neutral tool schemas without unrestricted shell scope | registry | master provider tool definitions | runtime live bridge | tool owner | bound |
| 04 | `BuiltinToolRegistry::master_implemented_schema_fingerprint` | `crates/freehand-tools/src/lib.rs` | export deterministic fingerprint for the exact master-safe tool schema | registry | stable master tool-schema fingerprint string | runtime live bridge | tool owner | bound |
| 05 | `BuiltinToolRegistry::execute` | `crates/freehand-tools/src/lib.rs` | dispatch completed tool calls into the single owner implementation set | `ReasonReq04ToolCall` | tool execution output | runtime live bridge | tool owner | bound |
| 05a | `with_workspace_root` | `crates/freehand-tools/src/lib.rs` | bind one explicit workspace root around a single registry tool execution | canonical session cwd + tool execution closure | tool execution output with workspace lock applied | runtime live bridge | tool owner | bound |
| 06 | `execute_bash` | `crates/freehand-tools/src/lib.rs` | run one foreground shell command from the locked workspace root with timeout and explicit failure reporting | `command` + optional `timeout_seconds` | combined stdout/stderr text | registry execute | command tool owner | bound |
| 07 | `execute_read_file` | `crates/freehand-tools/src/lib.rs` | read UTF-8 text from one readable file, resolving relative paths from cwd and permitting external readable paths | `path` + optional `offset` + optional `limit` | numbered text window | registry execute | read-only file tool owner | bound |
| 08 | `execute_write_file` | `crates/freehand-tools/src/lib.rs` | create or overwrite one UTF-8 text file inside the locked root | `path` + `content` | write summary | registry execute | file-mutation tool owner | bound |
| 09 | `execute_edit_file` | `crates/freehand-tools/src/lib.rs` | replace one exact text occurrence in one locked in-root file | `path` + `old_string` + `new_string` | edit summary | registry execute | file-mutation tool owner | bound |
| 10 | `execute_multi_edit` | `crates/freehand-tools/src/lib.rs` | apply ordered exact text edits and write once at the end | `path` + ordered `edits` | edit summary | registry execute | file-mutation tool owner | bound |
| 11 | `execute_glob` | `crates/freehand-tools/src/lib.rs` | match locked-workspace files by relative or in-workspace absolute glob pattern with recursive filename fallback | `pattern` | newline-separated match list | registry execute | read-only search tool owner | bound |
| 12 | `execute_grep` | `crates/freehand-tools/src/lib.rs` | search readable UTF-8 text files by regex, resolving relative paths from cwd and permitting external readable paths | `pattern` + optional `path` | `path:line:text` matches | registry execute | read-only search tool owner | bound |
| 13 | `execute_ls` | `crates/freehand-tools/src/lib.rs` | list readable directory entries/recursive tree or report one file entry, resolving relative paths from cwd and permitting external readable paths | optional `path` + optional `recursive` | newline-separated directory listing or one file entry | registry execute | read-only file tool owner | bound |

## Sync Status Against Code

- Reasonix-aligned built-in names and schemas are bound in `freehand-tools`
- current implemented tool set is:
  - `bash`
  - `read_file`
  - `write_file`
  - `edit_file`
  - `multi_edit`
  - `glob`
  - `grep`
  - `ls`
  - `todo_write`
  - `complete_step`
  - `timer`
- task-management semantic action names such as `query_task_board`,
  `dispatch_subtask`, and `approve_submission` are not exposed as standalone
  tools; they are prompt/test semantic categories mapped to `task(op=...)` or
  a future owner-approved small tool surface
- generic and master-safe implemented tool schema fingerprints are bound in `freehand-tools`; the runtime master bridge consumes only the framework-only master-safe fingerprint
- tool execution scopes are bound in the registry owner; master runtime exposure excludes all non-framework tools, including file/search/write, shell, `todo_write`, and `complete_step`
- read-only path tools use the owner-supplied workspace root only as current cwd for relative paths and may read/query external readable paths
- file-mutation tools are locked to the owner-supplied workspace root and return typed workspace-boundary violations on write escape
- first-version `bash` is foreground-only, starts in the locked workspace root, defaults to a 900-second timeout, and does not claim filesystem/network sandboxing
- first-version file-mutation tools are text-only, workspace-locked, require existing parent directories, and write through one atomic owner path
- checkpointed live writable execution now depends on the code-bound `tool.preview` and `runtime.checkpoint-rewind` owner paths instead of runtime-local mutation shortcuts
- `bg_jobs`, `kill_shell`, `wait_job`, web, notebook, and symbol-aware mutation tools remain registered but explicitly unimplemented until their lifecycle/gates are designed
- `timer` is implemented as a framework tool; `freehand-tools` owns schema
  exposure, while `freehand-runtime` owns durable schedule execution and Master
  wakeup routing
- `glob` schema describes workspace-scoped matching from the current workspace,
  tells the model to prefer relative paths, accepts absolute patterns only under
  the locked workspace after canonical/symlink resolution, and rejects `~`,
  `..`, and external absolute discovery attempts
- `ls` reports one file entry as well as directories, so models can verify
  generated output existence without using failing exact-file glob patterns
- `task` schema describes TaskSpaceSnapshot-first orchestration, legal
  `list_tasks` status values, omit-status-for-all behavior, and the
  `dispatch.mode="none"` then configured-Worker `assign` production pattern
- `task` schema and runtime failure text explicitly correct missing top-level
  `op`, including valid create/assign/query/history examples so the model does
  not need repeated failed task calls to infer the argument shape
- timer schema includes the default >3-minute wait policy: schedule a timer,
  avoid dead-waiting in the current turn, and continue other ready Master-side
  work after scheduling
- timer schema includes the no-oral-schedule rule: Master completion text must
  not claim a timer exists until the `timer` tool returns `Timer scheduled`
- timer daily, weekly, and cron fields are local-time semantics. Cron is a
  strict 5-field expression: `minute hour day-of-month month weekday`, with
  Sunday=0 through Saturday=6.
- the generated wiki must be regenerated from `docs/mainline-calls/tool.registry.json` when this function map truth changes
