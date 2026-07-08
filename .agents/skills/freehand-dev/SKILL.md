---
name: freehand-dev
description: Use when working inside the Freehand repo on architecture, harness, config, provider, reasoning, node topology, UI protocol, gates, or test infrastructure. Enforces Freehand's contracts-plus-blocks-plus-orchestrators architecture, feature map ownership, directory locks, replay-first debugging, and required validation workflow.
---

# Freehand Dev

Use this skill for any non-trivial work in this repo.

## Start

1. Read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`.
2. Read `docs/architecture/feature-map.md`.
3. Use `Owner Routing Index` to map the problem area to exactly one `feature_id`.
4. Read the feature's bound function-map doc before non-trivial implementation or debug.
5. Read the feature's bound test-design doc before non-trivial implementation or debug.
6. Identify the target `feature_id`, owning crate, allowed paths, forbidden paths, required checks, debug artifacts, runtime paths, `test_design_doc`, `function_map_doc`, and `lifecycle_checks`.
7. If ownership is unclear, fix the map first or stop and ask.
8. Before coding, ask three questions:
   - is the information sufficient
   - is the logic closed-loop
   - is lifecycle management complete
9. If any answer is no, do read-only tracing and source search first. Ask the user only after read-only search cannot close the gap.
10. Before implementation for each module feature, write or update its test-design record first.
11. Test-design record must capture:
   - target feature and owner
   - lifecycle and logic path
   - white-box coverage plan
   - module black-box coverage plan
   - project black-box coverage impact
   - known gaps and non-goals
12. Function-map record must capture:
   - owner crate and owner module
   - code-bound entry symbols
   - request mainline
   - response mainline
   - error mainline
   - mainline call source when the feature is migrated
   - generated wiki path when the feature is migrated
   - shared multi-reference functions and why they are reused
   - call table bound to code paths
13. Tool-owning features must also capture:
   - tool spec owner
   - implemented vs unimplemented state
   - runtime exposure gate
   - execution owner symbol
   - side-effect and permission notes when relevant
14. If another worker cannot read the test design and function map and understand where coverage lives, where the mainline runs, and what remains risky, the design is incomplete.

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
- each configured agent must have explicit `node_id` and `paired_agent`.
- peer topology is config-owned: paired agents must be reciprocal and opposite mode in the first local topology version.
- runtime/daemon code must consume selected peer topology from `freehand-config`; it must not derive synthetic master/slave node ids.
- current first version master/slave scope is local one-master one-slave only.
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
- WebUI selected-session transcript rendering must preserve protocol/session transcript order and append-or-replace the latest same-session turn; do not sort visible cards by `runtime-turn-*` ordinal because ordinals can reset after restart or recovery.
- WebUI lifecycle animation must be scoped to current live turn render projection only; historical turn/tool rows must remain static even when they still carry protocol model_request or tool status fields.
- WebUI session-list truth is the render gate after it has loaded. Latest-active query, latest-turn ADP/SSE updates, and selected-session transcript projections may render only when the session id is listed, current draft, or current pending-submit; non-destructive `DeleteSession` can leave old turn truth queryable, so never use latest-active as a fallback after session-list truth exists.
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
- Reason context planning follows locked Reasonix/Codex direction:
  - stable prefix stays stable across ordinary turns
  - only explicit rewrite events may change prefix layout
  - prefer subagent search final-report enrichment over injecting raw exploration transcripts
  - admit subagent context into parent turns only as typed final conclusion segments
- `reason.rewrite-policy` in `freehand-blocks` owns when compaction / rollback / resume rebuild should trigger; `freehand-reason` only owns `SessionHistory` mutation after that decision
- `ReasonRewriteRuntime` in `freehand-reason` is the baseline consumer that may call `SessionHistory::stage_*` from policy-approved decisions
- Provider `TokenUsage` enters rewrite policy only through `freehand-blocks::prompt_tokens_from_usage`; do not hand-roll provider usage interpretation in runtime or UI
- `freehand-testkit` may host project black-box runtime harnesses before production CLI/server loops exist; keep harness behavior aligned with function maps and test design
- built-in tool specs and execution ownership live in `crates/freehand-tools`
- writable tool preview ownership also lives in `crates/freehand-tools`
- runtime must not hardcode demo tool schemas or demo tool execution outside `crates/freehand-tools`
- every new built-in tool must first land as a spec in the tool owner with explicit `implemented` state
- no tool may be exposed on the live provider path until its function map and test-design docs are updated in the same change set
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
- The first instruction capability slice is index-only. Provider-visible instruction injection must be added later through typed context-planner segments with explicit token budgets.

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
- Before claiming completion, run the feature's mapped test stack:
  - module white-box tests
  - module black-box tests
  - project black-box tests
- Do not parallel-run multiple `cargo test` processes that rely on timestamp-based temp runtime helpers inside the same owner area; cross-process temp-path collisions can create false persistence/runtime failures during spot checks.
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
- For WebUI/ADP state projection checks, use paired samples before claiming UI correctness. In dev mode prefer `freehand-cliS ... --url ws://127.0.0.1:4042/adp`; for release closeout use `freehand-cli ... --url ws://127.0.0.1:4041/adp`.
- For multi-round tool-loop claims, one-round success is invalid evidence. Use `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure` and require `rounds>=2`, `tool_executions>=1`, `failed_tools>=1`, plus terminal success from ADP/session truth before claiming closure.
- For completion-schema mismatch/live-tool bugs, verify the provider finish reason gate before UI work: completion-schema mismatch handling may run only on terminal-candidate finish reasons such as `stop` / `end_turn`; it is model response polishing, not system schema repair and not provider failure. `tool_use` and incomplete tool calls must become paired tool results back to the model, not schema polishing or terminal failures.
- For provider/network executor failures, keep them separate from schema mismatch and tool-result failures. Recoverable non-stream provider errors retry exactly five attempts with exponential backoff starting at 1s in production; final failure must expose the concrete provider error code such as `anthropic_http_status_500`.
- For provider-retry proof, model prose claiming retries is not evidence. Require provider-domain retry truth from error-center metadata, provider fixture/error injection, or runtime event projection; prompt-only sampling must fail.
- For task lifecycle headless proof, do not rely on a model prompt to create/review/approve/close tasks. Use protocol-owned task mutation commands over ADP, then verify `task.orchestration` list/history truth.
- For multi-task Phase 1 headless proof, use the S-profile `phase1-foundation-sample` create path first, then restart `com.freehand.daemonS` and run verify mode against the same blocked task, review task, execution, and agent ids. A fresh sample after restart is not recovery proof.
- For multi-task Phase 2A headless proof, use the S-profile `master-worker-foundation-sample` create path first, then restart `com.freehand.daemonS` and run verify mode against the same task, execution, and worker agent ids. A fresh sample after restart is not recovery proof, and model prose is not task-loop evidence.
- For multi-task Phase 2B EventInbox/MasterPoll proof, require four-part event cursors that include `event_id`, legacy three-part cursor compatibility tests, `replay_from_start=true` plus omitted limits for full drain, a final owner-backed non-replay cursor reread, and same-cursor verify after `restartS` returning zero events after cursor. Finite page limits or fresh post-restart samples are not cursor recovery proof.
- For WebUI multi-round rendering, never collapse `runtime-turn-N` / `runtime-turn-N-rM` into one all-in summary card. Render chronological per-round lifecycle cards, hide duplicate/internal continuation prompts after the first round, mark superseded rounds as continued, and keep the final summary at the bottom terminal row.
- For WebUI submit/history regressions, composer clearing is not proof of success. Verify the submitted text is immediately visible in the conversation stream, historical cards remain present, the latest card is appended in session order, a live turn with no public rows renders an explicit observable waiting row instead of a blank transcript, and at least two consecutive submits remain visible after later ADP refresh/timer updates.
- For same-session continuation regressions, UI transcript continuity is not enough. Add a provider-request black-box test proving the follow-up request contains prior user/assistant history from effective persisted turns, then run a real WebUI same-session follow-up prompt on S profile and verify the second answer can use first-turn-only context plus ADP reports both turn ids.
- For repaired-failure context economy, do not delete raw failed attempts from truth. Lock that `runtime-turn-N` / `runtime-turn-N-rM` repaired logical turns remain visible in persisted/UI/debug/error truth, while future default prompt context admits only the latest repaired round. A green UI transcript is not enough; inspect rendered/planned provider context or an owner test such as `effective_context_uses_last_repaired_round_without_raw_failed_attempt`.
- For WebUI restart/continuation regressions, verify a WebUI-created non-default session after daemon restart, then submit another turn to the same session and restart again. ADP `turn_ids` must strictly append without reusing an existing `runtime-turn-N`; runtime bootstrap must seed the next ordinal from all persisted sessions, not only the default runtime session.
- For provider recovery logic, classify errors as recoverable, unrecoverable, or periodic-recoverable. Periodic windows use provider-supplied seconds first, otherwise configured defaults.
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

## Memory Workflow

- Record exploration in `note.md`.
- Promote only verified, durable conclusions into `MEMORY.md`.
- Keep `CACHE.md` short and current for the next session.
- If feature truth changed, update function map, architecture docs, skill workflow, and memory files in the same task.

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
