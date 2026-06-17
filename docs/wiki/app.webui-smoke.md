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
- transport-facing app routes expose HTTP query for latest active turn and per-turn debug snapshot
- transport-facing app routes expose HTTP query for runtime-owned checkpoint summary projection
- transport-facing app routes expose SSE subscribe for latest turn and per-turn debug snapshot
- transport-facing app routes expose POST command ingress for protocol-owned validation and dispatch-port-backed owner routing
- the protocol-only transport implementation may be reused by a separate runtime host app, but it must remain protocol-only
- front-end cancel button and Escape key send protocol-owned CancelTurn commands through command ingress
- front-end Escape sends CancelLatestActiveTurn when submit is in flight but no concrete turn_id has reached the browser yet

## Response Mainline

- app boundary renders a protocol-driven WebUI page shell; live content is populated from existing query and SSE endpoints
- app boundary serves protocol-owned query and subscription payloads without becoming a reason or debug truth writer
- app boundary serves protocol-owned command dispatch receipts without claiming truth mutation success
- SSE subscribe routes emit one initial snapshot followed by continuous incremental projection updates over the same connection, and latest-turn subscribe keeps waiting when no turn exists yet
- WebUI submit success path actively re-queries latest turn truth after command receipt to cover command-complete-before-browser-subscriber timing
- front-end script projects protocol-owned `UiPublicTurnProjection` and `DebugStateSnapshot` into semantic message cards and detail panes, and preserves the user prompt in the public conversation stream
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
- unknown static assets return explicit 404
- checkpoint query uses protocol state only and must not parse runtime checkpoint files in the app boundary
- blank latest-turn subscribe does not fail early; it keeps waiting for the first matching turn
- direct reason, provider, node, or config coupling is a policy violation, not a fallback path
- cancel without an active turn clears only local input and does not invent a runtime mutation

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep client-specific slave-card visibility inside the protocol owner
  - allowed callers: CLI or WebUI adapters, tests
  - related tests: slave turn subscription smoke
  - why shared: app boundary must not duplicate client-specific projection logic

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `render_webui_smoke` | `apps/freehand-server/src/page.rs` | render protocol-driven WebUI shell and endpoint bindings | static page request | HTML shell | app entrypoint or root route | page module | bound |
| 02 | `assets::asset_response` | `apps/freehand-server/src/assets.rs` | serve split CSS and JS assets with explicit content type | asset path | CSS or JS response or 404 | app asset route | embedded assets | bound |
| 03 | `build_webui_router` | `apps/freehand-server/src/lib.rs` | define shared protocol-only HTTP, SSE, and static asset surface | protocol state plus dispatch port | router with root, assets, query, subscribe, and command routes | app entrypoint, tests, or runtime host | app router | bound |
| 04 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | expose protocol-owned command-ingress transport endpoint backed by an injected dispatch port | HTTP JSON command | HTTP dispatch receipt or failure payload | WebUI transport | protocol owner | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve shared protocol-only router on a listener | TCP listener plus protocol state plus dispatch port plus shutdown future | live HTTP and SSE transport boundary | app entrypoint, tests, or runtime host | app server | bound |
| 06 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate slave-card visibility by client kind | turn projection plus client kind | client-specific projection | app boundary | protocol owner | bound |
| 07 | `initializeThemeToggle` | `apps/freehand-server/assets/theme.js` | switch white and black visual theme only | UI theme choice | body theme class plus persisted localStorage setting | WebUI shell | theme module | bound |
| 08 | `subscription_event_stream / projection_to_sse_event` | `apps/freehand-server/src/lib.rs` | convert protocol-owned subscription updates into continuous HTTP SSE delivery | `UiSubscriptionEvent` receiver plus selector | streamed SSE events | subscribe routes | protocol state | bound |
| 09 | `refreshTurn / renderMessages / submit handler` | `apps/freehand-server/assets/webui.js` | consume protocol query and SSE public turn payloads, re-query latest turn after command receipt, and render semantic cards without owning filtering semantics | `UiPublicTurnProjection` JSON plus command dispatch receipt | DOM message blocks plus command status | WebUI shell | existing protocol endpoints | bound |
| 10 | `handle_query_checkpoints / refreshCheckpoints` | `apps/freehand-server/src/lib.rs / apps/freehand-server/assets/webui.js` | serve and render read-only checkpoint summaries from protocol state | protocol checkpoint snapshot | HTTP JSON checkpoint snapshot plus secondary inspector cards | WebUI shell | ui.protocol state | bound |
| 11 | `cancelActiveTurn` | `apps/freehand-server/assets/webui.js` | send CancelTurn for the active protocol turn from button or Escape key | latest protocol turn id | command dispatch receipt plus refreshed projection | WebUI shell | POST /ui/command | bound |

## Sync Status Against Mainline Call

- app boundary now renders a usable WebUI shell instead of a minimal text-only smoke
- theme code is split into `assets/theme.css` and `assets/theme.js`
- WebUI layout and protocol-consumer code is split into `assets/webui.css` and `assets/webui.js`
- app boundary now serves protocol-only HTTP query and SSE subscribe smoke routes from a reusable protocol-only library surface
- app boundary now serves protocol-only POST command ingress dispatch-receipt or failure smoke route from that shared transport surface
- app boundary now serves static embedded assets through an explicit 404ing route
- runtime host reuse now happens through injected state and dispatch port, not by duplicating transport behavior
- protocol-owned client-specific projection helper exists and is now a shared owner boundary for the app smoke
- subscribe routes now keep one SSE connection open and stream later matching updates after the initial snapshot
- WebUI submit path still explicitly refreshes latest turn truth after a successful command receipt
- WebUI checkpoint panel now refreshes protocol checkpoint summaries and sends explicit rewind commands without parsing runtime files
- app dependency boundary is intended to remain protocol-only and must not import reason, provider, node, or config semantics
- generated wiki must be regenerated from `docs/mainline-calls/app.webui-smoke.json` when this function-map truth changes
- WebUI Cancel button and Escape key now send CancelTurn through protocol command ingress instead of only clearing local input
- WebUI cancel path now covers the submit-in-flight window with CancelLatestActiveTurn
