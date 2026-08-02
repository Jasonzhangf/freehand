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
- WebUI Agent resource count is config-owned topology, not local UI state. Route edits through `UpdateAgentResourceConfig` -> runtime -> `config.core::update_agent_resource_config_in_path`; valid count is `1..=5`, current mode is shared provider via the first Worker template, and saved changes require daemon/Worker restart before AgentBoard can show live resources.
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
- Truth-owner crates must not depend on `crates/freehand-ui-protocol` for internal state. If a truth owner needs UI visibility, create owner-local projection types that preserve the real payload semantics, then bridge to `freehand-ui-protocol` in runtime/app wiring. Do not replace payloads with summary counts to satisfy a dependency gate.
- UI code must consume `crates/freehand-ui-protocol`, never provider crates directly.
- UI code must not classify tool calls from raw names, arguments, or result strings; tool display semantics belong in the `tool.display` pure parser owner and must flow through `freehand-ui-protocol`.
- UI code must not implement session CRUD as local browser state. Session create/rename/archive/restore/delete must enter `ui.protocol`, route through `runtime.ui-command-dispatch`, and persist through `reason.persistence` session metadata truth.
- UI app boundaries must stay protocol-only: they may render `freehand-ui-protocol` truth and shared contracts, but must not import `freehand-reason`, provider crates, node semantics, or config semantics for UI behavior.
- Any UI is an input ingress plus a read-only consumer of turn/debug state. UI may submit commands, but UI must not directly mutate reason truth, debug truth, or session truth.
- First version UI scope is CLI plus WebUI.
- WebUI default control/status transport is ADP WebSocket `/adp`; HTTP query plus SSE subscribe remains compatibility/static-page support. Do not mix either UI transport with node WebSocket pairing semantics.
- Daemon control/status automation is ADP WebSocket at `/adp`; WebUI, Android, CLI, and headless tests should converge on ADP command/query/subscribe frames for unified state inspection before relying on DOM-specific diagnosis.
- ADP is internal transport terminology. WebUI/Android user-facing labels, status text, failure cards, and diagnostic prompts must say connection/service/request/conversation, not ADP; ADP may appear in code symbols, docs, CLI/test output, and debug-only surfaces.
- Public ADP generated artifacts served to WebUI must not expose internal owner module paths such as `target_owner_module` or `crates/freehand-*`; keep crate-path routing metadata internal to dispatch envelopes and lock public artifacts with `xtask gates check`; removing a public ADP field requires a protocol-version/capability bump, not an in-place v1 shape change.
- WebUI localization is visible-text only. Do not translate JavaScript/Rust symbols, DOM ids/classes/data attributes, ADP variant strings, protocol field names, route names, asset filenames, or verifier selectors. After Chinese UI edits, run an audit for CJK inside known internal identifiers before build/gates, and keep asset version bumps plus browser/Android proof tied to the changed visible surface.
- UI command receipt rendering must map only known dispatch status codes to user-safe text through an exact whitelist. Strip only parameter suffixes such as `:` payloads or whitespace detail first; do not use substring classification such as `contains("task_")`. Unknown dispatch statuses are explicit unsupported-receipt errors, not success fallbacks, and user-visible text must not include `target_feature_id`, task ids, execution ids, control ids, or raw owner routing strings.
- WebUI selected-session transcript rendering must preserve protocol/session transcript order and append-or-replace the latest same-session turn; do not sort visible cards by `runtime-turn-*` ordinal because ordinals can reset after restart or recovery.
- WebUI lifecycle animation must be scoped to current live turn render projection only; historical turn/tool rows must remain static even when they still carry protocol model_request or tool status fields.
- WebUI live tool/model activity must render every in-flight round, not only the terminal summary. Fixed-session online verifiers must scope DOM and ADP assertions to the current run marker plus newly added turn ids; never use whole-transcript text matches, and never put the exact final marker text in the user prompt. Terminal DOM acceptance is a hard gate: selected terminal status, visible completed/blocked/failed status text, zero `[data-live="true"]` rows, and no residual dispatching/tool-running/model-waiting label must all pass. ADP terminal truth alone does not prove the browser cleared stale live state, so verifiers must fail rather than merely report stale live rows. After any live verifier failure, inspect failure metadata, exact `master.active-work` truth, and post-`finally` S-profile config/env separately; a mid-run summary can still show fixture config, and a live daemon-owned active-work checkpoint is not stale until the owning process exits or service-scoped restart completes.
- WebUI live request-cycle rendering is ordered by cycle truth, not append order. A local pending or accepted submit receipt must appear after older history but before the matching same-session live/tool/model turn. Pending/accepted receipt updates use `submit:<session_id>:<submit_id>`, but every real protocol turn uses `turn:<session_id>:<turn_id>` even when multiple provider/model rounds share one submit id. The browser DOM verification unit is `.turn-cycle-card`: one parent cycle card per request/model round, with child `.chat-message` rows for user, model waiting, tool calls, tool results, and final status. Current live cycle cards may update in place through keyed reconciliation; only terminal cycles are frozen by reusing the existing DOM node. Never fix a visible order bug by hiding tool rows, waiting for final summary, appending pending input after all protocol turns, full-clearing/rebuilding the transcript on every refresh, or using summary-only rendering.
- WebUI waiting/retry observability must identify the owning session and turn on persistent navigation/status surfaces. Consume `SessionList.active_turn_id` plus selected transcript `model_request` truth; do not rely on AgentBoard alone for a live Master provider retry, and do not relabel a parent Master retry as Worker-owned when a child Worker session is selected.
- For `scripts/verify-webui-live-tool-render-online.mjs`, read proof from the stage-specific snapshots: `duringTool` proves the live tool card, `duringContinuation` proves the tool-result-to-model-waiting card, and `finalState` proves terminal freeze/live cleanup. `serviceTurn` is an early pre-tool snapshot and may still show dispatching/live state; do not quote it as final failure after the script passes.
- WebUI session-list truth is the render gate after it has loaded. Latest-active query, latest-turn ADP/SSE updates, and selected-session transcript projections may render only when the session id is listed, current draft, or current pending-submit; non-destructive `DeleteSession` can leave old turn truth queryable, so never use latest-active as a fallback after session-list truth exists.
- Mobile home session navigation is the page surface, not a drawer or overlay. The first mobile page must render two in-flow sections: `正在运行` contains every live/retry/lifecycle-wakeable session without a fixed cap, `历史会话` contains persisted non-running or user-needed sessions ordered by display time, and the two lists must have disjoint session ids. Header session relationship panels may be inline expanders for the selected session only; they must not cover or replace the home session list.
- Before changing mobile/WebUI navigation or phone dashboards, read and update `docs/design/mobile-webui-ui-tree.md` and its manifest first. Do not patch individual UI symptoms until the route tree says which page owns the surface. `Home` and `SessionDetail(session_id)` are mutually exclusive on phone: after a session is selected, the global `正在运行` / `历史会话` dashboard must leave the body; selected-session Agent/task state belongs only in the Header/Agent sheet. `等待用户选择` is not `正在运行` unless owner truth has an open lifecycle that can wake without user input.
- WebUI surface registration must have one importable runtime registry consumed by both bootstrap and the architecture gate. Do not maintain a verifier-local surface-name list or prove registration with source-text matches. Validate immutable contracts, exact registry-key identity, non-empty typed fields and arrays, and unique surface/DOM-root identities. Online shared-state proof must drive deterministic loading/empty/error/confirmation projection through the served production modules in an isolated verifier-owned container; ambient Home data is not a stable empty-state fixture. Pass one expected asset version into page-side collection instead of hardcoding a second version inside browser evaluation.
- Mobile Home session management is selection-oriented: Home rows may expose multi-select and owner-backed delete/remove, but row-level rename is forbidden. Rename belongs only to the current selected `SessionDetail(session_id)` header and must dispatch `RenameSession` for that selected persisted session; verifiers must fail if Home renders inline rename controls or `home.rename_session` edges return.
- WebUI Search/list owner truth includes reason-owned metadata-only sessions created by `CreateSession`, not only persisted index rows with turns. Online Search verifiers must create or reuse one fixed metadata-only session and prove `QuerySessionSearch` returns it; Worker `worker-task-*` matches may only appear as nested child rows through TaskBoard parent truth, never as global top-level session results.
- WebUI New/session online verifiers must not generate random test-session spam. Use a production-inert browser test hook guarded by an explicit enable flag, create or reuse fixed conversation/task ids through the real New dialog, and wait for `QuerySessionList` owner truth after `CreateSession` before asserting UI selection, cwd projection, or persisted-session state. The hook must not alter normal production id generation.
- WebUI attachment failure-retention proof must use a real browser file input and a deterministic transport failure. Assert selected-pool truth from tray/thumb/remove rows, count text from command/status surfaces if that is where production UI renders it, and after failure prove selected session, cwd, pending prompt, visible pending card, and draft attachment remain available for retry.
- WebUI live transcript scrolling must preserve the operator's manual read position. Render updates may auto-follow only when the conversation scroll host is already near bottom or a local submit explicitly forces the new turn into view; ordinary render updates must not call `scrollIntoView`. Online proof must cover both "scrolled up stays put" and "bottom-pinned latest row remains visible above composer".
- WebUI fixed/sticky composer clearance must come from the measured composer height (`--composer-clearance`), not fixed mobile padding guesses. Online proof must compare the latest message rect with the composer rect.
- WebUI submit timeout/transport failure is ambiguous. Before rendering unknown-dispatch, refresh owner truth for sessions, selected transcript, latest turn, and lifecycle projections; if the submitted user text materialized, clear pending state and continue from refreshed conversation truth. If it is still unknown, do not clear selected/draft session, pending user input, or draft attachments; keep a visible pending card with unknown-dispatch status and instruct refresh before duplicate send. Online proof should force a deterministic WebSocket/offline failure from a draft session and assert the DOM is not `no sessions` / empty conversation.
- Internal framework sessions such as `master-lifecycle-*`, `master-timer-*`, and `worker-task-*` are owner/debug truth, not top-level user conversations. User-facing global session lists must be protocol-owned persisted session metadata only. Keep direct transcript/debug query paths available. WebUI may render `worker-task-*` only as indented temporary child rows under the owning persisted master session, using TaskBoard `parent_session_id` owner truth; do not make WebUI guess top-level visibility by id locally.
- Worker `worker-task-*` transcript projection belongs to reason/runtime truth, not browser cleanup. `ReasonPersistence::restore_turn_snapshots_for_ui` must return exact per-round UI snapshots for `runtime-turn-N` / `runtime-turn-N-rM`; complete authoritative snapshots avoid huge ledger replay, but incomplete authoritative snapshots must be backfilled from reason-ledger truth on selected `QuerySessionTurns` instead of collapsing to the final summary. Daemon bootstrap must use authoritative-only UI restore and must not parse every historical reason ledger. `QuerySessionTurns` must hide Worker task/continuation prompts from `user_text` while preserving Worker source, final/tool truth, and errors. WebUI may show loading/error for slow queries, but must not trim semantic payloads or hide repeated rows after DOM construction.
- Rollback/effective transcript fixes must also inspect provider-visible
  `SessionHistory.base_context_segments`. If UI/TaskBoard says a parent turn was
  rolled back or replaced but the next Master provider request still sees stale
  Worker/blocked state, check `historical_turn:*` session-memory in
  `session-history.json`. The owner fix belongs in `reason.persistence` restore
  filtering, not WebUI hiding, TaskBoard cleanup, or verifier prompt changes.
- WebUI lifecycle dashboards must scope TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl rows to the selected parent session via TaskBoard `parent_session_id` / task ids. A selected session with no child tasks must render empty current lifecycle state; never fall back to global Task Center history because that leaks old blocked/review/test tasks into unrelated conversations.
- WebUI Master/Worker/session relationship UI must be locked by documented protocol schema and resource contracts, not text or DOM guessing. Use persisted session metadata plus `UiTaskSnapshotProjection.parent_session_id`, `attached_session_ids`, `worker_session_id`, and `task_id`; DOM `data-*` anchors are verification projections only. Do not infer relationships from labels, `worker-task-*` prefixes, card order, or debug text. Online verifiers must also fail on missing `worker_session_id` instead of synthesizing `worker-task-*` ids.
- Mobile multi-Agent navigation is Header -> current-session Worker child tasks -> canonical Worker transcript, with an explicit return to the exact parent Master and direct sibling-Worker switching. Task tap must clear the previous Agent transcript before querying the TaskBoard-projected canonical `worker_session_id`; never leave a Master transcript visible under a Worker selection. The Agents runtime sheet is read-only navigation/progress and must not own configured Worker-capacity mutation; place that owner-backed control only in system Config. Do not render the Phase 2D inspector/history/control dashboard as the first mobile Agent sheet, synthesize `worker-task-*` ids in WebUI, or accept an empty Worker transcript when persisted Worker turns exist. Online proof must cover enter, exit, sibling switch, and transcript identity on the real daemon and Android WebView.
- For multi-child current-session inspection, enumerate TaskBoard rows by the selected parent session's `parent_session_id` / `attached_session_ids`, render every current child task without a fixed cap, and use the TaskBoard-projected `worker_session_id` for transcript checks. `scripts/verify-worker-subtasks-online.py --parent-session <session_id> --include-terminal --require-transcript` is the read-only ADP checker for inspecting every child task one by one; do not use global Task Center history, list truncation, or browser-synthesized `worker-task-*` ids as evidence.
- Header multi-Worker status belongs to the selected-session Header, not Home or an overlay. Render one compact row per TaskBoard-projected child task with Worker-only ordinal, owner status, owner-timestamp duration, and presentation-only expand details; nonterminal rows must refresh TaskBoard owner truth while clocks tick, and opening a Worker transcript must remain a separate action using `UiTaskSnapshotProjection.worker_session_id`.
- Master/Worker dispatch wait UI must mirror runtime owner truth: Worker transcripts are context-isolated from the Master until TaskBoard/TaskHistory/EventInbox returns typed child result truth; success/failure/blocked/interrupted outcomes are rigid parent-visible notifications; the selected Master composer stays usable while open children or source timers can wake lifecycle; periodic checks belong to runtime Timer/TaskBoard truth, not browser-synthesized timers.
- WebUI Master/Worker path-diagnostic proof is not closed by a Worker blocked transcript alone. Verify the current task id end-to-end: parent session renders `ToolPending` as `waiting lifecycle` / `Lifecycle running`, Worker detail renders `blocked` at assistant/final/bottom status, Master lifecycle makes a real task call with `{"op":"append",...}` that produces `TaskProgressed`, internal Worker prompts stay hidden, and global session list has no top-level `worker-task-*`.
- ADP WebSocket is UI/control/status transport, not node master/slave pairing transport; keep node pairing WebSocket semantics separate.
- Command ingress must stay split from query/subscribe routes. Query/subscribe commands are not valid command-ingress payloads and must be rejected explicitly.
- Before a UI command leaves `freehand-ui-protocol`, it must be wrapped in a protocol-owned owner-routing envelope; app boundaries must not invent their own command-to-owner routing.
- Runtime-backed command execution belongs in `freehand-runtime` or another explicit runtime owner crate, not in UI app crates.
- Protocol-only async transports must still respect runtime execution boundaries: if injected runtime dispatch performs synchronous provider/live work, call it through an explicit blocking boundary such as `tokio::task::spawn_blocking` instead of executing it inline on the async handler thread.
- Config-selected runtime host bootstrap should also prefer `freehand-runtime`; host apps should stay thin and must not reimplement config-selection-to-runtime wiring.
- Remote daemon / relay integration starts at config-owned `remote_daemon_registry`: account, daemon, endpoint candidates, route scoring, and QR/deep-link bootstrap belong to `config.core`; Android may import the selected bootstrap and load the daemon-hosted WebUI endpoint, but must not own account-directory truth, route scoring, Tailscale probing, or relay tunnel/pass-through semantics. Runtime directory/presence projection belongs to node-owned `remote_daemon_directory` through `RemoteDaemonDirectory::publish_registry` / `resolve_route`; a relay WebUI URL or relay host id is only an endpoint candidate until relay signaling/tunnel/pass-through IO is actually implemented and online-verified.
- CLI and WebUI may render different views, but they must share one `freehand-ui-protocol` truth.
- Android client work uses the same rule set: `apps/freehand-android` is only a thin WebView/platform bridge, and the daemon-hosted WebUI at `/?client=android-webview` is the only product UI. `bridge.html`, native Android conversation/settings/update UI, Android ADP/SSE/command projectors, and `/mock/android` are dead semantics and must not be restored. True-device Android acceptance requires canonical WebUI evidence (`FreehandWebUiLayout`, `data-webui-shell=true`, `layoutClient=android-webview`) plus screenshot; native fallback screenshots are failures.
- Android APK update Settings work must keep one owner split: WebUI may render only the Android-only bridge trigger/status card and call `window.FreehandAndroidApkUpdate.check()`, while `AndroidApkUpdater` owns manifest comparison, cache download, duplicate-check blocking, and FileProvider installer handoff. Bump the explicit WebUI asset version whenever the Android-served WebUI JS changes. Update availability is decided only by daemon manifest `versionCode` being higher than the installed package; release/staging must prove `dist/android/update.json`, runtime-home APK, and relay-served APK match the signed built artifact before diagnosing Android bridge logic. Never stage `*-unsigned.apk` for phone update: verify with `apksigner` and keep the signer compatible with the currently installed package. Local CDP bridge simulation is useful evidence, but true-device closure still requires installed app Settings click, installed package version/signature evidence, `FreehandApkUpdate` logcat/status evidence, and Android system installer or current-version proof.
- Android file-access permission prompts must be centralized at startup after each package install/update marker, before WebView/file actions. Use package `lastUpdateTime` rather than `versionCode` so same-version reinstall is a new prompt cycle; do not prompt again on ordinary starts within the same marker. Android 11+ `MANAGE_EXTERNAL_STORAGE` still requires package-scoped system-settings confirmation, and true-device acceptance must capture `FreehandFileAccess` rows with install marker plus granted/restricted state.
- Image attachment payload is current-turn-only: WebUI may send image base64 through `SubmitUserInput.metadata.attachments`, runtime may map it into provider-neutral current request attachments, and provider adapters may render protocol wire; reason/session/UI history stores metadata only. Online proof must assert raw base64 is absent from restored transcript/history and that adapter wire does not leak attachment id/name.
- Android turn-finish notification closure requires a live WebUI nonterminal-to-terminal transition, not a restored terminal snapshot. WebUI owns only the Android bridge call; Android owns `POST_NOTIFICATIONS`, channel, notification post, tap-return intent, de-dupe, and `FreehandNotification` logcat truth. True-device proof must include installed APK, permission/post evidence, `dumpsys notification`, and tap-return or a clearly stated ADB/device blocker.
- True-device Android WebView proof must include at least one real interaction when the changed surface is interactive. For mobile drawers/settings/sheets, prove the visible close path after scrolling and Android physical Back behavior; a loaded page plus no fatal logcat is not enough to claim the UI is usable.
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
- Every script that owns nested/foreign Git checkout commands must clear `unset $(git rev-parse --local-env-vars)` at its own entrypoint. Clearing only one caller such as pre-commit is insufficient because pre-push, Make, CI, or direct invocation can inherit the outer repository environment and make `git -C` resolve Freehand instead of the foreign checkout.
- Migration source exclusions apply to every blob recursively resolved from a declared source ancestor. Checking only the manifest's ancestor string lets excluded BrowserUse/Cookie/Profile/Takeover descendants enter source truth.
- OpenMinis lifecycle evidence uses one verifier-report admission path for repository, WebUI, Android-device, and legacy no-touch gates. Online/device/no-touch reports additionally require a valid Ed25519 signature from the externally held runner key pinned in the verifier; repository-local JSON, digest, assertions, and source cleanliness alone are forgeable and cannot promote lifecycle truth. Do not reject a required gate by id: verify its external signature plus code-locked command/proof/verifier identity, assertions, process result, report digest, source commit/tree, and clean source-attestation boundary.
- Static Rust migration call discovery must preserve recursive self-edges, imported function aliases and lexical globs, callable/block-local imports and value-item shadowing, Rust lexical receiver scope, typed closure parameters and container item identity, grouped self imports, supported smart-pointer autoderef, supported built-in container item types, named struct-pattern field types, and inherited trait-default dispatch. Resolve aliases and module/callable/block lexical globs, including qualified paths, before bare-name filtering; fail multiple real candidates as ambiguous; restore nested imports and local callable items with their block. Every destructured `let`, closure, loop, match-arm, direct/chained if-let, and while-let pattern binding replaces same-named outer receiver and iterable-item state even when its type is unresolved; evaluate initializers/scrutinees before installing the new scope, project named struct fields through the canonical field index, and clear/restore loop iterable metadata. Resolve external `#[cfg(test)] mod ...;` identity from module ancestry before indexing callables so file order cannot change truth. Unknown receivers that can match a repository-local method fail closed in production and tests, while syntactically typed built-in receivers and bindings rooted in known external method chains or iterators remain external. Parse cfg predicates structurally and discover production and test projections independently; process only file modules reachable through declarations active in that projection before merging all production truth with test-only definitions/callers. Never put cfg-exclusive external modules, imports, or same-name definitions into one scope. Traverse statements, nested expressions, and match arms in their active projection, and classify test-only only when removing `test` makes the active predicate false; `not(test)` remains production and `any(test, active_target)` is production, not test-only. Do not suppress all identifier receivers, treat every untyped binding as local, assign a container type to all destructured fields, record cfg-disabled subtree calls, or resolve callable parameters/local values as same-named module functions.
- Function-local Rust items need two explicit scopes: a callable-qualified declaration scope for stable impl/trait/type identity, and the real enclosing module for resolving unqualified calls inside their method bodies. Apply cfg projection before any local import, type, field, impl, trait, or method enters the call index; lexical visitor filtering after indexing is too late and permits phantom ambiguity or edges.
- OpenMinis migration topology is entrypoint-owned: every manifest node must be forward-reachable from `foundation.root`. Nodes at `contract_ready` or later, including verification-blocked nodes, must bind the exact non-empty set of incident manifest edge ids in `route_edge_ids`; missing, extra, unrelated, or unknown route ids fail lifecycle admission.
- When splitting call-discovery files for size, keep canonical state types in their original owner module and move bounded implementation bodies behind thin wrappers; moving the type into a child module changes every method symbol identity and creates false machine-call-map churn.
- New features and bug fixes both require lifecycle thinking, not just local code patches.
- In provider work, preserve raw provider events in debug mode and rely on unified semantic events for normal operation.
- In provider work, read local official protocol snapshots under `docs/references/provider-protocols/` before inventing wire behavior.
- Provider retry/failover that later recovers is transient same-turn `UiModelRequestActivity.transport`; publish it before sleeping, retrying, or entering fallback, then let the next semantic response clear it. Do not materialize intermediate retry/failover as `turn.error_events`, a persistent WebUI Error card, a separate reason turn, or a standalone `Provider` reasoning-flow row.
- In OpenAI-compatible adapters, JSON `error:null` is absent, not failure evidence. Only non-null wire error objects may emit provider error semantics.
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
- Worker `glob` is locked-workspace scoped, not relative-only: accept relative patterns, expand leading `~`, and accept absolute patterns only when they remain under the canonical locked workspace after symlink/canonical path handling; reject `..` and external absolute patterns. If online samples show repeated path failures inside the Worker `target_cwd`, fix owner semantics/tests instead of only adding prose.
- Treat user-facing symlink paths as first-class path truth. Worker path tools must absolute-normalize relative paths against the locked workspace, expand leading `~`, canonicalize symlink aliases before workspace-boundary decisions, and return `path_diagnostic` on failures with requested path, absolute path, nearest existing/canonical parent, missing suffix, and symlink ancestors. Positive tests should cover `glob`, `grep`, `read_file`, `ls`, and `write_file` through an absolute symlink alias that resolves inside the locked task cwd, plus missing-leaf diagnostics under a symlink parent.
- Path-tool online proof must inspect the real Worker provider loop, not only
  final model text. Force a Worker tool call such as `ls` against the
  user-facing symlink/missing path, then verify the next provider request
  contains the paired failed `tool_result` with full `path_diagnostic`
  requested/absolute/nearest/canonical/missing-suffix/symlink fields.
- Worker file probes should guide the model away from avoidable failures: `ls` can list directories or report one file entry for existence checks; `read_file` is only for existing UTF-8 files and should not be used on directories, generated files that do not exist yet, or binary sidecars like `.DS_Store`.
- Master provider context must carry current framework behavior and Task Center truth before the model decides to call tools. Inject and test `TaskSpaceSnapshot` with configured Worker, valid status filters, known tasks, agents, and recent events so the model does not spend turns probing `list_agents`, `list_tasks`, or history just to understand the framework.
- Master live provider tool surface includes local workspace tools, concrete-URL network fetch, and framework tools: expose locked-cwd `ls`/`read_file`/`grep`/`glob`/`write_file`/`edit_file`/`multi_edit`/`delete_range`, `web_fetch`, plus `task` and `timer`; still exclude shell, browser, broad `web_search`, `todo_write`, and `complete_step`. If the current selected session cwd is the requested local workspace, Master should use local tools directly instead of dispatching. Use `web_fetch` directly for known HTTP/HTTPS URLs. Worker owns different-cwd, isolated, concurrent, long-running, resumable, or independent capability-matched work through task `target_cwd`; Master context must expose configured Worker tool capabilities, and Master should dispatch to a Worker instead of blocking when the Worker surface has the needed capability. Cross-cwd or unavailable-tool Master calls must return paired failed tool results with exact local-vs-dispatch guidance and no file-content leak.
- Task tool guidance must include concrete argument shape, not only semantic prose. Lock top-level `op` in schema/tests/error text; show create/assign examples; prefer expanded absolute existing repository/workspace `target_cwd`, but keep leading-~/symlink aliases first-class when they resolve to an existing workspace. Task create/preflight diagnostics should report requested, expanded, nearest existing/canonical parent, symlink ancestors, and missing suffix before asking the user to clarify a path; still reject glob, broad-search, or output-directory targets.
- Master dispatch is lifecycle progress, not user-task completion. If the Master only creates/assigns Worker work or schedules a timer while the user objective still depends on future Task Center/timer truth, the completion schema must use `claim="waiting"` and UI must project `TerminalStatus::ToolPending` as lifecycle/running, not `Final`/completed.
- Agent reasoning continuation is framework-owned for both Master and Worker. The runtime reason loop validates completion schema, pairs every tool result with the pending tool call, appends history, and sends the next provider request until a valid terminal boundary. Master must not manually drive, resume, or emulate Worker reasoning; it consumes TaskBoard, AgentBoard, EventInbox, and review truth to make task-level decisions only.
- Normal reasoning must not stop after a tool call or a non-terminal provider response. Only explicit framework boundaries such as provider failure after governed retries, hard block, schema/loop exhaustion, cancellation, or a valid completion schema may stop the loop. Preserve the same rule for Master and Worker instead of adding role-specific continuation paths.
- Master/Worker lifecycle closure is per owner resource. TaskRuntime must own task/execution success, blocked, interrupted, rejected, approved, and closed truth; AgentLifecycle must own Worker process alive/restart/current binding truth; Master loop state must own attention retry/backoff and cursor truth; reason persistence must own parent session waiting/final turn truth. Do not claim lifecycle closure from UI text alone.
- Crash/restart lifecycle closure must be proven from durable owner truth, not a new user message. Cover at least: Master exits after EventInbox admission before provider decision (`pending_attention`, no retry state, attempt 0 resumes), Master provider/system failure before restart (`retry_event_id`/`retry_attempt` resumes attempt N), foreground Master dead owner (`master_work` + reason active-turn bootstrap recovery), and Worker exits while `Running` (expired lease -> `TaskInterrupted` -> Master reassignment of the same task).
- Master supervision uses TaskHistory plus AgentBoard truth. If a Worker dies, restarts, or loses heartbeat, the Master must treat that as resource state and decide same-task retry, cross-Worker takeover, or explicit blocked decision through Task Center mutation; a Worker process state by itself never completes user work.
- Do not rely on prompt guidance alone for parent/child lifecycle closure. Master user-session `claim="complete"` must be runtime-gated against Task Center child truth: reject while any same-parent child is actionable or unresolved (`Created`, `WaitingAgent`, `Assigned`, `Running`, `Interrupted`, `Paused`, `Blocked`, `ReviewSubmitted`, `Approved`, or `Rejected`), but do not let terminal historical children (`Cancelled`, `Failed`, `Closed`) keep a parent session in stale waiting. Never implement this gate as `status != Closed`; add positive and negative live regressions.
- Parent workset closure is by logical Master turn, not exact repair/tool-result round. Child tasks created by `runtime-turn-N`, `runtime-turn-N-r2`, and later rounds of the same user request must be grouped together; the first closed exact-round child must not start parent evaluation while same-logical-turn siblings remain open.
- Parent next-round evaluation context must include same-objective prior closed child review truth. The idempotency boundary stays the current closed workset, but prompt context for Master-created next-round tasks must include prior accepted child truth since the latest external user objective and exclude older user-turn child truth. A final integration-only prompt is a lifecycle bug because the parent cannot prove alpha/beta/gamma plus integration closure.
- Internal timing/wakeup capability is a standard `timer` framework tool, not task truth. Do not encode wait/schedule semantics as task input `{"op":"wait"}`, task notes, or task lifecycle state; `freehand-tools` owns the schema, and `freehand-runtime` owns durable timer state under `~/.freehand/state/timers` plus timer ledgers under `~/.freehand/ledgers/timers`.
- Timer wakeups must persist the wakeup prompt. When due with source-session truth, runtime injects that prompt as a new next-ordinal turn into the original session; it never reopens/resumes the old turn. Source-less timers use an internal turn. Relative, absolute, local-time daily/weekly, and local-time 5-field cron semantics stay independent from Task Center truth; Worker tool surfaces must not expose `timer`.
- If the next useful Master wait exceeds 3 minutes, the model-visible guidance must require timer input `{"op":"schedule",...}` instead of dead-waiting in the current turn. After scheduling, Master should continue other ready Master-side work. The persisted timer prompt must say what current truth to inspect, what waited condition to revisit, and what decision to make.
- Do not accept verbal timer claims as proof. Master may say a timer was
  scheduled only after the `timer` tool returns `Timer scheduled` in that turn;
  otherwise the wakeup is not durable truth and must be fixed in guidance/tests.
- Timer online verifiers must be timer-specific. Do not reuse generic
  `adp-turn-sample --sample success`: its fixed transcript evidence can fail
  before due handling and trigger fixture restoration, letting the real provider
  handle the wakeup. A valid timer proof must keep the fixture alive, require
  post-schedule `claim="waiting"` / `TerminalStatus::ToolPending`, then prove
  the due/restart-due follow-up by mock provider request, same source session
  turn count, timer state, and timer ledger truth before restoring config.
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

## OpenMinis UI Migration Gate Workflow

- Treat `docs/migrations/openminis-ui/ui-tree.manifest.json` as lifecycle truth only after `cargo run -p xtask -- gates check`; `design_baseline` and blocked nodes are not implementation truth.
- Source evidence must come from the deterministic `external/OpenMinis` checkout at the manifest/CI SHA. Fresh local clones provision the missing checkout only through `scripts/provision-openminis-source.sh`, which uses the canonical upstream, sparse source paths, an exact-SHA fetch, and one atomic owner-bearing first-run lock; concurrent waiters verify the winner, stale same-host owners are reclaimed only after PID liveness fails, and each process cleans only its own staging/lock artifacts. Validate the checkout root through Git rather than assuming `.git` is a directory, because worktrees and submodules use a `.git` file. An existing checkout with origin, HEAD, or dirty-state drift fails without mutation. The Rust gate stays read-only and never fetches. Inspect pinned blobs recursively with `git ls-tree -r --name-only <sha> -- <path>` and `git show <sha>:<blob>`; never read an unpinned working-tree file as proof.
- Static Rust call discovery must cfg-filter block-level `use`, `const`, and `static` items before they enter lexical scope. Explicit enum discriminants use deterministic `<Enum>::<Variant>::__discriminant_initializer` caller identities and obey the same production/test cfg projection as other initializers.
- Promote nodes one state at a time. `owner_mapped` does not need target symbols; every promoted state must exactly match one bound resource operation's owner, single source, target, and allowed-direct relation; contract states need protocol/surface fields; `source_bound` needs canonical repository-relative, symlink-free target/mainline paths, real target symbols, and exact bound mainline symbol segments.
- `online_verified` evidence must be a JSON attestation under `docs/migrations/openminis-ui/evidence/` matching node, gate, code-locked canonical command, proof kind, passed result, and online run id, and must bind a distinct `freehand.verifier-report.v1` report under that same root by canonical path plus exact SHA-256. Repository evidence must be generated by the non-recursive `cargo run -p xtask -- openminis-ui verify-node <node_id>` source-bound verifier, never by `xtask gates check` certifying itself. Validate verifier id, exact `node_id`/`migration_unit_id`, command, run id, successful process result, timestamps, required assertions, and one resolvable attested source commit/tree shared by all node reports, never from the attestation itself. The report attests the pre-promotion source revision: permit later changes only at its exact attestation/report paths plus `docs/migrations/openminis-ui/ui-tree.manifest.json`, independently validate the current manifest through the full lifecycle/binding/evidence/topology gate, and reject source/registry/verifier drift. `legacy_retired` additionally requires exactly one machine `legacy_scan_roots` row in the owning feature mainline, exact node/owner/path/removed-identity agreement, scan-root coverage of every bound target and removed path, registered physical path/symbol/import/caller disappearance, and dedicated online `legacy_touched=false` proof. Never let a node self-select an arbitrary scan root or fabricated removed identity; aggregate `migration_complete` requires every included node to be `legacy_retired`.
- OpenMinis migration call-map admission derives every direct production edge in `xtask/src/openminis_ui_migration/` without function-name filtering. Migration-owned callees retain legitimate non-test inbound callers; external callees retain only migration-owned direct callers, and outside-to-outside edges are forbidden. Callable-local associated paths resolve against callable scope. Block-local const/static initializers own deterministic independent callers and must not leak into the outer callable. Active nested impl/trait items and nested Rust functions are rejected explicitly until they have independent machine-registerable identities; cfg-disabled nested items, statements, expressions, and match arms must never enter graph truth.
- OpenMinis call-map discovery includes module and lexical `impl`/`trait` methods. Index inherent and trait-impl methods by receiver, use receiver/trait-qualified identities, resolve `Self`, imported receiver types in both call sites and `impl` receivers, plus statically typed receiver calls, and keep lexical item bodies independent from their enclosing function. Qualified UFCS calls must fail closed until explicitly modeled. Derive test ownership from Rust test/cfg attributes and ancestor modules, never `::tests::` spelling. A free-function-only `syn::Item`/`ExprCall` scan is incomplete truth.
- OpenMinis promoted target binding must retain the unique declaration occurrence file and require the accepting bound mainline row's exact canonical repository-relative `file_path` to equal it. Being under the same broad `target_paths` root is not a valid symbol-to-file binding.
- OpenMinis receiver inference must carry repository-local `Result`/`Option` success types through `?`, `unwrap`, and `expect`, including inline wrapper chains before a following local method call, and ancestor test-module identity must reach impl/trait methods. Reject local macro definitions and `include!` until expansion-aware call inspection exists; unexpanded bodies cannot satisfy exact edge truth.
- For promoted target directories, parse declarations only from supported Rust/Swift/JS/TS/Kotlin source files. Ignore non-source Android assets as declaration inputs, but still fail when a requested target symbol is absent or non-unique among supported sources.
- Human UI tree and machine manifest must register the same entrypoint, forward edge id/from/to/semantic, and return from/to/semantic sets; Mermaid parity alone is insufficient. Reject malformed edge/return rows and table rows under unknown registry headings instead of skipping them.
- OpenMinis shared-function truth is automatically derived from the `syn`-parsed Rust module/import graph; current target cfg comes from `rustc --print cfg`, the test graph is explicit, and inactive/unsupported cfg items must fail rather than satisfy active truth. Never add a tracked-symbol whitelist or merge bare names across modules. Keep the complete multi-reference set, module-qualified exact direct callers/tests, and one real module-qualified caller-to-callee edge per `openminis_ui_migration` mainline row aligned. Qualified re-exports, reassigned local receiver truth, and inline container `unwrap`/`expect` receiver chains are part of the exact call graph and must stay covered by red tests and doc sync.
- OpenMinis migration map paths are gated in every lifecycle state: canonical repository-relative paths only, direct children of the unique function/mainline/test map directories, identical feature-id sets exactly equal to `touched_feature_ids` and containing the owner, and exact document path/self identity. Never defer this validation until `source_bound`.
- Pinned Swift source symbols and promoted target symbols must resolve through `declarations.rs` as exactly one parser-owned declaration occurrence: Swift compiler parse AST, Rust `syn`, and JS/TS/Kotlin tree-sitter AST. Preserve file/scope/ordinal occurrences so same-file duplicates fail, while comments, string/regex literals, call sites, and longer identifiers do not satisfy source truth. Every workflow full gate must install the Swift toolchain before `make ci`; syntax-error trees, local-scope JS/TS/Kotlin declarations, and lexical keyword scans are forbidden for declaration truth.
- Parse only workflow `jobs.*.steps[]`; every `make ci` job must have the unique pinned checkout and Swift setup earlier in the same job. Top-level metadata and another job cannot satisfy the pin. For evidence and retirement, accept only canonical repository-relative paths; reject absolute, dot-segment, backslash, and repository-escaping paths.

## Validation Workflow

- Test design and test implementation must evolve together in the same task when feature truth changes.
- Function-map logic description and code binding must evolve together with implementation in the same task when feature truth changes.
- Do not add implementation without first making the test-design path inspectable in docs.
- Before claiming completion, run the feature's required checks.
- Before claiming completion, satisfy the feature's `lifecycle_checks`.
- After any code/config/doc change in this repo, do not report completion from local tests alone. If the feature has a live surface, verify the changed behavior online through ADP/WebUI/browser evidence before claiming the change works.
- For master/worker or multi-agent autonomy claims, command-driven task samples are not sufficient. The proof must include a `SubmitUserInput`-only headless path, reject direct CLI task mutation in the mock proof, drive model/provider task calls with strict top-level JSON `op` through a deterministic fixture or real provider, query transcript plus TaskBoard/AgentBoard/AgentLifecycle/TaskHistory truth, and verify the same task/execution/agent ids after S-profile restart.
- A daemon Slave Worker loop is synchronous blocking runtime work and must not run inline on the Tokio async host thread. Route the long-running Worker/provider service through one explicit `tokio::task::spawn_blocking` owner boundary. Lock both directions: a nested runtime can be created/dropped successfully inside the boundary, and a blocking-task panic/join failure becomes an explicit daemon error.
- Production Worker online closure must use a clean external target cwd and a real deliverable. Require Worker-owned `TaskResumed`, initial plus periodic `TaskHeartbeat`, `TaskReviewSubmitted`, a concrete artifact verified from disk, Master-owned approve/close, and same task/execution/agent history after explicit Worker restart. A created/assigned/interrupted task is red evidence.
- Master/Worker cwd delegation must distinguish Master orchestration from Worker execution. Master cannot directly read/search/write external repo paths; it creates/assigns Worker tasks with the correct existing `target_cwd`. Worker `target_cwd` is the current agent cwd/workspace root (`A`). External target path `B` is not automatically a new cwd. Worker read/query tools may inspect readable `B`; Worker write/edit/delete outside `A` must return a paired failed tool result that says the write target is outside current cwd and instructs the agent to report the correct target workspace cwd back to Master. Do not collapse boundary failures into "path missing".
- Unrestricted shell cannot be exposed to Worker provider turns until there is a real write-boundary sandbox. Worker provider tool surfaces must exclude `bash`; injected shell calls must return a paired failed tool result instead of executing.
- Do not invent `/workspace`, `/tmp`, or sibling output directories when the user supplied a repo path. Missing target paths must report exact original/expanded/canonicalization evidence rather than broad searching or path substitution.
- For real-provider master/worker claims, task creation and assignment alone are failure evidence, not partial success. Run `scripts/verify-real-provider-master-worker-history.sh --task <task_id>` against every real-provider-created worker task; any history that is empty or only `TaskCreated,TaskAssigned` means the production worker runner/scheduler did not execute and the claim must stay red.
- For development validation, prefer the symlink service profile: `scripts/install-launchd.sh installS` for first setup and `scripts/install-launchd.sh restartS` after rebuilds. `restartS` refreshes only the launchd debug daemon binary and health-checks the env-backed bind from `~/.freehand/daemonS.env`; it must not start, configure, register, or health-check Relay. Relay deployment and Agent outbound-tunnel lifecycle belong to `relay.transport` and use their own verification map. S-profile daemon remains fixed at `127.0.0.1:4042`, and the global release service remains on `127.0.0.1:4041`.
- Config/env-only online verifiers that temporarily mutate
  `~/.freehand/config.toml` or `~/.freehand/daemonS.env` must restore the
  original files and restart only the existing S service with
  `launchctl kickstart -k "gui/$(id -u)/com.freehand.daemonS"`. Do not call
  `scripts/install-launchd.sh restartS` from a verifier `finally` unless that
  verifier intentionally validates rebuild/install, because rebuilding in the
  restore path can hang before the daemon reloads restored config and leave the
  live S-profile on fixture/provider settings. After restore, query
  `freehand-cliS adp-config-query`, grep fixture env markers, and check no
  verifier-owned cargo/rustc chain remains.
- ADP `SubmitUserInput` online verifiers that are not testing instruction
  capability should pass a minimal verifier cwd such as
  `/tmp/freehand-<feature>-cwd`, not the Freehand repo root. Repo-root cwd loads
  local `AGENTS.md` and `.agents/skills` into the provider request and can hit
  the 30s instruction-capability guard before the behavior under test reaches
  the provider. If the verifier intentionally tests instruction capability, make
  that the explicit feature gate and assert the manifest/request segment.
- Android WebView proof is not closed by local `127.0.0.1:4042` served hashes. For phone-visible changes, verify the relay HTML asset version, relay-served JS contains the changed marker after path rewriting, relay ADP smoke passes, and true-device CDP/screenshot shows the current DOM. A terminal backend session with stale `[data-live="true"]` on the phone is a WebUI projection/cache/connection bug until the relay-loaded DOM proves `liveCount=0`.
- If Jason reports that the phone still looks hung after a WebUI/relay fix, manual CDP reload evidence is not enough. Run an app-level true-device relaunch proof such as `FREEHAND_ANDROID_SKIP_INSTALL=1 apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>`, reconnect CDP to the new Freehand PID, and prove the post-relaunch DOM has the relay asset version, terminal selected turn, `liveCount=0`, and no stale provider-retry text before claiming the mobile path is recovered.
- Use the global `scripts/install-global.sh` plus `scripts/install-launchd.sh restart` path only for release/promotion closeout or when explicitly validating the installed release surface.
- For any WebUI, ADP, reasoning, stream, turn lifecycle, session, tool rendering, schema retry, composer, or status/progress change, online verification is mandatory before reporting success. The minimum proof is:
  - start or restart the real daemon on the chosen validation port, normally symlink dev `127.0.0.1:4042`; use release `127.0.0.1:4041` only for release closeout
  - drive the real WebUI in a browser, not only unit tests or static DOM inspection
  - submit at least one real request through the UI path that was changed
  - query ADP state for the same session/turn and compare it with visible UI state
  - save screenshot evidence under `artifacts/webui-online/` or another explicit repo artifact path
  - report the exact commands, ADP sample/query result summary, and screenshot path in the final answer
- When using Chrome DevTools Protocol for WebUI online proof from shell automation, spawn the browser inside the long-running automation process and stop only that explicit PID after evidence capture. A short-lived shell background Chrome can exit before CDP connects, producing false DevTools-port failures unrelated to Freehand.
- Prefer Playwright's cached `chromium_headless_shell` for CDP WebUI verifiers when available. On macOS, spawning `/Applications/Google Chrome.app/.../Google Chrome` while the user's normal Chrome is already running can reuse the existing app process and ignore new `--remote-debugging-port` flags, producing a false `timeout waiting for Chrome DevTools page target`.
- Do not say WebUI behavior is fixed, verified, or passing unless browser-visible evidence and ADP/session truth both prove the changed behavior. If online verification cannot run, state that explicitly and treat the work as unverified.
- ADP `SubmitUserInput` command receipts may be long-running while daemon-owned reasoning continues. Online verifiers must not restore config, close fixtures, or restart services only because the submit socket times out. Keep fixtures alive, poll the fixed session's new turn ids plus provider/page request evidence until terminal owner truth, then restore S-profile config/env.
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
- Do not parallel-run `scripts/run-cargo-test-with-evidence.sh`; its stdout/stderr evidence paths use a seconds stamp and parallel invocations can collide, producing mixed or misleading logs. Run evidence-wrapped cargo tests sequentially.
- Do not parallel-run multiple `cargo test` processes that rely on timestamp-based temp runtime helpers inside the same owner area; cross-process temp-path collisions can create false persistence/runtime failures during spot checks.
- If a focused `cargo test` appears to hang or emits no output during compile, rerun it through `scripts/run-cargo-test-with-evidence.sh -- <cargo test args...>`. This wraps cargo with a bounded timeout, writes stdout/stderr logs, and prints the exit code. Do not conclude "no cargo process" from a narrow `ps | rg cargo` check alone because the local command wrapper may appear as `rtk cargo` and the active child may be `rustc`.
- Canonical full local gate is `make ci`.
- Release closeout must run `scripts/release.sh` end-to-end and prove staged artifacts exist; global install closeout must run `scripts/install-global.sh` with a temp `FREEHAND_PREFIX` and prove installed host binaries execute.
- Installed daemon closeout must use a temp `HOME` plus real `~/.freehand/config.toml` shape, start `freehand-daemon serve --agent <name>`, curl `/health` and `/`, then stop only the exact daemon PID.
- Release WebUI/phone-facing closeout must prove the installed release daemon serves current workspace assets before UI/Android claims. Compare `apps/freehand-server/assets/webui.{js,css}` SHA-256 with `http://<release-bind>/assets/webui.{js,css}` and rerun the online verifier against release 4041.
- Android release-device closeout must set `FREEHAND_ANDROID_APK` to the release APK artifact when running `apps/freehand-android/scripts/verify-device-ui.sh`; otherwise the script default debug APK can overwrite the release install and invalidate release evidence.
- Android device verifier runs that use SDK `apkanalyzer` need a working Java runtime, normally `JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"` on this Mac. If `verify-device-ui.sh` reports `apkanalyzer_failed`, treat it as local verifier/toolchain blockage, not APK or device UI truth.
- Android S-profile device proof must use a client-reachable endpoint, never Mac `127.0.0.1` and never `adb reverse` as acceptance truth. Keep the S daemon loopback-only on `127.0.0.1:4042`, route the phone through the configured Tailscale relay, require `adb reverse --list` to be empty, read back the app-owned remote-registry config, and prove relay-served current asset URLs plus ADP, CDP DOM, app relaunch/new-PID, and screenshot truth.
- If Android has both `daemon-connection-registry.json` and legacy `daemon-connection.json`, canonical WebUI closure depends on the installed APK reading the sidecar. A `FreehandWebUiLayout` probe with inline stylesheet/no shell after relay root 404 is usually a stale APK loading the legacy `http://relay:port/` projection; rebuild/install current APK, read back installed `versionCode`, and prove the sidecar `webUrl` path loads `/relay/daemon/<host>/?client=android-webview`.
- Android device foreground truth must come from current resumed/focused `com.freehand.android` activity plus a `FreehandWebUiLayout` probe. Historical package mentions in dumpsys are not foreground evidence; if a system picker is foreground, exit that picker and relaunch Freehand before judging WebUI layout.
- Android multi-agent navigation proof must operate the real WebView target and capture current-session Agent sheet, canonical `worker_session_id` selection, Worker transcript without framework prompt leakage, and exact return to the parent Master session.
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
- For Master lifecycle attention, keep admission and dequeue as separate
  contracts. Admission must consume EventInbox source order and advance the
  cursor only after durable `pending_attention` admission or explicit
  non-attention classification. Dequeue may use severity/priority weighting,
  but it must be deterministic from persisted admission sequence, not
  wall-clock timing.
- Treat `TaskBlocked` as showstopper attention and give bounded task priority a
  large score contribution, while preserving deterministic aging so continuous
  high-weight arrivals cannot permanently starve an older lower-priority item.
  Retryable provider/model failures keep the same pending attention id,
  admitted sequence, and cursor; stale no-op events are removed and selection
  continues in the same runner tick.
- For multi-task Phase 2C worker-control proof, stateful task consequences such as pause, resume, and cancel must route through Task Center first and persist `applied` worker-control events only after the Task Center consequence succeeds. Safe-point requests persist `queued`; status queries persist `observed`. Restart proof must verify the same task, execution, agent, and control ids after `restartS`; a fresh sample is not recovery evidence.
- Production Worker pause is cooperative owner-truth cancellation, not a stale-result error path. The runner must monitor same-task/same-execution `WorkerControlOp::Pause`, wire it to `LiveReasonCancelToken`, stop only at live-bridge safe points, and then return idle without writing review/block/heartbeat failure over `TaskPaused`; persisted resume must re-enter the same task/execution.
- For Master busy attention work, foreground work state is the `master_work` resource, not task/session/timer truth. Persist active identity, priority, safe point, suspension state, and typed attention resolution under the active-work lock; never store raw Worker/control transcripts, provider payloads, or placeholder plan prose in that checkpoint. Live preemption is not closed until typed resolution is injected back into the original foreground reasoning continuation and online-proven.
- To bind a suspended Master isolated attention decision, the positive test must observe the exact foreground checkpoint as `SuspendedByAttention` while the decision executes and prove the task-scoped `master-lifecycle-*` session plus event/attempt-isolated turn and trace ids differ from the foreground identity. The paired negative test must inject a raw control/provider sentinel through the executor result and prove it is absent from foreground ReasonPersistence, `master_work`, and typed `AttentionResolution`; separate ids alone are not transcript-isolation proof.
- For parent-session Master/Worker evaluation, use `task_closed` as the resume
  signal only after every current Task Center child sharing the same
  `parent_session_id` is `Closed`. Build the follow-up from original user
  objective history, decomposed task goal/deliverables/acceptance, and accepted
  `TaskReviewSubmitted` truth; persist it in the original parent reason session
  and never expose raw Worker transcripts.
- Parent-session Master/Worker evaluation must read the original user objective
  from authoritative reason truth, not UI-coalesced transcript projection.
  Only the first user turn's first round is parent-goal truth; repair rounds,
  timer/control prompts, and UI projection coalescing must not replace the goal.
- Parent objective recovery must go through the reason persistence owner via
  `ReasonPersistence::restore_turn_start_snapshots`. Do not reconstruct parent
  goals from effective UI snapshots, latest repaired rounds, worker transcripts,
  metadata/debug ledgers, or ad hoc reason-ledger parsing in runtime.
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
- Atomic rename alone does not make shared JSON read-modify-write safe.
  `leases.json` mutations from independent Worker processes must hold the
  TaskStore advisory lock across load, mutate, and atomic replace. Boot cleanup
  must remove only the invalid task ids from current locked truth; never replace
  the file from a stale pre-lock lease snapshot.
- Worker process health/restart truth belongs to `agent.lifecycle`, not
  launchd, daemon host glue, UI, or PID probing code. Worker construction must
  write typed `ProcessStarted`; every idle/active poll tick and long-running
  task heartbeat must write typed `ProcessHeartbeat`; AgentBoard/AgentLifecycle
  query projection derives `alive` from the owner TTL while retaining task,
  execution, and activity history. Online proof must stop one explicit Worker
  PID, observe `alive=false` after TTL for the same agent/task/execution, then
  restart the same agent with a new PID/process instance and `restart_count+1`.
- A Worker must never auto-requeue its own `TaskInterrupted` truth. Leave the
  same task interrupted until Master evaluates TaskHistory, AgentBoard, overall
  goal, and subgoal, then explicitly selects same-Worker retry or cross-Worker
  takeover. The interrupted Worker's AgentBoard projection must become idle,
  clear current task/execution/turn binding, and retain typed interrupted last
  activity; stale running/current binding is red UI and scheduling truth.
- Agent reasoning is framework-driven, not Master-driven: the runtime reason
  loop owns schema validation, tool-result re-entry, history pairing, and
  continuation for both Master and Worker turns. Do not design Master-side
  "continue the Worker" loops. Master consumes TaskBoard/AgentBoard/EventInbox
  truth and decides task-level actions; Worker runner owns claim/heartbeat and
  converts Worker completion schema into Task Center truth.
- `TaskBlocked`, `TaskReviewSubmitted`, `TaskReviewRejected`,
  `TaskReviewApproved`, `TaskInterrupted`, `TaskCancelled`, and `TaskClosed`
  must release the Worker's current lifecycle binding.
  AgentBoard `current_task_id`, `current_execution_id`, and `current_turn_id`
  mean active/current framework execution only; blocked/review/terminal audit
  truth belongs in `last_activity` with `current_activity.kind=idle`. Boot must
  repair older persisted lifecycle snapshots that still point current task or
  execution binding at already released task truth, including execution-only
  stale bindings with `current_task_id=null`.
- Launchd-managed Worker lifecycle proof must use isolated HOME, unique Worker
  labels, and `FREEHAND_LAUNCHD_SKIP_ENABLE=1` so temporary verifier labels do
  not leave persistent `launchctl enable` overrides. Kill only the explicit
  gamma PID, let KeepAlive restart it, then verify AgentBoard owner truth shows
  the same task/execution, new PID/process instance, and `restart_count=1`.
- When global `~/.freehand` EventInbox/TaskBoard contains unrelated historical truth, run Master/Worker lifecycle fixtures with an isolated temporary `HOME/.freehand`; do not delete, skip, or rewrite global truth to obtain a pass. Switch both the fixture Master and Worker provider configurations through the config owner before submitting work, and stop only the explicit fixture/server/worker PIDs started by that verifier.
- Before an isolated online verifier launches a workspace binary such as
  `target/debug/freehand-daemon`, rebuild that exact binary after source changes;
  a green unit test does not refresh a separately launched executable. Fixture
  decisions must match accepted `review_summary`/TaskHistory truth, not expected
  tokens embedded in task goals, acceptance text, or tool-call arguments.
- Three-Worker convergence verifier foreground behavior must match product
  semantics: the initial `SubmitUserInput` turn creates/assigns the first child
  tasks and returns `claim="waiting"`. It must not busy-poll child review
  history inside the same foreground turn until a fixture poll budget fails;
  lifecycle progress is observed afterward through ADP TaskBoard/TaskHistory
  and parent-evaluation SessionTurns truth.
- Master `claim="waiting"` must be backed by owner truth that can wake the
  lifecycle without another user message: an open same-session child task or
  active/running source timer. Terminal child history with no source timer must
  not persist as lifecycle `ToolPending`; user-choice waits close as
  blocked/user-needed or complete with evidence. Also inspect rebuilt
  `SessionHistory.base_context_segments`, not only UI projection, so internal
  parent-evaluation/timer prompts never become provider-visible user memory.
- For WebUI multi-round rendering, never collapse `runtime-turn-N` / `runtime-turn-N-rM`, same-text assistant rows, same `tool_call_id` updates, tool requests, or tool results into one all-in summary card. Render every protocol-projected request, response, tool activity, and terminal row as its own chronological visible card/section. Do not hide continuation/user rows or raw tool result lines in the browser; internal framework prompts may be hidden only by the protocol/runtime projection owner by emitting empty `user_text`. Keep raw completion schema/debug-only lines out of the public stream through the existing protocol/debug boundary, not through DOM compression.
- For WebUI submit/history regressions, composer clearing is not proof of success. Verify the submitted text is immediately visible in the conversation stream, historical cards remain present, the latest card is appended in session order, a live turn with no public rows renders an explicit observable waiting row instead of a blank transcript, and at least two consecutive submits remain visible after later ADP refresh/timer updates.
- For WebUI ambiguous submit recovery, never render user-facing `unknown` or "任务未知" once owner truth can verify acceptance. Refresh session list, selected transcript, latest turn, and TaskBoard; if same-parent TaskBoard task truth created after the submit window exists, clear pending state and render an accepted service receipt card until transcript truth materializes. Do not fall back to the clean "New conversation" empty state, and do not use `updated_at` because historical heartbeat/review updates can mis-correlate old tasks.
- For same-session continuation regressions, UI transcript continuity is not enough. Add a provider-request black-box test proving the follow-up request contains prior user/assistant history from effective persisted turns, then run a real WebUI same-session follow-up prompt on S profile and verify the second answer can use first-turn-only context plus ADP reports both turn ids.
- For repaired-failure context economy, do not delete raw failed attempts from truth. Lock that `runtime-turn-N` / `runtime-turn-N-rM` repaired logical turns remain visible in persisted/UI/debug/error truth, while future default prompt context admits only the latest repaired round. A green UI transcript is not enough; inspect rendered/planned provider context or an owner test such as `effective_context_uses_last_repaired_round_without_raw_failed_attempt`.
- For WebUI restart/continuation regressions, verify a WebUI-created non-default session after daemon restart, then submit another turn to the same session and restart again. ADP `turn_ids` must strictly append without reusing an existing `runtime-turn-N`; runtime bootstrap must seed the next ordinal from all persisted sessions, not only the default runtime session.
- WebUI hidden diagnostic prompts and online verifier samples must obey the active Master tool surface. Current Master may call locked local workspace tools in the selected session cwd plus `task`/`timer`; use cross-cwd or unavailable-tool samples when a failed tool card is needed, not an in-cwd `read_file` call that should succeed.
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
- WebUI Settings provider selectors must follow owner-projected `QueryConfigStatus`
  truth until the operator edits a selector. Do not treat an empty select value
  from pre-config render as user draft; provider definition upsert must preserve
  the current primary and fallback selection, and only explicit
  `UpdateAgentProviderSelection` may change agent provider bindings.
- CDP/WebUI online verifiers run functions in the browser context. Any helper
  used inside `Runtime.evaluate` must be injected/serialized into that
  expression or defined in the page; otherwise polling can silently swallow
  `ReferenceError` and report only a timeout. Capture DOM plus console/log
  events on failure before restoring fixtures.
- Current provider failover ownership covers non-stream requests only. Do not enable stream failover until partial output, tool-call side effects, rollback, and resume have one typed contract and positive/negative lifecycle coverage.
- For reason-turn stop logic, validate completion schema before terminal acceptance. Reject and explain invalid terminal submissions.
- UI protocol black-box tests must cover standard user-visible flows, not only internal event wiring.
- `cargo test --workspace` is the regression umbrella and must carry white-box plus module/project black-box coverage as those tests are added.
- When tests are added, changed, or found incomplete, update the module's test-design record in the same change set.
- When request/response/error mainlines or shared function usage change, update the function-map doc in the same change set.
- When migrated mainline-call truth changes, update `docs/mainline-calls/**` and regenerate `docs/wiki/**` in the same change set.
- Exact Rust call-graph visitors must unwrap transparent `Expr::Paren` / `Expr::Group` callee wrappers before direct-path resolution; recursively visiting the inner path alone does not record a call edge.
- OpenMinis `contract_ready` and later UI fields must exactly match the resource operation binding's `ui_contract`; non-pending manifest strings are not contract truth. Its `surface_path` must be normalized, repository-relative, and canonically resolve to an in-repository file.
- Exact Rust call-graph discovery gives active module const/static and impl/trait associated-const initializers deterministic owner-qualified `::__initializer` callers, rejects inactive cfg initializers, and follows parsed `Deref<Target = Local>` for local target methods without projecting unrelated external-trait calls onto that target.
- OpenMinis verifier evidence accepts only an immutable 40-character object ID whose Git object type is exactly `commit`; a tree or tag object cannot stand in for `repository_commit`.
- Evidence that predates a lifecycle promotion may admit only explicitly normalized lifecycle fields in the canonical manifest. Path allowlisting alone is insufficient because it lets operation, source, target, and map contracts drift under old evidence.
- When adding or editing a migrated feature, keep the mainline JSON path and its internal `function_map_doc`, `test_design_doc`, `generated_wiki_doc`, and feature-map links canonical or the workspace gate must fail.
- When adding or editing a migrated call table, keep every `bound` row tied to real file paths and resolvable symbols; do not use prose such as "handler" as a symbol path.
- Run `cargo run -p xtask -- mainlines generate` and `cargo run -p xtask -- mainlines check` sequentially, not in parallel; both touch the generated wiki surface and parallel execution can create false out-of-date failures.
- When tool surface or tool execution truth changes, update tool design, function map, test design, and runtime exposure checks in the same change set.
- When `tool.registry` changes affect live provider exposure, run both owner/workspace gates and one real config-selected `reason-live` smoke when credentials are available; selected-agent bootstrap still requires the configured pair-token env even for CLI live-turn verification.
- When context-segment admission, cache-shape policy, or subagent context flow changes, update `reason.context-planner` design, test design, function map, and memory in the same task.
- For busy Master `AttentionResolution` continuation, consume the typed resolution exactly once, refresh TaskSpaceSnapshot, and admit it as turn-volatile/no-cache context before the next foreground provider request. Pair stale pre-resolution tool calls with failed tool results and no side effects; discard stale terminal candidates before terminal persistence. In no-pending-tool branches, never call `Option::take()` on the resolution before checking whether pending tools exist, or the terminal continuation path loses the resolution.
- For live context-distribution claims, inspect persisted `planned_context.ordered_segments` and diagnostics from the reason ledger, not only unit tests. Prove stable/session-stable prefix shape, stable prefix hash, tool schema hash, and volatile/no-cache tail placement across at least one multi-round S-profile sample before claiming closure.
- For schema-polishing proof, do not rely on prompt-only steering against a live model. A real provider can obey the completion contract immediately or call unrelated tools, so deterministic schema-mismatch closeout needs a provider fixture, mock executor, or injected first invalid response plus no-tool contamination checks.
- For Master workspace-boundary changes, lock all direct Master workspace and checkpoint authority to canonical `runtime_home` (`~/.freehand`). Keep external session CWD as routing truth only: workspace or shell execution outside runtime home must return a paired failed tool result with explicit `task`/Worker guidance, while the framework-scoped `task` tool remains available for external `target_cwd` delegation. Online proof must submit a real external-CWD request, show no external content leakage, query the same Task Center task/history truth, and distinguish delegation from actual Worker completion.
- For remote daemon connectivity, a relay endpoint candidate is not proof of
  relay success. Closure requires `remote_relay_transport` truth with host
  registration, account directory projection, registered namespaced WebUI HTTP
  root/assets/query/health pass-through, registered `/adp` WebSocket
  pass-through, and explicit missing-host failure. Android relay closure must
  install the APK, persist/read back an app-owned `remote_registry` relay
  endpoint, and prove the canonical WebUI layout loads through that relay URL
  on the real device; direct S-profile `adb reverse` proof is not relay proof.
  Config owns bootstrap/route candidates, node owns directory/route resolution,
  and Android remains an import/WebView host; none of those may silently own
  relay pass-through IO. Online relay verifiers must start only scoped
  upstream/relay processes and clean up exact recorded PIDs.
- For diagnostics/log UI, consume only runtime owner projections. Do not render
  raw logs, absolute runtime paths, provider payloads, secrets, or browser-local
  diagnostic guesses. Online proof must compare DOM rows to ADP `QueryDiagnostics`
  owner truth, prove redaction/no absolute-path leakage, and prove the top-level
  persisted session list is unchanged.
- For provider capability Settings UI, do not treat CLI/ADP capability tests as
  proof that the phone Settings surface is wired. Closure requires a browser-click
  proof from the Settings provider card/control to `TestProviderWebSearch`, visible
  DOM pass/fail status, S-profile restore, and request-shape evidence for hosted
  `web_search` when a fixture provider is used.
- A safe config projection is not proof that every agent process loaded that
  config. For Master/Worker provider divergence, compare config/binary mtimes to
  exact service PIDs and start times, inspect each agent-specific env file by
  presence/fingerprint only, then prove the loaded route from live provider
  metadata. A primary-provider 401 followed by
  `RuntimeLive05ProviderFailover` and successful fallback execution proves
  failover, but it does not repair or validate the primary credential. Lifecycle
  closure additionally requires Worker terminal task truth, Master review/close
  events, parent-session terminal truth, and an empty WebUI running-session list.
  A historical Worker failure stays in transcript truth; normal SessionDetail may
  mark it recovered only when the same TaskBoard-projected Worker session has a
  later Success and that exact task is closed. Keep the raw error in debug mode,
  and never use a different task's success to rewrite an older failure/cancel.
- For mobile UI tree closeout, run `node scripts/verify-mobile-ui-tree-goal-audit.mjs`
  after the slice verifiers. Treat `mobile_ui_tree_goal_audit_blocked` as an
  explicit blocker report, not completion; an Android locked/dozing state must be
  shown with the latest `apps/freehand-android/scripts/verify-device-ui.sh`
  artifact and ADB window signals before reporting the remaining gap.
- For WebUI modular surface splits, asset smoke is only a serving/import gate.
  After moving code out of `webui.js`, the online verifier must capture browser
  `Runtime.exceptionThrown` or console failures and fetch every new module asset;
  a green `node --check` plus asset route test does not prove bootstrap executed
  in the live browser.

## Memory Workflow

- Record exploration in `note.md`.
- Promote only verified, durable conclusions into `MEMORY.md`.
- Keep `CACHE.md` short and current for the next session.
- If feature truth changed, update resource map, function map, architecture docs, skill workflow, and memory files in the same task.

## Closure Checklist

Use this checklist for both new features and bug fixes:

- Codex review gate uses `codex --profile cc review` by default. If that route returns HTTP 402 / insufficient quota before a final verdict, rerun the identical review prompt with `codex --profile tcm review`. This is an external reviewer transport switch only; do not weaken tests, skip review, or treat it as product-runtime fallback.
- information sufficient
- logic closed-loop
- lifecycle management complete
- owner and function map updated if truth changed
- function-map call table and symbol binding still match code
- metadata/request isolation still holds for cross-module calls
- test-design record updated and still matches implementation
- runtime/debug evidence path still valid

If any line is not true, do not claim completion.

## Task Phase 1 Lifecycle Closeout Rule

- For `task.orchestration` concurrency fixes, do not treat a query projection as an owner recovery point. Use `TaskRuntime::boot_read_only` for ADP/query/projection paths; keep `TaskRuntime::boot` for master/worker runner ownership recovery.
- Task ledger writes must enter `TaskStore::append_event_and_snapshot`: ledger append, snapshot atomic write, index rewrite, and disk-based seq allocation stay inside the same task-ledger flock.
- Lease-expiry recovery must carry the stale execution generation (`fencing_token` / `interrupted_execution_id`) on `TaskInterrupted`, clear active execution truth, and reject late ExecutionFacts from the stale generation.
- Phase 1 closeout is not proven by focused task tests alone. Before removing Gap 7 or starting worker pooling, require full `cargo test -p freehand-runtime -- --test-threads=1` or an owner-documented separation of unrelated runtime failures.

## Stale Lifecycle Recovery Rule

- When a legacy session is stuck on `ToolPending` after restart, check three owner truths before UI claims: effective reason transcript, raw/rolled-back authoritative turn files, and TaskBoard/Timer owner state. Do not delete the session or trust a cached completed-parent marker by itself.
- Retained-offset reason ledgers (`expected 1, got N`) are hard errors only when authoritative snapshot truth is absent. If authoritative turn snapshots plus rollback markers exist, lifecycle/bootstrap paths must use that owner truth and keep raw rolled-back turn ids reserved for non-colliding follow-up ids.
- A parent repair round (`runtime-turn-N-rM`) may still carry the original user objective in the `freehand_runtime/original_task` context segment. Master parent evaluation must use that owner context instead of requiring `round == 1` only.
- WebUI `ToolPending` is lifecycle-running only with waiting tool/model activity or owner-projected open task/timer truth. No waiting activity plus no open task must render as user-choice wait, not stale `等待生命周期`, and refresh-error exits must clear selected session state.
- Live runtime bootstrap must close stale no-owner `ToolPending` before UI restore: use reason closed-turn truth plus `TaskRuntime::boot_read_only`, TimerStore, and live `master_work` owner truth; write `Blocked` for no-owner stale waits, but preserve `ToolPending` when an open child task, active/running source timer, or live Master work can wake it.
- Incomplete multi-round authoritative history with empty/poisoned reason ledger must not hard-fail whole-daemon bootstrap; annotate partial UI restore and skip that historical session.

## ADP Query Route Rule

- ADP frames must be versioned with protocol-owned `protocol_version` and complete the first-frame handshake before command/query/subscribe routing. Rust clients use `UiAdpRequest::Handshake`; WebUI must wait for `handshake_accepted`; server pre-handshake command/query/subscribe frames must fail before dispatch/query ports.
- ADP command surface for WebUI is single-source and public-only: Rust `UI_COMMAND_DESCRIPTORS` / `adp_protocol_manifest()` exports JSON + JS constructors via `export-adp-protocol`; WebUI must call `adpQueryOf`/`adpCommandOf`/`adpSubscribeOf` from `apps/freehand-server/assets/webui/generated/adp-protocol.js`. Internal scheduler/worker commands (`ClaimNextTask`, `ApplyExecutionFact`, `RunSchedulerTick`, `RunMasterPoll`) must stay out of served public ADP artifacts and be rejected by server public command ingress before runtime dispatch unless the server has `FREEHAND_ADP_INTERNAL_COMMAND_TOKEN` and the client proves the matching `adp.v3.internal_command_ingress:<token>` capability; CLI may only send that token when `FREEHAND_ADP_SEND_INTERNAL_COMMAND_TOKEN=1` is explicitly set for a trusted harness URL; self-advertised capability without server token match is not authorization truth. After command-set changes, regenerate both artifacts and keep `xtask gates check` green.
- ADP Query frames may only carry `UiCommandFrameClass::Query`. Server `handle_adp_query` must call `accept_query_ingress` before any runtime query port.
- Master poll split is rigid: `RunMasterPoll` is internal Command-frame mutation (`run_master_poll`) and is not admitted on public `/adp` without server-owned internal token match; `QueryMasterPoll` is public Query-frame read-only (`preview_master_poll`). Never re-admit `RunMasterPoll` on Query or public ADP command ingress.
- WebUI asset cache-busting has one owner: `apps/freehand-server/src/assets.rs::WEBUI_ASSET_VERSION`. Authoring files use `__WEBUI_ASSET_VERSION__`; server stamps at serve time. Online verifiers must compare against the served stamp, not hardcode a second source of truth forever.
- For Relay generic WebSocket work, keep directory control projection on `/relay/api/agents/subscribe` and opaque data frames on `/relay/agents/{agent_id}/connect/{path}`. Validate only account scope, Agent admission, local-only target path/query, frame type, backpressure, and close lifecycle; never parse or reconstruct session/task/provider semantics from data frames. Online verification must close one connection normally, drop another abruptly, prove the Agent remains online, open a subsequent connection successfully, then stop the Agent and prove offline projection plus pre-upgrade 404. Both normal Close and abrupt disconnect send request-end exactly once and wait for Agent response-end; neither may remove pending truth early or terminate the shared Agent tunnel. Data response delivery must await per-exchange capacity rather than treat a full bounded channel as a protocol error. Control, data, and error attachments each require a generation fence; rejected duplicate/stale cleanup may detach only the generation it admitted. A WebSocket response error or channel close before `ResponseEnd` remains an explicit error-chain termination, never normal completion.
- Preserve `RelayDataProtocol` through Agent-side local WebSocket request construction: inject the local ADP bearer only for `Adp`, never for generic `WebSocket`. Relay channel URL builders must preserve deployment prefixes and mount `/relay` exactly once when config already ends in `/relay/`.
- Relay server cookie security is required deployment truth in `FREEHAND_RELAY_SECURE_COOKIE=true|false`. Use `false` only for an explicitly declared direct HTTP deployment and `true` for TLS; never infer policy from forwarded request headers or provide a host default.
- Treat Relay HTTP proxy cookies as a bidirectional credential boundary: remove Relay-session and Agent-local ADP authentication cookies before forwarding requests, block upstream `Set-Cookie` from entering the Relay origin, and preserve only unrelated application cookies. Keep Agent-local ADP authentication on the typed Agent-side ADP WebSocket builder; never use HTTP cookie pass-through as an authentication fallback. Verify both request and response directions in the module black box and repeat the response-header check through the deployed distinct-network Relay.
- Relay HTTP response bodies are opaque data-tunnel bytes for every content type. Stream them with bounded backpressure and never decode, inspect, rewrite, or namespace HTML/JS/CSS inside `relay.transport`; any browser base-path projection belongs to the Agent UI/protocol owner and requires its own registered resource edge.
- Relay client cancellation travels on the typed error socket, but queued response frames travel on the data socket, so cancellation and response delivery have no cross-socket ordering guarantee. Mark the pending exchange cancelled without removing it, absorb already-queued response data without projecting payload, and retain cancellation truth until the Agent sends `ResponseEnd` on the data socket. Cancellation after known local completion is idempotent; cancellation for an exchange that never existed remains an explicit protocol error.
- Relay exchange admission is routable only when live data and error attachments already exist. Keep raw exchange creation private, and make the registry perform attachment checks plus pending insertion in one mutation. Error attachment admission likewise checks the live control attachment and inserts error truth in one registry mutation. HTTP/WebSocket wrappers must consume these owners rather than reproduce prechecks. Bind each admission boundary as a resource operation, allowed relation, source edge, mainline step, test-design row, and source gate.
- Relay WebSocket text and close-reason bytes must decode with strict UTF-8 at the receiving boundary. Invalid text, malformed one-byte close payloads, and invalid close reasons enter the correlated typed error chain; never use lossy decoding or project replacement characters as successful payload truth. Treat raw Tungstenite `Frame` values as explicit bridge errors rather than synthesizing a Close frame.
- Before replacing a deployed Relay binary on a bind-mounted filesystem, verify free space can hold the complete new binary and build into a separate writable target outside the production mount. Install only after the ARM64 focused test and release build pass, then restart the exact container/service and verify binary SHA-256 plus health. Never allow an in-place install to truncate the active binary; if a failed build polluted the production target, relocate only the identified build artifact to a non-production volume before restoring service.
