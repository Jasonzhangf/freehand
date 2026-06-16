# Wiki: `tool.registry`

Generated from `docs/mainline-calls/tool.registry.json`. Do not edit by hand.

- owner crate: `crates/freehand-tools`
- owner module: `crates/freehand-tools/src/lib.rs`
- function map: `docs/function-maps/tool.registry.md`
- generated wiki: `docs/wiki/tool.registry.md`
- test design: `docs/testing/tool.registry.md`

## Request Mainline

- runtime asks the tool owner for a per-run registry
- registry exports provider-neutral tool definitions without importing provider adapter DTOs
- registry keeps Reasonix-aligned tool names, schemas, and `read_only` metadata in one owner
- path-based read-only tools resolve against one locked workspace root: the canonical process current working directory
- runtime may choose a subset of implemented definitions for live execution
- provider adapters render schemas; they do not own tool registry truth

## Response Mainline

- completed provider tool calls enter `BuiltinToolRegistry::execute`
- first real read-only execution set is: `read_file`, `glob`, `grep`, `ls`
- implemented tools return user/model-visible tool result text
- unsupported or unimplemented tools fail explicitly and do not become successful tool-result truth

## Error Mainline

- unknown tool names return `ToolRegistryError::UnknownTool`
- registered but not implemented tools return `ToolRegistryError::UnimplementedTool`
- invalid tool arguments return `ToolRegistryError::InvalidArguments`
- runtime and filesystem failures return `ToolRegistryError::ExecutionFailed`

## Shared Multi-Reference Functions

- `locked_workspace_root`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: derive the canonical locked workspace root for all first-version path tools
  - allowed callers: execute_read_file, execute_glob, execute_grep, execute_ls
  - related tests: read-file path-lock test, runtime live tool loop test
  - why shared: keeps directory-lock truth in one owner helper instead of per-tool duplication
- `resolve_locked_path`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: resolve path arguments and reject escapes outside the locked workspace root
  - allowed callers: execute_read_file, execute_grep, execute_ls
  - related tests: read-file path-lock test
  - why shared: keeps path-boundary enforcement single-sourced
- `render_tool_arguments_json`
  - owner: `crates/freehand-blocks/src/lib.rs`
  - purpose: render structured tool arguments without duplicating JSON conversion in runtime/tool owner code
  - allowed callers: provider adapters, tool registry diagnostics, tests
  - related tests: tool argument JSON render tests, tool registry execution tests
  - why shared: keeps tool argument conversion as a shared block instead of per-crate helpers

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `BuiltinToolRegistry::reasonix_aligned` | `crates/freehand-tools/src/lib.rs` | create per-run built-in registry aligned with Reasonix names and schemas | none | registry | runtime live bridge/tests | tool owner | bound |
| 02 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | declare built-in tool metadata, schema, read-only state, and implementation state | static registry truth | tool specs | registry constructor/tests | tool owner | bound |
| 03 | `BuiltinToolRegistry::implemented_definitions` | `crates/freehand-tools/src/lib.rs` | export currently executable provider-neutral tool schemas | registry | provider tool definitions | runtime live bridge | tool owner | bound |
| 04 | `BuiltinToolRegistry::execute` | `crates/freehand-tools/src/lib.rs` | dispatch completed tool calls into the single owner implementation set | ReasonReq04ToolCall | tool execution output | runtime live bridge | tool owner | bound |
| 05 | `execute_read_file` | `crates/freehand-tools/src/lib.rs` | read UTF-8 text from one locked in-root file with line-windowing | path plus optional offset plus optional limit | numbered text window | registry execute | read-only file tool owner | bound |
| 06 | `execute_glob` | `crates/freehand-tools/src/lib.rs` | match in-root files by glob pattern with recursive filename fallback | pattern | newline-separated match list | registry execute | read-only search tool owner | bound |
| 07 | `execute_grep` | `crates/freehand-tools/src/lib.rs` | search in-root UTF-8 text files by regex | pattern plus optional path | path:line:text matches | registry execute | read-only search tool owner | bound |
| 08 | `execute_ls` | `crates/freehand-tools/src/lib.rs` | list directory entries or recursive tree under locked root | optional path plus optional recursive | newline-separated directory listing | registry execute | read-only file tool owner | bound |

## Sync Status Against Mainline Call

- Reasonix-aligned built-in names and schemas are bound in `freehand-tools`
- current implemented tool set is: `read_file`, `glob`, `grep`, `ls`, `todo_write`, `complete_step`
- first-version path tools are locked to the canonical process current working directory and reject path escape outside that root
- write/edit/bash/web/notebook tools remain registered but explicitly unimplemented until their lifecycle/gates are designed
