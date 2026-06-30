# note.md

# 2026-06-30 WebUI session CRUD and tool card follow-up
  - user correction: WebUI still could not multi-select/delete sessions, so CRUD was not usable despite ADP/session protocol support.
  - implementation:
    - sidebar adds visible `session-bulk-toolbar` with selected count, Clear, and Delete
    - session rows now use checkboxes for multi-select and a separate session button for navigation
    - Delete sends ADP `DeleteSession` for every selected session, clears local selection, refreshes `QuerySessionList`/selected transcript
    - server smoke asserts HTML/JS include multi-select/delete controls and ADP `DeleteSession`
  - tool display follow-up:
    - removed old live wait helper path that could render extra waiting cards
    - tool card status bar now owns waiting/completed timing; card body only shows semantic target/result
    - `tool.display` projects `bash pwd` as `Read current working directory` without `command=pwd`
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `cargo test -p freehand-blocks -- --nocapture` -> 39 passed
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 39 passed
    - `target/debug/xtask mainlines generate/check` and `target/debug/xtask gates check` -> ok
    - `scripts/install-global.sh` completed full Rust/Android release regression and installed matching `~/.local/bin/freehand-daemon`
    - `scripts/install-launchd.sh restart` restarted fixed `127.0.0.1:4041`
    - fixed-port HTML contains `session-bulk-count`, `session-clear-selection-button`, `session-delete-selected-button`
    - fixed-port JS contains `selectedSessionIds`, `deleteSelectedSessions`, `DeleteSession`, `session-selector`, `renderSessionBulkToolbar`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed

# 2026-06-30 fixed daemon port verification
  - occupied launchd service `com.freehand.daemon` stopped first with `launchctl stop gui/$(id -u)/com.freehand.daemon`
  - exact PID `49108` was terminated only after stop did not exit it
  - `scripts/install-launchd.sh install` rebuilt/reinstalled and relaunched the daemon
  - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
  - `~/.local/bin/freehand-cli adp-session-manage --url ws://127.0.0.1:4041/adp --action create --session webui-fixed-4041-check --title 'Fixed 4041 Check' --cwd /Volumes/extension/code/freehand` -> success
  - empty cwd create via `adp-session-manage` failed explicitly as `empty_session_cwd`

# 2026-06-30 WebUI session bulk select-all and typography follow-up
  - user request:
    - add a `Select all` action to the session bulk toolbar
    - small text should not use heavy bold weight
  - user correction after screenshot:
    - `Select all` showed `9 selected` while several visible sessions were still unchecked
    - root cause was `isDraftSessionId(sessionId)` using `sessionId.startsWith("webui-session-")`
    - persisted real sessions also use `webui-session-*`, so bulk select skipped them as if they were draft
  - implementation:
    - page shell now renders `session-select-all-button`
    - WebUI JS selects all non-draft session ids into the existing multi-select set
    - draft status is now only `state.draftSessionId === sessionId`; id prefix is never used
    - bulk toolbar is split into a summary row and a wrapping action row
    - small bulk count/button text is normal weight (`font-weight: 500`) instead of the previous heavy look
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - fixed-port HTML at `127.0.0.1:4041` contains `session-select-all-button`, `session-clear-selection-button`, `session-delete-selected-button`, and `session-bulk-count`
    - fixed-port JS at `127.0.0.1:4041` contains `selectAllSessions`, `sessionSelectAllButton`, `selectedSessionIds`, `draftSessionId: null`, and `state.draftSessionId === sessionId`; it no longer contains `startsWith("webui-session-")`
    - fixed-port CSS at `127.0.0.1:4041` contains `session-bulk-summary`, `session-bulk-actions`, `session-bulk-button.select-all`, and `font-weight: 500`
    - Chrome AppleScript DOM click verification was blocked by Chrome's "Allow JavaScript from Apple Events" setting, so no visual DOM count proof was captured in this slice

# 2026-06-29 tool display semantic owner
  - user requirement:
    - tool classification must have a standard independent file and locked owner
    - every parser must be an independent function in an independent module
    - UI must not guess categories; UI only consumes parsed projection
  - implementation:
    - added `tool.display` owner in `crates/freehand-blocks/src/tool_display.rs`
    - added structured `ToolDisplayProjection` with kind/outcome/action/target/parameter_summary/summary/result_summary/fields/diff
    - added independent parser functions for read/list, file mutation, search, plan, shell, and generic tools
    - `ui.protocol` now attaches `UiToolActivity.display` during tool call projection and updates it on tool result projection
    - public tool summaries now prefer structured display action/summary/parameter/result over raw detail
    - WebUI `toolSummaryBody` consumes `display` fields and renders tool parameters/results as secondary grey lines instead of classifying raw tool text
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-blocks`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-cli -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live WebUI verification:
    - global install + launchd fixed-port restart completed
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok`
    - real WebUI page operation loaded `Failure sample`, submitted it, and captured screenshots under `artifacts/webui-tool-display-e2e/20260629-parameters/`
    - `03-tool-parameter-visible.png` captured in-flight read_file tool card with `path=definitely-missing-freehand-file.txt`
    - `05-reloaded-final-parameter.png` captured terminal state with `turn completed`, no waiting text, `Read file` card showing parameter and failed result, plus shell command card showing `command=... · timeout=60`
    - ADP truth for `webui-session-20260629044814-5eb78029` showed `runtime-turn-43-r4` Success with `display.parameter_summary=path=definitely-missing-freehand-file.txt`

# 2026-06-29 session CRUD protocol support
  - user requirement:
    - before UI presentation, session CRUD must be supported through shared protocol/runtime truth
    - WebUI must not invent local-only session management state
  - implementation:
    - `ui.protocol` added session management commands: `CreateSession`, `RenameSession`, `ArchiveSession`, `RestoreSession`, `DeleteSession`
    - `UiSessionSummary` and `UiSessionTranscriptProjection` now expose `title` and `archived`
    - `UiProtocolState` can project metadata-only sessions and archived-session list separately
    - `reason.persistence` owns `PersistedSessionMetadataEntry` in `~/.freehand/state/ui/<agent>/session-metadata.json`
    - `delete` is currently non-destructive delete-as-archive because physical deletion of turn truth needs explicit destructive lifecycle approval
    - `runtime.ui-command-dispatch` routes CRUD commands into `ReasonPersistence` and refreshes shared UI projection
    - `freehand-cli adp-session-manage` provides no-UI ADP CRUD control
  - verification:
    - `cargo test -p freehand-ui-protocol -- --nocapture` -> 38 passed
    - `cargo test -p freehand-reason -- --nocapture` -> 56 passed
    - `cargo test -p freehand-runtime -- --nocapture` -> 51 passed
    - `cargo test -p freehand-cli -- --nocapture` -> 12 passed
    - `cargo test -p freehand-server -- --nocapture` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo fmt --check`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
  - live ADP verification:
    - existing fixed daemon `127.0.0.1:4041` health and ADP smoke passed
    - current workspace daemon on `127.0.0.1:4092` health passed
    - `target/debug/freehand-cli adp-session-manage --action create --session adp-crud-session-20260629 --title 'ADP CRUD Session' --cwd /tmp` returned `target=reason.persistence status=session_metadata_updated`
    - rename returned `session_metadata_updated`
    - active query showed `adp-crud-session-20260629:0:empty`
    - archive hid it from active session list
    - restore made it visible again
    - delete hid it again through non-destructive archive semantics
    - unknown archive returned explicit `command_dispatch_target_not_found`
  - remaining risk:
    - WebUI session context menu/UI presentation is not implemented in this slice
    - archived-session list is protocol-supported but CLI query helper currently prints only active list unless extended further
    - 4092 debug daemon health still responded after validation, but process PID was not discoverable through allowed exact-PID lookup; no broad kill was attempted

# 2026-06-29 session cwd owner wiring
  - user requirement:
    - session must have a working directory and WebUI must allow choosing it
    - `/new` should not render system feedback as a chat message
  - implementation:
    - `SubmitUserInput.cwd` is protocol-owned and empty cwd is rejected as `empty_session_cwd`
    - `UiTurnProjection`, `UiSessionSummary`, and `UiSessionTranscriptProjection` expose cwd
    - runtime canonicalizes requested cwd, binds it to the selected session, persists it on `TurnRecord.cwd`, restores it after bootstrap, and inherits it on later same-session submits
    - tool execution uses `freehand-tools::with_workspace_root` so session cwd is passed as an explicit per-call workspace root instead of mutating process-global cwd/env
    - WebUI adds a cwd input, forwards `SubmitUserInput.cwd`, shows cwd in topbar/session metadata, and `/new` now renders a clean empty state instead of the old selected-session/no-turns chat card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-tools -- --nocapture`
    - `cargo test -p freehand-reason -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
  - live WebUI verification:
    - local WebUI smoke server on `127.0.0.1:4088` returned `/health=ok`
    - Playwright real page operation clicked `New session`, set cwd `/Volumes/extension/code/freehand`, verified no `selected session:` system card, submitted a prompt, and observed command receipt/status
    - screenshots:
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/01-initial-cwd-control.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/02-new-session-cwd-clean.png`
      - `artifacts/webui-session-cwd-e2e/20260629-session-cwd/03-submit-cwd-status.png`

# 2026-06-29 WebUI provider-request wait state repair
  - gap found:
    - submit/dispatch waiting was local WebUI state and did not prove provider request had been built/sent
    - runtime already emitted `RuntimeLive02ProviderRequestBuilt` debug truth, but ADP turn projection did not expose request-sent/model-response-waiting lifecycle state
  - fix:
    - `UiTurnProjection.model_request` carries protocol-owned request-sent/model-response-waiting state
    - `RuntimeLive02ProviderRequestBuilt` is mapped into `UiProtocolState::apply_model_request_waiting`
    - WebUI renders an animated "waiting for model response" card with elapsed time from protocol projection
    - model request wait clears on semantic response, tool call, tool result, usage, terminal, or error projection
  - verification:
    - `cargo fmt --check`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server -- --nocapture`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI tool-result and model-wait progress repair
  - gap found:
    - `ui.protocol` intentionally kept public tool summary body status-only, so WebUI could not show tool execution output
    - WebUI only timed submit/dispatch and tool execution waiting; after tool completion while waiting for model continuation, there was no dedicated timed lifecycle card
  - fix:
    - `UiToolActivity.detail` now carries completed/failed tool result detail from `ToolResultContract.output`
    - public tool summaries expose the protocol-projected result detail
    - WebUI tool cards render result detail together with status and elapsed execution time
    - WebUI renders a timed animated "waiting for model" card after completed/failed tool activity until terminal/update arrives
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol -- --nocapture`
    - `cargo test -p freehand-server`
    - `cargo test -p freehand-runtime -- --nocapture`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI lifecycle progress timing repair
  - gap found: tool waiting had animation/timer, but submit/dispatch waiting before first turn projection only had static pending/status text
  - fix:
    - WebUI records `submitStartedAt`
    - pending submit card renders animated running state and elapsed dispatch wait time
    - lifecycle status refreshes once per second for submit/dispatch and tool waiting
    - tool waiting status includes elapsed time in the main status strip as well as the tool card
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI assistant-card collapse repair
  - review found a remaining regression after multiround transcript merge: same logical turn still rendered multiple assistant cards because `derivePublicConversation` emitted one `AssistantText` per text chunk
  - fixed by collapsing assistant text inside each turn into one visible card while preserving tool summaries and terminal/error cards
  - verification:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`

# 2026-06-29 WebUI multiround restore closeout
  - verified live fixed-port daemon at `http://127.0.0.1:4041/` and `ws://127.0.0.1:4041/adp`
  - real browser evidence now shows one logical transcript item per execution cycle, with `runtime-turn-N` and `runtime-turn-N-rM` merged for display only
  - assistant cards now strip raw `<freehand_completion>...</freehand_completion>` blocks, while the Final card keeps user-facing completion content
  - restart restore now rebuilds UI projections from reason-ledger per-turn snapshots so earlier-round tool activity survives daemon restart
  - verification evidence:
    - `artifacts/webui-visual-session/20260629-multiround-slowtool-success/`
    - `artifacts/webui-visual-session/20260629-final-restore-merged-clean/`
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-runtime live_bootstrap_restores_multiround_tool_activity_into_ui_state`

# 2026-06-28 Minimonth config and WebUI alignment goal
  - config check:
    - requested source config `/Volumes/extension/.rcc/provider/minimonth/config.v2.toml` contains provider id `minimonth`, type `anthropic`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and a present API key
    - current runtime config `~/.freehand/config.toml` uses provider id `minimonth`, base URL `https://api.53hk.cn`, default model `MiniMax-M2.7`, and the active `master` and `worker` agents both point to `minimonth`
    - Freehand config schema requires explicit `protocol`; RCC `transportBackend` is not a Freehand runtime config field
  - goal doc:
    - added `docs/goals/webui-session-transcript-alignment-plan.md`
    - goal locks WebUI rendering to persisted session truth plus latest ADP overlay, Codex-style low-noise conversation/tool display, and Reasonix-style session restore/history rebuild
  - 2026-06-28 progress:
    - `~/.freehand/config.toml` updated to runtime provider `minimonth` with base URL `https://api.53hk.cn` and model `MiniMax-M2.7`; secret copied from RCC source without printing it
    - `freehand-cli --agent master` verified active provider `minimonth`, protocol `messages`, model `MiniMax-M2.7`, and Minimonth base URL
    - fixed session transcript ordering in `ui.protocol` and WebUI local overlay path so numeric turn ids such as `runtime-turn-10` do not sort before `runtime-turn-2`
    - added `freehand-cli adp-session-query --url ... [--session <id>]` for no-UI session list/transcript validation over ADP
    - WebUI input layer now has shortcut and slash-command affordances routed through existing ADP/query/cancel/sample helpers: `/help`, `/sessions`, `/reload`, `/success`, `/failure`, `/cancel`, `/clear`; shortcuts include Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2
    - targeted verification passed: `node --check apps/freehand-server/assets/webui.js`, `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-cli`, `cargo test -p freehand-server`, `cargo test -p freehand-runtime`
  - 2026-06-29 final smoke:
    - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`
    - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success` returned `adp_turn_sample_ok` with `turn=runtime-turn-41` and `rounds=1`

# 2026-06-28 WebUI conversation/session product repair
  - user correction:
    - current WebUI is not acceptable as a chat product because it behaves like a dashboard/slide surface
    - primary missing product contract: persistent session concept, session management, and refresh recovery
    - status/permission/tool failures must be rendered inside the conversation lifecycle, not as disconnected panels or silent failures
  - implementation direction:
    - add protocol-owned session list and session-turn query truth
    - WebUI restores selected session from localStorage and queries persisted/protocol state after refresh
    - page shell becomes normal chat layout: session list + conversation transcript + composer
    - permissions preflight/failure state will be attached to the same visible turn/status chain after session UI is stabilized
  - DeepSeek-Reasonix reference findings:
    - actual relevant implementation is `/Volumes/extension/code/DeepSeek-Reasonix/desktop`, not `~/code/reasonix`
    - Reasonix restores tabs/session paths on startup, persists session files only on turn completion, and front-end event subscription is live-only
    - front-end rebuilds visible transcript from session history first (`historyMessagesToItems`), then applies live events as updates (`turn_started`, `text`, `tool_dispatch`, `tool_result`, `turn_done`)
    - blocking prompts are explicitly replayed after subscription reconnect (`ReplayPendingPrompts`) so UI never waits silently without a visible pending action
    - Freehand equivalent must be: session/index/transcript is restart truth; ADP is latest live signal; WebUI/Android/CLI render session truth plus ADP deltas, never ADP-only history


# 2026-06-28 live tool failure UI projection repair
  - new real-session failure found after fixed-port/sample validation:
    - WebUI screenshot error was `dispatch port failure: failed to project live error turn from persistence: reason ledger sequence is invalid: expected 380, got 379`
    - real broken path is historical session `~/.freehand/ledgers/reason/master/runtime-session-master.jsonl`, not fresh ADP sample turns
    - current file inspection shows no internal blank lines and no extra trailing bytes beyond newline; `wc -l` > final `seq` came from counting the terminal newline, not an extra JSON row
    - likely real failure mode is restore racing a partially appended final ledger row during live error projection; current `load_reason_ledger` has no explicit "last line incomplete" recovery rule and parses whole file snapshot at once
    - WebUI also renders `adpFailure` before persisted conversation, so transport failure can visually preempt the user message / turn history
  - deeper runtime evidence after launchd restart:
    - historical `runtime-session-master` reason ledger contains old pre-`ToolResultContract.status` rows such as line 6 `tool_result={tool_call_id, output}` with no outer status field
    - historical metadata ledger `~/.freehand/ledgers/metadata/master/runtime-session-master.jsonl` line 495 contains two JSON objects concatenated on one physical line
    - metadata loader assumed one JSON object per line and metadata append had no file lock, so launchd bootstrap could fail on `trailing characters`

# 2026-06-28 live tool failure UI projection repair
  - root cause:
    - live bridge tool execution failure used to return `RuntimeLiveBridgeError::ToolExecutionFailed` before materializing failed turn truth, so protocol truth stayed active/non-terminal and WebUI could only show waiting
    - first repair wrote failed truth to persistence but runtime dispatch `Err` branch still skipped UI projection; fixed by refreshing `UiProtocolState` from authoritative persistence before returning dispatch failure
    - second live validation exposed a history-pollution bug: dispatch failure projection was aggregating all restored session turns; fixed by projecting only the current runtime-turn ordinal
    - failed terminal projection still left waiting tool activities as waiting; fixed `ui.protocol` to mark still-waiting tool activities `Failed`
  - locked by tests:
    - `live_bridge_fails_explicitly_on_unknown_tool_name`
    - `live_bridge_fails_explicitly_on_registered_unimplemented_tool_name`
    - `live_dispatch_projects_failed_tool_turn_into_ui_state` now covers consecutive failures without historical tool leakage
    - `failed_terminal_marks_waiting_tool_activity_failed`
  - validation passed:
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-runtime`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `scripts/install-global.sh`
    - `scripts/install-launchd.sh restart`
    - fixed-port daemon `http://127.0.0.1:4041/health` -> 200 `ok`
    - real command ingress with `ls path=~/code/codex` -> HTTP 500 explicit `command_dispatch_port_failure`
    - latest-turn query for `runtime-turn-17` -> `terminal_status=Failed`, one current `tool_activities[0].status=Failed`, terminal/error public cards failed
    - latest-turn SSE emitted same failed turn projection

# 2026-06-28 low-noise tool card rendering
  - UI truth gap:
    - protocol projected tool cards still carried verbose generic wording like `Tool call requested` / `Tool result returned`
    - user only needs core tool semantics, blocking state, elapsed waiting time, and success/failure outcome
  - direction:
    - keep semantic tool identity in the shared protocol projection
    - render tool cards as a single updating card per `tool_call_id`
    - let WebUI add local elapsed-time animation for waiting cards instead of exposing raw term/detail in the main stream

# 2026-06-28 launchd fixed-port daemon bootstrap root cause
  - root cause:
    - `freehand-daemon serve --agent master` uses `RuntimeCommandDispatcher::from_default_config()`
    - that path requires `HOME` to resolve `~/.freehand/config.toml`
    - launchd environment did not provide `HOME`, so daemon bootstrap failed before bind even though the process itself remained alive briefly
  - fix direction:
    - launchd install must inject explicit `HOME` into both `daemon.env` and plist environment

# 2026-06-28 launchd restart readiness closeout
  - observed failure mode:
    - `scripts/install-launchd.sh restart` returned before the daemon was actually ready for `GET /health`
    - immediate curl after restart could fail even though launchd had already kicked the service
  - root cause:
    - startup window race, not a fixed-port or ADP protocol regression
  - fix:
    - `scripts/install-launchd.sh install` and `scripts/install-launchd.sh restart` now wait for `/health` readiness before reporting success
  - validation:
    - `scripts/install-launchd.sh restart`
    - bounded poll reached `health_ready_after=2`
    - `curl -4fsS http://127.0.0.1:4041/health` -> `ok`
    - `freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` -> `adp_smoke_ok ... subscription_accepted ... query_result ... ingress_command_kind_mismatch`

# 2026-06-28 ADP success/failure sample closeout
  - added CLI/headless sample command:
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample success`
    - `freehand-cli adp-turn-sample --url ws://127.0.0.1:4041/adp --sample failure`
  - WebUI composer now has `Success sample` and `Failure sample` buttons that load the same prompts; actual submit still uses normal ADP command path
  - verification:
    - `cargo test -p freehand-cli` -> 11 passed
    - `cargo test -p freehand-server` -> 11 passed
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci` -> EXIT 0
    - `scripts/install-global.sh` -> EXIT 0
    - `scripts/install-launchd.sh restart`
    - installed fixed-port success sample -> `runtime-turn-21`, `terminal_status=Success`
    - installed fixed-port failure sample -> `runtime-turn-22`, `terminal_status=Failed`

# 2026-06-28 WebUI tool card/status repair
  - root cause: whole-turn projection could duplicate same `tool_call_id` as separate waiting activities, and WebUI rendered tool summaries as static cards without stable tool identity
  - fix:
    - `ui.protocol` now upserts duplicate tool calls by `tool_call_id`
    - public tool summaries carry `tool_call_id`
    - WebUI normalizes same-tool cards by `tool_call_id`, adds waiting animation, clears composer immediately on submit, and routes command status through one renderer
  - validation passed:
    - `node --check apps/freehand-server/assets/webui.js`
    - `cargo test -p freehand-ui-protocol`
    - `cargo test -p freehand-server`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`
    - live local smoke: `freehand-server webui-serve-smoke --bind 127.0.0.1:4062`, `curl /` returned WebUI shell containing composer/status elements

# 2026-06-28 review/launchd/ui projection closeout
  - verified after fixes:
    - runtime final multi-round projection now aggregates only cross-round `tool_calls` and `tool_results`
    - final visible text / usage / errors / terminal status come from the final round
    - WebUI debug 404 now renders as `debug pending`; SSE transport errors now render as `debug stream reconnecting`
    - launchd wrapper requires explicit `FREEHAND_DAEMON_BIN` and fails on prefix mismatch instead of silently running an old binary
  - validation passed:
    - `cargo test -p freehand-runtime`
    - `cargo test -p freehand-server`
    - `cargo test -p xtask`
    - `cargo run -p xtask -- mainlines generate`
    - `cargo run -p xtask -- mainlines check`
    - `cargo run -p xtask -- gates check`
    - `make ci`

- 2026-06-27 launchd global daemon install closeout
  - added service scripts:
    - `scripts/freehand-daemon-launchd.sh`
    - `scripts/install-launchd.sh`
    - `scripts/uninstall-launchd.sh`
  - real install executed: `scripts/install-launchd.sh` exit 0
  - installed real commands:
    - `~/.local/bin/freehand-cli`
    - `~/.local/bin/freehand-server`
    - `~/.local/bin/freehand-daemon`
    - `~/.local/bin/freehand-daemon-launchd`
  - LaunchAgent installed:
    - label `com.freehand.daemon`
    - plist `~/Library/LaunchAgents/com.freehand.daemon.plist`
    - env `~/.freehand/daemon.env` mode 0600
    - logs `~/.freehand/logs/daemon.stdout.log` and `~/.freehand/logs/daemon.stderr.log`
    - fixed WebUI `http://127.0.0.1:4041/`
    - `RunAtLoad=true`, `KeepAlive=true`
  - verified active daemon:
    - launchctl showed `pid = 55614`, then exact PID killed to verify KeepAlive
    - launchd restarted it as `pid = 65923`, `runs = 3`
    - `curl /health` -> 200 `ok`
    - `curl /` -> 200, 5040-byte Freehand WebUI HTML
  - stdout log contains `freehand-daemon listening on http://127.0.0.1:4041`
  - permission note: localhost bind needs no macOS Accessibility/Full Disk permission; changing bind to LAN/Tailscale may trigger one-time firewall prompt
  - reinstall behavior note: `scripts/install-launchd.sh install` intentionally re-copies host binaries via `scripts/install-global.sh`, so repeated reinstall can make macOS re-evaluate the daemon binary; ordinary restarts must use `scripts/install-launchd.sh restart` and should not rewrite install state

- 2026-06-27 release/global-install/daemon startup closeout
  - added release truth: `scripts/release.sh` runs `make ci`, Android JVM tests, Rust release binaries, Android release APK, and artifact staging
  - added global install truth: `scripts/install-global.sh` installs `freehand-cli`, `freehand-server`, `freehand-daemon` to `${FREEHAND_PREFIX:-$HOME/.local}/bin`
  - Android `assembleRelease` repeatedly hung/failed in Lint Vital (`lintVitalAnalyzeRelease`, `lintVitalRelease` missing intermediate files); fixed at config owner by `lint { checkReleaseBuilds = false }` in `apps/freehand-android/app/build.gradle.kts`
  - release script uses Gradle `--no-daemon` to avoid persistent release-script child process leakage
  - verified release script exit 0 with staged artifacts: `freehand-cli`, `freehand-server`, `freehand-daemon`, `freehand-android-release-unsigned.apk`
  - verified install-global exit 0 with temp `FREEHAND_PREFIX`; installed binaries executable
  - verified installed daemon startup with temp `~/.freehand/config.toml`: `freehand-daemon serve --agent master --bind 127.0.0.1:4059`, `/health` 200 `ok`, `/` 200 with WebUI HTML
  - config smoke lesson: first local topology requires paired agents to resolve the same pair token value; separate env names with different values fail bootstrap explicitly

- 2026-06-27 android-client doc alignment pass
  - current truth: Android scaffold already exists under `apps/freehand-android`
  - live render host: `apps/freehand-android/app/src/main/assets/bridge.html`
  - design preview: `apps/freehand-server/assets/mocks/android/mobile-mock.html`
  - plan update: align design / execution / testing docs to the real native shell + protocol-only bridge split

- 2026-06-24T08:00+08:00 android-client execution plan locked
  - reviewed: `apps/freehand-android/` (existing scaffold) vs `apps/freehand-server/assets/mocks/android/mobile-mock.html` (locked design)
  - gap: WebView loads crude `mobile-shell.html`; no SSE; `TimelineProjector` only handles a tiny subset
  - plan doc: `docs/design/android-client-v1-android-shell.md`
  - execution order:
    1. bundle mobile-mock.html+css into Android assets; flip WebView loadUrl
    2. add SSE subscribe to ProtocolClient
    3. expand TimelineProjector to full ui.protocol mapping
    4. JS bridge: snapshot from projector -> window.__freehand.applySnapshot
    5. wire native controllers to projector state
    6. command ingress + cancel via existing CommandIngress
    7. theme: dark/light via Android night mode
    8. local.properties for SDK path
    9. compile + adb install
    10. run integration smoke against running freehand-daemon
  - hard constraints unchanged: no direct reason/provider/node imports; only ui.protocol consumer + command ingress

## 2026-06-24T11:35:57.426Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T193523692-397984-479
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: Android 客户端 milestone 已完成：APK 编译通过，SSE 协议客户端+WebView 渲染壳+native 控制器全部就位，freehand-daemon SSE 路由验证通过。剩余 applySnapshot JS 函数需在 HTML 中补齐（下一步）。
- evidence: 1. assembleDebug BUILD SUCCESSFUL, app-debug.apk 6.4M 2. curl http://127.0.0.1:4040/ui/subscribe/turn/latest → 200 text/event-stream 3. SseEventStream.kt, TimelineProjector.kt, MainActivity.kt, HostConfig.kt 全部代码已就位

OkHttp 4.12 SSE 需要 okhttp-sse 单独 artifact；Android buildFeatures 没有 webView flag；HostConfig URL 必须与 freehand-server 真实路由对齐（latest-active-turn 不是 turn/latest）；emulator 在 exec_command PTY 退出后会被杀，无法在当前环境维持长跑

## 2026-06-24T13:04:05.537Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T210335356-398243-738
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB device 100.104.163.65:5555 connection refused; pairing code expired. Need new pairing code from device.
- evidence: adb connect 100.104.163.65:5555 -> Connection refused; adb pair -> protocol fault

Gradle 9.6 incompatible with AGP 8.2.2; must pin Gradle 8.7 via wrapper. ADB pairing codes expire quickly.

## 2026-06-24T13:40:03.410Z stopless learned

- requestId: openai-responses-XLC.key1-glm-5.2-20260624T213913294-398413-908
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: ADB 100.104.163.65:5555 connection refused, device wireless debug likely off. Need user to re-enable. Meanwhile fixing Gradle build issue (Gradle 9.6 incompatible with AGP 8.2.2, reverting to Gradle 8.7). Also need to sync all code changes, rebuild APK, then install once device is online.
- evidence: nc -zv 100.104.163.65 5555 -> Connection refused. adb connect 100.104.163.65:5555 -> failed. Device PLZ110 was previously connected via adb pair but connection lost after adb server restart.

ADB wireless debug ports expire; always need fresh connect. Gradle 9.6 incompatible with AGP 8.2.2.

## 2026-06-25T00:05:00+08:00 android-client tailscale-first closeout

- Android app no longer ships demo/mobile-shell assets as runtime truth.
- removed dead assets:
  - `apps/freehand-android/app/src/main/assets/mobile-mock.html`
  - `apps/freehand-android/app/src/main/assets/mobile-shell.html`
  - `apps/freehand-android/app/src/main/res/layout/activity_main.xml`
- runtime UI narrowed:
  - native side keeps the single input bar
  - drawer keeps only connection settings
  - fake session / agent quick-switch actions removed
- connection truth changed to tailscale-first:
  - bundled config host = `100.66.1.82`
  - bundled profile = `tailscale-main`
  - upgrade URLs switched to `100.66.1.82:4040`
  - `autoLanScan = false`
  - `HostStore.DEFAULT_HOST = 100.66.1.82`
  - old persisted localhost / `192.168.*` values are overridden by `MainActivity.selectPreferredHost()`
- build truth:
  - stale gradle wrapper lock at `~/.gradle/wrapper/dists/gradle-8.7-bin/.../gradle-8.7-bin.zip.lck`
  - removed stale lock, downloaded gradle 8.7 successfully
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- install truth:
  - `adb -s 100.104.163.65:5555 install -r .../app-debug.apk` -> Success
- packaged config truth:
  - `unzip -p apps/freehand-android/app/build/outputs/apk/debug/app-debug.apk assets/config/client.json`
  - confirms host `100.66.1.82`, profile `tailscale-main`, `autoLanScan=false`
- remaining runtime evidence gap:
  - device screenshots are still covered by lockscreen / black overlay
  - `dumpsys window` shows `com.freehand.android/.ui.MainActivity` as focused app/task, but screenshot evidence is not user-visible UI yet
  - this blocks final visual acceptance, not code/build/install truth

## 2026-06-24T23:59:36.327Z stopless learned

- requestId: openai-responses-orangeai.key1-glm-5.2-20260625T075914215-399715-2210
- sessionId: 019ec8e6-9975-7d63-bc73-db8708b21596
- stopReason: 闪退根因已修，app 已不闪退。但截图仍然是黑屏（设备锁屏层覆盖），且 daemon 未运行在 Tailscale 地址上，Android 连不上 SSE。下一步：启动 daemon 绑 0.0.0.0:4040，再截图验证 bridge.html 渲染。
- evidence: 1. SecurityException crash stacktrace: DaemonDiscovery.getWifiIp -> WifiManager.getConnectionInfo -> ACCESS_WIFI_STATE permission missing. 2. 新 APK 已重装并验证不闪退: pid=26255 alive, focusedApp=com.freehand.android. 3. APK 内 config 确认 host=100.66.1.82 profile=tailscale-main. 4. daemon 未运行在 0.0.0.0:4040，设备 SSE 连接返回空. 5. 截图 18KB 黑屏是设备锁屏层覆盖不是 app 问题.

DaemonDiscovery 的 scanLan 路径在 autoLanScan=false 时仍然可以通过 health check 失败 fallback 触发。必须在 discover() 入口就拦截，不能依赖 config flag。另外 Android 新安装的 APK 需要重新 install，不能假设 build 产物已部署。

## 2026-06-25T13:08:00+08:00 android-client protocol and daemon truth closed

- Android bundled daemon truth is now unified to `100.66.1.82:4041`.
- Removed dead Android discovery owner:
  - deleted `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonDiscovery.kt`
- Fixed Android command ingress protocol shape:
  - old wrong payload: `{"type":"SubmitUserInput","text":"..."}`
  - canonical payload now matches `UiCommand` serde external-tag form:
    - `{"SubmitUserInput":{"text":"..."}}`
    - `{"CancelLatestActiveTurn":{}}`
- Fixed old persisted-host override gap:
  - same Tailscale host + legacy port `4040` now upgrades to bundled `4041`
- Runtime truth verified on real daemon process:
  - `env FREEHAND_PAIR_TOKEN_SHARED=devpair target/debug/freehand-daemon serve --agent master --bind 127.0.0.1:4041`
  - `curl http://127.0.0.1:4041/health` -> `200 ok`
  - `curl http://127.0.0.1:4041/ui/query/latest-active-turn` after submit returns submitted turn projection
  - `curl -sN http://127.0.0.1:4041/ui/subscribe/turn/latest` emits canonical `event: turn`
  - submitted prompt `reply with one short sentence and valid freehand completion schema` completed with `terminal_status=Success`
- Android build truth reverified:
  - `cd apps/freehand-android && ./gradlew assembleDebug --no-daemon` -> BUILD SUCCESSFUL
- Remaining device-side blocker:
  - `adb connect 100.104.163.65:5555` -> `failed to authenticate`
  - TCP `5555` is reachable, but host cannot currently reinstall APK or capture fresh runtime logs until device re-authorizes ADB.

Current real root cause split:
- earlier `connected + daemon unreachable` was app-side premature connected-state mutation plus wrong port collision (`4040` hitting `fin`)
- current Android command failure root cause was protocol payload mismatch, now fixed

## 2026-06-26 数据/控制 分离审计 + MetadataKind 死变体清理

### 审计结论（见上）

### 死变体清理 - 已完成 commit 5eae53e

### provider adapter error 接入 metadata 中心化 - 已完成 commit e4542f7

用户指令：Gap 2 closure path 第 1-2 步。

设计决策：
- provider adapter crates 保持 protocol-only，不加 freehand-metadata 依赖
- metadata 写入发生在 runtime bridge 的 executor 错误返回路径（单次 + 流式）
- 新 pipeline node: `RuntimeLive05ProviderError`（MetadataKind::Provider）
- 新 helper: `record_provider_error_metadata` + `emit_provider_error_debug`
- 白盒测试: HTTP 500 → metadata ledger 写入 RuntimeLive05ProviderError

当前 gap 状态：
- provider error ✓（RuntimeLive05ProviderError）
- 请求构造成功路径 — 依赖 RuntimeLive02ProviderRequestBuilt（已有）
- 响应解析成功路径 — 依赖 raw capture callback（已有可观测性）
- OpenAI executor — 当前无 executor，未来接入时复用 RuntimeLive05ProviderError 模式

用户指令：物理删除 `MetadataKind::Control` + `MetadataKind::DebugLink`（两个变体生产代码 0 次使用）。

用户要求：审计当前"推理与请求响应生命周期"中数据链 vs metadata 控制流的隔离状态。
范围：只读 audit。无代码改动。

### A. 核心结构已就位

- `crates/freehand-metadata/src/lib.rs` 是 metadata 唯一 owner
  - `MetadataCenter` (in-memory) + `MetadataLedger` (durable JSONL)
  - `MetadataWriteOwner` / `MetadataWriteNode` / `MetadataSubject` / `MetadataEntry` / `MetadataEnvelope` / `MetadataKind`
  - `validate_metadata_envelope` 强制 owner/node/subject + 拒绝 request-like key (`request.*`/`payload.*`/`prompt.*`/`input.*`/`content`/`text`/`messages` 等)
  - `is_reserved_request_key` 在 rust 字符串层做白名单，是元数据与请求数据硬隔离的第一道闸
- `crates/freehand-debug/src/lib.rs` 是 debug 唯一 owner
  - `DebugHub` + 3 类 sink (Memory / Stdout / File JSONL / Replay)
  - 独立 `DebugObservationFailure` 流 (`DebugHub::subscribe_failures`)
  - 观测-only，禁止承载请求内容
- `crates/freehand-contracts/src/lib.rs` 持有请求节点类型 (`ReasonReq01..05`, `ReasonResp01..03`, `ErrorErr01`)
- 静态 gate: `xtask/src/main.rs::verify_data_control_boundaries`
  - 拒绝 `ReasonReq*` 携带 metadata/debug/control 字段或类型
  - 拒绝 metadata owner struct 携带 request payload 字段
  - 拒绝 metadata owner struct 携带 control execution payload (cancel token / retry / checkpoint / route policy / gate decision)
  - 拒绝 `Metadata*` 类型出现在 `crates/freehand-metadata` 之外
  - 红测在 `cargo test -p xtask`

### B. 中心化元数据写入路径（已落实）

- 单例 ledger 路径: `~/.freehand/ledgers/metadata/<agent_id>/<session_id>.jsonl`
  - 由 `crates/freehand-runtime/src/lib.rs::metadata_ledger_path` 唯一生成
  - 没有第二处拼路径的代码（`metadata_ledger_path` 仅在 `freehand-runtime` 内出现）
- 唯一写入 helper: `write_live_bridge_metadata` (in `freehand-runtime`)
  - 构造 `MetadataWriteOwner`（`feature_id="provider.reason-live-bridge"`, `crate_name="freehand-runtime"`, `symbol_path` 由 spec 传入）
  - 构造 `MetadataWriteNode`（`pipeline_node` 显式标注：`RuntimeLive01RestoreResolved` / `RuntimeLive02ProviderRequestBuilt` / `RuntimeLive03ToolExecuted` / `RuntimeLive04TurnClosed`）
  - 入参是 `RuntimeMetadataWriteSpec`，不是裸 string/JSON
- 已经接入 metadata 中心化的 producer（按写入次数统计）：
  - `freehand-reason` (`ReasonTurnEngine::write_metadata`): 2 处 (`start_turn` + `apply_provider_output`)
  - `freehand-runtime` (`write_live_bridge_metadata`): 5 个 pipeline_node 节点
  - `freehand-node` (`LocalNodeRuntime::write_metadata`): 6 个节点
- debug producer 与 metadata 中心化 producer 数量级相同：
  - `freehand-reason` (`emit_debug`): 14 处生命周期点
  - `freehand-runtime` (`emit_live_bridge_debug`): 5 个 pipeline_node 节点
  - `freehand-node` (`emit_debug`): 6 个节点
- metadata write failure 在所有三个 producer 都是显式错误（`MetadataWriteFailed` / `NodeRuntimeError::MetadataWriteFailed` / `RuntimeLiveBridgeError::MetadataFailed`），无 fallback 吞错

### C. 隔离现状 — 已锁住

- `MetadataKind` 在生产代码只使用 4 个变体：
  - `RuntimeState` 10 次
  - `Routing` 5 次
  - `Cache` 4 次
  - `Provider` 2 次
  - `Control` 0 次
  - `DebugLink` 0 次（声明了但无生产 producer，参见 F）
- `metadata_ledger_path` 是 metadata 持久化的唯一真源路径生成点
- `MetadataCenter::by_trace` 是当前唯一的 metadata 查询接口（按 trace_id 反查）
- 测试覆盖：metadata ledger append/reload、corrupt ledger reject、validation-failed ledger reject、metadata write failure 不污染 turn truth（reason/node 两边都锁）

### D. 已记录的 gap（`docs/architecture/architecture-gaps.md`）

- Gap 2 明确：`metadata.core` 的 provider/debug producer 未全覆盖
  - 未接 producer：`freehand-provider-anthropic` / `freehand-provider-openai` / `freehand-debug`
  - 状态：非违规，gate 不会拦，closure path 已写在 gap 文件里

### E. 用户原则对照

1. "数据链与 metadata 控制流要分离" — 已落实：metadata 中心独立 crate + 静态 gate + 类型级禁止（`is_reserved_request_key` + `xtask::is_forbidden_request_field_*`）
2. "metadata 需要统一中心管理，不能零散写" — 已落实：唯一 ledger 路径 = `metadata_ledger_path`；唯一 helper = `write_live_bridge_metadata`；3 个 producer 都使用同一中心；没有第二份 metadata owner struct
3. "需要写入记录" — 已落实：ledger append-only JSONL 持久化 + `load_records` 回放 + `by_trace` 查询 + 静态 metadata/request gate 把"想散写"的尝试拦在编译期

### F. 待办（更新于死变体删除 + Gap 1/2 closure 后）

1. ~~**`MetadataKind::DebugLink` 与 `Control` 是死变体** — 已物理删除（commit 5eae53e）~~
2. **provider 成功路径 metadata 写入**（Gap 2 剩余项）— RuntimeLive02ProviderRequestBuilt 前补充请求构造验证 metadata / 响应解析成功路径。当前已覆盖 error 路径，成功路径有 raw capture 兜底。优先级低。
3. **`MetadataCenter` 查询接口单一**（`by_trace` 之外）— 没有 `by_owner` / `by_kind` / `by_node` 维度。当前审计只能 grep `MetadataKind::`，多 producer 写入的可观测性受限于 trace_id 单一维度
4. **MetadataCenter 是 `Mutex<MetadataCenter>` 形式持有** — 写入串行化。`freehand-runtime` 多处持有同一个 `Arc<Mutex<MetadataCenter>>`，并发 producer 写入需要锁
5. **`verify_data_control_boundaries` 静态扫描只覆盖 `freehand-contracts` 的 `ReasonReq*`** — 不扫描 `ReasonReq*` 之外的请求节点（如 `ReasonReq04ToolCall`/`ReasonReq05ToolResultReentry`/`ReasonResp01..03`/`ErrorErr01`），不扫 `freehand-ui-protocol` 里的 `UiCommand` 是否携带 metadata/debug 字段。已知受限范围（gate 文件注释里没写）

## 2026-06-26T04:49:25.671Z stopless learned

- requestId: openai-responses-minimax.key1-MiniMax-M3-20260626T124912992-402594-347
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: MetadataKind 死变体 Control + DebugLink 物理删除并验证完成
- evidence: git commit 5eae53e；make ci EXIT 0；enum 验证 grep -A 5 只剩 4 个变体 (Routing/Provider/Cache/RuntimeState)

死变体必须按 hard rule 10 物理删除并同步 docs+gate+tests+memory，不能靠注释保留；本次 make ci 一次通过验证 enum + gate + docs sync 是闭环的

## 2026-06-26T05:18:48.331Z stopless learned

- requestId: openai-responses-mimo.key2-mimo-v2.5-20260626T131837059-403051-804
- sessionId: 019f0212-cf1b-7003-85bb-b0ada9de6601
- stopReason: 两轮 commit 均完成，make ci EXIT 0。第一轮物理删除 MetadataKind 死变体 Control+DebugLink；第二轮 provider adapter error 接入 metadata 中心化（RuntimeLive05ProviderError）。docs/function-map/mainline-test/gap 同步更新。
- evidence: git log: 5eae53e (delete dead variants) + e4542f7 (provider error metadata)。make ci EXIT 0。cargo test: 324 passed。白盒测试 live_bridge_writes_provider_error_metadata_on_executor_failure 验证 HTTP 500 → ledger 写入 RuntimeLive05ProviderError

1. metadata 与 debug 保持物理隔离比通过 DebugLink 变体交叉引用更干净 2. provider adapter 保持 protocol-only 不加 metadata 依赖，metadata 写入在 runtime bridge 错误返回路径 3. RuntimeLive05ProviderError 作为 provider error 的唯一 metadata 入口可复用于未来 OpenAI executor

## 2026-06-27 UI status/tool SSE repair

- Root cause: WebUI treated `/ui/query/debug/{turn}` 404 as command failure while turn SSE can arrive before debug snapshot; debug subscribe also returned 404, so late debug could not arrive over SSE. Existing tests only covered debug-present query/SSE, not turn-before-debug race.
- Fix path: `app.webui-smoke` keeps debug HTTP query snapshot-only but makes debug SSE wait for late snapshots; WebUI renders missing debug as pending instead of command failure.
- Tool lifecycle gap: `UiTurnProjection.tool_calls` only carried names, so WebUI could only guess running. Added `UiToolActivity` plus `apply_tool_result`; `reason.turn` broadcasts `ReasonBroadcastEvent::ToolResult`; runtime maps it into UI state so latest-turn SSE carries waiting -> completed updates.
- Locked by tests: `cargo test -p freehand-ui-protocol`, `cargo test -p freehand-server`, `cargo test -p freehand-reason`, `cargo test -p freehand-runtime`; mainline docs regenerated.

## 2026-06-28 ADP daemon control/status

- User direction: WebUI, Android, CLI, and headless automation should share ADP for status/query/control, so failures can be inspected through ADP instead of UI-specific guessing.
- Implemented `UiAdpRequest`/`UiAdpResponse` in `ui.protocol` and `/adp` WebSocket in shared server transport; daemon exposes it on fixed launchd port through the existing injected runtime dispatcher and shared `UiProtocolState`.
- Debug finding: ADP subscription must return explicit `SubscriptionAccepted`, otherwise clients cannot distinguish waiting from a dead connection; command dispatch also must not block the connection loop, or subscription status events can starve behind long provider work.
- Verified: `daemon_adp_websocket_controls_command_query_and_subscription`, `daemon_adp_rejects_query_sent_as_command_frame`, full `make ci`, global install, launchd restart, and real Node WebSocket smoke against `ws://127.0.0.1:4041/adp`.
2026-06-28: 生成 ADP unified UI closeout 计划文档 `docs/goals/adp-unified-ui-closeout-plan.md`，目标是把 WebUI / Android / CLI-headless 收口到 daemon `/adp` 统一控制面，HTTP/SSE 仅保留兼容路径，并同步补齐固定端口、后台守护、自动化验证与 docs/mainline/wiki 真源。

2026-06-28: WebUI default ADP slice landed locally. `page.rs` adds `data-adp-endpoint="/adp"`; `webui.js` now opens one ADP WebSocket and routes query/subscribe/command through `UiAdpResponse` frames. Removed default `fetch`/`EventSource` usage from WebUI live path; HTTP/SSE/POST routes remain compatibility surfaces. Verified `node --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`, local page/JS smoke on `127.0.0.1:4073`, and local WebSocket smoke on `ws://127.0.0.1:4074/adp` with accepted/event/query/failure frames.

2026-06-28: Android default ADP slice landed locally. `MainActivity` now wires `AdpEventStream` as the default live shell transport; `HostConfig.adpUrl` / `ClientConfig.adpPath` make `/adp` explicit; `TimelineProjector.applyAdp` consumes ADP query/subscription/failure frames and projects failure visibly to `bridge.html`. `ProtocolClient` and `SseEventStream` remain compatibility classes, not default shell path. Verified Android JVM tests, `xtask mainlines generate/check`, `xtask gates check`, and `cargo test -p freehand-server android_mock_route_returns_design_preview`.

2026-06-28: CLI/headless ADP smoke landed locally. `freehand-cli adp-smoke --url ws://.../adp` uses typed `UiAdpRequest/UiAdpResponse`, sends subscribe/query/query-as-command frames, and requires accepted/event/query plus explicit `ingress_command_kind_mismatch`. Verified `cargo test -p freehand-cli` with local mock WebSocket server and a real local `freehand-server webui-serve-smoke` `/adp` smoke.

## 2026-06-28 WebUI shortcuts slash closeout

- Live failure ADP sample re-run passed on fixed daemon: runtime-turn-32-r2, rounds=2, tool_executions=1, terminal_status=Success, read_file tool_activity status=Failed; proves tool execution failure is model-visible result, not system dispatch failure.
- WebUI JS contains shortcutHelp, keydown handlers for Cmd/Ctrl+Enter, Esc, Cmd/Ctrl+R, Cmd/Ctrl+K, Cmd/Ctrl+1, Cmd/Ctrl+2; slash commands /help /sessions /reload /success /failure /cancel /clear are present and server asset smoke locks them.

- After reinstall/restart, fixed-port served JS hash matched workspace hash 8b8df0fa84b37ec7c7802ca8ce5d7c88a2859ab2c7370e3655f68230f5195379.
- Full install-global passed, launchd pid 27507 healthy on 127.0.0.1:4041.
- Live ADP smoke passed. Sequential failure sample passed as runtime-turn-33-r2 with rounds=2/tool_executions=1. Sequential success sample passed as runtime-turn-35 with terminal_status=Success.
- Verification caution: running success/failure samples concurrently can produce command_dispatch_port_failure because runtime dispatch has a single active turn boundary; do not treat parallel sample verification as valid positive evidence.
- Found and fixed WebUI slash UX bug: liveTurnStatus always overrode local commandStatus, so /help and /sessions looked inert on completed turns; slash inputs also remained in composer. Added sticky command status and slash input consumption.

## 2026-06-28 goal completion audit continuation

- Objective file re-read: `/Users/fanzhang/.codex/attachments/098e982a-74ac-494e-8e66-6ebb387506f0/pasted-text-1.txt`.
- Minimax config current evidence:
  - RCC source `/Volumes/extension/.rcc/provider/minimax/config.v2.toml` declares providerId/id `minimax`, `type=anthropic`, `baseURL=https://api.minimaxi.com/anthropic`, `defaultModel=MiniMax-M3`.
  - Runtime truth `~/.freehand/config.toml` matches `provider=minimax`, `protocol=messages`, `defaultModel=MiniMax-M3`, and master/worker agents use `provider = "minimax"`.
  - `FREEHAND_PAIR_TOKEN_SHARED=test-pair-token ~/.local/bin/freehand-cli --agent master` and source `cargo run -p freehand-cli -- --agent master` both printed `provider=minimax provider_protocol=messages default_model=MiniMax-M3 base_url=https://api.minimaxi.com/anthropic`.
- Fixed daemon evidence:
  - `launchctl print gui/$(id -u)/com.freehand.daemon` showed `state = running`, pid `27507`, `keepalive | runatload`.
  - `curl -4fsS http://127.0.0.1:4041/health` returned `ok`.
- Missing screenshot evidence added under `artifacts/webui-session-alignment/20260628-continued/`:
  - `12-before-daemon-restart-history.png`: pre-restart selected session/history state.
  - `13-after-daemon-restart-session-restored.png`: service-scoped launchd restart + reload restored `runtime-session-master` and prior latest turn `runtime-turn-36-r2`.
  - `14-webui-bash-submit-cleared-pending.png`: WebUI submit cleared composer and showed pending/dispatching state.
  - `15-webui-tool-waiting-animation.png`: WebUI showed tool waiting/running state during `bash sleep 8`.
  - `16-webui-tool-completed-after-wait.png`: WebUI showed completed tool turn after wait sample.
- Explicit gate evidence:
  - `make ci` rerun with log `/tmp/freehand-make-ci.log`, tail showed `xtask mainlines check: ok`, `xtask gates check: ok`, and `MAKE_CI_EXIT=0`.
- Current post-restart headless ADP evidence:
  - `~/.local/bin/freehand-cli adp-smoke --url ws://127.0.0.1:4041/adp` passed with `subscription_accepted`, `subscription_event`, `query_result`, and `ingress_command_kind_mismatch`.
  - Sequential success sample passed: `runtime-turn-38`, `rounds=1`, `tool_executions=0`.
  - Sequential failure sample passed: `runtime-turn-39-r2`, `rounds=2`, `tool_executions=1`.
  - `adp-session-query --session runtime-session-master` returned 36 ordered turns through `runtime-turn-39-r2`.
- Latest projection for `runtime-turn-39-r2` has `terminal_status=Success` and one `read_file` tool activity with `status=Failed`.

2026-06-29: Current `/new` session issue is a render-state bug, not an ADP transport bug. The draft session can be created, but empty `QuerySessionTurns` results must not clear `draftSessionId`, and the main transcript should prefer the selected session over the global latest turn. Also avoid timestamp-only draft IDs because repeated `/new` can collide inside the same second.

2026-06-29: WebUI layering contract clarified. ADP / `ui.protocol` stays stable; UI expands only in control and presentation layers. Session attachments must be session-scoped, history must use placeholders only, and draft attachments must clear on success while surviving failure for retry. New durable docs: `docs/design/webui-layered-controls-and-attachments.md` and `docs/goals/webui-layered-controls-attachments-plan.md`.

2026-06-29: WebUI layered controls first implementation landed. Controls: attach file/image/video, preview, selected-session refresh, read-only model selector. Attachment draft metadata is session-scoped in localStorage, current page `File` handles are kept for retry, restored metadata is marked metadata-only, and submitted text gets placeholder lines only. Success clears draft after ADP command receipt; ADP timeout/dispatch failure restores composer text and retains attachment draft. Evidence: `node --check`, `cargo test -p freehand-server`, `cargo test -p freehand-ui-protocol`, `xtask mainlines generate/check`, `xtask gates check`, `make ci MAKE_CI_EXIT=0`, ADP smoke on `4079`, screenshots under `artifacts/webui-layered-controls-e2e/20260629-layered-attachments-success-v2/` and `...failure-v2/`.

2026-06-29: WebUI selected-session reload evidence added under `artifacts/webui-layered-controls-e2e/20260629-layered-session-reload-v2/`; before and after reload both showed `strip-session=session-webui-smoke` and `conversation=turn-webui-smoke`.

2026-06-29: WebUI tool rendering收口继续压缩。当前 live 结论：历史 completed turn 不再闪，tool 终态只用 compact color dot 表达成功/失败；正文只保留一条核心语义线，参数或 diff 已经足够时不再重复 result summary。真实 daemon `127.0.0.1:4102` 验证成功，failure sample 截图见 `artifacts/webui-state-render-fix/20260629-compact-tool-display-live-v2/01-live-failure-sample.png`，计数为 `wait_model_continue_count=0`, `compact_tool_state_count=1`, `tool_block_count=1`, `running_state_count=0`.

2026-06-30: WebUI 新 session 工作目录选择修复。根因：cwd 协议链已存在，但 WebUI 只有 composer 底部手输 cwd，`/new`/New session 没有明确的新 session workspace 选择动作，并且无 cwd 时会静默创建 draft。修复：session rail 新增 `Workspace directory` + `Use for new session`，`/new` 和 New session 都要求先选择 cwd，draft submit 无 cwd 会显式阻断。验证：`node --check`, `cargo fmt --check`, `cargo test -p freehand-server`, `xtask mainlines generate/check`, `xtask gates check`；真实页面 `127.0.0.1:4103` 截图在 `artifacts/webui-cwd-session-e2e/20260630-new-session-workspace/`，负向 `no_cwd_status=new session requires a workspace directory`，正向 `strip_cwd=/Volumes/extension/code/freehand`，提交后 `strip_turn=runtime-turn-43` 且 cwd 保持绑定。

2026-06-30: 用户纠正 session/workspace 设计分层。正确方向：拆成基础 Agent 层和应用执行框架层。基础 Agent 负责 cwd 持久化、session 目录、全局 workspace 命名、权限、启动/生命周期、global/worker 工作模式、中断恢复；session 目录形如 `~/.freehand/sessions/local/<absolute-path-slash-to-minus>/<uuid>/`。应用层负责 WebUI/master agent 接收用户命令、理解任务、派发 worker、汇总结果；正常流程用户只对 master/global agent 对话，worker 由 master 派遣，WebUI 直连 worker 仅作为 debug 模式。此前 `afb046e` 强制 WebUI new session 必填 cwd 是应用层误修，应后续 forward-fix 删除/替换。

2026-06-30: WebUI forward-fix direction now locked: sidebar must show `New conversation` and `New task` as separate actions. `New conversation`/`/new` is global conversation and must not require cwd. `New task`/`/task` requires a visible task target cwd and uses existing ADP `CreateSession` metadata path for cwd-bound task session creation. Do not claim master-worker task dispatch is complete from this UI slice alone.
