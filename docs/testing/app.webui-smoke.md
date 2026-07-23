# Test Design: `app.webui-smoke`

- feature_id: `app.webui-smoke`
- owner: `apps/freehand-server`
- lifecycle path under test:
  - app boundary receives protocol-owned query/projection truth
  - app boundary receives protocol-owned command ingress intent and returns dispatch receipt/failure only
  - app boundary renders a usable protocol-driven WebUI shell
  - app boundary serves split theme, WebUI, and shared logo assets
  - app boundary renders a compact session rail with separate new-conversation and new-task affordances; new task requires a visible target cwd, while new conversation does not require cwd
  - app boundary renders protocol-owned debug query projection
  - app boundary renders slave-card visibility only for WebUI
  - CLI and WebUI divergences stay protocol-safe
  - app boundary remains decoupled from reason/provider/node/config semantics
  - app boundary serves protocol-owned HTTP query and SSE subscribe routes
  - app boundary can serve ADP runtime-query-port results without importing runtime/task/error-center owner crates
  - app boundary can serve ADP task list and error-center subscription initial snapshots through the runtime query port without importing runtime/task/error-center owner crates
  - WebUI Phase 2D status drawer can query and render TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl owner projections without browser-persisted task/control truth
  - WebUI mobile Agent Dashboard derives one presentation model from owner-backed projections; the Header shows current running Agent count, Worker task lifecycle buckets, and active/review/blocked task title, while the first tap opens only the current session's Worker child-task list
  - WebUI Header session relationship surface is canonical for Master/Worker navigation: collapsed state is a compact dashbar, expanded state is a dropdown session tree capped at half the viewport, and the Worker return path is in the Header
  - WebUI Master/Worker relationship tests lock schema fields, not UI copy: persisted session metadata is the Master source, and `UiTaskSnapshotProjection.parent_session_id`, `attached_session_ids`, `worker_session_id`, and `task_id` are the only Worker relationship source; DOM `data-session-id` / `data-task-id` are checked only as projections of those fields
  - tapping one Worker task selects the TaskBoard-projected `worker_session_id`, immediately clears the prior Agent transcript, queries that Worker session transcript, closes the task sheet, and renders the Worker conversation; the browser must not synthesize a worker session id
  - while a Worker transcript is selected, WebUI renders an explicit Header `Back to Master` control bound to the projected parent session; returning clears the Worker transcript before querying the parent, and the Header tree remains available to switch directly between sibling Worker sessions
  - Worker transcript refresh must tolerate large owner-projected `SessionTurns` payloads without semantic trimming; a refresh failure must render an explicit connection/error card instead of leaving a blank `New conversation` state
  - the mobile Agents runtime sheet is read-only navigation/current-session progress and must not expose Worker-limit mutation; the config-owned Worker limit is edited only in the system Config surface while the Header may keep a read-only Worker-limit projection
  - WebUI lifecycle projections scope task counts, task cards, Worker cards, AgentBoard rows, EventInbox rows, task-history targets, and Worker-control targets to the selected parent session; an empty selected session must render zero current lifecycle rows instead of falling back to global TaskBoard history
  - opening or closing the mobile Agent sheet is presentation-only and preserves selected session, transcript, composer draft, pending submit, scroll anchor, and lifecycle clocks
  - WebUI default command/query/status path uses ADP WebSocket `/adp`; latest-turn SSE is consumed as a display-refresh mirror
  - WebUI exposes hidden success/failure diagnostic prompts through slash commands and keyboard shortcuts while preserving the normal ADP submit path; persistent Success/Failure composer buttons must not render
- WebUI control strip and session rail expose session switching, `/new` New dialog, `/task` task mode in that dialog, refresh, cwd selection, model selection, attachment upload, file/image/video preview, slash commands, and keyboard shortcuts as input-layer affordances
- WebUI settings shell exposes the owner-backed provider registry, current primary/fallback selectors, provider definition add/update form, provider web_search live-test trigger/status, Worker-limit settings, and Android-only APK update bridge card without parsing or directly mutating daemon config files; read-only connection, active-agent, sessions/workspace, skills, files, tasks, and diagnostics projections are status/debug surfaces, not Settings cards
- WebUI aspect-ratio layout classifier applies presentation-only shape attributes for phone portrait, tall phone, phone landscape, tablet portrait, tablet landscape, foldable unfolded, and desktop large without mutating protocol/session state
- WebUI root route renders `?client=android-webview` with server-side `tablet_portrait` initial layout attributes before JS loads, while the normal browser root remains unpinned
- WebUI phone/tall-phone/tablet-portrait layout defaults to the conversation workspace; sessions and debug/config detail panels are hidden in explicit overlay drawers and never consume the normal conversation flow
  - WebUI mobile session drawer can be opened by a right-swipe gesture from the main interface content area without mutating ADP/session truth, selected session, transcript state, composer draft, pending submit, scroll anchor, or lifecycle timers
  - WebUI mobile session/settings drawers must keep a sticky visible header with close control while drawer content scrolls; Android/browser back intent must blur focused form controls first and then close the WebUI dialog/Header tree/Agent sheet/mobile drawer before app-level exit/navigation
  - WebUI session drawer renders persisted sessions as agent -> session hierarchy, with task/global labels derived from protocol cwd and CRUD still routed by protocol session id
  - WebUI session rail exposes rename, remove via `DeleteSession`, and double-Esc rollback as protocol commands instead of local session truth; archive/restore affordances are intentionally absent from WebUI
  - WebUI attachment lifecycle keeps draft attachments session-scoped, clears them only after successful send, and preserves them across send failure for retry
  - WebUI transcript history renders attachment placeholders rather than raw payload blobs
  - HTTP query and POST command ingress remain compatibility transport routes; latest-turn SSE subscribe refreshes visible turn display without owning command dispatch
  - WebUI Cancel button and Escape key send `CancelTurn` through command ingress when a nonterminal turn is active
  - WebUI Escape sends latest-active cancellation during submit-in-flight before a concrete `turn_id` is known
  - latest-turn subscribe should wait for the first turn instead of failing on blank state
- white-box plan:
  - page shell render helper
  - embedded asset serving helper
  - router and serve helper
  - dependency boundary scan for protocol-only app wiring
- module black-box plan:
  - WebUI root shell smoke, including shared logo reference
  - WebUI theme and shared logo asset smoke
  - WebUI JS asset smoke
  - Android update route smoke for env/sidecar manifest JSON, explicit missing-sidecar failure, and explicit missing-APK 404
  - WebUI JS asset smoke locks ADP WebSocket command/query usage, rejects `fetch` as a live path, and requires `EventSource` only for latest-turn SSE display refresh
  - WebUI ADP subscription accepted/waiting status rendering smoke
  - WebUI ADP failure frame visible-card/status smoke
  - WebUI ADP request timeout visible-failure smoke
- WebUI ADP failure card ordering smoke: failure card must not render ahead of the current conversation items
- WebUI asset and online smoke must prove user-visible status text, failure cards, and verifier-submitted diagnostic prompts do not expose `ADP`; protocol naming is allowed only in internal code, docs, CLI, and test harness output
  - WebUI hidden failure diagnostic prompts must stay within the active Master tool surface: current-cwd workspace tools such as `read_file` should succeed, so use cross-cwd workspace-boundary or unavailable-tool samples when a failed tool card is needed
  - WebUI hidden success/failure diagnostic prompt asset smoke and no persistent sample-button smoke
  - WebUI keyboard shortcut smoke for submit, cancel, refresh, focus composer, and sample loading
  - WebUI slash command smoke for `/help`, `/sessions`, `/reload`, `/success`, `/failure`, `/cancel`, and `/clear`
  - WebUI attachment control smoke for add/remove/preview and session-scoped draft retention
  - WebUI attachment success-clear smoke
  - WebUI attachment failure-retain smoke
- WebUI settings shell smoke for desktop/mobile entry points, complete registry rendering without a fixed provider count, owner-backed current/fallback selectors that keep fallback truth on initial load and provider upsert until the operator explicitly edits selector draft state, owner-backed definition add/update and selection command wiring, Android-only APK update card/bridge/status callback wiring, visible invalid-save errors, visible restart-required success, absence of unsupported read-only status cards, no API-key/password inputs, no credential text, and no direct config write helpers
- Server APK update route smoke must prove `/android/update.json` serves explicit runtime env override or compiled sidecar truth with no-store cache headers, `/android/freehand-android.apk` serves the signed staged APK, and that a missing sidecar does not return a hardcoded old `versionCode` as a false current-version success.
  - WebUI mobile settings drawer back/close smoke locks sticky drawer header CSS plus `window.__freehandHandleAndroidBack`: focused settings input is blurred on first back, a following back closes the settings drawer, and the conversation remains visible without native fallback UI
  - mobile Agent header renders owner-backed Worker limit and system max five; increment/decrement stays within `1..=5` and save routes only through `UpdateAgentResourceConfig`
  - Agent resource save failure remains visible and does not rewrite the displayed owner projection as success
  - WebUI submit-success path refresh smoke
  - WebUI cancel button / Escape key command smoke
  - WebUI submit-in-flight latest-active cancel smoke
  - WebUI rename/remove asset smoke and negative archive/restore affordance smoke
  - WebUI double-Esc rollback asset smoke proving first Esc arms rollback and second Esc calls `RollbackLatestSessionTurn`
  - WebUI per-turn action asset smoke proving every rendered `.chat-message` has Copy, Edit from here, and New session actions; Edit from here uses repeated `RollbackLatestSessionTurn` command dispatch until the selected logical turn is removed, while New session creates a protocol-owned conversation before prefilling the composer
  - WebUI command ingress dispatch receipt smoke
  - WebUI command ingress dispatch failure projection smoke
  - WebUI command ingress dispatch join-failure projection smoke
  - WebUI command ingress query-route-misuse rejection smoke
  - WebUI ADP query-as-command failure does not mutate state and renders failure
  - WebUI ADP runtime-query-port failure frame smoke
  - WebUI ADP task list subscription initial snapshot smoke
  - WebUI ADP error-center subscription initial snapshot smoke
  - WebUI Phase 2D dashboard asset smoke for TaskBoard, AgentBoard, EventInbox, TaskHistory, WorkerControl query/render functions, worker-control command routing, and absence of browser localStorage task/control truth
  - WebUI Header session tree asset smoke for `renderSessionRelationHeader`, `renderSessionTree`, `selectedParentSessionSummary`, `renderWorkerSessionNavigation`, `sessionTreeOpen`, `.session-dashbar`, `.session-tree-dropdown`, `.session-tree-node.is-worker`, `data-session-id`, `data-task-id`, `UiTaskSnapshotProjection`, `TaskBoard.worker_session_id`, and half-screen dropdown max-height
  - WebUI mobile Agent Dashboard asset smoke for `buildMobileAgentDashboardModel`, `renderMobileAgentSummaryStrip`, `renderMobileAgentSheet`, `renderMobileAgentTaskList`, `openWorkerTaskSession`, and `setMobileAgentSheetOpen`; shell/CSS smoke locks the portrait-only Header lifecycle summary, Worker-task dialog, shared scrim, grayscale base, minimal active/review/blocked/closed accents, all-current-child-task rendering with no fixed cap, and `data-task-id` / `data-worker-session-id` card identity
  - WebUI mobile Agent Dashboard asset smoke locks Worker labels to Worker-only resource ordinals; the Master row in AgentBoard must not shift Worker labels into off-by-one `Worker 2/3/4` display for a three-Worker topology
  - WebUI mobile Agent Dashboard negative asset smoke rejects browser-local Agent/task truth, browser-synthesized Worker session ids, unrelated historical tasks, and the removed Master evaluation/Agent/history/control sheet sections
  - WebUI selected-session switch asset smoke locks `sessionRefreshInFlight`, `selectedSessionIsLoading`, and loading/failure render helpers so switching to a Worker or sibling session never flashes the clean `New conversation` empty state before the pinned `QuerySessionTurns` result returns
  - WebUI selected-session Worker transcript asset smoke locks retryable handling for `Worker session ... has no persisted transcript`: if the selected session matches a TaskBoard-projected non-terminal `worker_session_id`, the UI renders `Worker transcript waiting`, task id/status/assignee evidence, schedules the same-session retry, and does not set a persistent connection-failure card; terminal or non-TaskBoard refresh errors remain explicit failures
  - WebUI selected-session lifecycle asset smoke requires the `parent_session_id` filter for TaskBoard-derived rows, AgentBoard rows, EventInbox rows, history targets, and control targets, and rejects the former `parentTasks.length > 0 ? parentTasks : allTasks` global fallback
  - WebUI query projection smoke
  - WebUI debug query projection smoke
  - WebUI latest-turn SSE initial snapshot plus later update smoke
  - WebUI blank-state latest-turn SSE wait smoke
  - WebUI debug SSE initial snapshot plus later update smoke
  - WebUI debug SSE waits when turn projection arrives before debug snapshot instead of surfacing transient 404 as command failure
  - WebUI debug SSE error rendering distinguishes reconnecting transport state from missing-snapshot pending state
  - WebUI latest-turn query/SSE public projection excludes raw completion schema, internal reasoning, and detailed tool terms from public conversation while preserving user input
  - WebUI latest-turn SSE renders tool lifecycle status updates (`waiting` then `completed`) from protocol truth
  - WebUI ADP turn updates render tool lifecycle status updates (`waiting`, `completed`, and `failed`) from protocol truth
- WebUI JS/CSS asset smoke locks no same-tool card normalization, no DOM text de-duplication, immediate composer clearing on submit, Up/Down input history recall, submit/dispatch waiting timers, typed model-response waiting timers from protocol projection, phase-key timer reset, absence of UI-inferred waiting-model timers, current-live-turn-only animation gating, static rendering for selected-transcript `model_request` rows that are not current live, neutral non-animated inactive waiting state, waiting animation assets, compact terminal tool state dots, protocol-structured tool cards that render `display.action`, `display.kind`, `display.target`, `display.result_summary`, `display.summary`, all `display.fields`, `display.diff`, command fields, and raw detail lines without reading obsolete `display.result`, filtering generic tool results, or compressing repeated tool text
- WebUI JS/CSS asset smoke locks conversation scroll anchoring: `renderMessages` records whether the operator is near bottom, honors a `userScrollLocked` state when the operator scrolls upward, never calls `scrollIntoView` during ordinary render, and only forces bottom on explicit local submit or when already bottom-pinned
- WebUI JS/CSS asset smoke locks composer bottom clearance: JS measures `.composer-card`, writes `--composer-clearance`, updates it on resize/visual viewport/composer size changes, and CSS uses it for `.message-list` plus fixed mobile composer layouts instead of relying on a stale fixed bottom padding
- WebUI JS/CSS asset smoke locks chat transcript layout, execution lifecycle colors, and inactive tool precursor projection: user bubbles must be right-aligned and user-colored, assistant bubbles left-aligned, tool activity embedded as a semantic tool block inside the assistant bubble, model/retry continuation rows italicized as reasoning, desktop running cards may use blue status borders, desktop success/completed cards green borders, desktop failed cards red borders, mobile phone/tall/tablet portrait cards must not draw left borders or inset strips and must use colored background blocks/state text instead, completed tool precursor rounds must not restore as neutral waiting cards, and failed tool precursor rounds must not hide as neutral waiting cards
- WebUI JS/CSS asset smoke locks chat message time rendering: every visible
  user/assistant `.chat-message` card must contain one non-empty
  `.chat-message-time` generated from protocol/receipt `created_at` truth,
  and the time label must remain visible in phone/tall/tablet portrait layouts
  without replacing lifecycle status text.
- WebUI JS/CSS asset smoke locks mobile v3 phone-first structure: visible mobile chrome must not render desktop internal strips (`topbar-strip`, `strip-session`, `strip-turn`, `strip-cwd`, `slave-drawer`, `slave-chip`), selected session/turn verifier truth must live on shell `data-*`, phone bubbles must be lightweight with no left edge strip, tool blocks must be compact semantic rows with color-block backgrounds, `result:` / `failure:` tool detail lines must stay visible, and focused composer may enlarge the textarea and reveal cancel but must not re-open the full attachment/CWD/model/status strip or reserve more than compact composer height in the conversation flow
- WebUI layout-shape verifier locks the pure classifier against required viewport pairs: phone portrait, tall phone, phone landscape, tablet portrait, tablet/foldable landscape, foldable unfolded, and desktop large
- WebUI root shell smoke locks both branches of first paint: normal root has no Android layout attributes, and `/?client=android-webview` has `data-layout-client="android-webview"` plus `data-layout-shape="tablet_portrait"` on `body` and shell
- WebUI online mobile drawer verifier locks content-area right-swipe opening, button opening, drawer close, and agent -> nested sessions hierarchy inside the drawer
- WebUI online mobile Agent Dashboard verifier locks compact strip visibility on phone/tall-phone/tablet portrait, strip-to-sheet opening, close-button and scrim closing, mutual exclusion with the session/detail/settings drawers, and hidden sheet on desktop
- WebUI online mobile Agent Dashboard screenshot proof waits for settled bottom-sheet geometry rather than only the open/closed body attribute, so phone/tablet captures cannot pass while the sheet is still entering or leaving the viewport
- WebUI online mobile Agent Dashboard verifier snapshots selected session, transcript count/text, composer draft, pending submit projection, scroll position, and lifecycle clock count before and after sheet toggles; all state must remain unchanged except lifecycle clocks may advance monotonically
- WebUI online mobile Agent Dashboard verifier compares Header running-Agent count plus Worker task lifecycle buckets and sheet child-task cards with the same daemon-backed TaskBoard/AgentBoard truth, and rejects visible unrelated historical tasks, `ADP`, `runtime-turn-*`, execution ids, and the removed Master-evaluation/history/control panels
- WebUI online Header session tree verifier must click the Header dashbar, assert the dropdown height is at most half the viewport, assert the Master root plus every current Worker child session appears from protocol schema truth, fail if any same-parent child task lacks `UiTaskSnapshotProjection.worker_session_id`, require each Worker node's `data-session-id` to equal the TaskBoard `worker_session_id` and `data-task-id` to equal the TaskBoard `task_id`, click that Worker node, prove the selected session equals the projected `worker_session_id`, then click Header `Back to Master` and prove exact parent restoration
- WebUI root/asset smoke asserts versioned HTML asset URLs plus `Cache-Control: no-store, max-age=0` on both root HTML and assets; Android online proof must force-stop/relaunch the app and show the new dashboard behavior without clearing app data
- WebUI online mobile Agent Dashboard verifier taps a Worker task, proves the old Master transcript is cleared immediately, waits for the sheet to close, and proves the selected session id equals the task's projected `worker_session_id` while the conversation renders at least one persisted Worker turn; it then uses Header `Back to Master` and proves the selected id/transcript return to the exact parent session; a missing worker session is an explicit failure, not an empty conversation
- WebUI online selected-session switch proof must capture the immediate post-click state as `Loading conversation` or persisted Worker content; it must reject a transient `New conversation` empty state while `QuerySessionTurns` is in flight and must reject stale Master transcript rows under the Worker selection
- WebUI online Worker transcript proof must verify large Worker sessions arrive as reason-persistence logical-turn projections without browser-side semantic trimming; repeated repair-round rows must be fixed before WebUI rendering, not hidden after DOM construction
- WebUI online settings verifier locks opening the settings panel on desktop/mobile, sticky visible drawer close behavior after scrolling the long settings form, rendering all configured provider ids, adding a provider without auto-selecting it, switching current provider through the owner command, restoring the original selection, invalid update/selection failure, restart-required success, no secret inputs or DOM leakage, and conversation rendering still intact after closing the panel
- Android WebView Settings proof for APK update must open Config in the installed app, verify the `Android APK update` card is enabled, tap `Check APK update`, and observe status evidence from the native bridge; desktop/browser proof may only verify the explicit Android-app-only unavailable state.
- WebUI online settings verifier locks that provider id, fallback provider id, provider host, provider auth source, and model values are not loading placeholders and come from the runtime query path; it must prove provider definition upsert preserves current primary/fallback selection and that only `UpdateAgentProviderSelection` changes the selected provider
- WebUI JS asset smoke locks the render projection boundary: `buildConversationRenderModel`, `buildRenderTurn`, `buildRenderRows`, `buildToolActivityRenderRow`, `buildModelRequestRenderRow`, `turnIsCurrentLiveTurn`, and `renderModelHasLiveLifecycle` must exist, while old global model-request status helpers must not return.
- WebUI JS asset smoke locks that visible turns come from one selector preserving transcript order and merging selected-session transcript with the latest same-session turn by replace-or-append, then render through `RenderConversation` / `RenderTurn` / `RenderRow`, so stale transcript state cannot hide an in-flight or newly completed continuation after submit, historical turns cannot inherit current live lifecycle animation, and restarted/runtime-reused turn ordinals do not move new cards above older transcript rows
- WebUI online black-box coverage must assert every rendered chat message in the
  selected Master/Worker transcript carries a non-empty `.chat-message-time`
  label after refresh, proving served assets consume protocol-created turn time
  rather than leaving timestamp display browser-local or absent.
- WebUI selected-session black-box coverage must prove a manually clicked completed session remains selected even when a different latest active/interrupted session exists; latest active turn may render only when no selected session is pinned
- WebUI online black-box coverage must capture screenshots and DOM state for the required viewport matrix and assert `body[data-layout-shape]`, shell `data-layout-shape`, composer visibility, message list visibility, mobile drawer hidden-by-default state for portrait shapes, mobile session/detail drawer open-close behavior, no desktop-strip chrome in phone screenshots, and no historical live animation after terminal refresh
- WebUI online terminal waits must recognize success/completed, blocked, failed, cancelled, and interrupted projections; a blocked live turn is terminal evidence, not a verifier timeout
- WebUI online black-box coverage must prove scroll behavior in both directions: while the operator is scrolled above the latest row, live/query refreshes do not change the conversation scroll host position; when the operator is already near bottom, the latest updates remain visible above the measured composer top
- WebUI online black-box coverage for live tool rendering uses `scripts/verify-webui-live-tool-render-online.mjs` with a fixed persisted session and a local Responses fixture: the first provider round returns a `task` function call, runtime executes the real tool path, the second provider request is delayed, and DOM/ADP evidence must show a tool card before final summary plus a visible model-continuation waiting row before terminal success. Because the fixed session retains prior verifier turns, every assertion must be scoped to the current run marker and newly added base/continuation turn ids; whole-transcript text matches are invalid. Every real current-run turn card must have the unique key `turn:<session_id>:<turn_id>`, the number and order of real cards must match current-run ADP turns, and a frozen first-round card must remain byte-for-byte equivalent after the continuation/final round appends. While the model request is live, the Header and Agent observation surfaces must expose the owning live session id, turn id, and waiting/retry label instead of only a global retry count. Final acceptance requires the current run's selected continuation to project `Success`, all current-run and global `[data-live="true"]` rows to be zero, and terminal truth to override an outstanding submit receipt so the UI cannot show final text as `dispatching`.
- WebUI online black-box coverage for provider recovery uses `scripts/verify-provider-recovery-webui-online.mjs` with fixed persisted session `webui-provider-recovery-fixed`; while its provider fixture is active it must set `FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER=1` so historical background Master lifecycle work cannot consume the temporary provider fixture, and final proof must show the fixed session's selected retry turn stayed one `.turn-cycle-card` with one user card before terminal success.
- WebUI online black-box coverage for stop/continue uses `scripts/verify-webui-stop-continue-online.mjs` with fixed persisted session `webui-stop-continue-fixed` and a local Responses fixture: after the first provider request returns HTTP 500, the browser must visibly render current provider activity before cancel; clicking Cancel must materialize ADP `Cancelled` truth, clear all DOM live/waiting labels before the retry backoff window elapses, and prevent any second pre-continue provider request. The same selected persisted session must then accept a new prompt, append a distinct later `Success` turn after the cancelled turn, preserve the cancelled card, and not create any new random persisted session ids.
- WebUI online black-box coverage must assert the global session list text does not expose internal lifecycle/timer/worker session ids such as `master-lifecycle-*`, `master-timer-*`, or top-level `worker-task-*`; direct debug queryability belongs to `ui.protocol` coverage
- WebUI online black-box coverage must remove existing sessions through ADP before `/new`, then assert the new draft session has `selectedTurn=-`, zero chat messages, clean empty-state text, and no prior success/failure/tool/waiting-data leakage
- WebUI online anti-regression coverage must prove the negative case where ADP session list is empty while `QueryLatestActiveTurn` can still return an old turn from a non-destructively deleted session; after browser refresh, the page must show a clean empty conversation with no selected turn, no chat cards, and no old `runtime-turn-*` text
- WebUI online Phase 2D coverage must query the same service endpoint for TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl, then assert lifecycle observer status text/card counts match service truth, the mobile Header shows current running Agent count plus Worker task lifecycle buckets, the first tap renders only current-session child tasks, every current child task is inspectable without a fixed list cap, a task tap queries its owner-projected Worker session and renders that conversation, and the visible drawer text does not expose raw `ADP`, `runtime-turn-*`, task id, or execution id plumbing
- WebUI mobile Agent Dashboard positive coverage must prove the Header and child-task list come from current-session TaskBoard/AgentBoard truth and that task selection refreshes `QuerySessionTurns` for the projected Worker session; `scripts/verify-worker-subtasks-online.py --parent-session <id>` is the read-only ADP checker for enumerating every current child task and verifying each projected Worker transcript one by one
- `node scripts/verify-webui-path-diagnostic-online.mjs` is the fixed-session
  WebUI online closure for path-tool diagnostics: it creates/reuses the fixed
  persisted parent session `webui-path-diagnostic-fixed-v2`, submits the prompt
  through the WebUI composer DOM, fixture-drives Master `task` dispatch with
  current-run `claim="waiting"` after the assign result re-enters the Master
  provider loop, fixture-drives the Worker `ls` call against
  `/Users/fanzhang/github/codex`, verifies the second Worker provider request
  contains the paired failed `tool_result` with full `path_diagnostic`, waits
  for the Master lifecycle coordinator to persist `task(op="append")`
  `blocked_decision` truth as `TaskProgressed`, clicks the Header session tree
  node whose `data-session-id` / `data-task-id` match the TaskBoard-projected
  `worker_session_id` / `task_id`, and asserts the parent transcript
  renders `waiting lifecycle`/`running` rather than completed while the Worker
  transcript renders `blocked` diagnostic truth without exposing internal
  Worker prompts or top-level `worker-task-*` sessions; the script keeps launchd restarts service-scoped and sets the
  existing `FREEHAND_LAUNCHD_HEALTH_WAIT_SECONDS` to 120 seconds by default so
  slow S-profile startup is measured by `/health` truth instead of a duplicate
  restart path.
- WebUI online mobile Agent Dashboard proof for Worker limit must reject task-card labels above the configured Worker resource range, so `worker`, `worker-2`, and `worker-3` render as Worker 1/2/3 even when AgentBoard also contains the Master row
- WebUI mobile Agent Dashboard negative coverage must prove unrelated global/history tasks never appear, missing `worker_session_id` is an explicit unavailable error rather than an empty synthetic session, switching Agents never leaves the previous Agent transcript visible under the new selection, the Agents sheet contains no capacity-save control, and toggling the sheet does not reset or recreate semantic state
- WebUI mobile Agent Dashboard race coverage must start a parent transcript query, click a Worker task before it resolves, and assert the late parent `SessionTurns` response is discarded while the projected Worker transcript remains selected and visible
- WebUI selected-session loading coverage must prove late responses clear only the matching in-flight marker, query failures render an explicit refresh-failure card, and a genuinely loaded empty persisted session is the only path that renders `New conversation`
- WebUI selected-session active Worker coverage must prove a transient missing persisted transcript is observable as retryable waiting with TaskBoard evidence and later clears when `SessionTurns` arrives for the same owner-projected Worker session
- WebUI online selected-session lifecycle coverage must prove that a newly selected session with no child tasks shows `0 current task(s)`, `0 current agent(s)`, and `0 current event(s)` even when the daemon TaskBoard still contains unrelated historical blocked tasks/events
- WebUI JS asset smoke locks that submitted input remains observable after composer clear: pending submit cards render after existing history, pending input is cleared only after the same user text is materialized in visible turn rows, a live protocol turn with no public rows renders an explicit observable waiting row, and a latest terminal/interrupted turn still renders when selected-session transcript state is empty instead of producing a blank transcript
- WebUI JS asset smoke locks cycle-timeline order: pending and accepted submit receipt cards are inserted before the matching submit/live/recent same-session turn instead of being appended after all protocol turns, each request/model round renders one parent `.turn-cycle-card` containing the user/model/tool/final phase rows, pending/accepted receipts use `submit:<session_id>:<submit_id>` while every real protocol turn uses `turn:<session_id>:<turn_id>` even when several turns share one submit id, and terminal cycles are frozen by reusing the existing DOM node so a later request cannot mutate a prior terminal cycle card.
- WebUI JS asset smoke locks that submit command timeout/transport failure is an unverified receipt: the catch path calls the owner-truth refresh helper first, materialized submitted text or same-parent TaskBoard task truth clears pending state, same-parent TaskBoard acceptance renders an accepted service receipt instead of a clean empty session while transcript truth is absent, unverified receipts keep the pending user card, selected/draft session, and draft attachments visible, the status says service truth is being checked, and the UI tells the operator not to send a duplicate while verification continues instead of clearing into an empty session
- `node scripts/verify-webui-ambiguous-submit-recovery.mjs` loads the served WebUI asset with a fixed session id and proves three submit-recovery branches without creating random persisted sessions: materialized transcript truth clears pending state, same-parent TaskBoard task truth clears pending state and renders an accepted service receipt even when the task is already closed, while an unverified receipt keeps the fixed selected session plus pending card instead of rendering an empty conversation or a user-visible unknown state
- WebUI JS asset smoke locks that an empty selected session remains visually clean: empty `SessionTurns` binds the selected session, clears the previous active turn/debug state, suppresses the generic waiting-data card, and ignores latest-active turns from other sessions
- WebUI JS asset smoke locks the shared session truth gate: `setSessionList`, `setTurnProjection`, ADP query results, ADP subscription events, latest-turn SSE events, and `setSessionTranscript` must all reject turns or transcripts whose session id is not listed after session-list truth has loaded, except for the current draft or pending-submit session
- WebUI JS asset smoke locks that draft session id generation uses the shared browser-safe id helper and does not directly require `crypto.randomUUID`, because release/Tailscale HTTP is not a secure browser context
- WebUI JS asset smoke locks that latest-turn merging must not key only on `turn_id`; WebUI replacement requires the same `session_id` plus `turn_id`, and must not use visible `user_text` as identity because live provider retry projections can arrive before the transcript projection materializes the user row for the same turn
- WebUI JS asset smoke locks Android/mobile foreground recovery: `pageshow`, window `focus`, `online`, and visible `visibilitychange` events call a throttled `refreshProtocolStateAfterForeground`, which re-queries owner truth while preserving selected/pending session state and never renders clean empty conversation merely because the page resumed.
- WebUI JS asset smoke locks that tool card rendering consumes protocol `display` fields, including `parameter_summary`, and does not implement category parsing from raw tool argument/result text
  - WebUI JS asset smoke locks chronological per-round rendering so `runtime-turn-N` and `runtime-turn-N-rM` render as separate lifecycle cards instead of one all-in summary card
  - WebUI JS asset smoke locks internal runtime continuation prompt hiding and raw completion-schema stripping while preserving Final card projection at the end of the round sequence
  - WebUI JS asset smoke locks that `/new` opens the New dialog, new conversation routes through `CreateSession` without cwd, new task requires a selected or typed cwd and routes through `CreateSession` with cwd, optional `SubmitUserInput.cwd` forwarding remains available, and the old selected-session/no-turns system chat card stays absent
  - WebUI JS asset smoke locks that remove uses `DeleteSession`, archive/restore/query-archived paths are absent from the WebUI app, rename uses `RenameSession`, and double-Esc rollback uses `RollbackLatestSessionTurn`
  - WebUI terminal status projection keeps cancelled/failed cards visually distinct from success
  - WebUI terminal/final summary rendering extracts the complete `Summary` block from terminal text before debug fields, then uses a dedicated final-summary renderer rather than generic paragraph rendering; it preserves source response formatting, renders plain single-line summaries as one readable block, renders explicit source newlines/line-start labels/numbering as multiple blocks, and keeps debug fields hidden unless debug details are enabled
  - WebUI slave-card render smoke
  - CLI/WebUI divergence smoke via protocol projection
  - app dependency boundary smoke
- project black-box impact:
  - app boundary proves WebUI can consume `freehand-ui-protocol` without owning reason/provider semantics
  - app boundary gives diagnostics a repeatable way to generate success/failure ADP scenarios from WebUI without a second transport path or persistent composer buttons
  - app boundary proves it does not need direct reason/provider/node/config imports
  - machine-readable mainline truth remains the only source for generated wiki artifacts
- fixtures / replay inputs / runtime evidence paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/replays/ui`
  - WebUI smoke stdout fixture
- known gaps:
  - smoke server command ingress and runtime query port use static protocol-owned ports; daemon tests cover real runtime owner adapters
- WebUI still re-queries latest turn truth over ADP after submit receipt because a command may complete before the browser consumes the next streamed event
- sync status between design and implementation:
  - WebUI shell rendering is landed
  - split theme/WebUI static assets are landed
  - HTTP query and continuous SSE subscribe transport smoke is landed
  - HTTP command ingress dispatch-receipt/failure smoke is landed
  - WebUI root shell now exposes `/adp`, and WebUI JS uses ADP WebSocket for command/query/subscription truth plus EventSource for latest-turn SSE display refresh
- WebUI layout shape classifier and CSS shape rules are landed; `scripts/verify-webui-layout-shapes.mjs` locks the pure classifier, and `scripts/webui_verify_online.mjs` captures the online viewport matrix against S profile
- WebUI Android WebView first paint is server-pinned through the root `client=android-webview` query so the app enters directly in portrait drawer layout instead of flashing the desktop grid before JS recalculates
- WebUI mobile portrait drawer layout is landed: phone/tall-phone/tablet-portrait default to a phone-first conversation workspace, while session CRUD/list and debug/config detail panels open through mobile overlay controls without changing ADP/session truth
  - WebUI mobile v3 is not a desktop layout shrink: app bar is compact, conversation stream is continuous, internal runtime/session strips are absent, status floats as a small pill near the composer, and focused composer is the only state that reveals attachment/model/cwd controls
- WebUI mobile session drawer right-swipe gesture and persisted-session -> indented worker-child hierarchy are landed and covered by asset smoke plus `scripts/webui_verify_online.mjs`
  - WebUI session rail now supports `/new` as the New dialog for global conversation or cwd-bound task creation, compact session summaries, and selected-session draft creation without inventing a separate navigation path
  - WebUI settings shell is landed as a provider registry, provider definition add/update, provider web_search live-test trigger/status, active primary/fallback selector, and Worker-limit drawer, not a generic config/status drawer; it consumes owner-backed `QueryConfigStatus`, provider definition edits route through `UpsertProviderConfig`, active selection changes route through `UpdateAgentProviderSelection`, provider search tests route through `TestProviderWebSearch`, and unsupported agent/session/workspace/skills/files/tasks/diagnostics controls remain absent until owner-backed write contracts exist
  - mobile Agent capacity control is config truth: no localStorage capacity, no synthetic AgentBoard rows, and no claim that added Workers are live before restart/process startup
- WebUI online verifier owns its Settings valid-save fixture: it backs up S-profile config/env, injects a verifier-only credential env before the browser run, edits the currently selected primary provider instead of switching the agent to a configured fallback provider, and restores config/env afterward so Settings proof does not depend on stale local launchd environment or violate the primary/fallback-distinct config contract
  - WebUI online verifier now captures Phase 2D drawer proof by querying service truth through the same endpoint and comparing TaskBoard, AgentBoard, EventInbox, TaskHistory, and WorkerControl status text plus visible card counts
  - WebUI New dialog task path selection and composer cwd input are landed; new conversation creates protocol-owned session metadata through ADP `CreateSession` without cwd, while new task requires an explicit selected or typed cwd and creates a cwd-bound session through ADP `CreateSession`
  - WebUI root shell intentionally does not expose persistent success/failure buttons, while WebUI JS still carries paired diagnostic prompts for slash commands and shortcuts
  - WebUI terminal display defaults to summary-only; evidence, learned notes, and completion reason require debug details to be enabled
  - WebUI JS must keep shortcuts and slash commands as input-layer affordances that call existing ADP query/command helpers instead of mutating protocol truth directly
- WebUI session creation and selection must remain input-layer affordances over ADP/query state, not local truth writers
- WebUI selected empty-session rendering now shows only the clean empty-state prompt and does not leak prior session turns or generic system feedback into a new session
  - WebUI release online verifier now clears old sessions by ADP `DeleteSession` before proving `/new`, preventing stale persisted sessions from masking or reproducing clean-session failures
  - WebUI session rename/remove/rollback controls remain ADP command affordances over protocol/runtime/persistence truth, not local transcript mutation
  - command-ingress dispatch-port failure and join-failure projection coverage is landed
  - ADP query transport now accepts an injected runtime query port while the app dependency boundary remains protocol-only
- ADP subscribe transport now uses the injected runtime query port for task list and error-center initial snapshots while keeping app dependency boundary protocol-only
  - Phase 2D lifecycle observer is landed as a read-only owner projection plus WorkerControl command affordance; it keeps task/control truth in runtime owners and uses only transient render cache in the browser
  - submit-success path now refreshes latest turn truth after command receipt
  - cancel button and Escape key now send `CancelTurn` instead of only clearing local input
  - submit-in-flight cancel path uses `CancelLatestActiveTurn` before a concrete `turn_id` arrives
  - ADP query/subscription now drive WebUI command/query truth, while latest-turn SSE also refreshes visible chat bubbles and HTTP query remains compatibility coverage
  - debug query remains snapshot-only, while ADP debug subscriptions wait for late debug snapshots so turn/debug timing races are not user-visible failures; ADP failure frames render visible status/cards instead of stale pending
  - latest-turn SSE compatibility and WebUI ADP asset checks now have regression coverage for tool waiting/completed status updates, default ADP routing, and EventSource display refresh wiring
  - attachment placeholder and draft-retention semantics are now part of the design contract and must stay session-scoped
- WebUI tool cards no longer normalize by `tool_call_id`; waiting state animation assets are served, and submit clears the composer immediately while preserving pending user input in the stream
- WebUI submit/dispatch pending state and tool waiting state now both refresh with visible elapsed time instead of static waiting text
- WebUI model-response waiting state is driven by protocol-projected typed `model_request.kind`, not local-only guessing or non-empty detail strings; lifecycle clocks are keyed by session/turn/model phase instead of a single global `modelRequestStartedAt`
- WebUI frozen terminal cycle-card coverage permits same-key replacement only
  when the existing frozen card is missing owner `created_at`, first-response
  timing, or total-elapsed timing and the refreshed transcript provides that
  exact missing field; it must not allow a later request to mutate
  already-timed terminal card semantics
- WebUI selected-session transcript refresh keeps nonterminal `model_request` rows visible with static waiting/retrying/switching status when they are not the current live turn
- WebUI selected-session lifecycle refresh keeps the current latest
  `ToolPending` turn under owner-truth watchdog refresh until a later terminal
  transcript projection arrives, while historical `ToolPending` cards stop
  driving refresh after terminal truth becomes selected
- WebUI Header/mobile Agent observation must treat a later same-session
  terminal transcript turn as authoritative over older `SessionList.active_turn_id`
  or `ToolPending` status, so blocked/failed/closed truth cannot keep showing as
  a running Agent after Master/Worker lifecycle has already closed.
- WebUI Header/mobile Agent observation must not fall back to an unrelated
  globally active session while any Master or Worker session is selected; selected
  pages may observe only the selected session or the selected Worker's parent
  Master.
- WebUI renders provider retry/failover activity only as updating transport detail inside the current Model request row; recovered turns replace that row with normal response truth and do not retain an Error card or a stale `Provider` flow row
- WebUI completed/failed tool cards now render protocol-projected semantic target/body while status/outcome stays in the status line, and tool-complete-to-next-model waiting has its own elapsed timer
- WebUI current-live-turn-only wait gating is landed so historical completed turns cannot keep blinking, even when the selected-session transcript still contains protocol model_request fields from earlier rounds
- WebUI neutral inactive waiting state is landed so text-only/restored turns cannot be mislabeled as live streaming, while inactive tool precursor rounds are projected from their protocol tool status rather than left as neutral waiting cards
- WebUI tool terminal state now uses compact color dots without compressing repeated title/summary/body text or filtering generic success strings and success/failure result echoes from the primary tool body
- WebUI tool rendering now consumes `UiToolActivity.display` for semantic action/target/parameter/result/field/command/diff rendering; parser ownership is outside the UI app and result outcome is carried by status plus visible owner-projected result lines
- WebUI selected-session transcript now preserves per-round lifecycle cards for same execution-cycle round ids; browser-side continuation prompt hiding is forbidden, and internal prompts may be absent only when the protocol/runtime projection owner emits empty `user_text`
- WebUI transcript now renders per-round chat bubbles instead of one large turn execution card; user input is right-aligned, assistant/final text is left-aligned, and tool activity stays inside the assistant bubble as a semantic tool block
- WebUI selected-session transcript now folds latest same-session turn updates into the render view before drawing cards, avoiding stale transcript/latest-turn source competition
- WebUI selected-session startup/manual-click priority is landed: `refreshAllProtocolState` queries latest active turn only when no selected session exists, while ADP/SSE latest-turn updates from other sessions are ignored for the selected transcript
- WebUI draft-session empty state is landed without the old selected-session/no-turns system feedback card
- WebUI assistant text stays in its owning round card, and raw `<freehand_completion>` blocks do not pollute the main chat stream
  - protocol-only transport library reuse is landed
  - app remains protocol-only by dependency gate
  - migrated mainline-call source and generated wiki are kept in sync with this test design
