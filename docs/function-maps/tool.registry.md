# Function Map: `tool.registry`

- feature_id: `tool.registry`
- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- mainline call source: `docs/mainline-calls/tool.registry.json`
- generated wiki: `docs/wiki/tool.registry.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `tool_call.execute_workspace_path`
  - `tool_call.execute_external_http`
  - `tool_call.project_registry_to_ui`
- owner entry symbols:
  - `BuiltinToolRegistry::reasonix_aligned`
  - `BuiltinToolRegistry::definitions`
  - `BuiltinToolRegistry::implemented_definitions`
  - `BuiltinToolRegistry::implemented_schema_fingerprint`
  - `BuiltinToolRegistry::master_implemented_definitions`
  - `BuiltinToolRegistry::master_implemented_schema_fingerprint`
  - `BuiltinToolRegistry::worker_implemented_definitions`
  - `BuiltinToolRegistry::worker_implemented_schema_fingerprint`
  - `BuiltinToolRegistry::execution_scope`
  - `BuiltinToolRegistry::registry_projection`
  - `BuiltinToolRegistry::execute`
  - `with_workspace_root`
  - `reasonix_aligned_builtin_specs`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `tool_call`
  - `workspace_path`
  - `external_http_resource`
- touched resources:
  - `checkpoint`
  - `ui_projection`
- resource operations:
  - `tool_call.execute_workspace_path`
  - `tool_call.execute_external_http`
  - `tool_call.project_registry_to_ui`
- forbidden shortcuts:
  - Runtime command restore paths must not mutate workspace paths without checkpoint owner admission.
  - Master workspace tool surface must not expose a Worker task cwd as direct Master authority; Master local tools are locked to the current selected session cwd only.
  - `execute_bash`
  - `execute_write_file`
  - `execute_edit_file`
  - `execute_multi_edit`

## Request Mainline

- runtime asks the tool owner for a per-run registry
- registry exports provider-neutral tool definitions without importing provider adapter DTOs
- registry can export one stable implemented-tool schema fingerprint for planner/cache diagnostics without leaking provider DTOs into reason owners
- registry keeps Reasonix-aligned tool names, schemas, and `read_only` metadata in one owner
- the worker-safe export exposes the exact model-visible Worker tool surface
  and excludes shell/task/timer/unimplemented names so Worker guidance can name
  valid tools without hardcoding a second list
- registry keeps task-management semantic action categories out of exposed tool
  names; task-management behavior enters through `task` with typed `op`
- registry schema guidance must be concise but complete enough to prevent
  model trial calls: `glob` declares workspace-scoped patterns, prefers
  relative patterns, expands leading `~`, accepts absolute patterns only inside
  the locked workspace, and rejects `..` or external absolute discovery
  attempts; known paths should use `ls` for existence/type checks and
  `read_file` for files
- `task` schema points Master to the injected `TaskSpaceSnapshot` before
  exploratory query/list/history calls, says `status` is omitted for all
  visible tasks instead of `status="all"`, requires every task call to include
  top-level `op`, and documents the production create/assign pattern with
  `{"op":"create",...,"dispatch":{"mode":"none"}}` plus the configured Worker
- `task` schema tells the Master to prefer an expanded absolute existing
  repository/workspace path for `target_cwd`, while treating leading-~/symlink
  aliases as valid only when they resolve to an existing workspace; it forbids
  glob patterns, broad search paths, and not-yet-created output directories
- registry exposes `timer` as a standard internal framework tool for durable
  wakeups; timer semantics must not be encoded as task lifecycle operations
- registry can project one UI-safe built-in tool registry view containing
  schema, examples, guidance, read-only state, implementation state, execution
  scope, and Master/Worker exposure without executing tools or exposing
  provider-hosted broad search as a local `web_search` function
- timer schema tells the Master to schedule instead of dead-waiting when the
  next useful wait exceeds 3 minutes, then continue other ready Master-side
  work
- timer schema tells the Master not to claim a timer was scheduled unless the
  current turn has a successful `Timer scheduled` tool result; if no other work
  is ready after scheduling, the completion evidence should cite that timer
  result
- the generic registry retains all implemented definitions for non-master owners and owner tests
- the master-safe export excludes unrestricted shell scope, browser, and broad
  `web_search`, but exposes local locked-workspace tools, concrete-URL
  `web_fetch`, plus `task` and `timer`;
  Master live turns can directly read/search/write/edit inside the current
  selected session cwd, fetch known HTTP/HTTPS URLs, and must not receive
  shell, browser, broad `web_search`, `todo_write`, or `complete_step` schemas
- the master-safe schema fingerprint is derived from exactly the same safe export
- runtime classifies every registered tool as framework, workspace, shell, or network before execution policy is applied
- relative path-based tools resolve from one owner-supplied current workspace root
- `glob` is workspace-scoped: it accepts relative patterns and absolute
  patterns only when they remain under the locked workspace root, including
  leading-`~` and symlink aliases that canonicalize back into that root
- read-only path tools remain locked to the current workspace root after
  canonical/symlink resolution; external absolute paths are explicit
  workspace-boundary violations
- file-mutation tools remain locked to the current workspace root
- writable live exposure additionally depends on `tool.preview` and `runtime.checkpoint-rewind`
- provider adapters render schemas; they do not own tool registry truth
- runtime and WebUI may consume `BuiltinToolRegistry::registry_projection` only
  as read-only owner projection; WebUI must not hardcode a parallel tool list

## Response Mainline

- completed provider tool calls enter `BuiltinToolRegistry::execute`
- task-management tool definitions expose one strict top-level JSON `op`
  discriminator with exact examples such as `task({"op":"create",...})`,
  rather than pseudo-call syntax or one standalone tool per semantic action
- task-management tool field descriptions carry valid status filters,
  `record_execution` state names, dispatch restrictions, and configured-Worker
  routing examples so the model does not need failed calls to infer framework
  behavior
- `glob` tool definitions carry the workspace-scoped path contract and examples
  of valid relative, leading-`~`, or in-workspace absolute patterns, plus
  invalid `..` and external absolute patterns
- path tools canonicalize symlink aliases before boundary decisions so
  user-facing workspace aliases can resolve to canonical task cwd paths without
  false denial
- path tools absolute-normalize relative inputs against the locked workspace and
  include `path_diagnostic` on resolution failures with requested path,
  absolute path, nearest existing parent, nearest existing canonical parent,
  missing suffix, and symlink ancestors
- `ls` reports directory entries or one file entry, so existence checks on
  generated files do not require a failing `glob` call; it also tells the
  model not to keep listing guessed missing output directories
- `read_file` guidance tells the model to use `ls` first when the target may be
  a directory and not to read guessed files, binary sidecars, or
  not-yet-created output directories/files
- `web_fetch` tool definitions expose one bounded HTTP/HTTPS text fetch for a
  concrete URL; this is network capability, not broad search or browser
  automation, and it remains owned by `tool.registry` rather than
  `provider_request`
- timer tool definitions expose strict JSON `op` values `schedule`, `cancel`,
  and `list` for
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
  - `delete_range`
- first real network execution set is:
  - `web_fetch`
- worker model-visible execution set is exactly:
  - `complete_step`
  - `delete_range`
  - `edit_file`
  - `glob`
  - `grep`
  - `ls`
  - `multi_edit`
  - `read_file`
  - `todo_write`
  - `web_fetch`
  - `write_file`
- UI-safe registry projection includes global guidance that exact JSON schemas
  should be followed instead of trial calls, path tools are locked to the
  workspace with relative, leading-`~`, absolute, and symlink rules, provider
  hosted `web_search` is not a local Freehand function tool, and Master/Worker
  exposure differs by role
- implemented tools return user/model-visible tool result text
- unsupported or unimplemented tools fail explicitly and do not become successful tool-result truth

## Error Mainline

- unknown tool names return `ToolRegistryError::UnknownTool`
- registered but not implemented tools return `ToolRegistryError::UnimplementedTool`
- invalid tool arguments return `ToolRegistryError::InvalidArguments`
- writable path escape returns typed `ToolRegistryError::WorkspaceBoundaryViolation`
- foreground `bash` timeout and non-zero exit return `ToolRegistryError::ExecutionFailed`
- `web_fetch` HTTP, network, timeout, argument, byte-limit, and UTF-8 decode
  failures return explicit `ToolRegistryError` values and do not become
  successful tool-result truth
- runtime and filesystem failures return `ToolRegistryError::ExecutionFailed`

## Shared Multi-Reference Functions

- `locked_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: derive the canonical current workspace root for relative path resolution and writable path locking, respecting the explicit per-call workspace context installed by `with_workspace_root`
  - allowed callers: `read_file`, `glob`, `grep`, `ls`
  - related tests: read-file workspace-boundary test, runtime live tool loop test
  - why shared: keeps current-cwd truth in one owner helper instead of per-tool duplication
- `with_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: install an explicit thread-local workspace root for one tool execution so session cwd does not mutate process-global cwd or environment
  - allowed callers: runtime live bridge tool execution
  - related tests: runtime live tool execution with requested session cwd
  - why shared: keeps session workspace execution in the tool owner instead of process-global env switching in runtime
- `resolve_read_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve read-only path arguments from current cwd for relative paths, expand leading `~`, canonicalize symlink aliases, reject paths outside the locked workspace, and return owner path diagnostics on failures
  - allowed callers: `read_file`, `grep`, `ls`
  - related tests: read-file, grep, ls external absolute rejection tests, symlink-alias tests, and missing relative path diagnostic test
  - why shared: keeps read path resolution single-sourced without treating readable external files as workspace truth
- `PathResolutionDiagnostic`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: render model-visible path failure truth from the tool owner instead of relying on model guesses
  - allowed callers: `resolve_read_path`, `resolve_locked_path`, `resolve_locked_write_path`
  - related tests: missing relative path diagnostic test and missing symlink leaf diagnostic test
  - why shared: keeps absolute path conversion, nearest-existing parent, canonical parent, missing suffix, and symlink ancestor reporting consistent across path tools
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
  - purpose: export one Master schema surface with locked local workspace tools plus `task` and `timer`, and a matching deterministic fingerprint
  - allowed callers: runtime master live bridge, tests
  - related tests: master tool-surface exclusion test, runtime planner diagnostics tests
  - why shared: keeps master exposure policy in the registry owner instead of filtering schemas in runtime
- `BuiltinToolRegistry::worker_implemented_definitions` / `BuiltinToolRegistry::worker_implemented_schema_fingerprint`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: export the exact Worker model-visible tool surface, including `web_fetch`, used by Worker live bridge guidance and provider requests
  - allowed callers: runtime Worker live bridge, tests
  - related tests: worker tool-surface exclusion test and Worker guidance prompt-guard tests
  - why shared: keeps Worker tool availability in the registry owner so runtime guidance does not invent shell/readlink or drift from actual schemas
- `BuiltinToolRegistry::execution_scope`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: classify registered tools for execution-time role/workspace policy
  - allowed callers: runtime live bridge, tests
  - related tests: master tool-surface exclusion and cross-workspace boundary tests
  - why shared: keeps tool category truth out of runtime string lists
- `BuiltinToolRegistry::registry_projection`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: project UI-safe registry metadata, examples, guidance, scope, and Master/Worker exposure without executing tools or exposing local `web_search`
  - allowed callers: runtime UI query bridge and tests
  - related tests: `cargo test -p freehand-tools registry_projection -- --nocapture`
  - why shared: keeps the Tools dashboard and model-visible schema guidance on the same owner truth instead of duplicating tool lists in WebUI or runtime
- `execute_web_fetch`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: execute one bounded HTTP/HTTPS text fetch for a concrete URL and return status/content-type/body text or explicit HTTP/network/decode failure
  - allowed callers: `BuiltinToolRegistry::execute`
  - related tests: local HTTP fixture `web_fetch` execution test and invalid URL/argument tests
  - why shared: keeps network fetch semantics in the tool owner instead of provider/runtime prompt patches

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `BuiltinToolRegistry::reasonix_aligned` | `crates/freehand-tools/src/lib.rs` | create per-run built-in registry aligned with Reasonix names and schemas | none | registry | runtime live bridge/tests | tool owner | bound |
| 02 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | declare built-in tool metadata, schema, read-only state, and implementation state | static registry truth | tool specs | registry constructor/tests | tool owner | bound |
| 03 | `BuiltinToolRegistry::master_implemented_definitions` | `crates/freehand-tools/src/lib.rs` | export master-safe provider-neutral tool schemas with local workspace tools, concrete-URL `web_fetch`, `task`, and `timer`, without unrestricted shell scope | registry | master provider tool definitions | runtime live bridge | tool owner | bound |
| 04 | `BuiltinToolRegistry::master_implemented_schema_fingerprint` | `crates/freehand-tools/src/lib.rs` | export deterministic fingerprint for the exact master-safe tool schema | registry | stable master tool-schema fingerprint string | runtime live bridge | tool owner | bound |
| 04a | `BuiltinToolRegistry::worker_implemented_definitions` | `crates/freehand-tools/src/lib.rs` | export exact Worker-safe provider-neutral tool schemas without shell/task/timer/unimplemented names | registry | Worker provider tool definitions | runtime live bridge | tool owner | bound |
| 04b | `BuiltinToolRegistry::worker_implemented_schema_fingerprint` | `crates/freehand-tools/src/lib.rs` | export deterministic fingerprint for the exact Worker-safe tool schema | registry | stable Worker tool-schema fingerprint string | runtime live bridge | tool owner | bound |
| 05 | `BuiltinToolRegistry::execute` | `crates/freehand-tools/src/lib.rs` | dispatch completed tool calls into the single owner implementation set | `ReasonReq04ToolCall` | tool execution output | runtime live bridge | tool owner | bound |
| 05a | `with_workspace_root` | `crates/freehand-tools/src/lib.rs` | bind one explicit workspace root around a single registry tool execution | canonical session cwd + tool execution closure | tool execution output with workspace lock applied | runtime live bridge | tool owner | bound |
| 06 | `execute_bash` | `crates/freehand-tools/src/lib.rs` | run one foreground shell command from the locked workspace root with timeout and explicit failure reporting | `command` + optional `timeout_seconds` | combined stdout/stderr text | registry execute | command tool owner | bound |
| 07 | `execute_read_file` | `crates/freehand-tools/src/lib.rs` | read UTF-8 text from one file inside the locked workspace after canonical/symlink path resolution | `path` + optional `offset` + optional `limit` | numbered text window | registry execute | read-only file tool owner | bound |
| 08 | `execute_write_file` | `crates/freehand-tools/src/lib.rs` | create or overwrite one UTF-8 text file inside the locked root | `path` + `content` | write summary | registry execute | file-mutation tool owner | bound |
| 09 | `execute_edit_file` | `crates/freehand-tools/src/lib.rs` | replace one exact text occurrence in one locked in-root file | `path` + `old_string` + `new_string` | edit summary | registry execute | file-mutation tool owner | bound |
| 10 | `execute_multi_edit` | `crates/freehand-tools/src/lib.rs` | apply ordered exact text edits and write once at the end | `path` + ordered `edits` | edit summary | registry execute | file-mutation tool owner | bound |
| 11 | `execute_glob` | `crates/freehand-tools/src/lib.rs` | match locked-workspace files by relative or in-workspace absolute glob pattern with recursive filename fallback | `pattern` | newline-separated match list | registry execute | read-only search tool owner | bound |
| 12 | `execute_grep` | `crates/freehand-tools/src/lib.rs` | search UTF-8 text files by regex inside the locked workspace after canonical/symlink path resolution | `pattern` + optional `path` | `path:line:text` matches | registry execute | read-only search tool owner | bound |
| 13 | `execute_ls` | `crates/freehand-tools/src/lib.rs` | list locked-workspace directory entries/recursive tree or report one file entry after canonical/symlink path resolution | optional `path` + optional `recursive` | newline-separated directory listing or one file entry | registry execute | read-only file tool owner | bound |
| 13a | `execute_web_fetch` | `crates/freehand-tools/src/lib.rs` | fetch one concrete HTTP/HTTPS URL with timeout and byte limit | `url` + optional `timeout_seconds` + optional `limit` | fetched text result or explicit HTTP/network/decode error | registry execute | network fetch tool owner | bound |
| 15 | `BuiltinToolRegistry::registry_projection` | `crates/freehand-tools/src/lib.rs` | project UI-safe tool registry metadata, examples, guidance, scope, and Master/Worker exposure without executing tools | registry | UI-safe tool registry projection | runtime UI query bridge | tool owner | bound |

## Sync Status Against Code

- Reasonix-aligned built-in names and schemas are bound in `freehand-tools`
- current implemented tool set is:
  - `bash`
  - `read_file`
  - `write_file`
  - `edit_file`
  - `multi_edit`
  - `delete_range`
  - `glob`
  - `grep`
  - `ls`
  - `web_fetch`
  - `todo_write`
  - `complete_step`
  - `timer`
- task-management semantic action names such as `query_task_board`,
  `dispatch_subtask`, and `approve_submission` are not exposed as standalone
  tools; they are prompt/test semantic categories mapped to the strict task
  JSON `op` field or a future owner-approved small tool surface
- generic, master-safe, and worker-safe implemented tool schema fingerprints are
  bound in `freehand-tools`; the runtime master bridge consumes only the
  local-workspace-plus-framework master-safe fingerprint, and Worker live guidance/provider
  requests consume the worker-safe surface instead of guessing tool names
- tool execution scopes are bound in the registry owner; master runtime exposure includes locked local workspace file/search/write/edit tools, concrete-URL `web_fetch`, plus `task` and `timer`, while excluding shell, browser, broad `web_search`, `todo_write`, and `complete_step`
- registry projection is bound in `freehand-tools` and is the only Tools
  dashboard source for schema/examples/guidance/exposure; no browser-local
  function tool list or local `web_search` row is allowed
- read-only path tools use the owner-supplied workspace root as the locked
  boundary for relative, absolute, and leading-`~` paths after
  canonical/symlink resolution
- file-mutation tools are locked to the owner-supplied workspace root and return typed workspace-boundary violations on write escape
- first-version `bash` is foreground-only, starts in the locked workspace root, defaults to a 900-second timeout, and does not claim filesystem/network sandboxing
- first-version file-mutation tools are text-only, workspace-locked, require existing parent directories, and write through one atomic owner path
- checkpointed live writable execution now depends on the code-bound `tool.preview` and `runtime.checkpoint-rewind` owner paths instead of runtime-local mutation shortcuts
- `web_fetch` is implemented as a bounded concrete-URL network tool; broad
  `web_search`, browser automation, notebook, background jobs, kill/wait shell,
  and symbol-aware mutation tools remain unimplemented until their
  lifecycle/gates are designed
- `timer` is implemented as a framework tool; `freehand-tools` owns schema
  exposure, while `freehand-runtime` owns durable schedule execution and Master
  wakeup routing
- `glob` schema describes workspace-scoped matching from the current workspace,
  tells the model to prefer relative paths, expands leading `~`, accepts
  absolute patterns only under the locked workspace after canonical/symlink
  resolution, and rejects `..` and external absolute discovery attempts
- path resolution failures include owner-rendered `path_diagnostic` evidence so
  missing leaf paths under symlink parents are not misreported as unexpanded
  symlink failures
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
