# WebUI Session Transcript Alignment Plan

## Goal

Make Freehand WebUI behave like a real multi-turn chat client: persistent sessions survive reload/restart, transcript rendering follows session truth plus latest ADP signal, tool/error/status rendering aligns with Codex-style low-noise conversation semantics, and context/history rewrite logic follows the Reasonix persistence model.

## Acceptance Criteria

This work is accepted only when all items below are proven with tests and real browser evidence.

1. WebUI restores a selected session and transcript after browser refresh.
2. WebUI restores persisted sessions after daemon restart from authoritative session persistence, not ADP live cache.
3. WebUI and future Android/TUI clients render from the same rule: `persisted session truth + latest ADP signal overlay`.
4. Multi-turn conversation order is stable: user message, assistant/tool lifecycle, terminal/error status, and next user turn render in chronological conversation order.
5. Tool execution uses one visible lifecycle item per `tool_call_id` or equivalent execution id; waiting/completed/failed states update the same item instead of creating duplicate cards.
6. Waiting states are visible during execution with elapsed-time animation; the UI must not look frozen while provider/tool work is running.
7. Tool execution failure is returned to the model as a tool-result/error observation when it is a normal execution-result failure; it must not be projected as a system stop unless the runtime itself failed.
8. System/runtime/transport failures remain explicit visible failures and do not get converted into successful model/tool results.
9. Success and failure samples exist and are exercised through ADP and WebUI.
10. Screenshots are saved as evidence for reload recovery, multi-turn order, tool lifecycle update, waiting animation, success sample, and failure sample.
11. WebUI supports keyboard shortcuts and slash commands for common chat actions without bypassing ADP/protocol truth.

## Scope

### In Scope

- WebUI session list, selected-session state, transcript restoration, and multi-turn display.
- ADP query/subscribe/command usage as the default WebUI control/status path.
- `ui.protocol` projection for session list/session turns/tool lifecycle/error lifecycle when needed.
- Runtime bootstrap restoration of persisted sessions into UI protocol state.
- Reason persistence query/index path if session list or transcript cannot be rebuilt from current authoritative truth.
- Codex-style user-facing rendering semantics: concise conversation timeline, low-noise tool summaries, clear waiting/error status.
- Reasonix-style persistence/context recovery: session history is restored first, live events only overlay incremental changes.
- Real browser/manual or automated screenshot proof against fixed daemon port.
- Minimax config alignment before live provider validation.
- WebUI keyboard shortcuts and slash commands for send/cancel/reload/session/sample/help flows.

### Out Of Scope

- Visual redesign unrelated to chat/session correctness.
- Provider adapter redesign unless Minimax config cannot run with the current Anthropic-compatible path.
- Android UI implementation beyond keeping protocol semantics compatible.
- TUI implementation beyond using it as reference for renderer semantics.
- Any fallback path that hides missing session truth, missing ADP signal, or provider/runtime errors.

## Current Evidence And Prerequisite

### Minimax Config Check

Source config requested by the user:

- `/Volumes/extension/.rcc/provider/minimax/config.v2.toml`
- provider id: `minimax`
- provider type: `anthropic`
- base URL: `https://api.minimaxi.com/anthropic`
- default model: `MiniMax-M3`
- auth type: `apikey`
- API key is present and non-empty

Current Freehand runtime config:

- `~/.freehand/config.toml`
- provider id: `minimonth`
- provider type: `anthropic`
- protocol: `messages`
- base URL: `https://api.53hk.cn`
- default model: `MiniMax-M2.7`
- API key is present but differs from the RCC Minimax key
- `agents.master.provider = "minimonth"`
- `agents.worker.provider = "minimonth"`

Conclusion:

- The RCC Minimax source config itself contains the base URL and API key the user identified.
- The current Freehand runtime config is not aligned with that source. It uses a different provider id, base URL, model, and key.
- Before live WebUI reasoning validation, update or add a Freehand-compatible `[providers.minimax]` entry and point the active agents at it.
- Freehand config schema requires `[providers.<id>]`, explicit `protocol`, `baseURL`, `defaultModel`, and `[providers.<id>.auth]`. RCC `transportBackend` is not a Freehand runtime field and must not be copied into `~/.freehand/config.toml`.

Expected Freehand-compatible shape, with secret value copied from the RCC source without printing it:

```toml
[providers.minimax]
id = "minimax"
enabled = true
type = "anthropic"
protocol = "messages"
baseURL = "https://api.minimaxi.com/anthropic"
defaultModel = "MiniMax-M3"

[providers.minimax.auth]
type = "apikey"
apiKey = "<copy from /Volumes/extension/.rcc/provider/minimax/config.v2.toml>"
```

Then update active agents:

```toml
[agents.master]
provider = "minimax"

[agents.worker]
provider = "minimax"
```

### Reference Projects

Codex display reference:

- Root: `/Users/fanzhang/code/codex` or `/Volumes/extension/code/codex`
- First-pass discovered relevant paths:
  - `/Users/fanzhang/code/codex/codex-rs/rollout-trace/src/reducer/conversation.rs`
  - `/Users/fanzhang/code/codex/codex-rs/rollout-trace/src/reducer/tool.rs`
  - `/Users/fanzhang/code/codex/codex-rs/rollout-trace/src/model/conversation.rs`
  - `/Users/fanzhang/code/codex/codex-rs/core/src/tools/tool_dispatch_trace.rs`
  - `/Users/fanzhang/code/codex/codex-rs/core/src/tools/lifecycle.rs`
- Next worker must inspect the actual Codex TUI/UI rendering entrypoints before implementation. Do not assume the reducer files are sufficient.

Reasonix persistence/context reference:

- Root found locally: `/Volumes/extension/code/DeepSeek-Reasonix`
- Relevant files:
  - `/Volumes/extension/code/DeepSeek-Reasonix/desktop/app.go`
  - `/Volumes/extension/code/DeepSeek-Reasonix/desktop/wire.go`
  - `/Volumes/extension/code/DeepSeek-Reasonix/desktop/frontend/src/lib/useController.ts`
  - `/Volumes/extension/code/DeepSeek-Reasonix/desktop/frontend/src/components/Transcript.tsx`
  - `/Volumes/extension/code/DeepSeek-Reasonix/desktop/frontend/src/components/ToolCard.tsx`

Reasonix behavior to preserve:

- Restore sessions/tabs from persisted session history first.
- Rebuild transcript from persisted history before applying live events.
- Treat live stream events as incremental overlay, not the source of historical truth.
- Match tool dispatch/result by stable id and update one item.
- Replay pending prompts after subscription reconnect so the UI never waits silently.

## Design Rules

1. Session truth is authoritative history; ADP is live control/status overlay.
2. UI clients must not reconstruct history from live ADP events only.
3. UI clients may store selected session id locally, but not transcript truth.
4. `ui.protocol` owns query/projection semantics; WebUI owns DOM rendering only.
5. Runtime bootstrap must restore persisted sessions into queryable UI state before serving WebUI as ready.
6. Tool display must be semantic and low-noise: tool name, target summary, status, elapsed time, outcome.
7. Verbose terms, raw args, raw provider payload, debug lines, and stack details belong behind details/debug panels, not the primary transcript.
8. Normal tool execution failures are model-visible tool results; system/runtime failures are user-visible terminal/system errors.
9. No fallback or silent degradation: missing persistence, missing ADP, bad config, permission failure, transport failure, and runtime failure must be visible and testable.
10. Do not add duplicate session/tool projection logic in WebUI if the semantic belongs in `freehand-ui-protocol` or `freehand-blocks`.
11. Shortcuts and slash commands are input affordances only; they must call the same ADP query/command functions as buttons and must not mutate session/reason truth directly.

## Owner And File Map

Feature owners to inspect before editing:

- `app.webui-smoke`
  - function map: `docs/function-maps/app.webui-smoke.md`
  - test design: `docs/testing/app.webui-smoke.md`
  - owner paths:
    - `apps/freehand-server/src/lib.rs`
    - `apps/freehand-server/src/page.rs`
    - `apps/freehand-server/assets/webui.js`
    - `apps/freehand-server/assets/webui.css`
- `ui.protocol`
  - function map: `docs/function-maps/ui.protocol.md`
  - test design: `docs/testing/ui.protocol.md`
  - owner path: `crates/freehand-ui-protocol/src/lib.rs`
- `runtime.ui-command-dispatch`
  - function map: `docs/function-maps/runtime.ui-command-dispatch.md`
  - test design: `docs/testing/runtime.ui-command-dispatch.md`
  - owner path: `crates/freehand-runtime/src/lib.rs`
- `reason.persistence`
  - function map: `docs/function-maps/reason.persistence.md`
  - test design: `docs/testing/reason.persistence.md`
  - owner paths:
    - `crates/freehand-reason/src/persistence.rs`
    - `crates/freehand-reason/src/lib.rs`
- `config.core`
  - function map: `docs/function-maps/config.core.md`
  - test design: `docs/testing/config.core.md`
  - owner path: `crates/freehand-config/src/lib.rs`

## Technical Plan

### 1. Stabilize Working State

- Inspect `git status --short`.
- Do not revert unrelated WIP.
- If current WIP already contains partial session query/runtime restore work, either complete and verify it or isolate it before broader UI work.
- Fix any existing failing test introduced by partial work before claiming a clean baseline.

### 2. Align Minimax Runtime Config

- Copy the API key from `/Volumes/extension/.rcc/provider/minimax/config.v2.toml` into a Freehand-compatible provider entry without printing the secret.
- Add or update `[providers.minimax]` in `~/.freehand/config.toml`.
- Point active agents to `provider = "minimax"` if live validation should use Minimax.
- Validate config-selected daemon startup from the real `~/.freehand/config.toml`.
- Verify fixed port health: `curl -4fsS http://127.0.0.1:4041/health`.

### 3. Reference Codex And Reasonix

- Inspect Codex UI/TUI renderer and conversation/tool lifecycle reducers.
- Extract only behavior rules, not code:
  - chronological transcript order
  - tool call summary wording
  - waiting/running/completed/failed states
  - error placement and terminal status
  - collapsed vs expanded details
- Inspect Reasonix session restore and transcript rebuild path.
- Record findings in `note.md`.
- Update this goal doc or related design docs only if new durable requirements are discovered.

### 4. Lock Session Query Truth

- Ensure `ui.protocol` has query shapes for:
  - session list
  - session turns/transcript for a specific session
  - latest active turn
  - debug state for selected turn
- Ensure query and subscribe remain separate.
- Add tests proving:
  - session list is ordered deterministically
  - session turn query does not leak turns across sessions
  - empty session set is explicit, not fallback to latest active turn
  - persisted closed turns appear after restart/bootstrap

### 5. Restore Persisted Sessions Into UI State

- Runtime startup must load authoritative reason persistence and materialize queryable session state into `UiProtocolState`.
- ADP live events must overlay the selected current turn without becoming the only history source.
- Add tests proving:
  - daemon/runtime bootstrap restores all persisted sessions
  - active/latest state does not replace historical selected session transcript
  - corrupt persistence fails explicitly

### 6. Rewrite WebUI Rendering Around Session + ADP

- WebUI page model:
  - session sidebar/list
  - selected session transcript
  - composer bound to active session
  - live status bar derived from ADP state
  - optional details/debug panel
- On load:
  - connect ADP
  - query session list
  - select stored session id if still present
  - query selected session transcript
  - subscribe to latest/session updates
- On refresh:
  - selected session id may come from localStorage
  - transcript must come from protocol/session query
  - UI must not lose history
- On submit:
  - clear composer immediately
  - append visible pending user turn in the same transcript order
  - render waiting state until ADP/session updates arrive
  - reconcile pending item with session truth by stable id

### 7. Tool And Error Rendering

- Tool item main line should show:
  - action verb
  - target summary
  - state: waiting/running/completed/failed
  - elapsed time while non-terminal
  - compact result summary when terminal
- Tool details panel may show args/result/debug, but main transcript must not be raw term spam.
- Tool dispatch/result updates must target one stable item.
- Normal tool execution failure must re-enter model loop as tool-result/error observation if the model can continue.
- Runtime/system failures must render as explicit failure in the same turn and terminalize only when owner semantics say terminal.
- Add paired success/failure/non-terminal/already-terminal tests.

### 7.1 Keyboard Shortcuts And Slash Commands

Required shortcuts:

- `Cmd/Ctrl+Enter`: submit current composer text.
- `Escape`: cancel current active turn or clear local pending input through existing cancel flow.
- `Cmd/Ctrl+R`: refresh sessions, selected transcript, latest turn, debug, and checkpoints through ADP queries.
- `Cmd/Ctrl+K`: focus composer.
- `Cmd/Ctrl+1`: load success sample.
- `Cmd/Ctrl+2`: load failure sample.

Required slash commands:

- `/help`: show available slash commands and shortcuts in command status.
- `/sessions`: refresh session list and selected transcript.
- `/reload`: run the full ADP refresh path.
- `/success`: load the success sample into composer.
- `/failure`: load the failure sample into composer.
- `/cancel`: run the existing cancel path.
- `/clear`: clear only local pending input/composer, not persisted transcript.

Slash commands must be handled before normal submit and must not create fake user turns.

### 8. Screenshot And Live Evidence

Save screenshots under a deterministic evidence directory, for example:

- `artifacts/webui-session-alignment/<timestamp>/01-session-list.png`
- `artifacts/webui-session-alignment/<timestamp>/02-reload-restored-transcript.png`
- `artifacts/webui-session-alignment/<timestamp>/03-multiturn-order.png`
- `artifacts/webui-session-alignment/<timestamp>/04-tool-waiting-animation.png`
- `artifacts/webui-session-alignment/<timestamp>/05-tool-completed-one-card.png`
- `artifacts/webui-session-alignment/<timestamp>/06-tool-failure-model-continues-or-system-error.png`
- `artifacts/webui-session-alignment/<timestamp>/07-success-sample-terminal.png`
- `artifacts/webui-session-alignment/<timestamp>/08-failure-sample-visible.png`

Screenshot evidence must be produced by actual page operation on the fixed WebUI port, not static HTML inspection alone.

## Verification Matrix

### Static And Unit Verification

- `node --check apps/freehand-server/assets/webui.js`
- `cargo test -p freehand-ui-protocol`
- `cargo test -p freehand-reason`
- `cargo test -p freehand-runtime`
- `cargo test -p freehand-server`

### Targeted Regression Verification

- Session list query success and empty-state tests.
- Session turns query success and cross-session negative tests.
- Runtime bootstrap persisted-session restore success and corrupt-persistence failure tests.
- Tool lifecycle update tests for waiting, completed, failed, non-terminal, and already-terminal states.
- Tool execution-result failure continues model loop when semantically recoverable.
- Runtime/system failure projects explicit terminal/user-visible failure without becoming a tool result.

### Mainline And Gate Verification

- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `make ci`

### Headless ADP Verification

- `freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp`
- `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success`
- `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure`
- Add or use a headless session query command/API to prove session list and selected transcript without browser DOM.

### Live Browser Verification

- Start or restart the installed daemon on fixed port.
- Open `http://127.0.0.1:4041/`.
- Run at least two turns in one session.
- Refresh the browser and prove history remains.
- Restart daemon and prove session list/transcript restore from persistence.
- Run success and failure samples from WebUI.
- Trigger at least one tool lifecycle with visible waiting animation and one-card update.
- Save screenshots listed in the evidence plan.

## Risks And Mitigations

- Risk: WebUI uses ADP as history truth.
  - Mitigation: session transcript query must render first; ADP only overlays live state.
- Risk: tool failures are treated as system stop.
  - Mitigation: split execution-result failure from runtime/system failure in protocol and tests.
- Risk: refresh recovery works only from browser localStorage.
  - Mitigation: localStorage may store selected id only; transcript must come from session query.
- Risk: Codex/Reasonix alignment becomes visual imitation instead of behavior alignment.
  - Mitigation: extract behavior rules and lock with tests/screenshots.
- Risk: Minimax validation accidentally uses old `minimonth`.
  - Mitigation: assert selected agent provider id/base URL/model before live validation.

## Implementation Order

1. Read project rules, `CACHE.md`, `MEMORY.md`, `note.md`, owner function maps, and test designs.
2. Record current dirty worktree and avoid reverting unrelated changes.
3. Align and validate Minimax runtime config if live provider testing is required.
4. Inspect Codex renderer/reducer and Reasonix session restore/transcript paths.
5. Update test design and function maps before changing behavior.
6. Implement or finish `ui.protocol` session list/session turns query truth.
7. Implement or finish runtime bootstrap restoring persisted sessions into `UiProtocolState`.
8. Rewrite WebUI load/refresh/submit rendering around `session + ADP overlay`.
9. Implement low-noise one-item tool lifecycle rendering and error classification display.
10. Add success/failure/non-terminal/already-terminal tests.
11. Run targeted tests, mainline generation/checks, gates, and `make ci`.
12. Run fixed-port live ADP and WebUI validation.
13. Capture screenshots and record paths in final report.
14. Update `note.md`, promote verified durable conclusions to `MEMORY.md`, and commit only the relevant verified changes.

## Definition Of Done

Done means:

- Current Freehand runtime Minimax provider selection is explicit and verified before live reasoning checks.
- WebUI history survives browser refresh and daemon restart.
- WebUI renders multi-turn transcript order correctly from session truth.
- ADP live state updates the current transcript without replacing history truth.
- Tool calls update one semantic item with visible waiting/completed/failed states.
- Tool execution failures and system failures are separated in model loop behavior and UI projection.
- Success and failure samples pass through headless ADP and real WebUI.
- Screenshot evidence exists on disk and is referenced in the final report.
- Required tests, mainline checks, gates, and `make ci` pass, or any unrun check is explicitly reported with the reason.
