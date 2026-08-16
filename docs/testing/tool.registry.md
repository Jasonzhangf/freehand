# Test Design: `tool.registry`

- feature_id: `tool.registry`
- owner: `crates/freehand-tools`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `tool_call.execute_workspace_path`
  - `tool_call.execute_external_http`
  - `tool_call.project_registry_to_ui`
  - `tool_call.discover_social_candidate`
  - `tool_call.verify_search_url`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `tool_call.execute_workspace_path` | bound | `cargo test -p freehand-tools -- --nocapture` covers built-in schema, locked workspace path, absolute/symlink path, external absolute rejection, read/search/write, preview, and tool display tests | `cargo test -p freehand-tools -- --nocapture` covers registry execution smokes for read_file/glob/grep/ls/write_file/edit_file/multi_edit and failure guidance | `cargo test -p freehand-runtime live_bridge -- --nocapture` covers runtime live tool-loop smokes and Worker online evidence that tool calls execute only through the locked workspace owner |
| `tool_call.execute_external_http` | bound | `cargo test -p freehand-tools web_fetch_executes_against_local_http_url -- --nocapture` covers bounded HTTP fetch execution and argument validation tests cover invalid URL/limit/timeout handling | `cargo test -p freehand-tools web_fetch_executes_against_local_http_url -- --nocapture` covers registry execution through the owner `BuiltinToolRegistry::execute` path | `cargo test -p freehand-runtime live_bridge_admits_long_operator_task_without_semantic_truncation -- --nocapture --test-threads=1` proves Master/Worker provider request context advertises `web_fetch`; online proof must inspect provider request/tool-call evidence before claiming model behavior |
| `tool_call.project_registry_to_ui` | bound | `cargo test -p freehand-tools registry_projection -- --nocapture` covers UI-safe registry projection, Master/Worker exposure, scope, examples, path guidance, and no local `web_search` row | `cargo test -p freehand-runtime tool_registry -- --nocapture` covers runtime `QueryToolRegistry` bridge from owner projection into UI DTOs | `node scripts/verify-webui-tools-registry-online.mjs` proves browser Tools dashboard renders ADP owner projection, not local tool truth, and does not create top-level sessions |
| `tool_call.discover_social_candidate` | pending | `cargo test -p freehand-tools camo -- --nocapture` covers typed camo social discovery producing `SearchDiscoveryCandidate` and `SearchDiscoveryDelivery`; unsupported Weibo/X platform is explicit failure | `cargo test -p freehand-reason non_sourced` covers non-sourced turn accepting typed hosted and social discovery observations | `node scripts/verify-provider-hosted-web-search-online.mjs` proves S-profile hosted and social search output projects into typed discovery evidence |
| `tool_call.verify_search_url` | pending | `cargo test -p freehand-tools camo -- --nocapture` covers typed camo URL verification producing `SearchVerificationDelivery` with access status, timestamp, page title, and access attempt truth; non-camo verified sources are rejected at final delivery | `cargo test -p freehand-reason search_evidence -- --nocapture` covers sourced-search final gate rejecting unverified, failed, and non-camo sources | `node scripts/verify-provider-hosted-web-search-online.mjs` proves S-profile camo URL verification produces typed access status with attempt truth |

- lifecycle path under test:
  - registry is created per run
  - Reasonix-aligned tool names and schemas are exported in stable registry order
  - generic implemented tools execute against the explicit per-call workspace root and return explicit result text
  - the master-safe export is a stable local-workspace-plus-framework subset
    containing locked workspace file/search/write/edit tools, concrete-URL
    `web_fetch`, plus `task` and `timer`
  - unimplemented registered tools fail explicitly
  - unknown tools fail explicitly
  - UI-safe registry projection exposes schema/examples/guidance/exposure only
    and never executes tools from the Tools dashboard
- white-box plan:
  - registry name/schema export tests
  - implemented schema fingerprint stability tests
  - implemented schema fingerprint change detection tests
  - master-safe tool export includes locked local workspace tools (`ls`,
    `read_file`, `grep`, `glob`, `write_file`, `edit_file`, `multi_edit`,
    `delete_range`) plus `web_fetch`, `task`, and `timer`, and excludes shell,
    browser, broad `web_search`, `todo_write`, and `complete_step`
  - worker-safe tool export is exact and excludes shell/task/timer,
    unimplemented names, and observed bad invented names such as `shell`,
    `readlink`, `pwd`, `cat`, and `find`
  - tool execution scope classification distinguishes framework, workspace, and
    unrestricted process tools in the registry owner
  - registry projection coverage proves `task` and `timer` are Master-only
    framework tools, `web_fetch` is Master+Worker network scope, `bash` is
    implemented but not live-exposed, Worker-only progress tools are hidden
    from Master, and path guidance includes locked workspace, absolute,
    leading-`~`, and symlink rules
  - read-only path tools reject existing external absolute paths after
    canonical/symlink resolution instead of treating external reads as valid
    workspace truth
  - `glob` schema export locks the workspace-scoped contract, prefers relative
  patterns, expands leading `~`, allows absolute patterns only inside the
  locked workspace, and explicitly rejects `..` and external absolute patterns
  so the model should not discover external repos through failing glob calls
- path tools accept absolute symlink aliases that canonicalize back into the
  locked workspace; `glob`, `grep`, `read_file`, `ls`, and `write_file` must not
  false-deny user-facing symlink paths inside the task cwd
- path tool failures report owner-rendered `path_diagnostic` truth for relative
  path absolute-normalization and absolute symlink-parent missing-leaf cases
- `read_file` schema export tells the model to use `ls` first when a target may
  be a directory and not to read guessed files, binary sidecars, or
  not-yet-created output directories/files
- `ls` schema export says it can list directories or report one file entry, so
  file-existence checks do not need exact-file glob trial calls; it also tells
  the model not to keep listing guessed missing output directories
  - `web_fetch` schema and execution tests cover concrete `http://`/`https://`
    URL fetching with timeout/byte limits and explicit failure for non-HTTP
    schemes; this is not broad search/browser capability
  - writable path escape returns a typed workspace-boundary error instead of
    an unstructured invalid-argument string
- `read_only` metadata tests
- task-management semantic action names are not exposed as standalone tools
- `task` remains the task-management tool surface and requires typed `op`
- `task` schema export locks TaskSpaceSnapshot-first guidance, omit-status for
  all visible tasks, legal `list_tasks` status values, `interrupted`
  `record_execution` state, and no `auto`/`self` production dispatch
- `task` schema export locks that every task call must include top-level `op`,
  shows valid create/assign examples, prefers expanded absolute existing
  repository/workspace `target_cwd`, accepts leading-~/symlink aliases only
  when they resolve to an existing workspace, and rejects glob, broad-search,
  or output-directory targets
- `timer` is exposed as a separate standard internal framework tool, not as
  task input `{"op":"wait"}` or task note text
- `bash` success-path, workspace-cwd, explicit workspace root, timeout, and non-zero-exit tests
- live runtime checkpoint routing must not treat non-file-mutation tools such as `bash` as preview/checkpointable file mutations
  - `read_file` line-window and external absolute rejection tests
  - `write_file` create/overwrite/path-lock tests
  - `edit_file` exact-single-match and rejection tests
  - `multi_edit` ordered apply and replace-all tests
  - `glob` recursive and simple-filename pattern tests
  - `glob` in-workspace absolute pattern positive test
  - path-tool symlink-alias positive test for `glob`, `grep`, `read_file`,
    `ls`, and `write_file`
  - `glob` schema prompt-guard test for leading-`~`, parent traversal, and
    external absolute patterns
  - `read_file`/`ls` schema prompt-guard test for directory-as-file, guessed
    generated reports, binary sidecars, and repeated listing of missing output
    directories
  - path diagnostic tests for relative path absolute-normalization and absolute
    symlink-parent missing-leaf evidence
  - `grep` recursive match and external absolute rejection tests
  - `ls` flat, recursive listing, and external absolute rejection tests
  - `ls` file-entry test for existence checks on known output paths
  - `todo_write` argument validation and success tests
  - `complete_step` argument validation and success tests
  - `timer` schema export covers relative, absolute, local-time recurring,
    local-time cron, max-runs, examples, and weekday/skip-weekend fields
  - `timer` schema export includes the default >3-minute wait rule, the
    no-dead-wait instruction, the continue-other-ready-work instruction, and
    prompt-duty text for revisiting waited truth
  - `timer` schema export includes the no-oral-schedule rule: completion text
    cannot claim a timer exists unless the `timer` tool returned
    `Timer scheduled` in the same turn
  - unknown/unimplemented tool error tests
  - task tool op-dispatch surface tests
  - task tool schema prompt-guard test for TaskSpaceSnapshot/current-truth
    usage, no `status="all"`, required top-level `op`, absolute-or-resolving
    symlink `target_cwd`, dispatch mode, and worker execution states
- module black-box plan:
  - runtime live bridge can advertise implemented tool definitions without hardcoded demo tools
  - runtime live bridge can execute a real implemented read-only registry tool inside the master runtime home and re-enter the result
  - runtime live bridge returns a paired failed result for an external requested session cwd without exposing external file content
- project black-box impact:
  - master live turns can receive and execute locked local workspace
    file/search/write/edit tools in the current selected session cwd,
    concrete-URL `web_fetch`, plus `task` and `timer`; unsandboxed `bash`,
    browser, broad `web_search`, `todo_write`, and `complete_step` remain
    unavailable; different-cwd, isolated, concurrent, long-running, or
    resumable work must enter through `task` and a Worker
  - worker read-only path tools and file-mutation tools remain locked to the
    current agent cwd after canonical/symlink resolution; for Worker provider
    turns that current agent cwd is the locked task cwd
  - `read_file`, `grep`, `ls`, and `glob` remain locked-workspace scoped while
    accepting absolute or leading-`~` aliases only when they canonicalize back
    under the locked workspace
  - `node scripts/verify-webui-path-diagnostic-online.mjs` proves the
    symlink/missing-leaf path diagnostic through the WebUI surface: fixed
    parent session DOM submit, Master waiting dispatch, Worker `ls` tool call,
    second Worker provider request with diagnostic `tool_result`, TaskBoard
    lifecycle to `TaskBlocked`, and WebUI Worker transcript rendering of the
    blocked diagnostic
  - generic registry coverage retains foreground `bash`, but the master live tool
    surface neither advertises nor executes it
  - provider live turn tool loop no longer depends on `echo_json` or forced `todo_write`
  - daemon and runtime smokes prove injected Master `read_file` returns a
    failed capability-boundary tool result with no file-content leak, while
    Worker `read_file` still runs through the registry-owned live path
  - S-profile timer online verifier proves a provider-visible Master tool call
    can call `timer`, receives the scheduled tool result in the next provider
    request, then fires the due timer into a new internal Master turn and
    persists `TimerScheduled`, `TimerFired`, and `TimerCompleted` ledger truth
  - the same verifier with `FREEHAND_TIMER_VERIFY_MODE=restart-due` proves a
    persisted active timer that expires across service-scoped `restartS` is
    claimed after restart and completed from durable timer truth
  - writable file-mutation tools now still enter the live path only through the registry owner instead of runtime orchestration
  - future bash/broad-search/browser/notebook tools still have one owner and cannot be implemented in runtime orchestration
  - WebUI Tools dashboard online proof must compare DOM cards against
    `QueryToolRegistry` ADP projection, prove no local `web_search` function
    row, and prove no top-level persisted session ids change
  - future task-management actions must enter through the small owner-scoped
    tool/op surface, while standard non-task internal actions such as timers
    use their own owner-approved internal tool surface
- mainline/wiki sync:
  - wiki generated from mainline call must stay in sync with tool owner code and function map updates
- fixtures / replay inputs / runtime evidence paths:
  - provider mock tool-use fixtures
  - `~/.freehand/ledgers/reason`
- known gaps:
  - `web_fetch` is implemented as a bounded concrete-URL network tool; `bg_jobs`,
    `kill_shell`, `wait_job`, broad `web_search`, browser, notebook, and
    symbol-aware mutation tools are still intentionally unimplemented until
    dedicated lifecycle and permission gates are locked
- sync status between design and implementation:
  - registry-backed foreground `bash`, workspace-locked read-only file/search, and first text-mutation tools are landed
  - explicit per-call workspace root support is landed through `with_workspace_root`
  - generic, worker-safe, and local-workspace-plus-framework master-safe
    implemented tool schema exports and fingerprints are landed for their
    respective consumers; worker surface tests lock the exact Worker list so
    runtime guidance does not invent `shell`/`readlink`/`pwd`/`cat`/`find`
  - writable tool live exposure is routed through the code-bound `tool.preview` plus `runtime.checkpoint-rewind` owner paths instead of runtime-local mutation shortcuts
  - runtime and daemon smokes now consume real registry tools instead of a forced demo first tool
