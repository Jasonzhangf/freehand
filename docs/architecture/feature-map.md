# Feature Map

This file is the registry for feature ownership and verification.

Use order:

1. identify target `feature_id`
2. confirm single `owner`
3. obey `allowed_paths` and `forbidden_paths`
4. use `debug_artifacts` as debug entry
5. use `runtime_paths` as scene evidence entry
6. run `required_checks`
7. if truth changed, update this file in same task

## Required Fields

- `feature_id`
- `owner`
- `allowed_paths`
- `forbidden_paths`
- `required_checks`
- `required_white_box_tests`
- `required_module_black_box_tests`
- `required_project_black_box_tests`
- `test_design_doc`
- `function_map_doc`
- `mainline_call_doc` for migrated features
- `generated_wiki_doc` for migrated features
- `debug_artifacts`
- `runtime_paths`
- `update_triggers`
- `lifecycle_checks`

## Owner Routing Index

Use this table before grep or implementation. Every bug or feature request must first map to one `feature_id`, then follow that feature's `function_map_doc` and `test_design_doc`.

| problem area | feature_id | owner module / crate | function map | test orchestration |
| --- | --- | --- | --- | --- |
| workspace gates, CI/CD, repo rules | `foundation.workspace` | `xtask`, workspace root | `docs/function-maps/foundation.workspace.md` | `docs/testing/foundation.workspace.md` |
| config load, agents, providers, remote daemon registry, startup selection | `config.core` | `crates/freehand-config` | `docs/function-maps/config.core.md` | `docs/testing/config.core.md` |
| shared IDs, cross-module contracts, request/response/error contracts | `contracts.core` | `crates/freehand-contracts` | `docs/function-maps/contracts.core.md` | `docs/testing/contracts.core.md` |
| provider-neutral semantics and recovery policy | `provider.semantic` | `crates/freehand-provider-core` | `docs/function-maps/provider.semantic.md` | `docs/testing/provider.semantic.md` |
| OpenAI-compatible wire rendering/parsing | `provider.openai-adapter` | `crates/freehand-provider-openai` | `docs/function-maps/provider.openai-adapter.md` | `docs/testing/provider.openai-adapter.md` |
| Anthropic Messages wire rendering/parsing/executor | `provider.anthropic-adapter` | `crates/freehand-provider-anthropic` | `docs/function-maps/provider.anthropic-adapter.md` | `docs/testing/provider.anthropic-adapter.md` |
| provider-selected live bridge into runtime-owned live reason turn | `provider.reason-live-bridge` | `crates/freehand-runtime` | `docs/function-maps/provider.reason-live-bridge.md` | `docs/testing/provider.reason-live-bridge.md` |
| built-in tool registry, Reasonix-aligned tool schemas, tool execution ownership | `tool.registry` | `crates/freehand-tools` | `docs/function-maps/tool.registry.md` | `docs/testing/tool.registry.md` |
| writable-tool preview truth and preview/execute parity | `tool.preview` | `crates/freehand-tools` | `docs/function-maps/tool.preview.md` | `docs/testing/tool.preview.md` |
| UI-visible tool semantic classification and display parsing | `tool.display` | `crates/freehand-blocks` | `docs/function-maps/tool.display.md` | `docs/testing/tool.display.md` |
| turn truth, provider-output application, terminal schema | `reason.turn` | `crates/freehand-reason` | `docs/function-maps/reason.turn.md` | `docs/testing/reason.turn.md` |
| session-history rewrite state and rewrite gates | `reason.session-history` | `crates/freehand-reason` | `docs/function-maps/reason.session-history.md` | `docs/testing/reason.session-history.md` |
| reason persistence, ledgers, restore, derived sidecars | `reason.persistence` | `crates/freehand-reason` | `docs/function-maps/reason.persistence.md` | `docs/testing/reason.persistence.md` |
| context planning, cache shape, segment admission | `reason.context-planner` | `crates/freehand-blocks` | `docs/function-maps/reason.context-planner.md` | `docs/testing/reason.context-planner.md` |
| compaction/rewrite/recovery trigger policy | `reason.rewrite-policy` | `crates/freehand-blocks` | `docs/function-maps/reason.rewrite-policy.md` | `docs/testing/reason.rewrite-policy.md` |
| independent debug/trace contracts, snapshots, hub/sinks | `debug.core` | `crates/freehand-debug` | `docs/function-maps/debug.core.md` | `docs/testing/debug.core.md` |
| AGENTS.md and skills capability discovery, validation, and deterministic manifest indexing | `instruction.capability-loader` | `crates/freehand-instructions` | `docs/function-maps/instruction.capability-loader.md` | `docs/testing/instruction.capability-loader.md` |
| internal control metadata center, writer ownership, write-node provenance, metadata/request isolation | `metadata.core` | `crates/freehand-metadata` | `docs/function-maps/metadata.core.md` | `docs/testing/metadata.core.md` |
| passive framework control status parsing, fixed control hooks, and rhythm decisions | `control.center` | `crates/freehand-control` | `docs/function-maps/control.center.md` | `docs/testing/control.center.md` |
| centralized framework error classification, recovery decisions, and error watermark metadata | `error.center` | `crates/freehand-control` | `docs/function-maps/error.center.md` | `docs/testing/error.center.md` |
| task lifecycle, task persistence, task runtime recovery, and agent registry skeleton | `task.orchestration` | `crates/freehand-task` | `docs/function-maps/task.orchestration.md` | `docs/testing/task.orchestration.md` |
| per-agent lifecycle state, AgentBoard projection, and lifecycle event reduction | `agent.lifecycle` | `crates/freehand-task` initially, split later if needed | `docs/function-maps/agent.lifecycle.md` | `docs/testing/agent.lifecycle.md` |
| safe-point runtime control for already-running worker executions | `worker.control` | `crates/freehand-task` initially, split later if needed | `docs/function-maps/worker.control.md` | `docs/testing/worker.control.md` |
| master/slave pairing, node status, delegation, slave turn publication | `node.master-slave` | `crates/freehand-node` | `docs/function-maps/node.master-slave.md` | `docs/testing/node.master-slave.md` |
| UI commands, query/subscribe, UI projections | `ui.protocol` | `crates/freehand-ui-protocol` | `docs/function-maps/ui.protocol.md` | `docs/testing/ui.protocol.md` |
| runtime wiring for UI command dispatch into owner modules | `runtime.ui-command-dispatch` | `crates/freehand-runtime` | `docs/function-maps/runtime.ui-command-dispatch.md` | `docs/testing/runtime.ui-command-dispatch.md` |
| production Master lifecycle review/recovery loop and slave Worker claim/execute/report loop | `runtime.master-worker-loop` | `crates/freehand-runtime` | `docs/function-maps/runtime.master-worker-loop.md` | `docs/testing/runtime.master-worker-loop.md` |
| writable-tool checkpoints, restore manifests, and runtime rewind | `runtime.checkpoint-rewind` | `crates/freehand-runtime` | `docs/function-maps/runtime.checkpoint-rewind.md` | `docs/testing/runtime.checkpoint-rewind.md` |
| CLI reason smoke and config-selected runtime harness | `app.cli-runtime-smoke` | `apps/freehand-cli` | `docs/function-maps/app.cli-runtime-smoke.md` | `docs/testing/app.cli-runtime-smoke.md` |
| CLI live provider turn and completion loop smoke | `app.cli-live-turn` | `apps/freehand-cli` | `docs/function-maps/app.cli-live-turn.md` | `docs/testing/app.cli-live-turn.md` |
| WebUI/protocol-only app boundary smoke | `app.webui-smoke` | `apps/freehand-server` | `docs/function-maps/app.webui-smoke.md` | `docs/testing/app.webui-smoke.md` |
| account authentication, Agent presence, remote HTTP/ADP proxy, and Relay deployment | `relay.transport` | `crates/freehand-relay`, `apps/freehand-relay-server` | `docs/function-maps/relay.transport.md` | `docs/testing/relay.transport.md` |
| runtime-backed HTTP/SSE UI daemon host | `app.runtime-daemon` | `apps/freehand-daemon` | `docs/function-maps/app.runtime-daemon.md` | `docs/testing/app.runtime-daemon.md` |
| ACP v1 agent transport and session bridge | `app.acp-server` | `crates/freehand-acp` | `docs/function-maps/app.acp-server.md` | `docs/testing/app.acp-server.md` |
| Android/protocol-only app boundary client | `app.android-client` | `apps/freehand-android` | `docs/function-maps/app.android-client.md` | `docs/testing/app.android-client.md` |

If a problem does not fit this table, update this routing index before making code changes. Do not create a second owner by patching an adjacent module.

## Resource Ownership Index

This table is the feature-map backlink for `docs/resource-maps/core.json`. The resource map remains the first truth for identity, truth store, operations, projections, relations, and gates; this table makes feature ownership discoverable from the feature map before opening feature-local docs.

| feature_id | owned resources | resource map |
| --- | --- | --- |
| `foundation.workspace` | `workspace_gate_policy` | `docs/resource-maps/core.json` |
| `config.core` | `config`, `remote_daemon_registry` | `docs/resource-maps/core.json` |
| `config.account-config-sync` | `account_config_document` | `docs/resource-maps/core.json` |
| `reason.persistence` | `session` | `docs/resource-maps/core.json` |
| `reason.turn` | `turn`, `search_evidence` | `docs/resource-maps/core.json` |
| `reason.context-planner` | `request_context` | `docs/resource-maps/core.json` |
| `provider.reason-live-bridge` | `provider_request` | `docs/resource-maps/core.json` |
| `provider.semantic` | `provider_response`, `provider_hosted_search` | `docs/resource-maps/core.json` |
| `tool.registry` | `tool_call`, `workspace_path`, `external_http_resource` | `docs/resource-maps/core.json` |
| `task.orchestration` | `task` | `docs/resource-maps/core.json` |
| `agent.lifecycle` | `agent` | `docs/resource-maps/core.json` |
| `runtime.master-worker-loop` | `timer`, `master_work` | `docs/resource-maps/core.json` |
| `error.center` | `error` | `docs/resource-maps/core.json` |
| `metadata.core` | `metadata` | `docs/resource-maps/core.json` |
| `debug.core` | `debug_trace` | `docs/resource-maps/core.json` |
| `ui.protocol` | `ui_projection`, `input_attachment` | `docs/resource-maps/core.json` |
| `app.webui-smoke` | `ui_surface` | `docs/resource-maps/core.json` |
| `runtime.ui-command-dispatch` | `runtime_command`, `runtime_agent_activity` | `docs/resource-maps/core.json` |
| `app.runtime-daemon` | `runtime_daemon_host` | `docs/resource-maps/core.json` |
| `app.acp-server` | `acp_transport` | `docs/resource-maps/core.json` |
| `runtime.checkpoint-rewind` | `checkpoint` | `docs/resource-maps/core.json` |
| `node.master-slave` | `node_pairing`, `remote_daemon_directory` | `docs/resource-maps/core.json` |
| `relay.transport` | `relay_account`, `agent_presence`, `relay_control_tunnel`, `relay_data_tunnel`, `relay_error_tunnel`, `relay_update_artifact` | `docs/resource-maps/core.json` |
| `app.android-client` | `android_connection_config`, `android_apk_update`, `android_file_access`, `android_notification` | `docs/resource-maps/core.json` |
| `instruction.capability-loader` | `instruction_capability` | `docs/resource-maps/core.json` |

## Architecture Gap Registry

Non-violation pending items live in `docs/architecture/architecture-gaps.md`. Each gap has explicit `feature_id`, owner, risk, and closure path. Gaps are not regressions; they document known-incomplete scope without gate violation.

## Seed Entries

### `foundation.workspace`

- owner: `xtask`, workspace root
- allowed_paths: `.ignore`, `Cargo.toml`, `Makefile`, `.github/workflows/**`, `.githooks/**`, `.agents/skills/freehand-dev/**`, `scripts/**`, `xtask/**`, `docs/architecture/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`, `docs/goals/**`, `docs/loops/**`, `docs/release.md`, `CACHE.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider and reason implementation crates unless scaffold-related
- module_registry: `docs/module-registry/foundation.workspace.json`
- verification_map: `docs/verification-maps/foundation.workspace.json`
- required_checks:
  - `cargo test --workspace`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - xtask gate rule tests
  - xtask mainline render/generation tests
  - xtask mainline manifest cross-link tests
  - xtask mainline call-table binding tests
  - xtask CI/CD command-alignment tests
  - xtask source-search boundary tests
  - xtask metadata/request leak-gate tests
- required_module_black_box_tests:
  - xtask gate smoke
  - xtask mainlines check smoke
- required_project_black_box_tests:
  - workspace harness smoke
- test_design_doc: `docs/testing/foundation.workspace.md`
- function_map_doc: `docs/function-maps/foundation.workspace.md`
- mainline_call_doc: `docs/mainline-calls/foundation.workspace.json`
- generated_wiki_doc: `docs/wiki/foundation.workspace.md`
- resource_map_doc: `docs/resource-maps/core.json`
- debug_artifacts:
  - none
- runtime_paths:
  - `~/.freehand/logs`
- update_triggers:
  - workspace member changes
  - gate policy changes
  - repo workflow changes
  - CI/CD full-gate alignment changes
  - source-only search boundary changes
  - release or global-install script changes
  - launchd service install/uninstall script changes
  - mainline generation shape changes
  - generated wiki freshness policy changes
  - mainline manifest cross-link policy changes
  - mainline call-table binding policy changes
  - metadata/request leak-gate policy changes
  - loop governance docs or report-only loop policy changes
- lifecycle_checks:
  - information sufficient
  - logic closed-loop
  - lifecycle management complete

### `control.center`

- owner: `crates/freehand-control`
- allowed_paths: `crates/freehand-control/**`, `crates/freehand-runtime/**`, `crates/freehand-ui-protocol/**`, `docs/design/**`, `docs/function-maps/control.center.md`, `docs/testing/control.center.md`, `docs/architecture/feature-map.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, task execution state, node pairing transport, UI app-local semantic parsers
- required_checks:
  - `cargo test -p freehand-control`
  - `cargo test -p freehand-runtime live_bridge_accepts_simple_status_stop_hook_without_completion_schema -- --nocapture`
  - `cargo test -p freehand-ui-protocol public_conversation_strips_hidden_control_status_blocks -- --nocapture`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - status parser accepts valid simple stop
  - status parser rejects missing required fields
  - status parser emits next-step rhythm decision without side effects
- required_module_black_box_tests:
  - runtime mock provider status stopHook writes control metadata
  - UI protocol strips hidden status blocks from public projection
- required_project_black_box_tests:
  - none for this skeleton; online WebUI required before task lifecycle UI claims
- test_design_doc: `docs/testing/control.center.md`
- function_map_doc: `docs/function-maps/control.center.md`
- debug_artifacts:
  - control hook metadata ledger rows
- runtime_paths:
  - `~/.freehand/ledgers/metadata`
- update_triggers:
  - status schema fields change
  - hook point semantics change
  - rhythm decision policy changes
  - hidden status projection filters change
  - task action tool admission changes
- lifecycle_checks:
  - status schema remains no-side-effect
  - every hook write carries writer owner and node provenance
  - public projection strips hidden control blocks
  - task mutations remain action-tool owned, not status-owned

### `error.center`

- owner: `crates/freehand-control`
- allowed_paths: `crates/freehand-control/**`, `crates/freehand-runtime/**`, `crates/freehand-ui-protocol/**`, `apps/freehand-server/**`, `apps/freehand-daemon/**`, `apps/freehand-cli/**`, `docs/design/**`, `docs/function-maps/error.center.md`, `docs/testing/error.center.md`, `docs/mainline-calls/error.center.json`, `docs/wiki/error.center.md`, `docs/architecture/feature-map.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, UI app-local error policy, task state mutation without accepted action metadata
- required_checks:
  - `cargo test -p freehand-control`
  - `cargo test -p freehand-runtime live_bridge_records_error_center_metadata_for_schema_repair -- --nocapture`
  - `cargo test -p freehand-runtime live_bridge_retries_recoverable_provider_errors_then_succeeds -- --nocapture`
  - `cargo test -p freehand-runtime live_bridge_fails_after_ten_provider_retries_with_error_code -- --nocapture`
  - `cargo test -p freehand-runtime live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing -- --nocapture`
  - `cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture`
  - `cargo test -p freehand-runtime runtime_query_reads_error_center_metadata_without_raw_text -- --nocapture`
  - `cargo test -p freehand-daemon daemon_adp_queries_runtime_error_center_truth -- --nocapture`
  - `cargo test -p freehand-cli -- --nocapture`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - schema mismatch errors classify as schema/validation/repair_schema until retry cap; repair_schema means model response polishing
  - retry cap changes schema recovery action to stop_turn
  - provider executor failures classify as provider/recoverable/retry_same_step before retry cap and provider/recoverable/fail_turn at retry cap
  - tool failures classify as tool/validation/repair_schema
  - error-center metadata query projection filters by trace/turn/domain and omits raw text
- required_module_black_box_tests:
  - runtime writes `error.center` metadata for completion schema rejection
  - runtime writes `error.center` metadata for failed tool result before provider re-entry
  - runtime writes `error.center` metadata for provider executor retry attempts and final retry-exhausted failure before materializing terminal failure
  - metadata write failure blocks the originating decision
  - daemon ADP query returns runtime-backed error-center metadata projection
- required_project_black_box_tests:
  - S-profile daemon ADP `adp-error-query` against a real error-center metadata row
- test_design_doc: `docs/testing/error.center.md`
- function_map_doc: `docs/function-maps/error.center.md`
- mainline_call_doc: `docs/mainline-calls/error.center.json`
- generated_wiki_doc: `docs/wiki/error.center.md`
- debug_artifacts:
  - error center metadata ledger rows
- runtime_paths:
  - `~/.freehand/ledgers/metadata`
- update_triggers:
  - error domain/class/recovery policy changes
  - schema polishing rhythm changes
  - provider/tool/task/node failure routing changes
  - error metadata watermark fields change
- lifecycle_checks:
  - every classified decision carries `error.center` writer owner and write-node provenance
  - runtime does not convert provider/tool/schema failures to owner state changes before error-center metadata admission
  - public/request payload text is hashed only, not written into error metadata entries
  - ADP projection remains read-only and does not repair malformed metadata into invented error semantics

### `instruction.capability-loader`

- owner: `crates/freehand-instructions`
- allowed_paths: `crates/freehand-instructions/**`, `Cargo.toml`, `xtask/**`, `docs/design/instruction-capability-loader-design.md`, `docs/design/design-doc-index.md`, `docs/function-maps/instruction.capability-loader.md`, `docs/testing/instruction.capability-loader.md`, `docs/mainline-calls/instruction.capability-loader.json`, `docs/wiki/instruction.capability-loader.md`, `docs/architecture/feature-map.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, UI app-local prompt assembly, runtime loose directory scanning, direct provider payload mutation
- required_checks:
  - `cargo test -p freehand-instructions`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - global `~/.freehand/AGENTS.md` index entry
  - local `AGENTS.md` entries from project root to cwd
  - global `~/.freehand/skills/**/SKILL.md` skill entries
  - local `.agents/skills/**/SKILL.md` entries from project root to cwd
  - invalid skill frontmatter records an explicit manifest error without dropping valid entries
  - manifest fingerprint stays deterministic for stable inputs
- required_module_black_box_tests:
  - compile manifest from a fixture tree and write it under `~/.freehand/state/instructions/capability-manifest.json`
  - compiled manifest contains source paths, scope, precedence, content size, and content hash, not provider payloads or secrets
- required_project_black_box_tests:
  - pending until runtime/context planner consumes the compiled manifest; current slice is index-only
- test_design_doc: `docs/testing/instruction.capability-loader.md`
- function_map_doc: `docs/function-maps/instruction.capability-loader.md`
- mainline_call_doc: `docs/mainline-calls/instruction.capability-loader.json`
- generated_wiki_doc: `docs/wiki/instruction.capability-loader.md`
- debug_artifacts:
  - instruction capability manifest
  - instruction capability compile errors
- runtime_paths:
  - `~/.freehand/AGENTS.md`
  - `~/.freehand/skills`
  - `<project>/AGENTS.md`
  - `<project>/.agents/skills`
  - `~/.freehand/state/instructions/capability-manifest.json`
- update_triggers:
  - AGENTS.md discovery order changes
  - skill root discovery order changes
  - skill frontmatter schema changes
  - manifest schema or fingerprint changes
  - context-planner consumption of instruction capabilities
- lifecycle_checks:
  - authoring directories are discovery inputs only
  - runtime consumers must consume a compiled/validated manifest
  - invalid capabilities surface as manifest errors, not silent fallback
  - manifest remains deterministic and bounded

### `task.orchestration`

- owner: `crates/freehand-task`
- allowed_paths: `crates/freehand-task/**`, `crates/freehand-tools/**`, `crates/freehand-runtime/**`, `docs/design/**`, `docs/function-maps/task.orchestration.md`, `docs/testing/task.orchestration.md`, `docs/architecture/feature-map.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, UI app-local task state, node pairing transport internals except future dispatch adapters
- required_checks:
  - `cargo test -p freehand-task`
  - `cargo test -p freehand-tools`
  - `cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - task create writes ledger, snapshot, and index
  - task runtime boot rebuilds memory state from persisted snapshot
  - self agent is registered as available on first boot
  - no-dispatch create becomes `WaitingAgent`
  - agent create/close persists and recovers agent registry state
  - assign moves waiting tasks to assigned state
  - claim_next chooses the highest-priority assigned task for an agent and starts a lease
  - record_execution writes progress only for running tasks
  - list_tasks filters task snapshots by status and assignee
  - history returns ordered task ledger timeline
  - cancel releases agent state and blocks resume
  - TaskBoard query returns owner-backed task board truth
  - ExecutionFact sync updates Task Center without parsing raw prose
  - SchedulerTick emits elapsed/stale/timeout facts without business decisions
- required_module_black_box_tests:
  - runtime task tool create routes through task persistence
  - runtime task tool query reads persisted task truth
  - runtime task tool history reads task ledger timeline
  - runtime task tool list_agents exposes self agent
  - runtime task tool create_agent/assign/cancel/close_agent covers registry lifecycle
  - runtime task tool claim_next covers priority queue claim
  - runtime task tool record_execution covers worker progress event writes
  - runtime task tool list_tasks covers queue projection filtering
  - runtime TaskBoard query covers blocked/review/stale board filters
  - runtime ExecutionFact sync covers recovering/blocked/review_ready updates
  - runtime SchedulerTick covers stale/soft-timeout/hard-timeout fact emission
- required_project_black_box_tests:
  - restart/reboot recovery query returns the same task truth
- test_design_doc: `docs/testing/task.orchestration.md`
- function_map_doc: `docs/function-maps/task.orchestration.md`
- mainline_call_doc: `docs/mainline-calls/task.orchestration.json`
- generated_wiki_doc: `docs/wiki/task.orchestration.md`
- debug_artifacts:
  - task ledger rows
  - task snapshots
  - agent snapshots
- runtime_paths:
  - `~/.freehand/ledgers/tasks`
  - `~/.freehand/state/tasks`
  - `~/.freehand/state/agents`
  - `~/.freehand/state/task-runtime`
- update_triggers:
  - task state machine changes
  - task tool op surface changes
  - task persistence path changes
  - agent registry status changes
  - agent lifecycle op changes
  - TaskBoard query surface changes
  - ExecutionFact contract changes
  - SchedulerTick fact semantics changes
  - startup recovery behavior changes
- lifecycle_checks:
  - task ledger remains append-only truth
  - snapshot is rebuildable cache, not sole truth
  - runtime memory state is rebuilt from persistence on boot
  - running tasks are backed by leases and expired leases recover to `Interrupted`
  - agent and cwd are not permanently bound
  - worker execution cannot close task without review/approval
  - recovering execution facts do not terminalize tasks
  - scheduler tick emits facts only and does not make business decisions

### `agent.lifecycle`

- owner: `crates/freehand-task` initially; split to a dedicated crate only after the Phase 1 boundary proves `task.orchestration` is the wrong owner
- allowed_paths: `crates/freehand-task/**`, `crates/freehand-runtime/**`, `crates/freehand-ui-protocol/**`, `apps/freehand-cli/**`, `apps/freehand-daemon/**`, `docs/design/**`, `docs/function-maps/agent.lifecycle.md`, `docs/testing/agent.lifecycle.md`, `docs/mainline-calls/agent.lifecycle.json`, `docs/wiki/agent.lifecycle.md`, `docs/architecture/feature-map.md`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, UI app-local lifecycle inference, raw assistant prose parsers, task mutation outside Task Center, standalone model-facing `agent` tool
- required_checks:
  - `cargo test -p freehand-task`
  - `cargo test -p freehand-runtime`
  - `cargo test -p freehand-ui-protocol`
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - lifecycle reducer records `model_thinking` from typed model/provider request events
  - lifecycle reducer records `tool_running` from typed tool events
  - lifecycle reducer records `recovering` from typed failed-tool/schema/provider retry facts
  - lifecycle reducer records `blocked` from typed blocker facts
  - lifecycle reducer rejects raw assistant prose as lifecycle input
  - AgentBoard projection includes current activity, elapsed time, task/execution/turn binding, and model/tool/error counters
- required_module_black_box_tests:
  - runtime can query AgentBoard truth without UI-local state
  - CLI/ADP headless sample can query lifecycle truth for the same agent id
  - restart recovery can query the same agent lifecycle/board id after daemon restart once persistence is implemented
- required_project_black_box_tests:
  - S-profile `127.0.0.1:4042` headless AgentBoard/lifecycle query proof before claiming Phase 1 closeout
- test_design_doc: `docs/testing/agent.lifecycle.md`
- function_map_doc: `docs/function-maps/agent.lifecycle.md`
- mainline_call_doc: `docs/mainline-calls/agent.lifecycle.json`
- generated_wiki_doc: `docs/wiki/agent.lifecycle.md`
- debug_artifacts:
  - AgentLifecycleSnapshot projection
  - AgentBoard projection
  - lifecycle reducer event fixture
- runtime_paths:
  - `~/.freehand/state/agents`
  - `~/.freehand/state/tasks`
  - `~/.freehand/ledgers/tasks`
  - `~/.freehand/ledgers/metadata`
- update_triggers:
  - lifecycle state vocabulary changes
  - AgentLifecycleSnapshot fields change
  - AgentBoard projection changes
  - lifecycle reducer input events change
  - AgentBoard ADP/CLI query surface changes
  - worker_control safe-point state exposure changes
- lifecycle_checks:
  - Agent Lifecycle remains an intrinsic agent state/projection, not a default model-facing tool
  - lifecycle reducer consumes typed events only and never parses raw assistant prose
  - task mutation remains in Task Center
  - UI and Android consume projection truth and do not infer lifecycle from raw tool names or logs
  - Worker Control may read lifecycle truth but must not mutate it directly as task truth

### `worker.control`

- owner: `crates/freehand-task` initially; split to a dedicated crate only after Phase 2C proves the control inbox should not live beside Task Center truth
- allowed_paths: `crates/freehand-task/**`, `crates/freehand-runtime/**`, `crates/freehand-ui-protocol/**`, `apps/freehand-cli/**`, `apps/freehand-daemon/**`, `docs/design/**`, `docs/function-maps/worker.control.md`, `docs/testing/worker.control.md`, `docs/mainline-calls/worker.control.json`, `docs/wiki/worker.control.md`, `docs/architecture/feature-map.md`, `docs/goals/**`, `MEMORY.md`, `note.md`
- forbidden_paths: provider adapter wire DTO internals, UI app-local worker control state, raw prompt-history mutation, raw transcript rewrite, workspace/session truth mutation, task create/assign/claim/review/approve/reject/close through worker control
- required_checks:
  - `cargo test -p freehand-task worker_control -- --nocapture`
  - `cargo test -p freehand-ui-protocol worker_control -- --nocapture`
  - `cargo test -p freehand-runtime worker_control -- --nocapture`
  - `cargo test -p freehand-cli -- --nocapture`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - worker control validates task, execution, agent, assignee, active execution, op-specific payloads, and terminal-task rejection before writing truth
  - `query_status` writes a framework-derived status event and returns Task Center plus Agent Lifecycle status
  - `ask_at_safe_point`, `add_constraint`, `request_checkpoint`, and `request_submission_now` persist pending/deferred safe-point events without mutating task status
  - `pause`, `resume`, and `cancel` write worker-control events and then route task-state consequences through Task Center APIs
  - wrong execution id, terminal task, missing question, and missing constraint write no control event and no task mutation
- required_module_black_box_tests:
  - runtime dispatch routes protocol-owned `WorkerControl` commands into the worker-control owner and projects explicit results
  - protocol validates and owner-routes `WorkerControl` while rejecting query-route misuse and missing fields
  - CLI no-UI worker-control sample proves query, safe-point enqueue, pause, resume, cancel, and restart same-id control-event recovery
- required_project_black_box_tests:
  - S-profile `127.0.0.1:4042` headless worker-control foundation sample and restart same-id verify before claiming Phase 2C closeout
- test_design_doc: `docs/testing/worker.control.md`
- function_map_doc: `docs/function-maps/worker.control.md`
- mainline_call_doc: `docs/mainline-calls/worker.control.json`
- generated_wiki_doc: `docs/wiki/worker.control.md`
- debug_artifacts:
  - worker-control ledger rows
  - worker-control snapshot projection
  - Task Center task history for pause/resume/cancel consequences
- runtime_paths:
  - `~/.freehand/state/task-runtime/<owner>/worker-control`
  - `~/.freehand/ledgers/worker-control/<owner>`
  - `~/.freehand/state/tasks`
  - `~/.freehand/ledgers/tasks`
- update_triggers:
  - worker control op vocabulary changes
  - worker-control event schema changes
  - safe-point status semantics change
  - Task Center consequence mapping changes
  - ADP/CLI worker-control command shape changes
- lifecycle_checks:
  - Worker Control only targets already-running worker executions
  - framework-answerable status reads owner truth and still writes an auditable control event
  - worker-model-answerable questions/constraints are queued for safe points and do not mutate prompt history directly
  - task lifecycle consequences still enter Task Center APIs
  - terminal task states reject runtime control
  - persisted control events survive daemon restart and same-id verification

### `config.core`

- owner: `crates/freehand-config`
- allowed_paths: `crates/freehand-config/**`, `crates/freehand-contracts/**`, `docs/architecture/**`
- forbidden_paths: `apps/**` provider adapter internals
- required_checks:
  - `cargo test -p freehand-config`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - config load/validate tests
  - startup mode config tests
  - slave startup permission config tests
  - multi-agent named-table config tests
  - reciprocal peer-topology config tests
  - multi-provider named-table config tests
  - remote daemon registry, route selection, and QR bootstrap bundle contract tests
  - provider auth source resolution tests
  - provider protocol declaration tests
  - provider unknown-field rejection tests
  - restart-only config activation tests
- required_module_black_box_tests:
  - config file load smoke
  - named agent selection smoke
  - named provider selection smoke
- required_project_black_box_tests:
  - CLI agent-start config + provider projection smoke
- test_design_doc: `docs/testing/config.core.md`
- module_registry: `docs/module-registry/config.core.json`
- function_map_doc: `docs/function-maps/config.core.md`
- mainline_call_doc: `docs/mainline-calls/config.core.json`
- generated_wiki_doc: `docs/wiki/config.core.md`
- debug_artifacts:
  - config snapshot path
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/logs/config`
- update_triggers:
  - config schema changes
  - remote daemon registry, route selection, or QR bootstrap schema changes
  - provider registry schema changes
  - provider selection rules change
  - config resolution order changes
  - runtime home layout changes
  - startup file contract changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - multi-agent config ownership remains single-source
  - multi-provider config ownership remains single-source
  - startup mode lifecycle is fully covered
  - provider selection lifecycle is fully covered
  - config update path is closed-loop
  - one-process-one-agent startup rule remains explicit
  - paired node topology remains config-owned and reciprocal
  - remote daemon accounts, daemon endpoint candidates, route diagnostics, and QR bootstrap bundles remain config-owned and secret-safe
  - migrated mainline call source and generated wiki stay in sync with the function map

### `config.account-config-sync`

- owner: `crates/freehand-account-config`; `apps/freehand-relay-server` provides the authenticated Relay-host composition boundary
- allowed_paths: `crates/freehand-account-config/**`, `apps/freehand-relay-server/**`, `docs/module-registry/config.account-config-sync.json`, `docs/verification-maps/config.account-config-sync.json`, `docs/function-maps/config.account-config-sync.md`, `docs/testing/config.account-config-sync.md`, `docs/mainline-calls/config.account-config-sync.json`, `docs/wiki/config.account-config-sync.md`, `docs/resource-maps/core.json`, `docs/architecture/feature-map.md`, `scripts/verify-relay-account-config-smoke.sh`, `Makefile`, `xtask/**`, `Cargo.toml`, `Cargo.lock`, `MEMORY.md`, `note.md`
- forbidden_paths: Relay store/agent directory/tunnel truth, provider credential values, whole-machine `config.toml` upload, WebUI local-state truth, ADP/business payload config content
- required_checks:
  - `cargo test -p freehand-account-config -- --nocapture`
  - `cargo test -p freehand-account-config --test account_config_http_blackbox -- --nocapture`
  - `cargo check -p freehand-relay-server`
  - `scripts/verify-relay-account-config-smoke.sh`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - same-account multi-device document reads
  - revision/etag generation and If-Match conflict rejection
  - cross-account isolation
  - secret-field and whole-config rejection
  - failed persistence never publishes a new revision
- required_module_black_box_tests:
  - authenticated GET/PUT round trip through the merged Relay host router
  - missing token, stale If-Match 409 with server document, and cross-account 404 rejection
- required_project_black_box_tests:
  - standalone Relay host process serves account-scoped config and preserves the document across restart
- test_design_doc: `docs/testing/config.account-config-sync.md`
- module_registry: `docs/module-registry/config.account-config-sync.json`
- function_map_doc: `docs/function-maps/config.account-config-sync.md`
- mainline_call_doc: `docs/mainline-calls/config.account-config-sync.json`
- generated_wiki_doc: `docs/wiki/config.account-config-sync.md`
- debug_artifacts:
  - account-scoped config document files under `FREEHAND_RELAY_ACCOUNT_CONFIG_DIR`
- runtime_paths:
  - `FREEHAND_RELAY_ACCOUNT_CONFIG_DIR` (Claw: `/var/lib/freehand-relay/account-config`)
- update_triggers:
  - account config document schema changes
  - revision/etag or If-Match conflict policy changes
  - secret-boundary or safe-projection changes
  - Relay-host config route wiring changes
- lifecycle_checks:
  - account-scoped document is the only server-side config truth
  - secret values never enter the document or any response
  - conflict updates return the server current document instead of last-write-wins
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.cli-runtime-smoke`

- owner: `apps/freehand-cli`
- allowed_paths: `apps/freehand-cli/**`, `crates/freehand-testkit/**`, `crates/freehand-reason/**`, `crates/freehand-config/**`, `scripts/verify-provider-retry-online.sh`, `scripts/verify-master-worker-autonomy-online.sh`, `scripts/verify-real-provider-master-worker-history.sh`, `docs/architecture/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/app.cli-runtime-smoke.json`, `docs/wiki/app.cli-runtime-smoke.md`
- forbidden_paths: `crates/freehand-provider-*/**` except consumed semantic outputs only
- required_checks:
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - none beyond dispatch helper coverage
- required_module_black_box_tests:
  - CLI startup config smoke
  - CLI reason compaction smoke
  - CLI recovery block smoke
  - CLI ADP mock WebSocket smoke
  - CLI ADP success/failure turn sample mock WebSocket smoke
  - CLI ADP task query smoke
  - CLI ADP master-worker autonomy sample mock WebSocket smoke
- required_project_black_box_tests:
  - app boundary config -> harness-backed reason E2E smoke
  - no-UI ADP smoke against local daemon/server `/adp`
  - no-UI ADP success/failure turn samples against daemon/server `/adp`
  - no-UI provider retry fixture proof against S-profile daemon `/adp`
  - no-UI ADP task list/history query against daemon `/adp`
  - no-UI master-worker autonomy provider fixture proof against S-profile daemon `/adp`
  - real-provider master-worker history verifier that fails assigned-only task histories
- test_design_doc: `docs/testing/app.cli-runtime-smoke.md`
- function_map_doc: `docs/function-maps/app.cli-runtime-smoke.md`
- mainline_call_doc: `docs/mainline-calls/app.cli-runtime-smoke.json`
- generated_wiki_doc: `docs/wiki/app.cli-runtime-smoke.md`
- debug_artifacts:
  - CLI smoke stdout fixtures
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/state/turns`
  - `~/.freehand/state/tasks`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/tasks`
- update_triggers:
  - CLI command shape changes
  - smoke scenario changes
  - harness boundary changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - CLI remains a true app boundary, not direct crate test glue
  - config selection still has one-process-one-agent truth
  - reason smoke path still routes through shared harness and rewrite policy owner paths
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.cli-live-turn`

- owner: `apps/freehand-cli`
- allowed_paths: `apps/freehand-cli/**`, `crates/freehand-runtime/**`, `crates/freehand-config/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-provider-core/**`, `crates/freehand-reason/**`, `docs/function-maps/**`, `docs/testing/**`
- forbidden_paths: `crates/freehand-reason/**` semantic-owner changes unrelated to provider-neutral consumption
- required_checks:
  - `cargo test -p freehand-cli`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - none beyond argument dispatch helpers
- required_module_black_box_tests:
  - CLI live-turn single-shot mock smoke
  - CLI live-turn stream mock smoke
  - CLI live-turn invalid-schema retry smoke
  - CLI live-turn unsupported-provider smoke
- required_project_black_box_tests:
  - app boundary config-selected anthropic provider drives one real turn through live bridge
- test_design_doc: `docs/testing/app.cli-live-turn.md`
- function_map_doc: `docs/function-maps/app.cli-live-turn.md`
- mainline_call_doc: `docs/mainline-calls/app.cli-live-turn.json`
- generated_wiki_doc: `docs/wiki/app.cli-live-turn.md`
- debug_artifacts:
  - CLI live-turn stdout fixtures
- runtime_paths:
  - `~/.freehand/state/config`
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - CLI live-turn command shape changes
  - live bridge summary projection changes
  - config-selected anthropic path changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - CLI remains app boundary only
  - live turn still routes through runtime-owned live bridge instead of duplicating provider/runtime semantics
  - config-selected anthropic path remains closed-loop
  - completion loop projections stay on the app boundary and do not leak tagged schema text
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.webui-smoke`

- owner: `apps/freehand-server`
- allowed_paths: `apps/freehand-server/**`, `crates/freehand-ui-protocol/**`, `scripts/verify-webui-foundation-contracts.mjs`, `scripts/lib/adp-verifier-client.mjs`, `scripts/lib/adp-verifier-client.test.mjs`, `scripts/verify-model-group-ui-online.mjs`, `scripts/verify-provider-hosted-web-search-online.mjs`, `scripts/verify-provider-recovery-webui-online.mjs`, `scripts/verify-provider-registry-ui-online.mjs`, `scripts/verify-provider-web-search-settings-ui-online.mjs`, `scripts/verify-web-fetch-tool-online.mjs`, `scripts/verify-webui-ambiguous-submit-recovery.mjs`, `scripts/verify-webui-diagnostics-online.mjs`, `scripts/verify-webui-image-attachment-online.mjs`, `scripts/verify-webui-live-tool-render-online.mjs`, `scripts/verify-webui-mobile-ui-tree-online.mjs`, `scripts/verify-webui-new-session-online.mjs`, `scripts/verify-webui-path-diagnostic-online.mjs`, `scripts/verify-webui-session-restore-error-exit-online.mjs`, `scripts/verify-webui-session-search-online.mjs`, `scripts/verify-webui-stop-continue-online.mjs`, `scripts/verify-webui-timer-dashboard-online.mjs`, `scripts/verify-webui-tools-registry-online.mjs`, `scripts/verify-worker-recovered-history-online.mjs`, `scripts/webui_verify_online.mjs`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`, `docs/goals/**`, `docs/mainline-calls/**`, `docs/wiki/**`, `docs/resource-maps/core.json`
- forbidden_paths: `crates/freehand-runtime/**`, `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-config/**`, `crates/freehand-provider-*/**` except consuming already-owned UI protocol projections
- required_checks:
  - `cargo test -p freehand-server`
  - `node scripts/verify-webui-foundation-contracts.mjs`
  - `make session-paging-online`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - none beyond app boundary rendering helpers
- required_module_black_box_tests:
  - WebUI command ingress accept smoke
  - WebUI command ingress dispatch failure projection smoke
  - WebUI command ingress dispatch join-failure projection smoke
  - WebUI command ingress query-route-misuse rejection smoke
  - WebUI default ADP query/subscribe/command asset smoke
  - WebUI ADP failure frame visible-state smoke
  - WebUI ADP runtime-query-port failure frame smoke
  - WebUI query projection smoke
  - WebUI debug query projection smoke
  - WebUI latest-turn SSE subscribe smoke
  - WebUI debug SSE subscribe smoke
  - WebUI slave-card render smoke
  - CLI/WebUI divergence smoke via protocol projection
- required_project_black_box_tests:
  - app boundary WebUI consumes `freehand-ui-protocol` projection truth without provider/reason imports
- test_design_doc: `docs/testing/app.webui-smoke.md`
- function_map_doc: `docs/function-maps/app.webui-smoke.md`
- mainline_call_doc: `docs/mainline-calls/app.webui-smoke.json`
- generated_wiki_doc: `docs/wiki/app.webui-smoke.md`
- debug_artifacts:
  - WebUI smoke stdout fixture
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
- update_triggers:
  - WebUI command shape changes
  - WebUI projection shape changes
  - UI protocol projection rules change
  - generated wiki freshness policy changes
- lifecycle_checks:
  - WebUI remains app/render boundary only
  - WebUI consumes `freehand-ui-protocol` truth
  - query and subscribe remain protocol-owned
  - slave-card divergence remains protocol-safe
  - migrated mainline call source and generated wiki stay in sync with the function map

### `relay.transport`

- owner: `crates/freehand-relay` for account, presence, and proxy semantics; `apps/freehand-relay-server` is the thin process/deployment host; `apps/freehand-daemon` may only expose the registered compatibility-host edge to the Relay public API
- allowed_paths: `crates/freehand-relay/**`, `apps/freehand-relay-server/**`, registered compatibility-host wiring in `apps/freehand-daemon/Cargo.toml` and `apps/freehand-daemon/src/main.rs`, `docs/module-registry/relay.transport.json`, `docs/verification-maps/relay.transport.json`, `docs/function-maps/relay.transport.md`, `docs/testing/relay.transport.md`, `docs/lifecycles/relay-outbound-tunnel.json`, `docs/mainline-calls/relay.transport.json`, `docs/mainline-calls/app.runtime-daemon.json`, `docs/wiki/relay.transport.md`, `docs/wiki/app.runtime-daemon.md`, `docs/resource-maps/core.json`, `docs/architecture/feature-map.md`, `docs/architecture/dev-gates.md`, `scripts/verify-relay-deployment-smoke.sh`, `scripts/verify-remote-relay-local-online.sh`, `scripts/verify-dual-path-update.sh`, `Makefile`, `xtask/**`, `Cargo.toml`, `Cargo.lock`, `MEMORY.md`, `note.md`
- forbidden_paths: account/password/token/presence semantic ownership in `apps/freehand-server/**`, `apps/freehand-daemon/**`, `crates/freehand-config/**`, WebUI/Android session/task truth, provider configuration payloads
- required_checks:
  - `cargo test -p freehand-relay -- --nocapture`
  - `cargo test -p freehand-relay --test relay_http_blackbox -- --nocapture`
  - `cargo clippy -p freehand-relay --all-targets -- -D warnings`
  - `cargo check -p freehand-relay-server`
  - `scripts/verify-relay-deployment-smoke.sh`
  - `scripts/verify-remote-relay-local-online.sh`
  - `scripts/verify-dual-path-update.sh <tailscale-manifest-url> <relay-manifest-url>`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - password hashes and token hashes persist while raw passwords and raw tokens do not
  - account and Agent presence survive store restart
  - heartbeat lease projects online/offline without rewriting Agent-owned session truth
  - Agent client heartbeat reads typed runtime status/count on every tick and
    source failure closes the control-owned tunnel instead of reusing stale truth
  - corrupt store and store write failures fail explicitly
- required_module_black_box_tests:
  - register/login/Bearer and cookie authentication
  - same-account directory and cross-account isolation
  - authenticated heartbeat and lease expiry
  - namespaced WebUI HTTP rewrite and ADP WebSocket round trip
  - configured update manifest/APK bytes, unconfigured route failure, missing file, and traversal rejection
- required_project_black_box_tests:
  - standalone release binary restart preserves account/token/presence truth and deployment manifest starts without product daemon/WebUI wiring
  - Claw Relay serves the same versionCode/sha256/size APK bytes as the explicit Tailscale daemon update route
- test_design_doc: `docs/testing/relay.transport.md`
- module_registry: `docs/module-registry/relay.transport.json`
- verification_map: `docs/verification-maps/relay.transport.json`
- function_map_doc: `docs/function-maps/relay.transport.md`
- mainline_call_doc: `docs/mainline-calls/relay.transport.json`
- generated_wiki_doc: `docs/wiki/relay.transport.md`
- debug_artifacts:
  - Relay structured HTTP errors
  - standalone service stderr
- runtime_paths:
  - `/var/lib/freehand-relay/store.json`
  - `/etc/freehand-relay/relay.env`
  - `/var/lib/freehand-relay/updates`
- update_triggers:
  - Relay account/token contract changes
  - Agent heartbeat/directory projection changes
  - HTTP/ADP proxy route changes
  - deployment unit/env changes
  - Relay update artifact route or release-bundle contract changes
- lifecycle_checks:
  - Relay restart restores account/token/presence truth before serving
  - expired Agent presence is offline and cannot be proxied
  - every request is authenticated and account-scoped before presence or proxy access
  - Relay never reads or mutates Agent session/task/provider truth

### `app.runtime-daemon`

- owner: `apps/freehand-daemon`
- allowed_paths: `apps/freehand-daemon/**`, `crates/freehand-runtime/**`, `crates/freehand-task/**` for daemon test fixture seeding only, `apps/freehand-server/**`, `scripts/freehand-daemon-launchd.sh`, `scripts/install-launchd.sh`, `scripts/verify-launchd-restart-guard.sh`, `scripts/verify-launchd-restart-guard-online.sh`, `scripts/verify-master-three-worker-e2e-online.sh`, `docs/resource-maps/core.json`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`, `docs/mainline-calls/app.runtime-daemon.json`, `docs/wiki/app.runtime-daemon.md`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-config/**`, `crates/freehand-provider-*/**` except through `crates/freehand-runtime`
- module_registry: `docs/module-registry/app.runtime-daemon.json`
- verification_map: `docs/verification-maps/app.runtime-daemon.json`
- required_checks:
  - `cargo test -p freehand-daemon`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - daemon bootstrap config coverage
  - config-selected bootstrap coverage
  - runtime-dispatcher wiring coverage
  - dependency boundary scan
- required_module_black_box_tests:
  - daemon submit-user-input HTTP smoke
  - daemon latest-turn query smoke
  - daemon restart latest-turn query/SSE restore smoke
  - daemon restart next-turn-id continuation smoke
  - daemon provider failure HTTP smoke
  - daemon checkpoint rewind HTTP smoke
  - daemon ADP WebSocket command/query/subscribe smoke
  - daemon ADP task list/history query smoke
  - daemon ADP query-as-command rejection smoke
  - daemon direct-message dispatch smoke
  - daemon slave-mode production Worker runner bootstrap smoke
  - Relay-configured Slave loopback WebUI/ADP, outbound tunnel, typed presence,
    and shared-cancellation lifecycle smoke
  - launchd permanent-startup failure plateau, transient restart, and bounded
    rapid-failure circuit smoke
- required_project_black_box_tests:
  - real runtime owner injection over shared HTTP/SSE/command and ADP WebSocket transport without app-owned business logic
- test_design_doc: `docs/testing/app.runtime-daemon.md`
- function_map_doc: `docs/function-maps/app.runtime-daemon.md`
- mainline_call_doc: `docs/mainline-calls/app.runtime-daemon.json`
- generated_wiki_doc: `docs/wiki/app.runtime-daemon.md`
- debug_artifacts:
  - daemon stdout fixture
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - runtime transport injection changes
  - daemon bootstrap contract changes
  - daemon service-manager startup contract changes
  - daemon startup/runtime exit classification or launchd retry policy changes
  - shared app transport injection shape changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - daemon depends on `freehand-runtime`, not directly on reason/node/provider/config owners
  - app transport remains shared and protocol-only
  - runtime dispatch and UI projection stay closed-loop through one shared state handle
  - config-selected bootstrap remains one-process-one-agent; Master hosts UI
    transport, while an explicitly Relay-configured Slave hosts only its own
    loopback UI/ADP namespace beside the production Worker runner
  - launchd retries transient host failures only; permanent startup failures and
    bounded rapid-failure storms stop with explicit host-control state
  - migrated mainline call source and generated wiki stay in sync with the function map

### `app.acp-server`

- owner: `crates/freehand-acp`; `apps/freehand-daemon` owns only ACP process bootstrap wiring
- allowed_paths: `crates/freehand-acp/**`, `apps/freehand-daemon/**` for ACP entry wiring, `docs/design/acp-v1-agent-server-design.md`, `docs/module-registry/app.acp-server.json`, `docs/verification-maps/app.acp-server.json`, `docs/function-maps/app.acp-server.md`, `docs/testing/app.acp-server.md`, `docs/mainline-calls/app.acp-server.json`, `docs/wiki/app.acp-server.md`, `docs/resource-maps/core.json`, `docs/architecture/feature-map.md`, `docs/architecture/workspace-layout.md`, `docs/design/design-doc-index.md`, `MEMORY.md`, `note.md`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-task/**`, `crates/freehand-provider-*/**`, `crates/freehand-node/**`, `crates/freehand-metadata/**`, ADP wire implementation in `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-acp -- --nocapture`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `cargo run -p freehand-daemon -- acp` piped initialize + session/new + session/prompt returning stopReason end_turn
- required_white_box_tests:
  - extract_text joins ContentBlock text blocks
  - monotonic_id is strictly increasing within a process
  - cancel token flip changes subsequent observation atomically
  - SDK enforces NDJSON JSON-RPC framing, handshake gate, and ACP parameter/content validation before the adapter runs
- required_module_black_box_tests:
  - daemon `acp` subcommand over real stdio returns only NDJSON JSON-RPC frames on stdout and keeps stderr clean
- required_project_black_box_tests:
  - installed `freehand-daemon acp` handshake + session/new + session/prompt returning stopReason end_turn
- test_design_doc: `docs/testing/app.acp-server.md`
- function_map_doc: `docs/function-maps/app.acp-server.md`
- mainline_call_doc: `docs/mainline-calls/app.acp-server.json`
- generated_wiki_doc: `docs/wiki/app.acp-server.md`
- resource_map_doc: `docs/resource-maps/core.json`
- debug_artifacts:
  - ACP stderr diagnostics
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/replays/acp`
- update_triggers:
  - ACP wire method or content shape changes
  - ACP session lifecycle mapping changes
  - ACP-to-UI protocol command/query mapping changes
  - daemon ACP entrypoint changes
- lifecycle_checks:
  - initialize precedes all ACP methods
  - ACP never owns session/turn truth or copies control metadata into business payload
  - prompt update stream terminates on typed terminal projection
  - no ACP success response is synthesized from an owner failure
### `app.android-client`

- owner: `apps/freehand-android`
- allowed_paths: `apps/freehand-android/**`, `apps/freehand-server/src/assets.rs`, `apps/freehand-server/src/lib.rs`, `docs/resource-maps/core.json`, `docs/module-registry/app.android-client.json`, `docs/function-maps/app.android-client.md`, `docs/testing/app.android-client.md`, `docs/mainline-calls/app.android-client.json`, `docs/wiki/app.android-client.md`, `docs/design/multi-platform-ui-architecture.md`, `MEMORY.md`, `note.md`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `crates/freehand-node/**`, `crates/freehand-config/**`, `crates/freehand-runtime/**` except through `freehand-ui-protocol` projections
- required_checks:
  - `cd apps/freehand-android && ./gradlew testDebugUnitTest assembleDebug`
  - `cargo test -p freehand-server --lib`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
  - `apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` when a device is available
- required_white_box_tests:
  - `HostConfigTest`
  - `DaemonConnectionConfigTest`
  - `ApkUpdateManifestTest`
  - remote daemon bootstrap deep-link import coverage
  - source scan rejects local HTML and native conversation/settings/projector symbols while allowing the contract-owned APK updater
- required_module_black_box_tests:
  - `cd apps/freehand-android && ./gradlew testDebugUnitTest assembleDebug`
  - APK packages `MainActivity` and does not package a local HTML conversation shell
  - `/mock/android` and its former CSS asset return HTTP 404
  - `/?client=android-webview` returns the canonical WebUI shell
- required_project_black_box_tests:
  - Android app immediately loads the configured daemon WebUI URL
  - Android app checks the selected daemon endpoint update manifest, downloads a higher-version APK, and opens the Android system installer through a FileProvider URI
  - Android deep-link import writes the scanned remote daemon bootstrap bundle into the app-owned config before WebUI navigation
  - canonical WebUI owns protocol query/subscribe/command, transcript, composer, settings, lifecycle, and errors
  - Android device script requires foreground activity, `data-webui-shell=true`, `layoutClient=android-webview`, mobile shape, no fatal logcat, and a screenshot
  - a locked/offline/non-foreground device produces an explicit blocker and never acceptance evidence
- test_design_doc: `docs/testing/app.android-client.md`
- function_map_doc: `docs/function-maps/app.android-client.md`
- mainline_call_doc: `docs/mainline-calls/app.android-client.json`
- generated_wiki_doc: `docs/wiki/app.android-client.md`
- debug_artifacts:
  - Android device verification artifact under `artifacts/android-device/<run>/`
- runtime_paths:
  - Android app-owned `files/daemon-connection.json`
  - scanned `freehand://daemon/import?payload=...` bootstrap bundles
- update_triggers:
  - Android WebView host or platform bridge changes
  - daemon WebUI URL or mobile layout contract changes
  - remote daemon bootstrap/deep-link config schema changes
  - Android device validation script changes
  - Android APK update manifest/download/install handoff changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - Android remains a thin WebView/platform bridge only
  - daemon-hosted WebUI is the only product UI
  - Android may import account/daemon endpoint config but must not own account directory truth or route scoring
  - Android does not own protocol projection, command transport, conversation, settings, status, update UI, error, reason, debug, session, provider, or metadata truth
  - Android owns only the `android_apk_update` package-update handoff; daemon release distribution remains the manifest/APK source
  - config and network failures remain explicit WebView/startup failures; no local replacement UI exists
  - `/mock/android` remains absent
  - migrated mainline call source and generated wiki stay in sync with the function map


### `provider.semantic`

- owner: `crates/freehand-provider-core`
- allowed_paths: `crates/freehand-provider-core/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`
- forbidden_paths: `apps/**`, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-provider-core`
- required_white_box_tests:
  - semantic request/event mapping tests
  - capability declaration tests
  - periodic recovery classification tests
  - debug raw-event retention policy tests
- required_module_black_box_tests:
  - streaming semantic event smoke
  - single-shot semantic response smoke
- required_project_black_box_tests:
  - provider-to-reason integration smoke
- test_design_doc: `docs/testing/provider.semantic.md`
- function_map_doc: `docs/function-maps/provider.semantic.md`
- mainline_call_doc: `docs/mainline-calls/provider.semantic.json`
- generated_wiki_doc: `docs/wiki/provider.semantic.md`
- debug_artifacts:
  - provider replay fixture path
  - provider raw event fixture path
- runtime_paths:
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/replays/providers`
- update_triggers:
  - request/response semantic changes
  - provider adapter boundary changes
  - debug artifact shape changes
  - capability declaration changes
  - recovery classification changes
- lifecycle_checks:
  - provider semantic path is closed-loop
  - provider failure path is explicit
  - replay/debug lifecycle stays valid
  - debug and non-debug retention policies remain explicit
  - provider semantic layer remains independent from `freehand-reason`
  - metadata and request-chain data remain type-isolated
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.openai-adapter`

- owner: `crates/freehand-provider-openai`
- allowed_paths: `crates/freehand-provider-openai/**`, `crates/freehand-provider-core/**`, `crates/freehand-blocks/**`, `docs/design/**`, `docs/references/provider-protocols/**`
- forbidden_paths: `crates/freehand-ui-protocol/**`, `apps/**` except wiring-only integration tests
- required_checks:
  - `cargo test -p freehand-provider-openai`
- required_white_box_tests:
  - responses request renderer tests
  - chat-completions request renderer tests
  - responses single-shot parser tests
  - chat-completions stream parser tests
  - partial tool-call accumulation tests
- required_module_black_box_tests:
  - openai adapter emits unified semantic outputs for responses
  - openai adapter emits unified semantic outputs for chat completions
- required_project_black_box_tests:
  - openai adapter to reason integration smoke
- test_design_doc: `docs/testing/provider.openai-adapter.md`
- function_map_doc: `docs/function-maps/provider.openai-adapter.md`
- mainline_call_doc: `docs/mainline-calls/provider.openai-adapter.json`
- generated_wiki_doc: `docs/wiki/provider.openai-adapter.md`
- debug_artifacts:
  - openai raw payload fixtures
  - openai stream replay fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/openai`
  - `~/.freehand/replays/providers/openai`
- update_triggers:
  - openai protocol support changes
  - responses/chat-completions render rules change
  - stream chunk accumulation changes
  - tool argument mapping changes
- lifecycle_checks:
  - responses and chat-completions boundaries remain explicit
  - partial tool-call lifecycle is explicit
  - adapter-private DTO boundary remains intact
  - adapter does not depend on `freehand-reason`
  - metadata does not become prompt/request content implicitly
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.anthropic-adapter`

- owner: `crates/freehand-provider-anthropic`
- allowed_paths: `crates/freehand-provider-anthropic/**`, `crates/freehand-provider-core/**`, `crates/freehand-blocks/**`, `docs/design/**`, `docs/references/provider-protocols/**`
- forbidden_paths: `crates/freehand-ui-protocol/**`, `apps/**` except wiring-only integration tests
- required_checks:
  - `cargo test -p freehand-provider-anthropic`
- required_white_box_tests:
  - messages request renderer tests
  - messages single-shot parser tests
  - SSE stream parser tests
  - messages HTTP executor tests
  - incremental SSE callback delivery tests
  - tool-use and fine-grained tool-stream accumulation tests
  - stop-reason mapping tests
- required_module_black_box_tests:
  - anthropic adapter emits unified semantic outputs for messages
  - anthropic executor emits unified semantic outputs for local single-shot and SSE mock servers
- required_project_black_box_tests:
  - anthropic adapter to reason integration smoke
- test_design_doc: `docs/testing/provider.anthropic-adapter.md`
- function_map_doc: `docs/function-maps/provider.anthropic-adapter.md`
- mainline_call_doc: `docs/mainline-calls/provider.anthropic-adapter.json`
- generated_wiki_doc: `docs/wiki/provider.anthropic-adapter.md`
- debug_artifacts:
  - anthropic raw payload fixtures
  - anthropic stream replay fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/replays/providers/anthropic`
- update_triggers:
  - anthropic messages render rules change
  - tool-use stream accumulation changes
  - stop-reason mapping changes
- lifecycle_checks:
  - messages stateless request boundary remains explicit
  - partial tool-call lifecycle is explicit
  - adapter-private DTO boundary remains intact
  - adapter does not depend on `freehand-reason`
  - metadata does not become prompt/request content implicitly
  - migrated mainline call source and generated wiki stay in sync with the function map

### `provider.reason-live-bridge`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-config/**`, `crates/freehand-provider-core/**`, `crates/freehand-provider-executors/**`, `crates/freehand-reason/**`, `crates/freehand-blocks/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-reason/**` semantic-owner changes unrelated to provider-neutral consumption, `apps/freehand-daemon/**`
- required_checks:
  - `cargo test -p freehand-runtime`
- required_white_box_tests:
  - live bridge request build tests
  - live bridge provider-core executor selection tests
  - live bridge anthropic single-shot mock tests
  - live bridge anthropic SSE mock tests
  - live bridge OpenAI-compatible protocol descriptor tests
  - live bridge broadcast capture tests
  - live bridge incremental stream broadcast tests
  - live bridge invalid-schema retry tests
  - live bridge continue-next-round tests
  - live bridge retry-exhausted failed-terminal tests
  - unsupported provider selection tests
  - provider HTTP failure tests
  - persistence restore/write tests on runtime-owned live bridge
  - tool-result re-entry into second provider request tests
- required_module_black_box_tests:
  - config-selected provider can drive one runtime-owned live turn with persistence and UI projection updates
  - config-selected restart can restore prior closed turns and continue ordinal allocation without turn-id reuse
- required_project_black_box_tests:
  - CLI live-turn smoke against local provider-compatible mock server
  - daemon submit-user-input HTTP smoke against local provider-compatible mock server
- test_design_doc: `docs/testing/provider.reason-live-bridge.md`
- function_map_doc: `docs/function-maps/provider.reason-live-bridge.md`
- mainline_call_doc: `docs/mainline-calls/provider.reason-live-bridge.json`
- generated_wiki_doc: `docs/wiki/provider.reason-live-bridge.md`
- debug_artifacts:
  - live bridge replay fixture path
  - local mock transcript fixtures
- runtime_paths:
  - `~/.freehand/ledgers/providers/anthropic`
  - `~/.freehand/ledgers/providers/openai`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/providers/anthropic`
  - `~/.freehand/replays/providers/openai`
- update_triggers:
  - config-to-provider bridge rules change
  - anthropic executor boundary changes
  - reason turn live ingestion path changes
  - provider-output apply error mapping changes
  - CLI live-turn command shape changes
- lifecycle_checks:
  - reason remains provider-implementation independent
  - live bridge owns runtime composition without duplicating adapter semantics and without direct concrete provider-crate dependencies
  - anthropic live path is closed-loop from config selection to turn truth, persistence, and UI projection
  - completion schema loop remains bridge composition, not provider or app semantics
  - migrated mainline call source and generated wiki stay in sync with the function map

### `tool.registry`

- owner: `crates/freehand-tools`
- allowed_paths: `crates/freehand-tools/**`, `crates/freehand-provider-core/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `apps/**`, `crates/freehand-provider-openai/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-reason/**`
- required_checks:
  - `cargo test -p freehand-tools`
- required_white_box_tests:
  - registry schema export tests
  - read-only / implemented metadata tests
  - foreground bash success / cwd / timeout / exit-failure tests
  - implemented tool execution tests
  - unknown/unimplemented tool rejection tests
- required_module_black_box_tests:
  - runtime live bridge tool-schema export smoke
  - runtime live bridge implemented tool execution smoke
- required_project_black_box_tests:
  - CLI live provider tool-loop smoke
  - daemon live provider tool-loop smoke
- test_design_doc: `docs/testing/tool.registry.md`
- function_map_doc: `docs/function-maps/tool.registry.md`
- mainline_call_doc: `docs/mainline-calls/tool.registry.json`
- generated_wiki_doc: `docs/wiki/tool.registry.md`
- debug_artifacts:
  - tool registry spec fixture path
- runtime_paths:
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/providers`
- update_triggers:
  - tool registry surface changes
  - tool schema changes
  - foreground bash execution policy changes
  - implemented tool execution behavior changes
  - runtime live bridge tool ownership changes
- lifecycle_checks:
  - tool schema ownership remains outside runtime orchestration
  - registered but unimplemented tools fail explicitly
  - foreground bash cwd lock and timeout policy remain explicit
  - first-version path tools remain locked to one workspace-root policy
  - implemented tool execution path is closed-loop into provider tool-result re-entry
  - writable live tool exposure remains gated by `tool.preview` and `runtime.checkpoint-rewind`

### `tool.preview`

- owner: `crates/freehand-tools`
- allowed_paths: `crates/freehand-tools/**`, `crates/freehand-contracts/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `apps/**`, `crates/freehand-provider-openai/**`, `crates/freehand-provider-anthropic/**`, `crates/freehand-reason/**`
- required_checks:
  - `cargo test -p freehand-tools`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - write-file preview parity tests
  - edit-file preview parity tests
  - multi-edit preview parity tests
  - preview path-lock rejection tests
  - preview invalid-argument rejection tests
- required_module_black_box_tests:
  - runtime writable-tool preview request smoke
  - runtime no-preview writable-tool rejection smoke
- required_project_black_box_tests:
  - live writable-tool path emits preview truth before execution smoke
- test_design_doc: `docs/testing/tool.preview.md`
- function_map_doc: `docs/function-maps/tool.preview.md`
- mainline_call_doc: `docs/mainline-calls/tool.preview.json`
- generated_wiki_doc: `docs/wiki/tool.preview.md`
- debug_artifacts:
  - preview parity fixture path
- runtime_paths:
  - `~/.freehand/state/checkpoints`
  - `~/.freehand/ledgers/checkpoints`
- update_triggers:
  - writable tool transform rules change
  - writable tool live exposure gate changes
  - preview contract changes
  - preview parity enforcement changes
- lifecycle_checks:
  - preview and execute share one semantic transform path
  - preview truth stays tool-owned and is not recomputed in runtime or UI
  - writable tools are not live-exposed without preview support
  - migrated mainline call source and generated wiki stay in sync with the function map

### `tool.display`

- owner: `crates/freehand-blocks`
- allowed_paths: `crates/freehand-blocks/**`, `crates/freehand-ui-protocol/**`, `apps/freehand-server/assets/webui.js`, `docs/architecture/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-tools/**` except tool-surface owner updates, `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `apps/**` except render-only consumption
- required_checks:
  - `cargo test -p freehand-blocks`
  - `cargo test -p freehand-ui-protocol`
  - `node --check apps/freehand-server/assets/webui.js`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - display kind classifier tests
  - class-specific parser tests
  - tool result display update tests
- required_module_black_box_tests:
  - UI protocol projects structured tool display from tool call and result contracts
  - WebUI asset smoke consumes `display` fields instead of parsing tool raw text
- required_project_black_box_tests:
  - ADP latest-turn projection carries structured tool display usable by WebUI and other clients
- test_design_doc: `docs/testing/tool.display.md`
- function_map_doc: `docs/function-maps/tool.display.md`
- mainline_call_doc: `docs/mainline-calls/tool.display.json`
- generated_wiki_doc: `docs/wiki/tool.display.md`
- debug_artifacts:
  - ADP latest-turn projection fixtures
- runtime_paths:
  - `~/.freehand/replays/ui`
- update_triggers:
  - tool display category changes
  - tool result display projection changes
  - public conversation tool summary rules change
- lifecycle_checks:
  - classification standard remains a single owner file
  - every parser stays an independent function
  - UI consumes parser output and does not guess tool category from raw terms
  - migrated mainline call source and generated wiki stay in sync with the function map

### `ui.protocol`

- ADP WebSocket upgrade 入口鉴权已落地：`handle_adp_socket` 在 WebSocket upgrade 前检查 `Authorization: Bearer <token>` 或 `Cookie: freehand_adp_auth=<token>`，未认证返回 401；自生成 32 字节 hex token（`/dev/urandom`），可通过 `FREEHAND_ADP_AUTH_TOKEN` 环境变量覆写；根页面 `Set-Cookie` 注入 token；relay 透传认证头

- owner: `crates/freehand-ui-protocol`
- allowed_paths: `crates/freehand-ui-protocol/**`, `crates/freehand-debug/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `apps/**` except transport-only adapters
- required_checks:
  - `cargo test -p freehand-ui-protocol`
- required_white_box_tests:
  - command/projection mapping tests
  - ingress acceptance/rejection tests
  - command dispatch routing tests
  - checkpoint rewind ingress validation and owner-routing tests
  - task query DTO validation and runtime-query-port shape tests
  - subscription selector and match tests
  - public turn projection tests
  - paged session list command validation positive/reverse tests
  - client-specific projection gating tests
  - debug-state projection and receiver-drain tests
- required_module_black_box_tests:
  - command ingress accept/reject smoke
  - command dispatch envelope owner-routing smoke
  - task query command DTO smoke
  - latest-turn subscribe and specific-turn query smoke
  - debug-state snapshot/query/subscribe smoke
  - CLI/WebUI divergence smoke via protocol projection
  - public conversation projection smoke
- required_project_black_box_tests:
  - protocol truth can back HTTP query and SSE subscribe adapters without app-owned projection duplication
- test_design_doc: `docs/testing/ui.protocol.md`
- function_map_doc: `docs/function-maps/ui.protocol.md`
- mainline_call_doc: `docs/mainline-calls/ui.protocol.json`
- generated_wiki_doc: `docs/wiki/ui.protocol.md`
- debug_artifacts:
  - UI protocol stream fixtures
  - node status snapshots
- runtime_paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
- update_triggers:
  - UI command shape changes
  - query/subscribe routing changes
  - public projection rules change
  - debug snapshot bridge changes
  - client-specific projection gating changes
- lifecycle_checks:
  - UI remains ingress plus read-only projection boundary
  - command ingress stays separate from query/subscribe
  - projection ownership stays in `freehand-ui-protocol`
  - client-specific projection gating stays protocol-owned
  - UI does not become reason/debug/session truth writer

### `contracts.core`

- owner: `crates/freehand-contracts`
- allowed_paths: `crates/freehand-contracts/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-config/**`, `crates/freehand-ui-protocol/**`, `docs/debug/**` except references
- required_checks:
  - `cargo test -p freehand-contracts`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - shared contract serialization tests
  - shared ID contract tests
  - error contract tests
- required_module_black_box_tests:
  - contract replay fixture decode smoke
- required_project_black_box_tests:
  - cross-crate contract compatibility smoke
- test_design_doc: `docs/testing/contracts.core.md`
- function_map_doc: `docs/function-maps/contracts.core.md`
- mainline_call_doc: `docs/mainline-calls/contracts.core.json`
- generated_wiki_doc: `docs/wiki/contracts.core.md`
- debug_artifacts:
  - shared contract replay fixture path
- runtime_paths:
  - `~/.freehand/replays/contracts`
  - `~/.freehand/state/contracts`
- update_triggers:
  - shared chain node changes
  - shared ID changes
  - error contract policy changes
  - serialization boundary changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - shared semantic ownership remains centralized
  - request/response/error contract paths are closed-loop
  - persistence and replay guarantees remain explicit
  - migrated mainline call source and generated wiki stay in sync with the function map

### `reason.turn`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`, `docs/architecture/**`, `docs/design/**`
- forbidden_paths: `crates/freehand-node/**` except runtime wiring boundaries, `crates/freehand-provider-*/**` except adapter interfaces
- required_checks:
  - `cargo test -p freehand-reason`
- required_white_box_tests:
  - per-turn truth projection tests
  - tool re-entry ownership tests
  - terminal schema validation tests
  - tagged completion parser integration tests
  - invalid completion schema rejection tests
  - failed terminal write tests
  - slow subscriber non-blocking tests
  - metadata producer provenance tests
  - metadata write failure stop-path tests
  - debug sink failure non-mutation tests
- required_module_black_box_tests:
  - turn semantic stream smoke
  - completion rejection/retry smoke
  - reason metadata/request isolation smoke
  - reason metadata durable-ledger persistence smoke
- required_project_black_box_tests:
  - reason-to-ui terminal projection smoke
- test_design_doc: `docs/testing/reason.turn.md`
- function_map_doc: `docs/function-maps/reason.turn.md`
- mainline_call_doc: `docs/mainline-calls/reason.turn.json`
- generated_wiki_doc: `docs/wiki/reason.turn.md`
- debug_artifacts:
  - turn replay fixture path
  - completion schema rejection fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/reason`
- update_triggers:
  - turn truth granularity changes
  - terminal schema changes
  - subscriber delivery policy changes
  - raw-event retention policy changes
  - session-history ownership changes
  - metadata producer write path changes
  - metadata producer durable-ledger persistence changes
  - debug observation-failure surfacing changes
- lifecycle_checks:
  - turn truth write path remains single-owner
  - terminal decision path is closed-loop
  - schema rejection and retry path are explicit
  - debug ledger and session truth boundaries remain explicit
  - context orchestration truth remains inside `freehand-reason`
  - turn startup rewrite state remains sourced from `reason.session-history`
  - provider adapter crates remain independent from `freehand-reason`
  - metadata and request-chain data remain type-isolated
  - metadata write failures remain explicit and stop affected turn mutation
  - migrated mainline call source and generated wiki stay in sync with the function map

### `reason.session-history`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-contracts/**`, `crates/freehand-blocks/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-ui-protocol/**` except projection-only consumers
- required_checks:
  - `cargo test -p freehand-reason`
- required_white_box_tests:
  - rewrite version persistence tests
  - explicit compaction rewrite tests
  - explicit rollback rewrite tests
  - explicit resume-rebuild rewrite tests
  - persisted json/file round-trip tests
  - ordinary-turn no-rewrite-version-bump tests
- required_module_black_box_tests:
  - session-history to start-turn rewrite propagation smoke
  - rewrite-gate consumption smoke
- required_project_black_box_tests:
  - reason-to-provider rewrite-version propagation smoke
- test_design_doc: `docs/testing/reason.session-history.md`
- function_map_doc: `docs/function-maps/reason.session-history.md`
- mainline_call_doc: `docs/mainline-calls/reason.session-history.json`
- generated_wiki_doc: `docs/wiki/reason.session-history.md`
- debug_artifacts:
  - session-history persisted fixture path
  - rewrite-ledger fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/replays/reason`
- update_triggers:
  - rewrite gate semantics change
  - persisted session truth format changes
  - turn-start rewrite sourcing changes
  - compaction/rollback/resume lifecycle changes
- lifecycle_checks:
  - rewrite version is single-owned by `freehand-reason`
  - non-ordinary rewrite modes enter planner only through explicit session-history gate methods
  - ordinary turns do not bump rewrite version
  - rewrite ledger retains diagnostics and applied-turn evidence
- persisted session truth remains serializable and reloadable

### `reason.persistence`

- owner: `crates/freehand-reason`
- allowed_paths: `crates/freehand-reason/**`, `crates/freehand-testkit/**`, `apps/freehand-cli/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/runtime/**`, `docs/debug/**`
- forbidden_paths: `crates/freehand-provider-*/**` except debug raw-ledger consumers at adapter boundaries, `crates/freehand-ui-protocol/**` except projection-only consumers
- required_checks:
  - `cargo test -p freehand-reason`
  - `cargo test -p freehand-testkit`
  - `cargo test -p freehand-cli`
- required_white_box_tests:
  - session snapshot render/load tests
  - session summary page order, cursor, metadata-only, archive, and unavailable-session tests
  - persistence cursor serialization tests
  - reason-ledger sequence ordering tests
  - snapshot-plus-tail recovery tests
  - ledger-only rebuild tests
  - provider-raw-ledger exclusion-from-session-truth tests
  - atomic snapshot replace tests
- required_module_black_box_tests:
  - persistence save/reload smoke
  - terminal turn materialization smoke
  - recovery from snapshot-plus-ledger-tail smoke
  - derived-sidecar rebuild smoke
- required_project_black_box_tests:
  - CLI persistence restore smoke
- test_design_doc: `docs/testing/reason.persistence.md`
- function_map_doc: `docs/function-maps/reason.persistence.md`
- mainline_call_doc: `docs/mainline-calls/reason.persistence.json`
- generated_wiki_doc: `docs/wiki/reason.persistence.md`
- debug_artifacts:
  - persisted session snapshot fixture path
  - reason ledger fixture path
  - corrupted persistence fixture path
  - provider raw debug fixture path
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/cache/session-index`
  - `~/.freehand/replays/reason`
- update_triggers:
  - snapshot file shape changes
  - reason ledger schema changes
  - recovery ordering changes
  - derived sidecar boundaries change
  - restart/resume flow changes
  - runtime home subdirectory changes
- lifecycle_checks:
  - only `freehand-reason` writes authoritative session/turn persistence
  - snapshot and reason-ledger ordering remains explicit and recoverable
  - provider raw debug data never becomes session truth
  - UI and index sidecars remain derived and rebuildable
  - recovery never depends on UI projections or provider raw payloads
  - metadata and request-chain data remain type-isolated across persisted artifacts

### `reason.rewrite-policy`

- owner: `crates/freehand-blocks`
- allowed_paths: `crates/freehand-blocks/**`, `crates/freehand-contracts/**`, `crates/freehand-reason/**`, `crates/freehand-testkit/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-blocks`
- required_white_box_tests:
  - compaction threshold tests
  - stale-prune-preferred tests
  - paused-auto-compaction tests
  - rollback-vs-rebuild recovery tests
  - insufficient-recovery-truth block tests
- required_module_black_box_tests:
  - rewrite policy decision smoke
  - restore recovery decision smoke
- required_project_black_box_tests:
  - provider usage event reaches rewrite policy through runtime harness
  - reason runtime reaches session-history rewrite gates only through policy-approved paths
  - missing recovery source blocks without mutating session truth
- test_design_doc: `docs/testing/reason.rewrite-policy.md`
- function_map_doc: `docs/function-maps/reason.rewrite-policy.md`
- mainline_call_doc: `docs/mainline-calls/reason.rewrite-policy.json`
- generated_wiki_doc: `docs/wiki/reason.rewrite-policy.md`
- debug_artifacts:
  - rewrite-policy replay fixture path
- runtime_paths:
  - `~/.freehand/ledgers/context`
  - `~/.freehand/replays/context`
  - `~/.freehand/state/turns`
- update_triggers:
  - compaction threshold changes
  - rewrite recovery classification changes
  - rollback or rebuild trigger policy changes
  - auto-compaction pause policy changes
- lifecycle_checks:
  - rewrite trigger policy remains separate from session-history mutation
  - missing runtime truth does not silently compact or recover
  - rollback, resume rebuild, and explicit block all remain distinct outcomes
  - runtime still may not invent rewrite modes outside the policy owner

### `reason.context-planner`

- owner: `crates/freehand-blocks`
- allowed_paths: `crates/freehand-blocks/**`, `crates/freehand-contracts/**`, `crates/freehand-reason/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**` except explicit request-consumer interfaces, `crates/freehand-ui-protocol/**`
- required_checks:
  - `cargo test -p freehand-blocks`
- required_white_box_tests:
  - context segment classification tests
  - context segment ordering tests
  - token-cap admission tests
  - subagent conclusion admission tests
  - raw subagent transcript rejection tests
  - cache-shape drift tests
  - rewrite-version bump tests
- required_module_black_box_tests:
  - planned request-content build smoke
  - metadata/request isolation smoke
  - subagent final-report enrichment smoke
- required_project_black_box_tests:
  - reason-to-provider stable-prefix smoke
  - compaction/rollback-only rewrite smoke
- test_design_doc: `docs/testing/reason.context-planner.md`
- function_map_doc: `docs/function-maps/reason.context-planner.md`
- mainline_call_doc: `docs/mainline-calls/reason.context-planner.json`
- generated_wiki_doc: `docs/wiki/reason.context-planner.md`
- debug_artifacts:
  - context planner replay fixture path
  - cache-shape drift fixture path
  - subagent final-report fixture path
- runtime_paths:
  - `~/.freehand/ledgers/context`
  - `~/.freehand/replays/context`
  - `~/.freehand/state/turns`
- update_triggers:
  - context segment class changes
  - context ordering changes
  - cache-shape policy changes
  - subagent context-admission changes
  - metadata/request boundary changes
- lifecycle_checks:
  - stable-prefix lock remains explicit
  - append-only tail lock remains explicit
  - rewrite-gate lock remains explicit
  - subagent conclusion-only admission remains explicit
  - provider renderers still do not own context planning
  - metadata and request-chain data remain type-isolated

### `debug.core`

- owner: `crates/freehand-debug`
- allowed_paths: `crates/freehand-debug/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `crates/freehand-node/**`, `apps/**`
- required_checks:
  - `cargo test -p freehand-debug`
- required_white_box_tests:
  - debug snapshot builder tests
  - trace envelope serialization tests
  - semantic/scene coordinate preservation tests
  - debug observation-failure stream tests
- required_module_black_box_tests:
  - debug snapshot caller-visible smoke
  - trace envelope JSON round-trip smoke
  - observation-failure subscriber smoke
- required_project_black_box_tests:
  - UI debug-state projection consumes `freehand-debug` snapshot truth
  - reason producer debug sink failure remains observation-only
- test_design_doc: `docs/testing/debug.core.md`
- function_map_doc: `docs/function-maps/debug.core.md`
- mainline_call_doc: `docs/mainline-calls/debug.core.json`
- generated_wiki_doc: `docs/wiki/debug.core.md`
- debug_artifacts:
  - debug trace envelope fixture path
  - debug snapshot fixture path
- runtime_paths:
  - `~/.freehand/ledgers`
  - `~/.freehand/replays`
  - `~/.freehand/logs`
- update_triggers:
  - trace envelope fields change
  - debug snapshot fields change
  - debug module dependency direction changes
  - debug observation-failure stream changes
  - debug ledger/replay ownership changes
- lifecycle_checks:
  - debug remains observation-only
  - debug does not become request/session/reason truth
  - semantic and scene positions remain paired
  - UI consumes debug projections without owning debug truth
  - sink-dispatch failures remain observable without becoming business truth

### `metadata.core`

- owner: `crates/freehand-metadata`
- allowed_paths: `crates/freehand-metadata/**`, `crates/freehand-contracts/**`, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-reason/**`, `crates/freehand-provider-*/**`, `crates/freehand-ui-protocol/**`, `apps/**`
- required_checks:
  - `cargo test -p freehand-metadata`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - metadata envelope validation tests
  - writer owner required-field tests
  - write-node required-field tests
  - request-data key rejection tests
  - metadata JSON round-trip tests
  - durable metadata ledger append/reload tests
  - durable metadata ledger parse-failure tests
  - durable metadata ledger write-failure atomicity tests
- required_module_black_box_tests:
  - metadata center write/query smoke
  - metadata center durable-ledger smoke
  - metadata/request isolation smoke
- required_project_black_box_tests:
  - workspace gate validates metadata owner docs, mainline source, and generated wiki
  - reason-turn producer tests validate first metadata writer integration and durable-ledger persistence
  - node runtime producer tests validate node-owned metadata admission and shared-ledger bootstrap wiring
- test_design_doc: `docs/testing/metadata.core.md`
- function_map_doc: `docs/function-maps/metadata.core.md`
- mainline_call_doc: `docs/mainline-calls/metadata.core.json`
- generated_wiki_doc: `docs/wiki/metadata.core.md`
- debug_artifacts:
  - metadata ledger fixture path
- runtime_paths:
  - `~/.freehand/ledgers/metadata`
  - `~/.freehand/replays/metadata`
- update_triggers:
  - metadata envelope fields change
  - writer owner contract changes
  - write-node provenance contract changes
  - metadata/request isolation policy changes
  - metadata center storage/query behavior changes
  - metadata durable-ledger path or replay behavior changes
  - metadata producer integration changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - every metadata write remains attributable to one owner feature and owner symbol
  - every metadata write remains attributable to one pipeline write node
  - metadata remains internal control/provenance data, not request-chain content
  - request content cannot be recovered from metadata fields
  - debug remains observation-only and does not become the metadata owner
  - durable metadata ledger append/reload remains request-isolated and replay-safe

### `runtime.ui-command-dispatch`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-reason/**`, `crates/freehand-node/**`, `crates/freehand-metadata/**`, `crates/freehand-ui-protocol/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`, `docs/architecture/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `apps/freehand-server/**` except protocol-only transport injection, `crates/freehand-provider-*/**`
- required_checks:
  - `cargo test -p freehand-runtime`
- required_white_box_tests:
  - config-selected runtime bootstrap tests
  - shared metadata-ledger bootstrap tests
  - shared metadata-ledger bootstrap failure tests
  - submit-input dispatch routing tests
  - cancel-turn dispatch tests
  - rewind-checkpoint dispatch tests
  - direct-message dispatch tests
  - resume-turn unsupported dispatch tests
  - runtime task query bridge tests
  - runtime ui-state projection update tests
  - runtime paged session list bridge avoids full transcript restore
  - Worker-selected identity, Master-only command rejection, and typed activity
    projection merge tests
- required_module_black_box_tests:
  - command dispatch receipt smoke
  - command dispatch owner-routing smoke
  - reason-backed turn projection smoke
  - checkpoint rewind receipt smoke
  - task list/history runtime query smoke
  - node-backed direct-message smoke
  - config-selected runtime bootstrap smoke
  - config-selected live node-metadata-ledger bootstrap smoke
- required_project_black_box_tests:
  - runtime dispatch owner stays outside app boundary smoke
- test_design_doc: `docs/testing/runtime.ui-command-dispatch.md`
- function_map_doc: `docs/function-maps/runtime.ui-command-dispatch.md`
- mainline_call_doc: `docs/mainline-calls/runtime.ui-command-dispatch.json`
- generated_wiki_doc: `docs/wiki/runtime.ui-command-dispatch.md`
- debug_artifacts:
  - runtime dispatch smoke fixtures
- runtime_paths:
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/state/tasks`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/tasks`
- update_triggers:
  - command-to-owner routing changes
  - runtime dispatch receipt/failure contract changes
  - runtime reason/node adapter behavior changes
  - runtime task query adapter behavior changes
  - app/runtime injection boundary changes
- lifecycle_checks:
  - apps remain protocol-only and do not become runtime owners
  - command dispatch owner routing remains explicit and single-sourced
  - reason turn truth mutation still stays inside `freehand-reason`
  - node direct-message/task semantics still stay inside `freehand-node`
  - task truth and filtering stay inside `freehand-task`
  - direct-session activity comes only from runtime active-turn truth; delegated
    activity comes from agent.lifecycle and is merged as typed control projection

### `runtime.master-worker-loop`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-tools/**`, `apps/freehand-daemon/**` for startup wiring only, `docs/function-maps/runtime.master-worker-loop.md`, `docs/testing/runtime.master-worker-loop.md`, `docs/mainline-calls/runtime.master-worker-loop.json`, `docs/wiki/runtime.master-worker-loop.md`, `docs/function-maps/app.runtime-daemon.md`, `docs/testing/app.runtime-daemon.md`, `docs/mainline-calls/app.runtime-daemon.json`, `docs/architecture/**`, `docs/goals/**`, `MEMORY.md`, `note.md`
- forbidden_paths: `crates/freehand-task/**` lifecycle semantics except through existing public APIs, provider wire DTO internals, UI app-local worker execution, daemon-owned business logic
- required_checks:
  - `cargo test -p freehand-tools worker_implemented -- --nocapture`
  - `cargo test -p freehand-runtime production_worker_runner -- --nocapture`
  - `cargo test -p freehand-runtime production_master_busy -- --nocapture`
  - `cargo test -p freehand-runtime runtime_live_submit -- --nocapture`
  - `cargo test -p freehand-daemon worker_mode -- --nocapture`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
- worker tool schema exposes the exact worker-safe surface, including governed
  workspace tools, while excluding recursive `task`, timers, unrestricted shell,
  and unimplemented/invented names
  - no assigned task returns an explicit idle outcome without mutating task truth
  - assigned task claims exactly once with one execution id and lease heartbeat
  - successful worker turn records `review_ready` against the claimed task/execution/agent
  - provider/network system failure after internal retries records `interrupted`
    for same-task retry; non-provider task failure records `blocked`; neither
    projects success
  - missing/invalid worker target cwd blocks before model execution
  - master live-turn tool policy remains runtime-home locked and shell-denied
- required_module_black_box_tests:
  - production worker runner uses shared Task Center owner namespace and worker execution identity
  - worker live turn persists under worker agent/session truth while task result persists under master-owned Task Center truth
  - slave daemon mode runs the worker loop and does not expose the Master UI dispatcher
  - restart can query the same task, execution, agent, and history truth
- required_project_black_box_tests:
  - S-profile Master creates and assigns external-cwd work; separate Slave daemon claims, executes, heartbeats, and reports `review_ready`, `interrupted`, or `blocked`
  - TaskHistory contains `TaskResumed`, `TaskHeartbeat`, and terminal execution fact for the same task/execution/agent ids
- test_design_doc: `docs/testing/runtime.master-worker-loop.md`
- function_map_doc: `docs/function-maps/runtime.master-worker-loop.md`
- mainline_call_doc: `docs/mainline-calls/runtime.master-worker-loop.json`
- generated_wiki_doc: `docs/wiki/runtime.master-worker-loop.md`
- debug_artifacts:
  - worker reason ledger
  - master-owned task history
  - worker daemon stdout/stderr
- runtime_paths:
  - `~/.freehand/state/tasks`
  - `~/.freehand/state/agents`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/tasks`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/logs`
- update_triggers:
  - worker claim/heartbeat/report lifecycle changes
  - worker tool capability policy changes
  - slave daemon startup or polling cadence changes
  - task-owner namespace or worker execution identity changes
  - production worker online E2E contract changes
- lifecycle_checks:
  - one process runs one configured agent
  - daemon only selects mode and hosts the runtime owner
  - Task Center remains the only task/agent/lease truth owner
  - worker does not receive recursive task-delegation capability
  - provider/runtime errors become explicit blocked execution facts
  - no assigned task is an idle no-op, not an invented success
  - migrated mainline call source and generated wiki stay in sync with the function map

### `runtime.checkpoint-rewind`

- owner: `crates/freehand-runtime`
- allowed_paths: `crates/freehand-runtime/**`, `crates/freehand-tools/**`, `crates/freehand-debug/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/design/**`, `docs/architecture/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `apps/freehand-server/**`, `crates/freehand-provider-*/**`, `crates/freehand-reason/**` persistence-owner changes unrelated to consumption
- required_checks:
  - `cargo test -p freehand-runtime`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - checkpoint manifest round-trip tests
  - checkpoint create/apply/restore ledger tests
  - preview-derived path-set snapshot tests
  - restore create/modify/delete state tests
  - no-preview writable-tool rejection tests
  - checkpoint corruption rejection tests
- required_module_black_box_tests:
  - runtime writable execution creates checkpoint before tool execute smoke
  - runtime explicit rewind restores workspace state smoke
  - runtime restart checkpoint-ledger inspection smoke
- required_project_black_box_tests:
  - CLI or daemon writable live-tool mutate-then-rewind smoke
- test_design_doc: `docs/testing/runtime.checkpoint-rewind.md`
- function_map_doc: `docs/function-maps/runtime.checkpoint-rewind.md`
- mainline_call_doc: `docs/mainline-calls/runtime.checkpoint-rewind.json`
- generated_wiki_doc: `docs/wiki/runtime.checkpoint-rewind.md`
- debug_artifacts:
  - checkpoint manifest fixture path
  - checkpoint ledger fixture path
- runtime_paths:
  - `~/.freehand/state/checkpoints`
  - `~/.freehand/ledgers/checkpoints`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
- update_triggers:
  - checkpoint manifest schema changes
  - checkpoint runtime path changes
  - preview-to-checkpoint lifecycle changes
  - explicit rewind contract changes
- lifecycle_checks:
  - runtime remains the only checkpoint/rewind owner
  - reason persistence remains separate from checkpoint restore truth
  - writable execution is blocked when checkpoint creation fails
  - rewind remains explicit and does not become fallback
  - migrated mainline call source and generated wiki stay in sync with the function map

### `node.master-slave`

- owner: `crates/freehand-node`
- allowed_paths: `crates/freehand-node/**`, `crates/freehand-contracts/**`, `crates/freehand-debug/**`, `crates/freehand-metadata/**`, `crates/freehand-runtime/**` for UI bridge only, `docs/architecture/**`, `docs/design/**`, `docs/function-maps/**`, `docs/testing/**`, `docs/mainline-calls/**`, `docs/wiki/**`
- forbidden_paths: `crates/freehand-provider-*/**`, `apps/**` except wiring-only entrypoint glue
- required_checks:
  - `cargo test -p freehand-node`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- required_white_box_tests:
  - master/slave pairing tests
  - remote daemon directory account/daemon projection and route-resolution tests
  - paired slave input-restriction tests
  - slave startup config permission tests
  - local websocket handshake tests
  - metadata producer owner and write-node provenance tests
  - debug producer bootstrap/pairing/slave-turn emission tests
  - debug sink failure observation-only tests
  - metadata write failure no-truth-materialization tests
  - pairing-loss relisten tests
  - slave turn subscription tests
  - status query and health-check tests
- required_module_black_box_tests:
  - node status snapshot smoke
  - remote daemon directory publish/resolve smoke
  - slave progress query smoke
  - node metadata ledger smoke
  - node debug snapshot subscription smoke
- required_project_black_box_tests:
  - master-delegate/slave-progress smoke
  - master-subscribe-slave-turn smoke
  - config-selected live runtime bootstrap shares node metadata ledger smoke
  - gates check for remote daemon registry to directory source edge
- test_design_doc: `docs/testing/node.master-slave.md`
- function_map_doc: `docs/function-maps/node.master-slave.md`
- mainline_call_doc: `docs/mainline-calls/node.master-slave.json`
- generated_wiki_doc: `docs/wiki/node.master-slave.md`
- debug_artifacts:
  - pairing ledger path
  - slave mode transition replay path
  - websocket handshake replay path
  - node status snapshot path
- runtime_paths:
  - `~/.freehand/state/nodes`
  - `~/.freehand/state/config`
  - `~/.freehand/ledgers/metadata`
  - `~/.freehand/ledgers/nodes`
  - `~/.freehand/replays/nodes`
- update_triggers:
  - pairing semantics changes
  - remote daemon directory / route-resolution semantics changes
  - input-permission semantics changes
  - slave input restrictions change
  - node mode lifecycle changes
  - slave startup config changes
  - websocket pairing changes
  - metadata provenance or shared-ledger wiring changes
  - turn subscription changes
  - generated wiki freshness policy changes
- lifecycle_checks:
  - input-permission lifecycle is complete
  - pairing path and rejection path are both explicit
  - runtime evidence remains traceable
  - local one-master one-slave scope remains explicit
  - pairing-loss and re-listen path remain explicit
  - migrated mainline call source and generated wiki stay in sync with the function map

## `ui.platform-architecture`

- feature_id: `ui.platform-architecture`
- owner crate: `apps/freehand-server` (WebUI), `apps/freehand-android` (Android WebView shell)
- owner: `docs/design/multi-platform-ui-architecture.md`
- function_map_doc: `docs/design/multi-platform-ui-architecture.md` (embedded in Section 6-9)
- test_design_doc: `docs/design/multi-platform-ui-architecture.md` (Section 11)
- mainline_call_doc: (not yet migrated)
- generated_wiki_doc: (not yet migrated)
- debug_artifacts:
  - responsive layout screenshots
  - mobile viewport render evidence
  - Android WebView render evidence
- runtime_paths:
  - `apps/freehand-server/assets/webui.css`
  - `apps/freehand-server/assets/theme.css`
  - `apps/freehand-server/assets/webui.js`
  - `apps/freehand-android/app/` (future)
- update_triggers:
  - design token system changes
  - responsive breakpoint changes
  - navigation model changes
  - TurnCard render contract changes
  - Android WebView bridge API changes
  - transport protocol changes
- lifecycle_checks:
  - design tokens extracted from hardcoded values
  - TurnCard contract matches `ui.protocol` projection truth
  - responsive breakpoints cover desktop/tablet/mobile
  - Android WebView loads same WebUI assets
  - JS Bridge does not duplicate protocol logic
