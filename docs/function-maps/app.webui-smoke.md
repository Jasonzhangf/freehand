# Function Map: `app.webui-smoke`

- feature_id: `app.webui-smoke`
- owner crate: `apps/freehand-server`
- owner module: `apps/freehand-server/src/lib.rs`
- owner entry symbols:
  - `render_webui_smoke`
  - `assets::asset_response`
  - `build_webui_router`
  - `serve_webui_listener`
  - `handle_command_ingress`
  - `handle_query_checkpoints`
  - `handle_adp_socket`
  - `handle_adp_connection`

## Request Mainline

- app boundary receives a minimal WebUI smoke invocation
- app boundary consumes `freehand-ui-protocol` projection truth only
- app boundary stays decoupled from reason, provider, node, and config semantics
- app boundary may render query snapshot, debug snapshot, and separate slave-card projection without owning protocol semantics
- app boundary serves a real WebUI shell that loads protocol-consumer JS and split CSS assets
- app boundary keeps theme assets separate from WebUI layout assets
- front-end default command/query/status path is ADP WebSocket `/adp`; latest-turn SSE is also consumed as a display-refresh mirror
- front-end session list separates global conversation and task creation through one New dialog: `/new`, `New conversation`, and `New task` open the dialog; global session creation does not require cwd, while task creation requires a selected or typed target cwd and creates a protocol-owned cwd-bound task session through ADP `CreateSession`
- front-end selected empty-session rendering binds empty `SessionTurns` to the selected session, clears previous active turn/debug state, and suppresses generic waiting-data system cards so new sessions do not leak prior session content
- after the session list projection has loaded, the WebUI treats that list as the render gate: `QueryLatestActiveTurn`, latest-turn ADP subscription, latest-turn SSE, and `SessionTurns` projections are accepted only for a listed session, the current draft session, or the current pending-submit session
- front-end draft session ids must be generated in both secure localhost and non-secure Tailscale HTTP contexts; WebUI uses a browser-safe id helper instead of directly requiring `crypto.randomUUID`
- front-end session rail exposes protocol-owned rename, remove via `DeleteSession`, and double-Esc rollback controls without storing session truth locally; archive/restore affordances are intentionally not rendered in WebUI
- front-end keeps success and failure scenario prompts as hidden slash/keyboard diagnostic affordances without rendering persistent sample buttons in the composer
- front-end composer control strip exposes attachment buttons, preview, selected-session refresh, cwd input, model selector, slash commands, and keyboard shortcuts as UI-layer affordances over ADP
- front-end settings shell exposes OpenMinis-inspired read-only configuration/status cards for connection, owner-backed active agent/provider/model projection, sessions/workspace, skills, files, tasks, and diagnostics; controls that imply provider/model/agent/config mutation are visibly disabled until an owner-backed write path exists
- front-end layout classifier maps viewport width plus aspect ratio into explicit shape attributes (`phone_portrait`, `tall_phone`, `phone_landscape`, `tablet_portrait`, `tablet_landscape`, `foldable_unfolded`, `desktop_large`) and may only rearrange existing components
- server-rendered root shell pins `client=android-webview` first paint to `tablet_portrait` on `body` and shell layout attributes, so Android WebView does not flash the default desktop grid before JS refinement
- front-end mobile portrait layout keeps the conversation workspace as the primary surface; session CRUD/list and debug/config detail surfaces are hidden in explicit overlay drawers opened from mobile controls and must not occupy the normal document flow
- front-end mobile session drawer can be opened by a right-swipe gesture from the main interface content area; the gesture is presentation-only and must not mutate ADP/session truth, transcript truth, composer draft, pending submit, scroll anchor, or lifecycle timers
- front-end session drawer renders protocol session summaries as an agent -> sessions hierarchy; grouping consumes protocol/future agent fields or known turn source identity and defaults unknown sessions to `master` without creating local session truth
- front-end attachment drafts are scoped by selected session, persist metadata only, append placeholder lines to the current send, clear on command receipt, and remain available for retry after dispatch failure
- transport-facing app routes expose HTTP query for latest active turn and per-turn debug snapshot
- transport-facing app routes expose HTTP query for runtime-owned checkpoint summary projection
- transport-facing ADP query route can call an injected protocol-owned runtime query port for read-only owner projections such as task list/history and error-center metadata before using protocol-state snapshots
- transport-facing ADP subscribe route accepts protocol-owned task list and error-center subscriptions and obtains initial projections from the injected runtime query port
- transport-facing app routes expose SSE subscribe for latest turn and per-turn debug snapshot
- HTTP query and POST command ingress remain compatibility routes; SSE latest-turn subscribe is consumed by WebUI as a display-refresh mirror without owning command dispatch
- per-turn debug SSE is a live subscription and waits for late debug snapshots when turn projection arrives before debug projection
- transport-facing app routes expose POST command ingress for protocol-owned validation and dispatch-port-backed owner routing
- front-end cancel button and Escape key send protocol-owned `CancelTurn` commands through command ingress
- front-end Escape sends `CancelLatestActiveTurn` when submit is in flight but no concrete `turn_id` has reached the browser yet
- command-ingress transport failures must stay explicit at the app boundary and may not collapse into success projection
- the protocol-only transport implementation may be reused by a separate runtime host app, but it must remain protocol-only

## Response Mainline

- app boundary renders a protocol-driven WebUI page shell; live content is populated from ADP query/subscribe/command frames plus latest-turn SSE display refresh events
- app boundary serves protocol-owned query and subscription payloads without becoming a reason/debug truth writer
- app boundary serves runtime-query-port payloads without importing runtime, task, metadata, or error-center owner crates
- app boundary serves task list subscription initial snapshots, error-center initial snapshots, and later runtime-published projection events without importing task, metadata, or error-center owner crates
- app boundary serves protocol-owned command dispatch receipts without claiming truth mutation success
- app boundary keeps one selected session at a time, can synthesize a draft session entry before the first turn exists, keeps the selected transcript pinned to that session instead of falling back to global latest turn, and keeps a new draft session visually clean instead of rendering system feedback as chat content
- app boundary serves protocol-owned command dispatch failures and dispatch-task join failures explicitly when the injected dispatch port fails
- ADP subscribe returns an explicit accepted/waiting state before later turn/debug events, so the WebUI can render waiting instead of appearing frozen
- SSE subscribe routes now emit one initial snapshot followed by continuous incremental projection updates over the same connection, and latest-turn subscribe must stay open on blank state until a turn exists
- debug SSE subscribe stays open when a debug snapshot is not available yet, while debug HTTP query remains snapshot-only and returns explicit 404
- front-end debug state distinguishes missing snapshot (`debug pending`) from debug SSE transport errors (`debug stream reconnecting`)
- WebUI submit success path still actively re-queries latest turn truth over ADP after command receipt to cover command-complete-before-browser-subscriber timing
- WebUI success/failure diagnostic prompts populate the composer through slash commands or keyboard shortcuts, then the operator can send them through the normal ADP submit path; these prompts must not appear as persistent composer buttons
- WebUI composer control strip renders low-noise controls below the composer without creating a second protocol or truth source
- WebUI settings shell renders low-noise compact cards/disclosures from protocol/runtime query projections plus session/browser connection state only; it does not parse `~/.freehand/config.toml`, expose credential values, or send config mutation commands
- WebUI attachment tray renders session-scoped draft metadata and file-handle availability; restored metadata stays visible but is not treated as rehydrated binary payload
- WebUI sends attachment placeholders as current-send text only, clears the composer immediately after submit, and keeps submitted text recoverable through local Up/Down input history instead of refilling the composer after dispatch failure
- WebUI checkpoint panel renders protocol checkpoint summaries from query state and keeps checkpoint files out of app-boundary truth
- WebUI cancel path sends `CancelTurn` for the current active turn, clears pending local input only after dispatch, and refreshes protocol truth
- WebUI cancel path uses `CancelTurn` when `turn_id` is known and `CancelLatestActiveTurn` during the submit-in-flight pre-SSE window
- WebUI Escape behavior is stateful: active/submit-in-flight cancels, non-empty composer clears, empty composer first Esc arms rollback, and second Esc calls `RollbackLatestSessionTurn` before refilling composer from the rolled-back user text
- front-end script projects protocol-owned ADP `UiQueryResult`, `UiSubscriptionEvent`, and `DebugStateSnapshot` frames into semantic message cards and detail panes, including the user prompt
- front-end script projects latest-turn SSE events through the same turn projection path as ADP subscription updates so SSE refreshes the visible chat bubbles
- front-end script applies the layout shape to `body[data-layout-shape]` and shell `data-layout-shape` on load, resize, orientation change, and `visualViewport` resize without mutating selected session, transcript, composer draft, pending submit, scroll anchor, ADP state, or lifecycle timers
- front-end script applies UI-only mobile drawer state through `body[data-mobile-drawer]` / shell data attributes; drawer open/close never mutates ADP, session truth, transcript truth, composer draft, pending submit, or lifecycle timers
- front-end script installs a right-swipe recognizer for mobile drawer layouts that opens the existing sessions drawer state and ignores composer, form control, dialog, drawer, and button targets
- front-end script renders session summaries under expandable agent groups, keeps session CRUD actions on protocol-owned session ids, and treats task/global as display labels derived from protocol cwd
- front-end script derives visible conversation turns through one render selector that preserves transcript order, merges the latest same-session turn by replace-or-append, then builds a `RenderConversation` / `RenderTurn` / `RenderRow` model before DOM rendering so a stale transcript cannot hide an in-flight or newly completed continuation turn
- front-end script must keep the selected session pinned during startup, manual session clicks, ADP latest-turn subscription updates, and latest-turn SSE display refresh; latest active turn may populate the page only when no selected session exists
- front-end script must reject stale latest-turn and stale `SessionTurns` projections after session-list truth is known, so non-destructive `DeleteSession` metadata cannot rehydrate hidden old turn truth into a newly created or refreshed session
- front-end script must not merge latest turn into transcript by `turn_id` alone; replacement requires same `turn_id` and same visible `user_text` so restarted/runtime-reused ordinals cannot overwrite historical rows with a new request
- front-end script renders protocol-projected tool lifecycle status from ADP turn projections so tool calls can show waiting, completed, and failed states over the same WebSocket without surfacing verbose tool term text in the main card
- front-end script normalizes tool cards by `tool_call_id`, renders waiting cards with animation and local elapsed timers, and clears the composer input immediately after submit while keeping the pending user card visible
- front-end script renders submit/dispatch waiting as an animated pending card with elapsed time until a turn projection arrives, then switches visible lifecycle status to tool executing with elapsed time when tool activity is waiting; timer state is scoped by render lifecycle keys, not a single global model-request clock
- front-end script keeps the submitted user input observable after the composer is cleared: pending submit renders after existing history, pending state is cleared only after the same user text is materialized in visible turn rows, a live turn with no public rows renders an explicit observable waiting row, and the latest terminal/interrupted turn remains renderable when selected-session transcript state is empty instead of producing a blank transcript
- front-end script renders protocol-projected typed model request waiting as an animated card with elapsed time, including `Thinking`, `SchemaRetry`, and `ToolResultContinuation`; WebUI must not infer schema retry from non-empty detail text or infer model waiting from completed/failed tool cards
- front-end script renders the main transcript as chat bubbles: user input is a right-aligned user-colored bubble, assistant output is a left-aligned assistant bubble, and tool activity is embedded as a semantic tool block inside the assistant bubble instead of a separate conversation role bubble
- front-end script renders completed/failed tool lifecycle inside the assistant bubble and keeps the status/outcome in the tool block status line; the block body stays semantic and target-focused instead of echoing success/failure result text as a separate user-visible item
- front-end script renders model/request continuation rows as italic reasoning text, while assistant/public answer text uses normal body typography
- front-end script renders inactive tool precursor rounds from protocol tool status: completed tool rounds become completed success cards, failed tool rounds become failed cards, and fully terminal continuation rounds remain separate success/failure cards
- front-end script uses explicit lifecycle color semantics: running assistant/tool bubbles are blue, successful/completed assistant/tool bubbles are green, and failed assistant/tool bubbles are red
- front-end script renders waiting/model continuation animation only for the current live turn; historical completed turns must not keep blinking or reuse the latest turn's lifecycle clock
- front-end script must render a neutral non-animated waiting state when a turn has no terminal status, no waiting tool activity, and no protocol-projected model request; it must not invent `streaming` from text-only or restored inactive state
  - front-end script renders completed/failed tool cards with color-state only and low-noise semantic lines, compressing repeated title/summary/body text and filtering generic result strings such as `succeeded: result returned`
- front-end script consumes `UiToolActivity.display` structured projection for tool category/action/target/result/diff fields and must not classify tools from raw argument or result strings
- front-end script renders `runtime-turn-N` plus later `runtime-turn-N-rM` round projections as chronological per-round lifecycle cards instead of merging the whole execution into one summary card; internal runtime continuation prompts are hidden from user-message rows
- front-end script keeps assistant text inside its owning round card, strips raw `<freehand_completion>` schema blocks from that card, and leaves final user-facing completion summary to the terminal card at the end of the round sequence
- terminal cards use protocol-projected status strings so cancelled and failed terminal states do not render as success
- terminal cards default to summary-only display; evidence, learned notes, and completion reason remain hidden unless WebUI debug details are enabled
- main conversation cards render only `public_conversation`; internal reasoning, usage, raw completion schema, provider payload, and debug lines stay outside the public stream while the user prompt remains visible
- theme module owns white/black theme switching and is separated from WebUI layout/runtime scripts
- CLI and WebUI divergence remains a rendering decision only, not a protocol decision
- the app is a render-only transport boundary, not a reasoning or provider boundary

## Error Mainline

- invalid smoke input or missing projection returns explicit app error
- transport/render wiring failures are surfaced explicitly
- ADP transport failures, decode failures, and protocol failure frames are rendered as visible failure cards and status text
- ADP command/query/subscribe request timeouts are explicit UI failures so the composer cannot stay in a silent dispatching state
- ADP failure cards must not render ahead of the current conversation timeline; transport failure is secondary to the current turn order
- unknown static assets return explicit 404
- cancel without an active turn clears only local input and does not invent a runtime mutation
- attachment dispatch failure preserves draft metadata/file handles for retry and must not silently drop the selected files
- restored attachment metadata without a current page file handle is visible as metadata-only and must not pretend the binary payload was rehydrated
- transient missing debug snapshots are rendered as pending debug state, not command failure
- debug SSE transport errors are rendered as reconnecting state and must not be hidden behind stale pending state
- dispatch port failures and spawn-blocking join failures both surface explicit HTTP 500 failure payloads
- runtime query port failures surface explicit ADP failure frames and do not become app-owned fallback state
- task/error-center subscription initial query failures surface explicit ADP failure frames and do not become app-owned fallback state
- settings controls that imply provider/model/agent config mutation remain disabled/read-only until `config.core` and `ui.protocol` expose owner-backed read/write contracts; WebUI must not invent local config state or write config files directly
- settings config query failures render an explicit unavailable status inside Settings; WebUI must not backfill provider/model values from local guesses
- direct reason/provider/node/config coupling is a policy violation, not a fallback path

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep client-specific slave-card visibility inside the protocol owner
  - allowed callers: CLI/WebUI adapters, tests
  - related tests: slave turn subscription smoke
  - why shared: app boundary must not duplicate client-specific projection logic
- `dispatch_port_failure`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep injected dispatch-port failures on one protocol-owned error projection contract
  - allowed callers: protocol-only app adapters, runtime-backed HTTP hosts, tests
  - related tests: command ingress dispatch failure smoke
  - why shared: app boundary must not invent a second HTTP failure vocabulary

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `render_webui_smoke` / `render_webui_smoke_for_client` / `handle_root` | `apps/freehand-server/src/page.rs` / `apps/freehand-server/src/lib.rs` | render protocol-driven WebUI shell and endpoint bindings, including Android WebView initial layout attributes from the root query client | static page request plus optional `client` query | HTML shell with default or Android-pinned first-paint layout | app entrypoint/root route | page module | bound |
| 02 | `assets::asset_response` | `apps/freehand-server/src/assets.rs` | serve split CSS/JS assets with explicit content type | asset path | CSS/JS response or 404 | app asset route | embedded assets | bound |
| 03 | `build_webui_router` | `apps/freehand-server/src/lib.rs` | define shared protocol-only HTTP/SSE/ADP/static asset surface | protocol state + dispatch port | router with root/assets/query/subscribe/command/ADP routes | app entrypoint/tests/runtime host | app router | bound |
| 04 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | expose protocol-owned command-ingress transport endpoint backed by an injected dispatch port | HTTP JSON command | HTTP dispatch receipt/failure payload | WebUI transport | protocol owner | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve shared protocol-only router on a listener | TCP listener + protocol state + dispatch port + runtime query port + shutdown future | live HTTP/SSE/ADP transport boundary | app entrypoint/tests/runtime host | app server | bound |
| 06 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate slave-card visibility by client kind | turn projection + client kind | client-specific projection | app boundary | protocol owner | bound |
| 07 | `initializeThemeToggle` | `apps/freehand-server/assets/theme.js` | switch white/black visual theme only | UI theme choice | body theme class + persisted localStorage setting | WebUI shell | theme module | bound |
| 08 | `subscription_event_stream` / `projection_to_sse_event` | `apps/freehand-server/src/lib.rs` | convert protocol-owned subscription updates into continuous HTTP SSE delivery, including waiting subscriptions for late debug snapshots | `UiSubscriptionEvent` receiver + selector | streamed SSE events | subscribe routes | protocol state | bound |
| 09 | `handle_adp_socket` / `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | expose protocol-owned ADP WebSocket frames for WebUI default query/subscribe/command control | ADP WebSocket frames + protocol state + dispatch port | ADP response frames and subscription events | WebUI shell | shared protocol transport | bound |
| 09a | `handle_adp_query` | `apps/freehand-server/src/lib.rs` | route ADP query frames to the injected runtime query port first, then protocol-state query when no runtime owner handles the query | ADP query frame + protocol state + runtime query port | ADP query result or failure frame | ADP socket route | runtime query port / protocol state | bound |
| 10 | `ensureAdpSocket` / `requestAdp` / `handleAdpFrame` / `ensureSseTurnSubscription` | `apps/freehand-server/assets/webui.js` | maintain the default WebUI ADP connection for command/query/subscription frames and consume latest-turn SSE as a display-refresh mirror | `UiAdpResponse` JSON frames + SSE turn events | visible WebUI state updates or failure cards | WebUI shell | daemon `/adp` + latest-turn SSE | bound |
| 11 | `refreshTurn` / `refreshAllProtocolState` / `refreshConfigStatus` / `renderMessages` / `conversationTurnsForRender` / `buildConversationRenderModel` / `buildRenderTurn` / `buildRenderRows` / `buildToolActivityRenderRow` / `buildModelRequestRenderRow` / `turnChatCards` / `userChatBubble` / `assistantChatBubble` / `renderToolSection` / `turnIsCurrentLiveTurn` / `renderModelHasLiveLifecycle` / `logicalSessionTurns` / `isInternalRuntimePrompt` / `stripFreehandCompletionBlock` / `normalizePublicConversation` / `modelRequestKind` / `modelRequestTimingKey` / `classifyLayoutShape` / `applyLayoutShape` / `viewportDimensionsForLayout` / `setMobileDrawer` / `closeMobileDrawer` / `syncMobileDrawerForLayout` / `installMobileSessionSwipeGesture` / `setComposerFocused` / `openNewSessionDialog` / `submitNewSessionDialog` / `renderCommandStatus` / `renderSettingsShell` / `showInspectorPanel` / `refreshDebug` / `submitUserInput` / `setSessionList` / `setSessionTranscript` / `sessionAgentId` / `groupedSessionsByAgent` / `renderSessionAgentGroup` / `renderSessionItem` / `startNewConversation` / `startNewTask` / `renameSelectedSession` / `deleteSelectedSessions` / `rollbackLatestSessionTurn` | `apps/freehand-server/assets/webui.js` | consume ADP query/subscription turn payloads and runtime config projection, manage selected and draft sessions, split global conversation creation from cwd-bound task creation through a New dialog, expose protocol-owned session rename/remove and double-Esc rollback, preserve transcript order, keep selected-session transcript pinned ahead of latest active turn during startup and manual session clicks, merge selected-session transcript with the latest same-session turn before rendering, build turn-scoped render models before DOM rendering, render chronological per-round chat bubbles without grouping same-ordinal runtime rounds into a superseded final card, map viewport width plus aspect ratio into DOM layout shape attributes without mutating protocol/session state, keep mobile portrait session/detail/settings surfaces in UI-only overlay drawers outside normal conversation flow, open the session drawer with a presentation-only content-area right-swipe gesture, render session summaries as agent -> sessions groups without owning session truth, render a read-only settings shell from protocol/runtime config projection plus session/browser connection state without config writes, keep mobile focused composer state as presentation-only data attributes, keep user input right-aligned, assistant/final/model rows left-aligned, embed tool rows inside assistant bubbles, hide duplicate/internal runtime continuation prompts after the first round, strip raw completion schema from assistant cards, keep same-tool updates in one semantic tool block, reset timers when typed model-request phase changes, restrict animation to the current live turn, and keep missing debug snapshots pending instead of failed | `UiAdpResponse` frames + config status query result + debug snapshot/pending state + command dispatch receipt + viewport size + mobile drawer button/settings button/gesture events | `RenderConversation` / `RenderTurn` / `RenderRow` plus settings cards -> DOM chat bubbles + command status + layout/drawer data attributes | WebUI shell | ADP protocol frames + CSS layout rules | bound |
| 11b | `loadAttachmentDrafts` / `persistAttachmentDrafts` / `addAttachmentFiles` / `renderAttachmentTray` / `textWithAttachmentPlaceholders` / `clearCurrentAttachments` | `apps/freehand-server/assets/webui.js` | manage session-scoped attachment draft metadata, render placeholder chips, append current-send placeholders, clear on successful command receipt, and preserve drafts after dispatch failure | selected session + browser `File` handles + metadata-only restored drafts | attachment tray DOM + placeholder text appended to submitted command | WebUI control layer | ADP command text placeholder only | bound |
| 11a | `toolSummaryBody` / `toolSemanticLines` / `derivePublicConversation` | `apps/freehand-server/assets/webui.js` | render protocol-projected `display` fields for embedded tool blocks without reclassifying raw tool text | `UiToolActivity.display` | low-noise semantic tool block body | WebUI shell | ui.protocol projection | bound |
| 12 | `handle_query_checkpoints` / `refreshCheckpoints` | `apps/freehand-server/src/lib.rs` / `apps/freehand-server/assets/webui.js` | serve compatibility checkpoint query and render read-only checkpoint summaries from ADP protocol state by default | protocol checkpoint snapshot | checkpoint snapshot + secondary inspector cards | WebUI shell | ui.protocol state | bound |
| 13 | `cancelActiveTurn` | `apps/freehand-server/assets/webui.js` | send `CancelTurn` or `CancelLatestActiveTurn` over ADP for the active protocol turn from button or Escape key | latest protocol turn id | ADP command receipt + refreshed projection | WebUI shell | daemon `/adp` | bound |
| 14 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | keep dispatch-port and spawn-blocking join failures explicit at the HTTP compatibility transport boundary | dispatch port error or join error | explicit HTTP 500 failure payload | command ingress | protocol failure mapper | bound |
| 15 | `loadSamplePrompt` | `apps/freehand-server/assets/webui.js` | load hidden success/failure diagnostic prompts into the composer without inventing a second command path or rendering persistent sample buttons | scenario kind | composer text + command status | WebUI shell | normal ADP submit path | bound |
| 16 | `initial_adp_subscription_projection` | `apps/freehand-server/src/lib.rs` | serve initial ADP subscription snapshot including runtime-backed task list and error-center projections | subscription command plus protocol state plus runtime query port | optional UI projection or explicit failure | `handle_adp_subscribe` | `UiRuntimeQueryPort::query_runtime` / `UiProtocolState::query` | bound |

## Sync Status Against Code

- app boundary now renders a usable WebUI shell instead of a minimal text-only smoke
- theme code is split into `assets/theme.css` and `assets/theme.js`
- WebUI layout/protocol-consumer code is split into `assets/webui.css` and `assets/webui.js`
- WebUI shell now advertises `data-adp-endpoint="/adp"` and the front-end opens a WebSocket to that endpoint by default
- WebUI shell now renders a compact session rail whose New controls open one dialog for either global conversation creation or cwd-bound task creation; `/new` opens the same dialog instead of directly creating a global draft
- app boundary now serves protocol-only HTTP query and SSE subscribe smoke routes from a reusable protocol-only library surface
- app boundary now serves protocol-only POST command ingress dispatch-receipt/failure smoke route from that shared transport surface
- HTTP/POST routes remain compatibility transport surfaces; WebUI JS uses ADP for command/query/subscription truth and EventSource for latest-turn SSE display refresh
- app boundary now surfaces explicit command-ingress dispatch-port failures and dispatch-task join failures instead of collapsing them into success
- app boundary now serves static embedded assets through an explicit 404ing route
- runtime host reuse now happens through injected state and dispatch port, not by duplicating transport behavior
- protocol-owned client-specific projection helper exists and is now a shared owner boundary for the app smoke
- subscribe routes now keep one SSE connection open and stream later matching updates after the initial snapshot
- debug subscribe route now also keeps one SSE connection open when the first debug snapshot is not available yet
- WebUI submit path still explicitly refreshes latest turn truth over ADP after a successful command receipt
- WebUI submit path forwards selected session id and optional selected cwd through `SubmitUserInput`; global conversation drafts may submit without cwd and use runtime default cwd, while task sessions are cwd-bound through `CreateSession`
- WebUI checkpoint panel now refreshes protocol checkpoint summaries and sends explicit rewind commands without parsing runtime files
- WebUI Cancel button and Escape key now send `CancelTurn` through protocol command ingress instead of only clearing local input
- WebUI cancel path now covers the submit-in-flight window with `CancelLatestActiveTurn`
- WebUI tool cards now render protocol-projected waiting/completed/failed lifecycle states from ADP turn projection truth
- WebUI success/failure diagnostic prompts remain available through slash commands and shortcuts, while persistent Success/Failure composer buttons are intentionally absent
- WebUI composer control strip now exposes file/image/video attachment, preview, selected-session refresh, cwd input, and read-only model selector controls without changing ADP framing beyond the protocol-owned `SubmitUserInput.cwd`
- WebUI settings shell is landed as a read-only drawer/panel with compact OpenMinis-style cards for connection, provider/model, sessions/workspace, skills, files, tasks, and diagnostics; provider/model/config mutation controls are disabled until owner-backed config contracts exist
- WebUI settings shell now queries `QueryConfigStatus` and renders owner-backed active agent/provider/model/auth-source values; config mutation controls remain disabled and credential values are not rendered
- WebUI layout shape classifier and CSS shape rules are landed for phone portrait, tall phone, phone landscape, tablet portrait, tablet landscape, foldable unfolded, and desktop large; the classifier only writes DOM data attributes and does not mutate protocol/session state
- WebUI root route now pins `?client=android-webview` first paint to `tablet_portrait` on the server-rendered body and shell while leaving normal Web roots unpinned for browser classification
- WebUI attachment drafts are session-scoped in local UI metadata, render as placeholder chips, append placeholder lines to the current submitted text, clear after successful command receipt, and remain for retry after dispatch failure
- WebUI mobile session drawer now supports content-area right-swipe opening and renders persisted sessions under expandable agent groups; task/global labels are display-only and derive from protocol cwd
- WebUI same-tool lifecycle updates now normalize by `tool_call_id`, waiting cards animate with local elapsed timers, and submit clears the input field immediately while retaining pending state in the conversation stream
- WebUI new/empty selected sessions now render a clean empty conversation prompt and do not project previous latest-active turns or generic waiting-data system feedback into the selected session
- WebUI session-list truth now gates all turn/transcript render inputs after the list has loaded, including latest-active query results, latest-turn ADP/SSE updates, and `SessionTurns`; old latest-active turn truth from a non-destructively removed session cannot reappear in a new clean session
- WebUI release online verification removes existing sessions through ADP before creating a new conversation, then proves the new draft has no selected turn and no leaked prior transcript
- WebUI submit/dispatch and tool-wait lifecycle states now both refresh once per second so users can see where the turn is blocked and how long it has waited
- WebUI model-request waiting state now comes from typed `UiTurnProjection.model_request.kind` and refreshes once per second with elapsed wait time; phase changes reset local timing
- WebUI completed/failed tool cards now show protocol-projected semantic target/body while status/outcome stays in the status line, and tool-complete-to-next-model waiting renders as its own timed lifecycle state
- WebUI waiting/model continuation cards are now gated to the current live turn so restored history cannot show fake animation after execution has stopped
- WebUI inactive text-only or restored turns now render as neutral waiting/active state instead of fake streaming; animation requires submit-in-flight, waiting tool activity, or protocol-projected model request truth
  - WebUI tool terminal state now uses green/red compact state dots and compresses repeated title/summary/body text instead of rendering mechanical status/result lines or separate success-result items
- WebUI tool cards now render `display.action`, `display.summary`, `display.parameter_summary`, `display.fields`, and `display.diff`; category parsing stays in `tool.display`, not in JavaScript, and success/failure result text is not primary body content
- WebUI selected-session transcript display now preserves chronological round bubbles for ids such as `runtime-turn-47` and `runtime-turn-47-r2`; the old all-rounds-in-one visible summary card is intentionally removed
- WebUI selected-session transcript rendering now uses `conversationTurnsForRender` plus `buildConversationRenderModel` to fold the latest same-session turn into the selected transcript while keeping lifecycle animation scoped to the current live turn
- WebUI startup and manual session clicks now keep the selected session pinned; latest active turn query/subscribe/SSE updates do not replace the transcript when they belong to another session
- WebUI inactive tool precursor cards now project completed tools as success cards and failed tools as failed cards instead of falling back to neutral waiting cards
- WebUI execution card CSS now uses explicit blue/green/red status borders for running/success/failed states
- WebUI main transcript now renders chat bubbles: user bubbles are right-aligned, assistant bubbles are left-aligned, tool activity is embedded in the assistant bubble as a semantic block, and model-wait/retry continuation rows use italic reasoning typography
- WebUI session rail now exposes Rename, Remove via `DeleteSession`, and double-Esc rollback through ADP commands; archive/restore controls are intentionally absent from the WebUI surface
- WebUI `/new` opens the New dialog instead of directly creating a draft; the old selected-session/no-turns system card stays absent from the chat stream
- WebUI assistant cards stay within their own round and strip raw `<freehand_completion>` blocks; final user-facing completion content remains in the terminal Final row at the bottom of the round sequence
- WebUI missing-debug race is locked by pending-state rendering plus late-debug ADP subscription coverage; ADP failure frames render as visible failure cards/status instead of stale pending
- WebUI user-facing labels, status rows, failure cards, and diagnostic prompts must not expose `ADP`; ADP remains an internal protocol/debug/automation term only
- app dependency boundary is intended to remain protocol-only and must not import reason/provider/node/config semantics
- app query transport now accepts an injected `UiRuntimeQueryPort`; the app still does not import runtime/task/error-center semantics
- app subscription transport now uses the injected `UiRuntimeQueryPort` for task list and error-center initial snapshots while keeping later updates on protocol subscription events
- generated wiki must be regenerated from `docs/mainline-calls/app.webui-smoke.json` when this function-map truth changes
