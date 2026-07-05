# Function Map: `app.android-client`

- feature_id: `app.android-client`
- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- owner entry symbols:
  - `com.freehand.android.ui.MainActivity` — app shell entrypoint, WebView host, controller composition
  - `com.freehand.android.data.AdpEventStream` — OkHttp ADP WebSocket query/subscribe/command consumer
  - `com.freehand.android.data.SseEventStream` — compatibility OkHttp SSE event stream consumer
  - `com.freehand.android.data.TimelineProjector` — ui.protocol event → UI state projection
  - `com.freehand.android.data.CommandIngress` — submit / cancel via protocol-owned HTTP command ingress
  - `com.freehand.android.data.ProtocolClient` — compatibility HTTP query + command POST against `freehand-ui-protocol`
  - `com.freehand.android.data.ClientConfig` — bundled config loading from `assets/config/client.json`
  - `com.freehand.android.data.HostStore` — host:port persistence in SharedPreferences
  - `com.freehand.android.data.HostConfig` — endpoint URL construction
- reference mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html`
- reference bridge: `apps/freehand-android/app/src/main/assets/bridge.html`

## Request Mainline

- Android client shell receives user input and forwards it as a protocol-owned ADP command frame
- Android client shell never mutates session, reason, debug, metadata, or provider truth locally
- Android client subscribes to `ui.protocol` turn / debug / status projections through daemon ADP WebSocket `/adp`
- Android client submits user actions (submit / cancel) only through protocol-owned ADP command frames
- Android client reads the latest snapshot via ADP query before any incremental ADP subscribe update is shown
- HTTP query, SSE subscribe, and POST command ingress remain compatibility paths, not the default Android live shell path
- Android client does not import or directly call `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`; it only consumes `freehand-ui-protocol` projections via HTTP
- Android client does not define a second dispatch port, a second session store, or a second completion-schema validator

## Response Mainline

- Android client renders the latest active turn projection as turn cards via `bridge.html` JS bridge
- Android client renders terminal text as the final projected message and never as raw provider payload or raw completion schema
- Android client renders tool calls and tool results as protocol-projected low-noise tool blocks with status-driven color, preserving `tool_call_id` while keeping verbose tool term text out of the main timeline
- Android client renders the top status strip from protocol-projected current-agent and slave summary
- Android client renders the right-slide drawer from local UI selection without altering truth
- Android client surfaces the connection state (connecting / connected / offline) as a local banner
- Android client surfaces agent status and turn status through protocol-projected status pills
- Android client respects light and dark themes via `mobile-mock.css` tokens
- target mobile closeout requires daemon connection config to be file-backed and Tailscale-first; the current `SharedPreferences` host/port persistence is a scaffold gap, not final connection truth

## Error Mainline

- invalid command ingress returns explicit ADP failure to the user; the Android client does not invent success
- network or ADP drop returns explicit client-visible connection state; no silent re-render and no fallback projection
- connection profile failures must expose active profile, endpoint, and concrete failure class; the client must not silently fall back to localhost, LAN scan, relay, or another profile
- provider / reason / debug error from `ui.protocol` is rendered as a red status pill; never re-projected as success
- cancel-without-active-turn clears only local input draft; does not invent a runtime mutation

## Shared Multi-Reference Functions

- `handle_android_mock`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: serve self-contained `mobile-mock.html` for design review
  - allowed callers: design-review operator
  - why shared: single preview route for all surfaces
- `crates/freehand-ui-protocol` ADP projection
  - purpose: daemon emits `UiAdpResponse::SubscriptionEvent` with `UiSubscriptionEvent` / `UiProjection::Turn` for any subscribing UI consumer
  - why shared: Android and WebUI consume the same ADP event shape
- `crates/freehand-ui-protocol` `UiCommand` enum
  - purpose: protocol-owned command ingress shape shared by all UI consumers
  - why shared: prevents a second command shape from being invented per UI surface

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | app shell entrypoint: load config, create controllers, start discovery | activity intent | app process | Android framework | `ClientConfig::load`, `HostStore::load`, controller ctors | bound |
| 02 | `ClientConfig::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | load bundled daemon config from `assets/config/client.json` with SharedPreferences overrides | Android Context | `ClientConfig` | `MainActivity::onCreate` | Gson asset parser | bound |
| 03 | `HostStore::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | load persisted host:port from SharedPreferences | Android Context | `HostConfig` | `MainActivity::onCreate` | SharedPreferences | bound |
| 04 | `HostConfig::adpUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | construct daemon ADP WebSocket URL | host + port | `ws://<host>:<port>/adp` | Android app shell | host config | bound |
| 05 | `AdpEventStream::start` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | open OkHttp WebSocket to `/adp`, subscribe latest turn, and query latest turn | no | ADP WebSocket session | `MainActivity::connectToDaemon` | OkHttp `newWebSocket` | bound |
| 06 | `AdpEventStream::sendCommand` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | wrap UiCommand JSON in an ADP command frame and send it over the active socket | UiCommand JSON | immediate send result + later command receipt/failure callback | `CommandIngress` | ADP WebSocket | bound |
| 07 | `CommandIngress::submit` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap user text in `{"SubmitUserInput":{"text":"..."}}` and dispatch through injected ADP sender | user text | `CommandResponse` | `InputBarController` | `AdpEventStream::sendCommand` | bound |
| 08 | `CommandIngress::cancelLatest` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap `{"CancelLatestActiveTurn":{}}` and dispatch through injected ADP sender | no | fire-and-forget | `MainActivity::onKeyDown` Escape | `AdpEventStream::sendCommand` | bound |
| 09 | `TimelineProjector::applyAdp` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | apply ADP query/subscription/failure frames to internal turn/slave/error state; update `latestRawTurnProjection` | `AdpEventStream.Event` | updated projector state | `AdpEventStream` onEvent callback | `applyAdpQueryResult`, `applyAdpProjection`, `applyTurnProjection` | bound |
| 10 | `AdpEventStream::stop` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | close active ADP WebSocket | no | no | `MainActivity::onPause`, `connectToDaemon` | OkHttp `WebSocket::close` | bound |
| 11 | `TimelineProjector::snapshot` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit full UI state map including `latest_turn` for native controllers | no | `Map<String, Any?>` | `MainActivity::pushSnapshotToWebView` | internal state | bound |
| 12 | `TimelineProjector::latestTurnProjectionJson` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit canonical `UiPublicTurnProjection` JSON for JS bridge | no | `String?` | `MainActivity::pushSnapshotToWebView` | `latestRawTurnProjection` | bound |
| 13 | `MainActivity::pushSnapshotToWebView` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | call `window.__freehand.applySnapshot(json)` on WebView via `evaluateJavascript` | projector snapshot | JS bridge invocation | SSE event callback, `onPageFinished` | `TimelineProjector::latestTurnProjectionJson` | bound |
| 14 | `bridge.html` JS `applySnapshot` | `apps/freehand-android/app/src/main/assets/bridge.html` | render `UiPublicTurnProjection.public_conversation` items as DOM turn cards | JSON snapshot | DOM cards | native `evaluateJavascript` | DOM API | bound |
| 15 | `HostStore::save` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | persist host:port to SharedPreferences | `HostConfig` | no | `DrawerController` callback, `selectPreferredHost` | SharedPreferences | bound |
| 16 | `MainActivity::selectPreferredHost` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | override legacy localhost / 192.168.* / port 4040 saved values with bundled config | saved + bundled `HostConfig` | resolved `HostConfig` | `discoverDaemon` | `HostStore::save`, `ClientConfig::saveOverride` | bound |
| 17 | `handle_android_mock` | `apps/freehand-server/src/lib.rs` | serve self-contained `mobile-mock.html` for design review | HTTP GET `/mock/android` | HTML body | design-review operator | embedded mock asset | bound |

## Sync Status Against Code

- all 17 call table rows are bound to real file paths and symbol names
- Android live shell now defaults to `AdpEventStream` for status/control; `ProtocolClient` and `SseEventStream` remain compatibility transport classes but are not the default `MainActivity` live path
- current config code reads bundled `assets/config/client.json` and stores host/port overrides in `SharedPreferences`; upcoming mobile closeout must replace this with an app-owned JSON config file while keeping `HostConfig::adpUrl` as the endpoint builder
- default remote access direction is Tailscale; relay profile support is schema-reserved and must stay inactive until relay protocol/auth is designed
- step 17 is code-bound to `apps/freehand-server/src/lib.rs::handle_android_mock`
- mainline call JSON and generated wiki must be regenerated from this function map
- mainline call source: `docs/mainline-calls/app.android-client.json`
- generated wiki: `docs/wiki/app.android-client.md`
- the Android client is explicitly forbidden from owning a second copy of any projection, command ingress, debug surface, or theme truth
- shared functions (`UiCommand`, SSE projection, query route) are consumed via HTTP; the Android client never imports Rust crates directly
- unit tests exist under `apps/freehand-android/app/src/test/java/com/freehand/android/data/` for `TimelineProjector`, `HostConfig`, and `CommandIngress` protocol shape
