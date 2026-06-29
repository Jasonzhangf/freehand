# Wiki: `app.webui-smoke`

Generated from `docs/mainline-calls/app.webui-smoke.json`. Do not edit by hand.

- owner crate: `apps/freehand-server`
- owner module: `apps/freehand-server/src/lib.rs`
- function map: `docs/function-maps/app.webui-smoke.md`
- generated wiki: `docs/wiki/app.webui-smoke.md`
- test design: `docs/testing/app.webui-smoke.md`

## Request Mainline

- app boundary receives a minimal WebUI smoke invocation
- app boundary consumes `freehand-ui-protocol` projection truth only
- app boundary stays decoupled from reason, provider, node, and config semantics
- app boundary may render query snapshot, debug snapshot, and separate slave-card projection without owning protocol semantics
- app boundary serves a real WebUI shell that loads protocol-consumer JS and split CSS assets
- app boundary keeps theme assets separate from WebUI layout assets
- front-end default control/status path is ADP WebSocket `/adp` for query, subscribe, and command frames
- front-end exposes success and failure sample prompt buttons that load reproducible ADP sample prompts into the composer without bypassing normal Send/ADP command flow
- transport-facing app routes expose HTTP query for latest active turn and per-turn debug snapshot
- transport-facing app routes expose HTTP query for runtime-owned checkpoint summary projection
- transport-facing app routes expose SSE subscribe for latest turn and per-turn debug snapshot
- HTTP query, SSE subscribe, and POST command ingress remain compatibility routes, not the WebUI default control/status path
- per-turn debug SSE is a live subscription and waits for late debug snapshots when turn projection arrives before debug projection
- transport-facing app routes expose POST command ingress for protocol-owned validation and dispatch-port-backed owner routing
- the protocol-only transport implementation may be reused by a separate runtime host app, but it must remain protocol-only
- front-end cancel button and Escape key send protocol-owned CancelTurn commands through command ingress
- front-end Escape sends CancelLatestActiveTurn when submit is in flight but no concrete turn_id has reached the browser yet
- command-ingress transport failures must stay explicit at the app boundary and may not collapse into success projection

## Response Mainline

- app boundary renders a protocol-driven WebUI page shell; live content is populated from ADP query/subscribe/command frames by default
- app boundary serves protocol-owned query and subscription payloads without becoming a reason or debug truth writer
- app boundary serves protocol-owned command dispatch receipts without claiming truth mutation success
- app boundary serves protocol-owned command dispatch failures and dispatch-task join failures explicitly when the injected dispatch port fails
- ADP subscribe returns an explicit accepted/waiting state before later turn/debug events, so the WebUI can render waiting instead of appearing frozen
- SSE subscribe routes emit one initial snapshot followed by continuous incremental projection updates over the same connection, latest-turn subscribe keeps waiting when no turn exists yet, and debug subscribe keeps waiting when no debug snapshot exists yet
- WebUI submit success path actively re-queries latest turn truth over ADP after command receipt to cover command-complete-before-browser-subscriber timing
- WebUI success/failure sample buttons populate the composer with the same sample prompts used by CLI/headless ADP sample automation, then the operator can send them through the normal ADP submit path
- front-end script projects protocol-owned ADP `UiQueryResult`, `UiSubscriptionEvent`, and `DebugStateSnapshot` frames into semantic message cards and detail panes, including the user prompt
- front-end debug state distinguishes missing snapshot (debug pending) from debug SSE transport errors (debug stream reconnecting)
- front-end script renders protocol-projected tool lifecycle status from ADP turn projections so tool calls can show waiting, completed, and failed states over the same WebSocket without surfacing verbose tool term text in the main card
- front-end script normalizes tool cards by tool_call_id, renders waiting cards with animation and local elapsed timers, and clears the composer input immediately after submit while keeping the pending user card visible
- front-end script renders submit and dispatch waiting as an animated pending card with elapsed time, then switches to tool executing with elapsed time when a waiting tool activity is visible
- front-end script groups runtime-turn-N plus runtime-turn-N-rM round projections into one logical execution cycle for display and collapses assistant text into one visible card per logical turn while preserving tool, terminal, and error cards
- front-end script projects checkpoint summaries into a secondary inspector card and sends explicit rewind commands through command ingress
- main conversation cards render only `public_conversation`; internal reasoning, usage, raw completion schema, provider payload, and debug lines stay outside the public stream while the user prompt remains visible
- theme module owns white and black theme switching and is separated from WebUI layout/runtime scripts
- CLI and WebUI divergence remains a rendering decision only, not a protocol decision
- the app is a render-only transport boundary, not a reasoning or provider boundary
- front-end cancel handling clears pending local input only after sending CancelTurn for the current active turn and refreshing protocol truth
- terminal cards use protocol-projected status strings so cancelled and failed terminal states do not render as success
- front-end cancel handling uses CancelTurn when turn_id is known and CancelLatestActiveTurn during submit-in-flight pre-SSE window

## Error Mainline

- invalid smoke input or missing projection returns explicit app error
- transport or render wiring failures are surfaced explicitly
- ADP transport failures, decode failures, and protocol failure frames are rendered as visible failure cards and status text
- unknown static assets return explicit 404
- checkpoint query uses protocol state only and must not parse runtime checkpoint files in the app boundary
- blank latest-turn subscribe does not fail early; it keeps waiting for the first matching turn
- dispatch port failures and spawn-blocking join failures both surface explicit HTTP 500 failure payloads
- direct reason, provider, node, or config coupling is a policy violation, not a fallback path
- cancel without an active turn clears only local input and does not invent a runtime mutation
- transient missing debug snapshots are rendered as pending debug state, not command failure
- debug SSE transport errors are rendered as reconnecting state and must not be hidden behind stale pending state

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep client-specific slave-card visibility inside the protocol owner
  - allowed callers: CLI or WebUI adapters, tests
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
| 01 | `render_webui_smoke` | `apps/freehand-server/src/page.rs` | render protocol-driven WebUI shell and endpoint bindings | static page request | HTML shell | app entrypoint or root route | page module | bound |
| 02 | `assets::asset_response` | `apps/freehand-server/src/assets.rs` | serve split CSS and JS assets with explicit content type | asset path | CSS or JS response or 404 | app asset route | embedded assets | bound |
| 03 | `build_webui_router` | `apps/freehand-server/src/lib.rs` | define shared protocol-only HTTP, SSE, ADP, and static asset surface | protocol state plus dispatch port | router with root, assets, query, subscribe, command, and ADP routes | app entrypoint, tests, or runtime host | app router | bound |
| 04 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | expose protocol-owned command-ingress transport endpoint backed by an injected dispatch port | HTTP JSON command | HTTP dispatch receipt or failure payload | WebUI transport | protocol owner | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve shared protocol-only router on a listener | TCP listener plus protocol state plus dispatch port plus shutdown future | live HTTP and SSE transport boundary | app entrypoint, tests, or runtime host | app server | bound |
| 06 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate slave-card visibility by client kind | turn projection plus client kind | client-specific projection | app boundary | protocol owner | bound |
| 07 | `initializeThemeToggle` | `apps/freehand-server/assets/theme.js` | switch white and black visual theme only | UI theme choice | body theme class plus persisted localStorage setting | WebUI shell | theme module | bound |
| 08 | `subscription_event_stream / projection_to_sse_event` | `apps/freehand-server/src/lib.rs` | convert protocol-owned subscription updates into continuous HTTP SSE delivery, including waiting subscriptions for late debug snapshots | `UiSubscriptionEvent` receiver plus selector | streamed SSE events | subscribe routes | protocol state | bound |
| 09 | `handle_adp_socket / handle_adp_connection` | `apps/freehand-server/src/lib.rs` | expose protocol-owned ADP WebSocket frames for WebUI default query, subscribe, and command control | ADP WebSocket frames plus protocol state plus dispatch port | ADP response frames and subscription events | WebUI shell | shared protocol transport | bound |
| 10 | `ensureAdpSocket / requestAdp / handleAdpFrame` | `apps/freehand-server/assets/webui.js` | maintain the default WebUI ADP connection and route query_result, subscription_accepted, subscription_event, command_receipt, and failure frames | `UiAdpResponse` JSON frames | visible WebUI state updates or failure cards | WebUI shell | daemon `/adp` | bound |
| 11 | `refreshTurn / renderMessages / logicalSessionTurns / stripFreehandCompletionBlock / normalizePublicConversation / renderCommandStatus / refreshDebug / submitUserInput` | `apps/freehand-server/assets/webui.js` | consume ADP query and subscription turn payloads, render semantic/tool/debug cards, collapse same execution-cycle rounds into one transcript group, collapse assistant text into one visible card, keep same-tool updates in one card, and keep missing debug snapshots pending instead of failed | `UiAdpResponse` frames plus debug snapshot/pending state plus command dispatch receipt | DOM message blocks plus command status | WebUI shell | ADP protocol frames | bound |
| 12 | `handle_query_checkpoints / refreshCheckpoints` | `apps/freehand-server/src/lib.rs / apps/freehand-server/assets/webui.js` | serve compatibility checkpoint query and render read-only checkpoint summaries from ADP protocol state by default | protocol checkpoint snapshot | checkpoint snapshot plus secondary inspector cards | WebUI shell | ui.protocol state | bound |
| 13 | `cancelActiveTurn` | `apps/freehand-server/assets/webui.js` | send CancelTurn or CancelLatestActiveTurn over ADP for the active protocol turn from button or Escape key | latest protocol turn id | ADP command receipt plus refreshed projection | WebUI shell | daemon `/adp` | bound |
| 14 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | keep dispatch-port and spawn-blocking join failures explicit at the HTTP compatibility transport boundary | dispatch port error or join error | explicit HTTP 500 failure payload | command ingress | protocol failure mapper | bound |
| 15 | `loadSamplePrompt` | `apps/freehand-server/assets/webui.js` | load success/failure ADP sample prompts into the composer without inventing a second command path | sample kind | composer text plus visible command status | WebUI shell | normal ADP submit path | bound |

## Sync Status Against Mainline Call

- app boundary now renders a usable WebUI shell instead of a minimal text-only smoke
- theme code is split into `assets/theme.css` and `assets/theme.js`
- WebUI layout and protocol-consumer code is split into `assets/webui.css` and `assets/webui.js`
- WebUI shell now advertises `data-adp-endpoint="/adp"` and the front-end opens a WebSocket to that endpoint by default
- WebUI shell now exposes success/failure sample buttons and WebUI JS carries the paired sample prompts
- app boundary now serves protocol-only HTTP query and SSE subscribe smoke routes from a reusable protocol-only library surface
- app boundary now serves protocol-only POST command ingress dispatch-receipt or failure smoke route from that shared transport surface
- HTTP/SSE/POST routes remain compatibility transport surfaces; WebUI JS no longer uses `fetch` or `EventSource` as its default live path
- app boundary now surfaces explicit command-ingress dispatch-port failures and dispatch-task join failures instead of collapsing them into success
- app boundary now serves static embedded assets through an explicit 404ing route
- runtime host reuse now happens through injected state and dispatch port, not by duplicating transport behavior
- protocol-owned client-specific projection helper exists and is now a shared owner boundary for the app smoke
- subscribe routes now keep one SSE connection open and stream later matching updates after the initial snapshot
- debug subscribe route now also keeps one SSE connection open when the first debug snapshot is not available yet
- WebUI submit path still explicitly refreshes latest turn truth over ADP after a successful command receipt
- WebUI checkpoint panel now refreshes protocol checkpoint summaries and sends explicit rewind commands without parsing runtime files
- app dependency boundary is intended to remain protocol-only and must not import reason, provider, node, or config semantics
- generated wiki must be regenerated from `docs/mainline-calls/app.webui-smoke.json` when this function-map truth changes
- WebUI Cancel button and Escape key now send CancelTurn through protocol command ingress instead of only clearing local input
- WebUI cancel path now covers the submit-in-flight window with CancelLatestActiveTurn
- WebUI tool cards now render protocol-projected waiting/completed/failed lifecycle states from ADP turn projection truth without exposing verbose term text by default
- WebUI same-tool lifecycle updates normalize by tool_call_id, waiting cards animate, elapsed timers render locally, and submit clears the composer immediately while retaining pending state in the conversation stream
- WebUI missing-debug race is locked by pending-state rendering plus late-debug ADP subscription coverage; ADP failure frames render as visible failure cards/status instead of stale pending
