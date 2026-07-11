# Test Design: `tool.registry`

- feature_id: `tool.registry`
- owner: `crates/freehand-tools`
- lifecycle path under test:
  - registry is created per run
  - Reasonix-aligned tool names and schemas are exported in stable registry order
  - generic implemented tools execute against the explicit per-call workspace root and return explicit result text
  - the master-safe export is a stable subset that excludes unrestricted shell tools
  - unimplemented registered tools fail explicitly
  - unknown tools fail explicitly
- white-box plan:
  - registry name/schema export tests
  - implemented schema fingerprint stability tests
  - implemented schema fingerprint change detection tests
  - master-safe tool export excludes unsandboxed shell execution while retaining
    task and workspace-scoped file/search tools
  - tool execution scope classification distinguishes framework, workspace, and
    unrestricted process tools in the registry owner
  - read-only path tools may inspect readable external absolute or parent paths
  - writable path escape returns a typed workspace-boundary error instead of
    an unstructured invalid-argument string
- `read_only` metadata tests
- task-management semantic action names are not exposed as standalone tools
- `task` remains the task-management tool surface and requires typed `op`
- `bash` success-path, workspace-cwd, explicit workspace root, timeout, and non-zero-exit tests
- live runtime checkpoint routing must not treat non-file-mutation tools such as `bash` as preview/checkpointable file mutations
  - `read_file` line-window and external-read tests
  - `write_file` create/overwrite/path-lock tests
  - `edit_file` exact-single-match and rejection tests
  - `multi_edit` ordered apply and replace-all tests
  - `glob` recursive and simple-filename pattern tests
  - `grep` recursive match and external-read tests
  - `ls` flat, recursive listing, and external-read tests
  - `todo_write` argument validation and success tests
  - `complete_step` argument validation and success tests
  - unknown/unimplemented tool error tests
  - task tool op-dispatch surface tests
- module black-box plan:
  - runtime live bridge can advertise implemented tool definitions without hardcoded demo tools
  - runtime live bridge can execute a real implemented read-only registry tool inside the master runtime home and re-enter the result
  - runtime live bridge returns a paired failed result for an external requested session cwd without exposing external file content
- project black-box impact:
  - master live turns cannot receive or execute unsandboxed `bash`; cross-workspace
    work must enter through `task` and a worker
  - master and worker read-only path tools may inspect readable external paths,
    but file-mutation tools remain locked to the current agent cwd
  - generic registry coverage retains foreground `bash`, but the master live tool
    surface neither advertises nor executes it
  - provider live turn tool loop no longer depends on `echo_json` or forced `todo_write`
  - daemon and runtime smokes prove `read_file` can run through the registry-owned live path only inside master runtime home
  - writable file-mutation tools now still enter the live path only through the registry owner instead of runtime orchestration
  - future bash/web/notebook tools still have one owner and cannot be implemented in runtime orchestration
  - future task-management actions must enter through the small owner-scoped tool/op surface, not new standalone semantic-action tool names
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with tool owner code and function map updates
- fixtures / replay inputs / runtime evidence paths:
  - provider mock tool-use fixtures
  - `~/.freehand/ledgers/reason`
- known gaps:
  - `bg_jobs`, `kill_shell`, `wait_job`, web, notebook, and symbol-aware mutation tools are still intentionally unimplemented until dedicated lifecycle and permission gates are locked
- sync status between design and implementation:
  - registry-backed foreground `bash`, read-only file/search with external-read support, and first text-mutation tools are landed
  - explicit per-call workspace root support is landed through `with_workspace_root`
  - generic and master-safe implemented tool schema exports and fingerprints are landed for their respective consumers
  - writable tool live exposure is routed through the code-bound `tool.preview` plus `runtime.checkpoint-rewind` owner paths instead of runtime-local mutation shortcuts
  - runtime and daemon smokes now consume real registry tools instead of a forced demo first tool
