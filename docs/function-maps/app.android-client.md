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
  - `com.freehand.android.data.DaemonConnectionConfig` — file-backed daemon connection config schema, parser, and validator
  - `com.freehand.android.data.DaemonConnectionConfigStore` — first-run bundled config bootstrap and app-owned JSON persistence
  - `com.freehand.android.data.ClientConfig` — Android Context adapter for the app-owned daemon connection config file
  - `com.freehand.android.data.HostConfig` — endpoint URL construction
  - `apps/freehand-android/scripts/verify-device-ui.sh` — explicit-serial Android device UI validation and blocker-evidence capture
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

- Android client uses `bridge.html` only as the native fallback/config-error shell; after the selected daemon ADP endpoint opens, the WebView loads the daemon-hosted WebUI page so mobile uses the same WebUI CSS, session UI, ADP rendering, and conversation layout as browser WebUI
- Android WebView settings force a mobile viewport (`useWideViewPort=false`, `loadWithOverviewMode=false`, `textZoom=100`) so the daemon-hosted WebUI aspect-ratio classifier sees phone-sized CSS dimensions instead of a desktop layout viewport
- Android client renders the latest active turn projection as turn cards via `bridge.html` JS bridge only before the remote WebUI is available
- Android client renders terminal text as the final projected message and never as raw provider payload or raw completion schema
- Android client renders tool calls and tool results as protocol-projected low-noise tool blocks with status-driven color, preserving `tool_call_id` while keeping verbose tool term text out of the main timeline
- Android client renders the top status strip from protocol-projected current-agent and slave summary
- Android client renders the right-slide drawer from local UI selection without altering truth
- Android client surfaces the connection state (connecting / connected / offline) as a local banner
- Android client surfaces agent status and turn status through protocol-projected status pills
- Android client respects light and dark themes via `mobile-mock.css` tokens
- daemon connection config is file-backed and Tailscale-first; bundled `assets/config/client.json` is bootstrap input only, and the app-owned JSON file is the long-term endpoint truth
- app-owned daemon config load may migrate only the known legacy bundled default (`tailscale-main` on `100.66.1.82:4042`) to the current release port `4041`; user-edited host/path/profile config remains authoritative and is not silently replaced

## Error Mainline

- invalid command ingress returns explicit ADP failure to the user; the Android client does not invent success
- network or ADP drop returns explicit client-visible connection state; no silent re-render and no fallback projection
- connection profile failures must expose active profile, endpoint, and concrete failure class; the client must not silently fall back to localhost, LAN scan, relay, or another profile
- provider / reason / debug error from `ui.protocol` is rendered as a red status pill; never re-projected as success
- cancel-without-active-turn clears only local input draft; does not invent a runtime mutation
- Android device validation script fails explicitly and records failed evidence when the APK is missing the configured launcher activity class or Freehand package/process emits fatal/exception logcat, and records blocker evidence only when ADB is offline, the device is locked, or Freehand is not foreground without app fatal evidence

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
| 01 | `MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | app shell entrypoint: load file-backed config, configure WebView for mobile viewport rendering, create controllers, start ADP connection | activity intent | app process | Android framework | `ClientConfig::store`, `DaemonConnectionConfigStore::load`, controller ctors | bound |
| 02 | `ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android Context to the app-owned daemon config file and bundled asset reader | Android Context | `DaemonConnectionConfigStore` | `MainActivity::onCreate` | app files dir + asset reader | bound |
| 03 | `DaemonConnectionConfigStore::load` / `DaemonConnectionConfig::parse` / `DaemonConnectionConfig::activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read app-owned daemon JSON or bootstrap bundled config, validate required schema, active profile, Tailscale-only mode, endpoint paths, relay-disabled state, and migrate the known legacy bundled default port without touching user-edited configs | config file + bundled JSON reader | validated `DaemonConnectionConfig` or explicit config error | `MainActivity::onCreate` | Gson `JsonParser`, schema validator, file IO | bound |
| 04 | `HostConfig::adpUrl` / `HostConfig::healthUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | construct selected daemon endpoint URLs from the active profile | profile host + port + paths | `ws://<host>:<port><adpPath>` and health URL | Android app shell | active profile config | bound |
| 05 | `AdpEventStream::start` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | open OkHttp WebSocket to `/adp`, subscribe latest turn, and query latest turn | no | ADP WebSocket session | `MainActivity::connectToDaemon` | OkHttp `newWebSocket` | bound |
| 06 | `AdpEventStream::sendCommand` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | wrap UiCommand JSON in an ADP command frame and send it over the active socket | UiCommand JSON | immediate send result + later command receipt/failure callback | `CommandIngress` | ADP WebSocket | bound |
| 07 | `CommandIngress::submit` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap user text in `{"SubmitUserInput":{"text":"..."}}` and dispatch through injected ADP sender | user text | `CommandResponse` | `InputBarController` | `AdpEventStream::sendCommand` | bound |
| 08 | `CommandIngress::cancelLatest` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap `{"CancelLatestActiveTurn":{}}` and dispatch through injected ADP sender | no | fire-and-forget | `MainActivity::onKeyDown` Escape | `AdpEventStream::sendCommand` | bound |
| 09 | `TimelineProjector::applyAdp` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | apply ADP query/subscription/failure frames to internal turn/slave/error state; update `latestRawTurnProjection` | `AdpEventStream.Event` | updated projector state | `AdpEventStream` onEvent callback | `applyAdpQueryResult`, `applyAdpProjection`, `applyTurnProjection` | bound |
| 10 | `AdpEventStream::stop` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | close active ADP WebSocket | no | no | `MainActivity::onPause`, `connectToDaemon` | OkHttp `WebSocket::close` | bound |
| 11 | `TimelineProjector::snapshot` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit full UI state map including `latest_turn` for native controllers | no | `Map<String, Any?>` | `MainActivity::pushSnapshotToWebView` | internal state | bound |
| 12 | `TimelineProjector::latestTurnProjectionJson` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit canonical `UiPublicTurnProjection` JSON for JS bridge | no | `String?` | `MainActivity::pushSnapshotToWebView` | `latestRawTurnProjection` | bound |
| 13 | `MainActivity::pushSnapshotToWebView` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | call `window.__freehand.applySnapshot(json)` on the fallback bridge only until remote daemon WebUI is loaded | projector snapshot | JS bridge invocation or no-op after remote WebUI load | ADP event callback, `onPageFinished` | `TimelineProjector::latestTurnProjectionJson` | bound |
| 14 | `bridge.html` JS `applySnapshot` | `apps/freehand-android/app/src/main/assets/bridge.html` | render `UiPublicTurnProjection.public_conversation` items as DOM turn cards in fallback/config-error mode only | JSON snapshot | DOM cards | native `evaluateJavascript` | DOM API | bound |
| 14a | `MainActivity::loadRemoteWebUi` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | hide native chrome after ADP open and load daemon-hosted WebUI so Android shares WebUI visual/session/ADP rendering truth | validated `HostConfig` | WebView URL `http://<host>:<port>/` | `MainActivity::connectToDaemon` ADP onOpen | daemon-hosted WebUI | bound |
| 15 | `DaemonConnectionConfigStore::write` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | persist validated daemon config to the app-owned JSON file | `DaemonConnectionConfig` | normalized JSON file or explicit config error | `MainActivity::saveHostConfig` | file IO + schema validator | bound |
| 16 | `MainActivity::saveHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | update the active profile endpoint, write app-owned JSON, and reconnect only after write success | edited `HostConfig` | updated config or visible config error | `DrawerController` callback | `DaemonConnectionConfigStore::write` | bound |
| 17 | `handle_android_mock` | `apps/freehand-server/src/lib.rs` | serve self-contained `mobile-mock.html` for design review | HTTP GET `/mock/android` | HTML body | design-review operator | embedded mock asset | bound |
| 18 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | validate explicit-serial Android device UI foreground state and capture blocker/failure evidence; APK launcher-class and fatal-logcat failures are classified before not-foreground blockers | adb serial + debug APK | passed/blocked/failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap | bound |

## Sync Status Against Code

- all 18 call table rows are bound to real file paths and symbol names
- Android live shell now defaults to `AdpEventStream` for status/control; `ProtocolClient` and `SseEventStream` remain compatibility transport classes but are not the default `MainActivity` live path
- current config code bootstraps bundled `assets/config/client.json` into an app-owned JSON file and uses that file as endpoint truth; `SharedPreferences` no longer owns daemon host/port persistence
- default remote access direction is Tailscale; relay profile support is schema-reserved and must stay inactive until relay protocol/auth is designed
- step 17 is code-bound to `apps/freehand-server/src/lib.rs::handle_android_mock`
- mainline call JSON and generated wiki must be regenerated from this function map
- mainline call source: `docs/mainline-calls/app.android-client.json`
- generated wiki: `docs/wiki/app.android-client.md`
- the Android client is explicitly forbidden from owning a second copy of any projection, command ingress, debug surface, or theme truth
- shared functions (`UiCommand`, SSE projection, query route) are consumed via HTTP; the Android client never imports Rust crates directly
- unit tests exist under `apps/freehand-android/app/src/test/java/com/freehand/android/data/` for `TimelineProjector`, `HostConfig`, and `CommandIngress` protocol shape
- device UI verification script exists for explicit-serial true-device evidence capture; current completion still requires a connected/unlocked device for acceptance evidence
