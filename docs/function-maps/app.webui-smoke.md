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

## Request Mainline

- app boundary receives a minimal WebUI smoke invocation
- app boundary consumes `freehand-ui-protocol` projection truth only
- app boundary stays decoupled from reason, provider, node, and config semantics
- app boundary may render query snapshot, debug snapshot, and separate slave-card projection without owning protocol semantics
- app boundary serves a real WebUI shell that loads protocol-consumer JS and split CSS assets
- app boundary keeps theme assets separate from WebUI layout assets
- transport-facing app routes expose HTTP query for latest active turn and per-turn debug snapshot
- transport-facing app routes expose SSE subscribe for latest turn and per-turn debug snapshot
- transport-facing app routes expose POST command ingress for protocol-owned validation and dispatch-port-backed owner routing
- front-end cancel button and Escape key send protocol-owned `CancelTurn` commands through command ingress
- front-end Escape sends `CancelLatestActiveTurn` when submit is in flight but no concrete `turn_id` has reached the browser yet
- the protocol-only transport implementation may be reused by a separate runtime host app, but it must remain protocol-only

## Response Mainline

- app boundary renders a protocol-driven WebUI page shell; live content is populated from existing query/SSE endpoints
- app boundary serves protocol-owned query and subscription payloads without becoming a reason/debug truth writer
- app boundary serves protocol-owned command dispatch receipts without claiming truth mutation success
- SSE subscribe routes now emit one initial snapshot followed by continuous incremental projection updates over the same connection, and latest-turn subscribe must stay open on blank state until a turn exists
- WebUI submit success path still actively re-queries latest turn truth after command receipt to cover command-complete-before-browser-subscriber timing
- WebUI cancel path sends `CancelTurn` for the current active turn, clears pending local input only after dispatch, and refreshes protocol truth
- WebUI cancel path uses `CancelTurn` when `turn_id` is known and `CancelLatestActiveTurn` during the submit-in-flight pre-SSE window
- front-end script projects protocol-owned `UiPublicTurnProjection` and `DebugStateSnapshot` into semantic message cards and detail panes, including the user prompt in the public conversation stream
- terminal cards use protocol-projected status strings so cancelled and failed terminal states do not render as success
- main conversation cards render only `public_conversation`; internal reasoning, usage, raw completion schema, provider payload, and debug lines stay outside the public stream while the user prompt remains visible
- theme module owns white/black theme switching and is separated from WebUI layout/runtime scripts
- CLI and WebUI divergence remains a rendering decision only, not a protocol decision
- the app is a render-only transport boundary, not a reasoning or provider boundary

## Error Mainline

- invalid smoke input or missing projection returns explicit app error
- transport/render wiring failures are surfaced explicitly
- unknown static assets return explicit 404
- cancel without an active turn clears only local input and does not invent a runtime mutation
- direct reason/provider/node/config coupling is a policy violation, not a fallback path

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: keep client-specific slave-card visibility inside the protocol owner
  - allowed callers: CLI/WebUI adapters, tests
  - related tests: slave turn subscription smoke
  - why shared: app boundary must not duplicate client-specific projection logic

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `render_webui_smoke` | `apps/freehand-server/src/page.rs` | render protocol-driven WebUI shell and endpoint bindings | static page request | HTML shell | app entrypoint/root route | page module | bound |
| 02 | `assets::asset_response` | `apps/freehand-server/src/assets.rs` | serve split CSS/JS assets with explicit content type | asset path | CSS/JS response or 404 | app asset route | embedded assets | bound |
| 03 | `build_webui_router` | `apps/freehand-server/src/lib.rs` | define shared protocol-only HTTP/SSE/static asset surface | protocol state + dispatch port | router with root/assets/query/subscribe/command routes | app entrypoint/tests/runtime host | app router | bound |
| 04 | `handle_command_ingress` | `apps/freehand-server/src/lib.rs` | expose protocol-owned command-ingress transport endpoint backed by an injected dispatch port | HTTP JSON command | HTTP dispatch receipt/failure payload | WebUI transport | protocol owner | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve shared protocol-only router on a listener | TCP listener + protocol state + dispatch port + shutdown future | live HTTP/SSE transport boundary | app entrypoint/tests/runtime host | app server | bound |
| 06 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate slave-card visibility by client kind | turn projection + client kind | client-specific projection | app boundary | protocol owner | bound |
| 07 | `initializeThemeToggle` | `apps/freehand-server/assets/theme.js` | switch white/black visual theme only | UI theme choice | body theme class + persisted localStorage setting | WebUI shell | theme module | bound |
| 08 | `subscription_event_stream` / `projection_to_sse_event` | `apps/freehand-server/src/lib.rs` | convert protocol-owned subscription updates into continuous HTTP SSE delivery | `UiSubscriptionEvent` receiver + selector | streamed SSE events | subscribe routes | protocol state | bound |
| 09 | `refreshTurn` / `renderMessages` / submit handler | `apps/freehand-server/assets/webui.js` | consume protocol query/SSE public turn payloads, re-query latest turn after command receipt, and render semantic cards without owning filtering semantics | `UiPublicTurnProjection` JSON + command dispatch receipt | DOM message blocks + command status | WebUI shell | existing protocol endpoints | bound |
| 10 | `cancelActiveTurn` | `apps/freehand-server/assets/webui.js` | send `CancelTurn` for the active protocol turn from button or Escape key | latest protocol turn id | command dispatch receipt + refreshed projection | WebUI shell | `/ui/command` | bound |

## Sync Status Against Code

- app boundary now renders a usable WebUI shell instead of a minimal text-only smoke
- theme code is split into `assets/theme.css` and `assets/theme.js`
- WebUI layout/protocol-consumer code is split into `assets/webui.css` and `assets/webui.js`
- app boundary now serves protocol-only HTTP query and SSE subscribe smoke routes from a reusable protocol-only library surface
- app boundary now serves protocol-only POST command ingress dispatch-receipt/failure smoke route from that shared transport surface
- app boundary now serves static embedded assets through an explicit 404ing route
- runtime host reuse now happens through injected state and dispatch port, not by duplicating transport behavior
- protocol-owned client-specific projection helper exists and is now a shared owner boundary for the app smoke
- subscribe routes now keep one SSE connection open and stream later matching updates after the initial snapshot
- WebUI submit path still explicitly refreshes latest turn truth after a successful command receipt
- WebUI Cancel button and Escape key now send `CancelTurn` through protocol command ingress instead of only clearing local input
- WebUI cancel path now covers the submit-in-flight window with `CancelLatestActiveTurn`
- app dependency boundary is intended to remain protocol-only and must not import reason/provider/node/config semantics
- generated wiki must be regenerated from `docs/mainline-calls/app.webui-smoke.json` when this function-map truth changes
