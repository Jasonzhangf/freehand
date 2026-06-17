# Tool Registry Design

## Scope

This doc locks the first built-in tool surface for Freehand.

- feature_id: `tool.registry`
- owner: `crates/freehand-tools`
- consumers:
  - `crates/freehand-runtime`
  - provider adapters through provider-neutral tool definitions only
- non-owners:
  - app crates
  - provider wire DTO crates
  - `crates/freehand-reason`

## Core Truth

- built-in tool names, schemas, descriptions, and `read_only` metadata live only in `freehand-tools`
- built-in tool implementation state is explicit:
  - `implemented = true`
  - `implemented = false`
- registered but unimplemented tools must fail explicitly when executed
- unknown tools must fail explicitly when executed
- runtime must not invent demo tools or hardcode provider-exposed tool schemas outside the registry

Current first implemented set:

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

Related lifecycle owners that must stay separate:

- `tool.registry`
  - owns names, schemas, `read_only`, `implemented`, and execution ownership
- `tool.preview`
  - owns writable-tool preview truth and preview/execute parity
- `runtime.checkpoint-rewind`
  - owns pre-write snapshots, restore manifests, and rewind lifecycle

## Separation Rules

- `freehand-tools` owns tool registry truth and execution ownership
- first-version path tools are directory-locked to the canonical process current working directory
- first-version `bash` runs with its current working directory locked to that same workspace root
- first-version file-mutation tools are also workspace-locked and may only target existing parent directories inside that root
- first-version file-mutation tools must write atomically through the tool owner; no app/runtime/orchestrator may write file content on their behalf
- first-version `bash` does not claim filesystem or network sandboxing; it only owns shell invocation, cwd locking, timeout enforcement, stdout/stderr capture, and explicit failure reporting
- `freehand-runtime` may:
  - construct a per-run registry
  - expose only `implemented_definitions()` to live provider requests
  - route completed tool calls into `BuiltinToolRegistry::execute`
- provider adapters may:
  - render provider-neutral tool definitions
  - parse provider wire tool calls into shared contracts
- provider adapters may not:
  - own tool spec truth
  - own built-in tool implementation state
  - execute tools directly
- `freehand-reason` may carry tool-call and tool-result truth, but may not own built-in tool registry contents

## Exposure Gate

A new tool is not allowed onto the live request path until all of the following are true:

1. the tool has a spec entry in `freehand-tools`
2. the tool is bound in `docs/function-maps/tool.registry.md`
3. the tool has test coverage declared in `docs/testing/tool.registry.md`
4. the tool lifecycle and permission model are described in durable design docs
5. `implemented = true` is justified by real execution code in `freehand-tools`
6. for writable tools, `tool.preview` and `runtime.checkpoint-rewind` must both be locked and implemented enough for checkpointed live execution

If any of the above is false, the tool may remain registered only as explicitly unimplemented.

## First-Version Direction

- registry names and schemas should stay aligned with the Reasonix tool surface where semantics match
- first implemented tools should prefer deterministic, low-side-effect tools
- first real command tool is foreground-only `bash`:
  - input is `command` plus optional `timeout_seconds`
  - default timeout is `900` seconds, aligned with the Reasonix foreground safety cap baseline
  - command starts in the locked workspace root as its current working directory
  - stdout and stderr are captured into one combined result
  - non-zero exit and timeout both fail explicitly and preserve captured output in the error message
  - background jobs remain out of scope until `bg_jobs` / `kill_shell` / `wait_job` lifecycle is designed
- first real file/search batch is read-only and workspace-locked:
  - `read_file`
  - `glob`
  - `grep`
  - `ls`
- first real file-mutation batch is still workspace-locked and text-only:
  - `write_file`
  - `edit_file`
  - `multi_edit`
- first-version file-mutation limits:
  - target path must stay inside locked workspace root
  - target parent directory must already exist
  - `edit_file` requires exactly one match
  - `multi_edit` applies ordered exact edits in memory and writes once at the end
  - shell, network, notebook, and symbol-aware mutation remain out of scope

## Error Policy

- unknown name -> explicit `UnknownTool`
- known but not implemented -> explicit `UnimplementedTool`
- malformed arguments -> explicit `InvalidArguments`
- `bash` timeout -> explicit `ExecutionFailed`
- `bash` non-zero exit -> explicit `ExecutionFailed`
- no fallback, no synthetic success, no silent skip

## Test Direction

- white-box:
  - spec export shape
  - implemented/unimplemented state
  - argument validation
  - explicit error classes
  - foreground `bash` success / cwd / timeout / exit-failure tests
- module black-box:
  - runtime can advertise implemented tool definitions without hardcoded demo tools
  - runtime can execute a completed tool call through the registry and re-enter the result
- project black-box:
  - live provider tool loop uses registry-owned tools rather than app/runtime-local mock tool logic

## Update Rule

If the tool surface changes, update in the same change set:

- `docs/architecture/feature-map.md`
- `docs/function-maps/tool.registry.md`
- `docs/testing/tool.registry.md`
- this design doc
- `MEMORY.md` when the truth becomes durable
