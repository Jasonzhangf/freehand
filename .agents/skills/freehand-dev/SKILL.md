---
name: freehand-dev
description: Use when working inside the Freehand repo on architecture, harness, config, provider, reasoning, node topology, UI protocol, gates, or test infrastructure. Enforces Freehand's contracts-plus-blocks-plus-orchestrators architecture, feature map ownership, directory locks, replay-first debugging, and required validation workflow.
---

# Freehand Dev

Use this skill for any non-trivial work in this repo.

## Start

1. Read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`.
2. Read `docs/resource-maps/core.json`.
3. Identify the source resource, target resource, and whether the relation is direct or indirect.
4. If the relation is indirect, follow the required intermediate resource; do not implement a direct shortcut.
5. Confirm every involved resource declares at least one projection; resources without observable/testable projections are incomplete.
6. For direct resource edges, require both an `allowed_direct=true` relation rule and a `source_edge_registry` row before treating a feature-local mainline row as source truth.
7. Confirm operation bindings have non-empty `operation_id`, `owner_feature_id`, `source_resource`, `target_resource`, `effect`, `mainline_call_doc`, and `binding_status`.
   - If `binding_status=pending`, require `pending_reason`, `pending_closure_doc`, and `pending_verification`; pending must name the closeout owner doc and verification entrance.
8. Confirm operation ids use `<source_resource>.<operation>` and the operation suffix is listed in the source resource's `operations`; operation bindings must not create unlisted capabilities.
9. Bound source edges must have `operation_id`, endpoints, mainline doc, step, file path, and symbol path registered in `source_edge_registry`; the registry file path must exist and the symbol path must resolve in source. Pending operations must not fake source edges.
10. Operation bindings do not imply direct relation permission by themselves. If a bound operation pair lacks an `allowed_direct` relation rule with a non-empty reason, fix the resource map before code.
11. For forbidden direct relations, confirm the same source/target pair is not also declared as `allowed_direct=true`; one resource pair cannot be both directly allowed and forbidden. Each forbidden direct relation must declare a non-empty `reason` and `required_via`, and the same source/target pair must have a matching `allowed_direct=false` relation rule with identical `via_resources`.
12. For forbidden direct relations, check `source_gate_status`; `checked` means one unique source shortcut gate with a non-empty reason and at least one actual forbidden package/import check enforces the boundary, `precise_checked` means one unique specific file/symbol body gate is checked, and `deferred` is invalid in gated resource truth.
13. Read `docs/architecture/feature-map.md`.
14. Confirm the feature-map `Resource Ownership Index` backlinks the resource owner feature to the same `resource_type`.
15. Use `Owner Routing Index` to map the problem area to exactly one `feature_id`.
16. Read the feature's bound function-map doc before non-trivial implementation or debug.
17. Read the feature's bound test-design doc before non-trivial implementation or debug.
18. Identify the target `feature_id`, owning crate, allowed paths, forbidden paths, required checks, debug artifacts, runtime paths, `test_design_doc`, `function_map_doc`, and `lifecycle_checks`.
19. If ownership is unclear, fix the map first or stop and ask.
20. Before coding, ask three questions:
   - is the information sufficient
   - is the logic closed-loop
   - is lifecycle management complete
21. If any answer is no, do read-only tracing and source search first. Ask the user only after read-only search cannot close the gap.
22. Before implementation for each module feature, write or update its test-design record first.
23. Test-design record must capture:
   - target feature and owner
   - lifecycle and logic path
   - `## Resource Operation Test Coverage` for every bound resource operation
   - each resource operation row maps status, white-box, module black-box, and project black-box coverage
   - `bound` rows must name current verification entrances; do not write pending/future placeholders in bound coverage cells
   - each `bound` coverage cell must include a command-style verification entry, not only prose
   - repo-owned command targets must exist: cargo package names, `scripts/...` files, and `make` targets are gate-checked
   - white-box coverage plan
   - module black-box coverage plan
   - project black-box coverage impact
   - known gaps and non-goals
24. Function-map record must capture:
   - owner crate and owner module
   - code-bound entry symbols
   - gated `## Resource Map Binding` with non-empty owned resources, touched resources, resource operations, forbidden shortcuts, and the source/target resources for each operation
   - request mainline
   - response mainline
   - error mainline
   - mainline call source when the feature is migrated
   - generated wiki path when the feature is migrated
   - shared multi-reference functions and why they are reused
   - call table bound to code paths
25. Tool-owning features must also capture:
   - tool spec owner
   - implemented vs unimplemented state
   - runtime exposure gate
   - execution owner symbol
   - side-effect and permission notes when relevant
26. If another worker cannot read the resource map, test design, and function map and understand where coverage lives, where the mainline runs, where source edges are registered, and what remains risky, the design is incomplete.

## Problem Routing

- Do not locate ownership by grep first.
- Locate by `Owner Routing Index` -> `feature_id` -> owner -> function map -> test-design doc.
- `docs/architecture/feature-map.md` is the feature owner registry.
- `docs/function-maps/<feature-id>.md` is the code-bound mainline and symbol registry.
- `docs/mainline-calls/<feature-id>.json` is the machine-readable mainline call source when that feature has migrated.
- `docs/wiki/<feature-id>.md` is the generated wiki artifact for migrated features.
- `docs/testing/<feature-id>.md` is the test orchestration registry.
- If the problem does not map to one owner, update the owner routing docs before code changes.
- If a touched function is not in the function map call table, update the function map in the same change.
- If a touched behavior changes coverage, update the test-design doc in the same change.

## Runtime Home

- Runtime home is `~/.freehand`.
- Use standard runtime paths:
  - `~/.freehand/state`
  - `~/.freehand/state/checkpoints`
  - `~/.freehand/state/config`
  - `~/.freehand/state/turns`
  - `~/.freehand/state/ui`
  - `~/.freehand/logs`
  - `~/.freehand/ledgers`
  - `~/.freehand/ledgers/checkpoints`
  - `~/.freehand/ledgers/metadata`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/ledgers/providers`
  - `~/.freehand/replays`
  - `~/.freehand/replays/metadata`
  - `~/.freehand/cache`
  - `~/.freehand/cache/session-index`
  - `~/.freehand/tmp`
- Runtime evidence belongs there, not in random ad hoc paths.
- Directory routes:
  - debug docs: `docs/debug/`
  - runtime docs: `docs/runtime/`
  - config docs: `docs/config/`
  - design docs: `docs/design/`
  - provider protocol references: `docs/references/provider-protocols/`
- Config source:
  - `~/.freehand/config.toml`
  - multi-agent layout uses `[agents.<name>]`

## Architecture Rules

- Global semantic types live in `crates/freehand-contracts`.
- `crates/freehand-contracts` owns cross-module shared semantic types, shared IDs, cross-module error contracts, and module-level error base contracts.
- `crates/freehand-contracts` does not own config schema, UI projection, or debug/trace envelope.
- Internal control/provenance metadata lives in `crates/freehand-metadata`.
- Every metadata write must carry writer owner and write-node provenance through `metadata.core`.
- Metadata entries must not carry request text, prompt content, message arrays, provider request payloads, or context segment content.
- Control semantics must be extracted from data pipelines and must not be encoded by rewriting request payloads, provider payloads, prompt text, or context text.
- Cancellation, retry, routing, checkpoint, gate, debug, and metadata control state must stay in explicit owner modules/ledgers/metadata/debug channels, not in `ReasonReq*` request-node payload fields.
- Shared pure semantic logic lives in `crates/freehand-blocks`.
- Before adding any function, inspect existing blocks and owner crates first.
- Do not add temporary helpers to `crates/freehand-reason` or `crates/freehand-node`.
- If logic smells reusable, semantic, parser-like, builder-like, validator-like, or projector-like, put it in `crates/freehand-blocks`.
- Provider wire DTOs stay inside `crates/freehand-provider-*`.
- Provider live bridge may select a provider driver from config, but protocol-specific endpoints, request bodies, tool declaration shapes, tool-result re-entry shapes, SSE parsing, and raw provider capture belong inside the provider adapter/executor crate. Runtime must not hardcode Responses, Chat Completions, or Anthropic Messages wire bodies.
- Provider semantic layer supports OpenAI-compatible and Anthropic first.
- Provider payload wire DTOs stay private to provider adapters.
- Turn semantics stay inside `crates/freehand-reason`.
- Turn truth is stored per turn and projected into conversation view.
- Only `crates/freehand-reason` may write session truth.
- Master/slave runtime stays inside `crates/freehand-node`.
- master/slave is input-permission configuration.
- local multiple agents are managed by `config.toml`, and one `config.toml` may define multiple local agents.
- config source path is only `~/.freehand/config.toml`.
- one process starts one agent, chosen by CLI agent name.
- each configured agent must have explicit `node_id` and `paired_agents`.
- peer topology is config-owned and compiled in declared order: every Master
  has one or more reciprocal opposite-mode Slave Worker peers; every Slave
  Worker has exactly one reciprocal Master peer.
- legacy singular `paired_agent` is invalid. Do not add a compatibility parser,
  primary-worker field, or reverse-lookup fallback.
- runtime/daemon code must consume selected peer topology from `freehand-config`; it must not derive synthetic master/slave node ids.
- current local execution topology supports one Master plus multiple explicit
  configured Worker identities, with one daemon process per agent.
- pairing transport is WebSocket handshake.
- each agent has a startup configuration file that decides its startup mode.
- whichever side is configured as `master` accepts user input and dispatches to local sub-agents or paired remote slaves.
- paired `slave` mode accepts input only from its paired source, which may be a user or another master.
- slave startup config includes at least `name`, `mode`, and `pair_token`.
- `allowed_pair_ip` is optional. If omitted, source IP is not filtered.
- `pair_token` must be configured as an environment variable reference.
- slave pairing source is fixed by config and changing it requires restart.
- if slave loses pairing, it keeps listening for later re-pairing.
- master may send task, query progress, directly talk, and subscribe to slave turn stream.
- UI code must consume `crates/freehand-ui-protocol`, never provider crates directly.
- UI code must not classify tool calls from raw names, arguments, or result strings; tool display semantics belong in the `tool.display` pure parser owner and must flow through `freehand-ui-protocol`.
- UI code must not implement session CRUD as local browser state. Session create/rename/archive/restore/delete must enter `ui.protocol`, route through `runtime.ui-command-dispatch`, and persist through `reason.persistence` session metadata truth.
- UI app boundaries must stay protocol-only: they may render `freehand-ui-protocol` truth and shared contracts, but must not import `freehand-reason`, provider crates, node semantics, or config semantics for UI behavior.
- Any UI is an input ingress plus a read-only consumer of turn/debug state. UI may submit commands, but UI must not directly mutate reason truth, debug truth, or session truth.
- First version UI scope is CLI plus WebUI.
- WebUI default control/status transport is ADP WebSocket `/adp`; HTTP query plus SSE subscribe remains compatibility/static-page support. Do not mix either UI transport with node WebSocket pairing semantics.
- Daemon control/status automation is ADP WebSocket at `/adp`; WebUI, Android, CLI, and headless tests should converge on ADP command/query/subscribe frames for unified state inspection before relying on DOM-specific diagnosis.
- ADP is internal transport terminology. WebUI/Android user-facing labels, status text, failure cards, and diagnostic prompts must say connection/service/request/conversation, not ADP; ADP may appear in code symbols, docs, CLI/test output, and debug-only surfaces.
- UI command receipt rendering must map only known dispatch status codes to user-safe text through an exact whitelist. Strip only parameter suffixes such as `:` payloads or whitespace detail first; do not use substring classification such as `contains("task_")`. Unknown dispatch statuses are explicit unsupported-receipt errors, not success fallbacks, and user-visible text must not include `target_feature_id`, task ids, execution ids, control ids, or raw owner routing strings.
- WebUI selected-session transcript rendering must preserve protocol/session transcript order and append-or-replace the latest same-session turn; do not sort visible cards by `runtime-turn-*` ordinal because ordinals can reset after restart or recovery.
- WebUI lifecycle animation must be scoped to current live turn render projection only; historical turn/tool rows must remain static even when they still carry protocol model_request or tool status fields.
- WebUI session-list truth is the render gate after it has loaded. Latest-active query, latest-turn ADP/SSE updates, and selected-session transcript projections may render only when the session id is listed, current draft, or current pending-submit; non-destructive `DeleteSession` can leave old turn truth queryable, so never use latest-active as a fallback after session-list truth exists.
- WebUI live transcript scrolling must preserve the operator's manual read position. Render updates may auto-follow only when the conversation scroll host is already near bottom or a local submit explicitly forces the new turn into view; ordinary render updates must not call `scrollIntoView`. Online proof must cover both "scrolled up stays put" and "bottom-pinned latest row remains visible above composer".
- WebUI fixed/sticky composer clearance must come from the measured composer height (`--composer-clearance`), not fixed mobile padding guesses. Online proof must compare the latest message rect with the composer rect.
- WebUI submit timeout/transport failure is ambiguous. Do not clear selected/draft session, pending user input, or draft attachments on submit failure; keep a visible pending card with unknown-dispatch status and instruct refresh before duplicate send. Online proof should force a deterministic WebSocket/offline failure from a draft session and assert the DOM is not `no sessions` / empty conversation.
- Internal framework sessions such as `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` are owner/debug truth, not top-level user conversations. User-facing global session lists must be protocol-owned persisted session metadata only. Keep direct transcript/debug query paths available. WebUI may render `worker-task-*` only as indented temporary child rows under the owning persisted master session, using TaskBoard `parent_session_id` owner truth; do not make WebUI guess top-level visibility by id locally.
- WebUI lifecycle dashboards must scope TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl rows to the selected parent session via TaskBoard `parent_session_id` / task ids. A selected session with no child tasks must render empty current lifecycle state; never fall back to global Task Center history because that leaks old blocked/review/test tasks into unrelated conversations.
- ADP WebSocket is UI/control/status transport, not node master/slave pairing transport; keep node pairing WebSocket semantics separate.
- Command ingress must stay split from query/subscribe routes. Query/subscribe commands are not valid command-ingress payloads and must be rejected explicitly.
- Before a UI command leaves `freehand-ui-protocol`, it must be wrapped in a protocol-owned owner-routing envelope; app boundaries must not invent their own command-to-owner routing.
- Runtime-backed command execution belongs in `freehand-runtime` or another explicit runtime owner crate, not in UI app crates.
- Protocol-only async transports must still respect runtime execution boundaries: if injected runtime dispatch performs synchronous provider/live work, call it through an explicit blocking boundary such as `tokio::task::spawn_blocking` instead of executing it inline on the async handler thread.
- Config-selected runtime host bootstrap should also prefer `freehand-runtime`; host apps should stay thin and must not reimplement config-selection-to-runtime wiring.
- CLI and WebUI may render different views, but they must share one `freehand-ui-protocol` truth.
- Android client work uses the same rule set: `apps/freehand-android` is the live shell, `apps/freehand-server/assets/mocks/android/mobile-mock.html` is preview-only, and `bridge.html` is the APK render host.
- No fallback, no silent downgrade, no duplicate semantic logic in orchestrators.
- Start development and debugging from the function map owner, never from random grep alone.
- Request/response/error mainlines must have logic descriptions in the function map, not only crate names.
- Any function used from multiple call sites must have one shared semantic description in the function map.
- function-call tables must bind to code symbols or explicitly say implementation binding is still pending.
- generated wiki must come from the machine-readable mainline call source; do not hand-edit generated wiki files.
- feature-map seed entries must stay unique per `feature_id`; duplicate owner blocks are invalid and must fail gate.
- `xtask gates check` validates migrated mainline-call sources as compiled manifests: JSON path, `feature_id`, function map, test design, generated wiki, and feature-map links must cross-link deterministically.
- `xtask gates check` validates migrated `bound` call-table rows: listed source files must exist and listed symbols must resolve in those files; use `binding pending`/`pending` only for unlanded bindings.
- `xtask gates check` validates CI/CD command alignment: `make ci` must include `mainlines check`, and pre-push, CI, and release workflows must route through the full gate.
- New features and bug fixes both require lifecycle thinking, not just local code patches.
- In provider work, preserve raw provider events in debug mode and rely on unified semantic events for normal operation.
- In provider work, read local official protocol snapshots under `docs/references/provider-protocols/` before inventing wire behavior.
- In reason-turn work, provider `finish_reason=stop/end_turn` is not enough to stop. Completion schema decides stop.
- In control-status work, the simple user-input stop field is `simple_question`, not `simple_request`. `simple_question=true` means the previous user input is a simple question/answer request and may allow natural stop; do not add aliases or fallback fields for this decision.
- Reason context planning follows locked Reasonix/Codex direction:
  - stable prefix stays stable across ordinary turns
  - only explicit rewrite events may change prefix layout
  - prefer subagent search final-report enrichment over injecting raw exploration transcripts
  - admit subagent context into parent turns only as typed final conclusion segments
- Dynamic model-visible input segments must not use arbitrary small runtime caps. User/operator prompts, previous visible output, schema feedback, and future task-space snapshots use content-derived admission budgets and are rejected only by the planner/model context policy, not by fixed 128/512-token local limits.
- Provider output budget defaults stay provider-owned. Anthropic live requests use `DEFAULT_ANTHROPIC_MAX_TOKENS=8192`; do not add smaller ad hoc runtime output caps.
- `reason.rewrite-policy` in `freehand-blocks` owns when compaction / rollback / resume rebuild should trigger; `freehand-reason` only owns `SessionHistory` mutation after that decision
- `ReasonRewriteRuntime` in `freehand-reason` is the baseline consumer that may call `SessionHistory::stage_*` from policy-approved decisions
- Provider `TokenUsage` enters rewrite policy only through `freehand-blocks::prompt_tokens_from_usage`; do not hand-roll provider usage interpretation in runtime or UI
- `freehand-testkit` may host project black-box runtime harnesses before production CLI/server loops exist; keep harness behavior aligned with function maps and test design
- built-in tool specs and execution ownership live in `crates/freehand-tools`
- writable tool preview ownership also lives in `crates/freehand-tools`
- runtime must not hardcode demo tool schemas or demo tool execution outside `crates/freehand-tools`
- every new built-in tool must first land as a spec in the tool owner with explicit `implemented` state
- no tool may be exposed on the live provider path until its function map and test-design docs are updated in the same change set
- Tool schemas must teach correct first calls, not rely on model trial/error. Keep descriptions concise but explicit about path/status/dispatch constraints, include one valid production pattern, and add prompt-guard tests for observed bad calls such as absolute/tilde `glob` and `task(status="all")`.
- Worker `glob` is locked-workspace scoped, not relative-only: accept relative patterns and absolute patterns only when they remain under the canonical locked workspace; reject `~`, `..`, and external absolute patterns. If online samples show repeated `glob` absolute-path failures inside the Worker `target_cwd`, fix owner semantics/tests instead of only adding prose.
- Treat user-facing symlink paths as first-class path truth. Worker path tools must canonicalize symlink aliases before workspace-boundary decisions; positive tests should cover `glob`, `grep`, `read_file`, `ls`, and `write_file` through an absolute symlink alias that resolves inside the locked task cwd.
- Worker file probes should guide the model away from avoidable failures: `ls` can list directories or report one file entry for existence checks; `read_file` is only for existing UTF-8 files and should not be used on directories, generated files that do not exist yet, or binary sidecars like `.DS_Store`.
- Master provider context must carry current framework behavior and Task Center truth before the model decides to call tools. Inject and test `TaskSpaceSnapshot` with configured Worker, valid status filters, known tasks, agents, and recent events so the model does not spend turns probing `list_agents`, `list_tasks`, or history just to understand the framework.
- Master live provider tool surface is framework-only: expose `task` and `timer`, not file/search/write tools, shell, `todo_write`, or `complete_step`. Worker owns external repo read/search/write through its task `target_cwd`; injected Master non-framework calls must return a failed capability-boundary tool result with Worker dispatch guidance and no file-content leak.
- Task tool guidance must include concrete argument shape, not only semantic prose. Lock top-level `op` in schema/tests/error text; show create/assign examples; require expanded absolute existing repository/workspace `target_cwd` instead of `~`, glob, or output paths.
- Master dispatch is lifecycle progress, not user-task completion. If the Master only creates/assigns Worker work or schedules a timer while the user objective still depends on future Task Center/timer truth, the completion schema must use `claim="waiting"` and UI must project `TerminalStatus::ToolPending` as lifecycle/running, not `Final`/completed.
- Do not rely on prompt guidance alone for parent/child lifecycle closure. Master user-session `claim="complete"` must be runtime-gated against Task Center child truth: if any task with the same `parent_session_id` is not closed, reject completion and force repair to `waiting` or further Task Center handling.
- Internal timing/wakeup capability is a standard `timer` framework tool, not task truth. Do not encode wait/schedule semantics as `task(op="wait")`, task notes, or task lifecycle state; `freehand-tools` owns the schema, and `freehand-runtime` owns durable timer state under `~/.freehand/state/timers` plus timer ledgers under `~/.freehand/ledgers/timers`.
- Timer wakeups must persist the wakeup prompt and resume Master through an internal turn when due. Relative, absolute, local-time daily/weekly, and local-time 5-field cron semantics stay independent from Task Center truth; Worker tool surfaces must not expose `timer`.
- If the next useful Master wait exceeds 3 minutes, the model-visible guidance must require `timer(op="schedule")` instead of dead-waiting in the current turn. After scheduling, Master should continue other ready Master-side work. The persisted timer prompt must say what current truth to inspect, what waited condition to revisit, and what decision to make.
- Do not accept verbal timer claims as proof. Master may say a timer was
  scheduled only after the `timer` tool returns `Timer scheduled` in that turn;
  otherwise the wakeup is not durable truth and must be fixed in guidance/tests.
- writable file-mutation tools may not reach the live provider path without a preview path in `freehand-tools` and checkpoint/rewind gating in `freehand-runtime`
- `reason.session-history` inside `freehand-reason` owns base context, rewrite mode/version, rewrite ledger, and persisted session-history snapshots.
- `reason.persistence` inside `freehand-reason` owns authoritative snapshot and reason-ledger persistence; UI sidecars and provider raw ledgers remain derived or debug-only.
- Non-ordinary rewrite modes may enter planner only through explicit session-history gate methods for compaction, rollback, or resume rebuild.
- `freehand-reason` and provider adapter crates must remain independent; neither side may depend on the other's implementation crate.
- Metadata/debug/provider/cache/control fields and request-chain content fields must stay hard-isolated by type and builder ownership.
- Metadata must not be smuggled into request text, and request content must not be recovered from metadata/debug fields.
- Control state must not be smuggled into request text or provider payload text; if control state needs model-visible expression, a single owning context builder must deliberately convert it into typed request data.
- Debug may observe metadata later, but debug is not the metadata write owner.
- When wiring a module as a metadata producer, add tests proving writer owner, write-node provenance, request-content absence, and explicit failure behavior before the producer mutates its owned truth.
- Restart recovery must use authoritative snapshots plus reason-ledger replay; UI sidecars and provider raw ledgers are never recovery truth.
- In UI protocol work, query and subscribe must stay separate, and source identity fields must remain explicit.
- Shared contract types should default to serializable, replayable, and persistable unless a higher-priority truth source says otherwise.
- Freehand AGENTS.md and skills discovery belongs to `instruction.capability-loader` in `crates/freehand-instructions`.
- Runtime, UI, and provider code must not scan AGENTS.md or skills authoring directories directly; they must consume the deterministic manifest compiled from `~/.freehand/AGENTS.md`, `~/.freehand/skills`, local `AGENTS.md`, and local `.agents/skills`.
- Provider-visible instruction admission must use `freehand-instructions::render_instruction_capability_context` plus `ContextSegmentKind::InstructionCapability`; provider adapters must not patch instruction content into wire payloads directly.

## Debug Workflow

- Start from `feature_id`, owner, `debug_artifacts`, and runtime paths in the function map.
- Use repo routes first:
  - `docs/debug/debug-playbook.md`
  - `docs/runtime/runtime-directories.md`
- Debug/search truth is source-first: use only source code, tests, maintained scripts, and canonical docs/function maps/test designs/mainline JSON as search targets.
- Prefer `scripts/source-search.sh <pattern>` for Freehand implementation searches; it is the gate-checked source-only wrapper around `rg`.
- Do not bypass `scripts/source-search.sh` with unsafe `rg` ignore overrides such as `--no-ignore`, `--unrestricted`, or `-u`; generated/runtime outputs are outside the implementation-search corpus.
- Do not search generated or runtime output when locating implementation truth: exclude `artifacts/**`, `target/**`, build outputs, screenshots, captured reports, generated `docs/wiki/**`, `.mempalace/**`, `memory/*-mempalace-corpus/**`, and `test-palaces/**`.
- Generated artifacts may be opened only as verification evidence after the producing command runs, not as a source-search corpus or implementation locator.
- Do not run `mempalace mine` directly on the repo root for Freehand unless `.gitignore` and the dry-run prove generated evidence is excluded; prefer a source-only curated corpus for memory indexing.
- When debugging, capture both semantic and scene position.
- Prefer replayable fixtures and event ledger evidence over plain logs.
- Check `~/.freehand` evidence paths before inventing new debug output locations.
- If a failure repeats twice, search externally for 3-5 candidate fixes before continuing to grind on one path.
- Keep asking during debug:
  - do I have enough information
  - is the logic path closed-loop
  - is lifecycle management complete
- If not, continue read-only source tracing first. Ask the user only when repo truth and runtime evidence cannot answer.

## Validation Workflow

- Test design and test implementation must evolve together in the same task when feature truth changes.
- Function-map logic description and code binding must evolve together with implementation in the same task when feature truth changes.
- Do not add implementation without first making the test-design path inspectable in docs.
- Before claiming completion, run the feature's required checks.
- Before claiming completion, satisfy the feature's `lifecycle_checks`.
- After any code/config/doc change in this repo, do not report completion from local tests alone. If the feature has a live surface, verify the changed behavior online through ADP/WebUI/browser evidence before claiming the change works.
- For master/worker or multi-agent autonomy claims, command-driven task samples are not sufficient. The proof must include a `SubmitUserInput`-only headless path, reject direct CLI task mutation in the mock proof, drive model/provider `task(op=...)` tool calls through a deterministic fixture or real provider, query transcript plus TaskBoard/AgentBoard/AgentLifecycle/TaskHistory truth, and verify the same task/execution/agent ids after S-profile restart.
- A daemon Slave Worker loop is synchronous blocking runtime work and must not run inline on the Tokio async host thread. Route the long-running Worker/provider service through one explicit `tokio::task::spawn_blocking` owner boundary. Lock both directions: a nested runtime can be created/dropped successfully inside the boundary, and a blocking-task panic/join failure becomes an explicit daemon error.
- Production Worker online closure must use a clean external target cwd and a real deliverable. Require Worker-owned `TaskResumed`, initial plus periodic `TaskHeartbeat`, `TaskReviewSubmitted`, a concrete artifact verified from disk, Master-owned approve/close, and same task/execution/agent history after explicit Worker restart. A created/assigned/interrupted task is red evidence.
- Master/Worker cwd delegation must distinguish Master orchestration from Worker execution. Master cannot directly read/search/write external repo paths; it creates/assigns Worker tasks with the correct existing `target_cwd`. Worker `target_cwd` is the current agent cwd/workspace root (`A`). External target path `B` is not automatically a new cwd. Worker read/query tools may inspect readable `B`; Worker write/edit/delete outside `A` must return a paired failed tool result that says the write target is outside current cwd and instructs the agent to report the correct target workspace cwd back to Master. Do not collapse boundary failures into "path missing".
- Unrestricted shell cannot be exposed to Worker provider turns until there is a real write-boundary sandbox. Worker provider tool surfaces must exclude `bash`; injected shell calls must return a paired failed tool result instead of executing.
- Do not invent `/workspace`, `/tmp`, or sibling output directories when the user supplied a repo path. Missing target paths must report exact original/expanded/canonicalization evidence rather than broad searching or path substitution.
- For real-provider master/worker claims, task creation and assignment alone are failure evidence, not partial success. Run `scripts/verify-real-provider-master-worker-history.sh --task <task_id>` against every real-provider-created worker task; any history that is empty or only `TaskCreated,TaskAssigned` means the production worker runner/scheduler did not execute and the claim must stay red.
- For development validation, prefer the symlink service profile: `scripts/install-launchd.sh installS` for first setup and `scripts/install-launchd.sh restartS` after rebuilds. `restartS` must refresh the launchd debug daemon binary copy before kickstart and health-check the env-backed bind from `~/.freehand/daemonS.env`; S-profile defaults stay fixed at `127.0.0.1:4042` and must not be moved to Tailscale by default bind detection. If online behavior looks stale, run `installS` once and verify served behavior before debugging application code. This runs `com.freehand.daemonS` on `127.0.0.1:4042` through `freehand-*S` commands and keeps global release service `com.freehand.daemon` on `127.0.0.1:4041` untouched.
- Use the global `scripts/install-global.sh` plus `scripts/install-launchd.sh restart` path only for release/promotion closeout or when explicitly validating the installed release surface.
- For any WebUI, ADP, reasoning, stream, turn lifecycle, session, tool rendering, schema retry, composer, or status/progress change, online verification is mandatory before reporting success. The minimum proof is:
  - start or restart the real daemon on the chosen validation port, normally symlink dev `127.0.0.1:4042`; use release `127.0.0.1:4041` only for release closeout
  - drive the real WebUI in a browser, not only unit tests or static DOM inspection
  - submit at least one real request through the UI path that was changed
  - query ADP state for the same session/turn and compare it with visible UI state
  - save screenshot evidence under `artifacts/webui-online/` or another explicit repo artifact path
  - report the exact commands, ADP sample/query result summary, and screenshot path in the final answer
- When using Chrome DevTools Protocol for WebUI online proof from shell automation, spawn the browser inside the long-running automation process and stop only that explicit PID after evidence capture. A short-lived shell background Chrome can exit before CDP connects, producing false DevTools-port failures unrelated to Freehand.
- Do not say WebUI behavior is fixed, verified, or passing unless browser-visible evidence and ADP/session truth both prove the changed behavior. If online verification cannot run, state that explicitly and treat the work as unverified.
- For WebUI lifecycle/helper edits, `node --check` is only syntax coverage. Capture browser console/page errors during a real fixed-port WebUI submit, because undefined runtime helpers such as lifecycle phase functions can pass syntax checks and fail only in browser execution.
- UI validation must prove the user's submitted text remains observable after send and after refresh, live lifecycle animation stops when the underlying ADP turn is terminal, and no historical turn keeps fake streaming/timer state after a newer turn starts.
- WebUI online automation must operate the current UI surface, not stale shortcuts. If `/new` or New Conversation opens a dialog, the verifier must wait for the dialog and confirm the intended mode before submitting prompts; otherwise prompts can land in a stale localStorage-selected session and produce false history failures.
- WebUI New conversation must persist session metadata through protocol-owned `CreateSession` before normal use. A browser-only draft can make `QuerySessionTurns` succeed while `QuerySessionList` remains empty; after reload the session-list truth gate will correctly reject those orphan turns.
- WebUI online terminal waits must accept every protocol terminal projection that ends live work, including success/completed, blocked, failed, cancelled, and interrupted. Do not make verifier progress depend only on the word `completed`.
- Animated mobile drawer/sheet screenshot proof must wait for settled viewport geometry, not only a body data attribute. For an opened bottom sheet, require its rect to occupy the intended viewport region; for a closed sheet, require it to move outside the viewport before capture.
- Before claiming completion, run the feature's mapped test stack:
  - module white-box tests
  - module black-box tests
  - project black-box tests
- Do not parallel-run multiple `cargo test` processes that rely on timestamp-based temp runtime helpers inside the same owner area; cross-process temp-path collisions can create false persistence/runtime failures during spot checks.
- If a focused `cargo test` appears to hang or emits no output during compile, rerun it through `scripts/run-cargo-test-with-evidence.sh -- <cargo test args...>`. This wraps cargo with a bounded timeout, writes stdout/stderr logs, and prints the exit code. Do not conclude "no cargo process" from a narrow `ps | rg cargo` check alone because the local command wrapper may appear as `rtk cargo` and the active child may be `rustc`.
- Canonical full local gate is `make ci`.
- Release closeout must run `scripts/release.sh` end-to-end and prove staged artifacts exist; global install closeout must run `scripts/install-global.sh` with a temp `FREEHAND_PREFIX` and prove installed host binaries execute.
- Installed daemon closeout must use a temp `HOME` plus real `~/.freehand/config.toml` shape, start `freehand-daemon serve --agent <name>`, curl `/health` and `/`, then stop only the exact daemon PID.
- Release WebUI/phone-facing closeout must prove the installed release daemon serves current workspace assets before UI/Android claims. Compare `apps/freehand-server/assets/webui.{js,css}` SHA-256 with `http://<release-bind>/assets/webui.{js,css}` and rerun the online verifier against release 4041.
- Android release-device closeout must set `FREEHAND_ANDROID_APK` to the release APK artifact when running `apps/freehand-android/scripts/verify-device-ui.sh`; otherwise the script default debug APK can overwrite the release install and invalidate release evidence.
- Android device foreground truth must come from current resumed/focused `com.freehand.android` activity plus a `FreehandWebUiLayout` probe. Historical package mentions in dumpsys are not foreground evidence; if a system picker is foreground, exit that picker and relaunch Freehand before judging WebUI layout.
- Phone/WebUI user-visible chrome must not show non-actionable internal labels such as raw `runtime-turn-*`, worker mode, task cwd, transport/protocol status, or other debug/session plumbing as decorative top chips. If the information is not directly actionable in that location, remove it from the conversation surface and put it behind Status, Debug details, Settings, or drawer affordances with a clear user purpose.
- Phone/WebUI focused composer must not reopen low-frequency attachment/CWD/model/status controls into the primary input surface. Verify with real browser mobile/tall viewport: focus composer, assert control strip, attachment tray, and command status are `display:none`, and screenshot the focused state.
- Phone/WebUI mobile cards must not use space-consuming left borders or inset-left shadows for assistant/tool/final state. Mobile state is conveyed by compact status text plus whole-card color backgrounds; verifier must assert assistant/tool/final computed `border-left-width=0px`, `box-shadow=none`, final summary `padding-left=0px`, and focused composer padding/height stay compact.
- WebUI Final/Summary readability claims require real rendered DOM evidence compared against ADP/session terminal text. Assert plain one-line summaries render as one `.final-summary-item`, explicitly structured source summaries render matching multiple `.final-summary-item` blocks, and no domain-keyword/punctuation guessing changes the visible structure; `node --check` or CSS review alone is not enough.
- Release launchd `restart` must rewrite env and plist before kickstart, matching `restartS`; otherwise launchd can keep stale env/plist wiring and owner-backed config update validation may fail only on release 4041.
- Android release packaging currently disables release lint checks in Gradle config; do not reintroduce Android Lint Vital into the release path without first proving it no longer hangs/fails on the pinned local toolchain.
- Minimum baseline:
  - `cargo build --workspace`
  - `cargo fmt --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace`
  - `cargo run -p xtask -- mainlines check`
  - `cargo run -p xtask -- gates check`
- For state machine, stream, timeout, retry, error projection, or resource cleanup changes, add both positive and negative tests.
- For live bridge error projection repairs, do not stop at persistence truth. Also verify runtime dispatch refreshes `UiProtocolState`, UI protocol marks user-visible activity status correctly, and fixed-port query plus SSE expose the same terminal/error state.
- For ADP online submit validation, prefer `scripts/verify-adp-fixed-session-observability-online.py --url ws://127.0.0.1:4042/adp --session <fixed-id>`. Do not wait on command receipts as the only liveness signal. The proof must use the correct internally tagged ADP envelope (`kind=command|query|subscribe`) and query the same selected session plus TaskBoard/AgentBoard/TaskHistory/WorkerControl in parallel; a pending turn with original `user_text` is valid observable truth, while an empty session after a correctly accepted command is red evidence.
- For WebUI/ADP state projection checks, use paired samples before claiming UI correctness. In dev mode prefer `freehand-cliS ... --url ws://127.0.0.1:4042/adp`; for release closeout use `freehand-cli ... --url ws://127.0.0.1:4041/adp`.
- For multi-round tool-loop claims, one-round success is invalid evidence. Use `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` and require `rounds>=2`, `tool_executions>=1`, `failed_tools>=1`, plus terminal success from ADP/session truth before claiming closure.
- For completion-schema mismatch/live-tool bugs, verify the provider finish reason gate before UI work: completion-schema mismatch handling may run only on terminal-candidate finish reasons such as `stop` / `end_turn`; it is model response polishing, not system schema repair and not provider failure. `tool_use` and incomplete tool calls must become paired tool results back to the model, not schema polishing or terminal failures.
- For provider/network executor failures, keep them separate from schema mismatch and tool-result failures. Recoverable non-stream provider errors retry exactly ten attempts inside provider/runtime before task/user-visible final failure; production backoff varies between 1s and 20s. Intermediate retries are internal evidence only, not task-visible state. Final provider/network exhaustion in a Worker task is `TaskInterrupted` so the same task can be retried with a new execution; content/path/model-terminal failures remain `TaskBlocked`.
- When the active provider family changes, update Worker retryable provider error classification for that family. OpenAI-compatible network/status failures such as `openai_http_request_failed`, `openai_stream_read_failed`, and retryable `openai_http_status_*` must map to `TaskInterrupted`; adapter/callback/content failures must not.
- For provider-retry proof, model prose claiming retries is not evidence. Require provider-domain retry truth from error-center metadata, provider fixture/error injection, or runtime event projection; prompt-only sampling must fail.
- For master-worker autonomy fixtures while production workerS is online, every created Worker task must include a real existing `target_cwd`. A no-cwd fixture task can be claimed by the production Worker and correctly blocked before the deterministic fixture finishes, creating false success-path `TaskBlocked` events.
- For task lifecycle headless proof, do not rely on a model prompt to create/review/approve/close tasks. Use protocol-owned task mutation commands over ADP, then verify `task.orchestration` list/history truth.
- For multi-task Phase 1 headless proof, use the S-profile `phase1-foundation-sample` create path first, then restart `com.freehand.daemonS` and run verify mode against the same blocked task, review task, execution, and agent ids. A fresh sample after restart is not recovery proof.
- For multi-task Phase 2A headless proof, use the S-profile `master-worker-foundation-sample` create path first, then restart `com.freehand.daemonS` and run verify mode against the same task, execution, and worker agent ids. A fresh sample after restart is not recovery proof, and model prose is not task-loop evidence.
- For multi-task Phase 2B EventInbox/MasterPoll proof, require four-part event cursors that include `event_id`, legacy three-part cursor compatibility tests, `replay_from_start=true` plus omitted limits for full drain, a final owner-backed non-replay cursor reread, and same-cursor verify after `restartS` returning zero events after cursor. Finite page limits or fresh post-restart samples are not cursor recovery proof.
- For multi-task Phase 2C worker-control proof, stateful task consequences such as pause, resume, and cancel must route through Task Center first and persist `applied` worker-control events only after the Task Center consequence succeeds. Safe-point requests persist `queued`; status queries persist `observed`. Restart proof must verify the same task, execution, agent, and control ids after `restartS`; a fresh sample is not recovery evidence.
- For parent-session Master/Worker evaluation, use `task_closed` as the resume
  signal only after every current Task Center child sharing the same
  `parent_session_id` is `Closed`. Build the follow-up from original user
  objective history, decomposed task goal/deliverables/acceptance, and accepted
  `TaskReviewSubmitted` truth; persist it in the original parent reason session
  and never expose raw Worker transcripts.
- Parent evaluation idempotency must consult terminal reason persistence
  carrying a deterministic evaluation marker, not only the Master event cursor
  or loop-state cache. Successful, waiting, and blocked evaluation turns are
  durable decisions because they may already have created next-round task
  truth; failed/interrupted/cancelled turns remain retryable.
- Any background owner that writes reason turns outside the foreground UI
  dispatcher must have a query-time projection refresh. `QuerySessionTurns`
  must restore the requested session from reason persistence before returning
  transcript truth, and internal parent-evaluation prompts must project no
  synthetic user message while the Master decision/final answer remains visible.
- An all-children-closed parent event is an evaluation trigger, not a completion
  criterion. Parent evaluation must receive original user objective history,
  child task goal/deliverables/acceptance, and accepted review truth. It must be
  allowed to create correction, improvement, or newly discovered child tasks.
  Reject any design or online verifier that proves only result aggregation; the
  verifier must force at least one next-round task before final completion.
- One Worker process executing three child tasks is not multi-agent proof.
  A three-Worker closure verifier must start three explicit Worker processes,
  prove three distinct live PIDs and configured agent ids, bind one initial task
  to each Worker, and verify TaskAssigned/TaskResumed/TaskHeartbeat/
  TaskReviewSubmitted history never crosses the assigned Worker identity.
- Task Center atomic JSON temp paths must be process-unique. A timestamp rounded
  to seconds is invalid because concurrent Worker boot/index writes can race on
  the same temp file and terminate one Worker before it claims work.
- When global `~/.freehand` EventInbox/TaskBoard contains unrelated historical truth, run Master/Worker lifecycle fixtures with an isolated temporary `HOME/.freehand`; do not delete, skip, or rewrite global truth to obtain a pass. Switch both the fixture Master and Worker provider configurations through the config owner before submitting work, and stop only the explicit fixture/server/worker PIDs started by that verifier.
- Before an isolated online verifier launches a workspace binary such as
  `target/debug/freehand-daemon`, rebuild that exact binary after source changes;
  a green unit test does not refresh a separately launched executable. Fixture
  decisions must match accepted `review_summary`/TaskHistory truth, not expected
  tokens embedded in task goals, acceptance text, or tool-call arguments.
- For WebUI multi-round rendering, never collapse `runtime-turn-N` / `runtime-turn-N-rM` into one all-in summary card. Render chronological per-round lifecycle cards, hide duplicate/internal continuation prompts after the first round, mark superseded rounds as continued, and keep the final summary at the bottom terminal row.
- For WebUI submit/history regressions, composer clearing is not proof of success. Verify the submitted text is immediately visible in the conversation stream, historical cards remain present, the latest card is appended in session order, a live turn with no public rows renders an explicit observable waiting row instead of a blank transcript, and at least two consecutive submits remain visible after later ADP refresh/timer updates.
- For same-session continuation regressions, UI transcript continuity is not enough. Add a provider-request black-box test proving the follow-up request contains prior user/assistant history from effective persisted turns, then run a real WebUI same-session follow-up prompt on S profile and verify the second answer can use first-turn-only context plus ADP reports both turn ids.
- For repaired-failure context economy, do not delete raw failed attempts from truth. Lock that `runtime-turn-N` / `runtime-turn-N-rM` repaired logical turns remain visible in persisted/UI/debug/error truth, while future default prompt context admits only the latest repaired round. A green UI transcript is not enough; inspect rendered/planned provider context or an owner test such as `effective_context_uses_last_repaired_round_without_raw_failed_attempt`.
- For WebUI restart/continuation regressions, verify a WebUI-created non-default session after daemon restart, then submit another turn to the same session and restart again. ADP `turn_ids` must strictly append without reusing an existing `runtime-turn-N`; runtime bootstrap must seed the next ordinal from all persisted sessions, not only the default runtime session.
- WebUI hidden diagnostic prompts and online verifier samples must obey the active Master tool surface. If Master is framework-only, use `task`/`timer` failure samples; do not ask Master to call workspace tools such as `read_file` just to exercise a failed tool card.
- WebUI online progress evidence may be a selected visible pending turn with dispatching status even before a live lifecycle card is emitted. Do not turn that state into a verifier timeout; wait separately for terminal provider truth.
- WebUI slash-command parsing must be exact-token only. Absolute path-leading
  user text such as `/Volumes/...` is a valid task prompt and must fall through
  to normal `SubmitUserInput` unless the first token exactly matches a known
  command such as `/new`, `/task`, or `/settings`. Online proof for submit
  observability must show the submitted text immediately visible and, on ADP
  timeout/error, both command status and turn status rendered as unknown/refresh
  needed instead of stale `dispatching...`.
- Launchd restart verification must allow enough time for wrapper startup, binary replacement, preflight, and daemon health. Prefer configurable waits over fixed short windows, and after any verifier early-failure path confirm fixture env markers were restored.
- For provider recovery logic, classify errors as recoverable, unrecoverable, or periodic-recoverable. Periodic windows use provider-supplied seconds first, otherwise configured defaults.
- Provider primary/backup failover proof must bind the primary error code, error-center recovery `failover_provider`, `provider.failover_from` / `provider.failover_to`, fallback model, final turn terminal state, and ADP error-query projection. Always restore and re-query the active profile after proof.
- A controlled online primary fixture may make failover deterministic, but the evidence boundary must stay explicit: name the fixture-backed primary, prove the real fallback request, lock the fixture request count, stop only the fixture's explicit process, and restore config/profile. Never report a controlled fixture 402 as a real upstream provider 402.
- A WebUI Settings verifier must edit the currently selected primary provider when a configured fallback exists. It must not make the fallback provider primary while leaving the same provider configured as fallback, because config truth correctly rejects identical primary and fallback routes.
- Current provider failover ownership covers non-stream requests only. Do not enable stream failover until partial output, tool-call side effects, rollback, and resume have one typed contract and positive/negative lifecycle coverage.
- For reason-turn stop logic, validate completion schema before terminal acceptance. Reject and explain invalid terminal submissions.
- UI protocol black-box tests must cover standard user-visible flows, not only internal event wiring.
- `cargo test --workspace` is the regression umbrella and must carry white-box plus module/project black-box coverage as those tests are added.
- When tests are added, changed, or found incomplete, update the module's test-design record in the same change set.
- When request/response/error mainlines or shared function usage change, update the function-map doc in the same change set.
- When migrated mainline-call truth changes, update `docs/mainline-calls/**` and regenerate `docs/wiki/**` in the same change set.
- When adding or editing a migrated feature, keep the mainline JSON path and its internal `function_map_doc`, `test_design_doc`, `generated_wiki_doc`, and feature-map links canonical or the workspace gate must fail.
- When adding or editing a migrated call table, keep every `bound` row tied to real file paths and resolvable symbols; do not use prose such as "handler" as a symbol path.
- Run `cargo run -p xtask -- mainlines generate` and `cargo run -p xtask -- mainlines check` sequentially, not in parallel; both touch the generated wiki surface and parallel execution can create false out-of-date failures.
- When tool surface or tool execution truth changes, update tool design, function map, test design, and runtime exposure checks in the same change set.
- When `tool.registry` changes affect live provider exposure, run both owner/workspace gates and one real config-selected `reason-live` smoke when credentials are available; selected-agent bootstrap still requires the configured pair-token env even for CLI live-turn verification.
- When context-segment admission, cache-shape policy, or subagent context flow changes, update `reason.context-planner` design, test design, function map, and memory in the same task.
- For live context-distribution claims, inspect persisted `planned_context.ordered_segments` and diagnostics from the reason ledger, not only unit tests. Prove stable/session-stable prefix shape, stable prefix hash, tool schema hash, and volatile/no-cache tail placement across at least one multi-round S-profile sample before claiming closure.
- For schema-polishing proof, do not rely on prompt-only steering against a live model. A real provider can obey the completion contract immediately or call unrelated tools, so deterministic schema-mismatch closeout needs a provider fixture, mock executor, or injected first invalid response plus no-tool contamination checks.
- For Master workspace-boundary changes, lock all direct Master workspace and checkpoint authority to canonical `runtime_home` (`~/.freehand`). Keep external session CWD as routing truth only: workspace or shell execution outside runtime home must return a paired failed tool result with explicit `task`/Worker guidance, while the framework-scoped `task` tool remains available for external `target_cwd` delegation. Online proof must submit a real external-CWD request, show no external content leakage, query the same Task Center task/history truth, and distinguish delegation from actual Worker completion.

## Memory Workflow

- Record exploration in `note.md`.
- Promote only verified, durable conclusions into `MEMORY.md`.
- Keep `CACHE.md` short and current for the next session.
- If feature truth changed, update resource map, function map, architecture docs, skill workflow, and memory files in the same task.

## Closure Checklist

Use this checklist for both new features and bug fixes:

- information sufficient
- logic closed-loop
- lifecycle management complete
- owner and function map updated if truth changed
- function-map call table and symbol binding still match code
- metadata/request isolation still holds for cross-module calls
- test-design record updated and still matches implementation
- runtime/debug evidence path still valid

If any line is not true, do not claim completion.
