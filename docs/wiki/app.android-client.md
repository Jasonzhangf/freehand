# Wiki: `app.android-client`

Generated from `docs/mainline-calls/app.android-client.json`. Do not edit by hand.

- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- function map: `docs/function-maps/app.android-client.md`
- generated wiki: `docs/wiki/app.android-client.md`
- test design: `docs/testing/app.android-client.md`

## Request Mainline

- user input enters via InputBarController -> CommandIngress::submit wraps SubmitUserInput UiCommand -> AdpEventStream::sendCommand sends a kind=command frame to /adp
- cancel enters via MainActivity onKeyDown Escape -> CommandIngress::cancelLatest wraps CancelLatestActiveTurn -> AdpEventStream::sendCommand sends a kind=command frame to /adp
- Android default status/control path opens daemon ADP WebSocket /adp and sends query/subscribe frames before rendering live state
- HTTP query, SSE subscribe, and POST command ingress remain compatibility transport paths, not the default Android live shell path
- Android client never mutates session/reason/debug/metadata/provider truth

## Response Mainline

- daemon ADP emits UiAdpResponse::SubscriptionEvent with UiSubscriptionEvent / UiProjection::Turn -> AdpEventStream receives Event -> TimelineProjector::applyAdp updates state + latestRawTurnProjection
- MainActivity::pushSnapshotToWebView calls window.__freehand.applySnapshot(json) on bridge.html
- bridge.html JS renders public_conversation items as DOM turn cards

## Error Mainline

- ADP onError -> projector.setConnectionState('error') + statusBanner.showPersistent
- ADP failure frame -> TimelineProjector::applyAdp marks visible error projection
- AdpEventStream::sendCommand send failure -> CommandResponse(ok=false, code='adp_send_failed')
- Android device validation script fails explicitly and records blocker evidence when ADB is offline, the device is locked, Freehand is not foreground, or fatal logcat entries are present

## Shared Multi-Reference Functions

- `handle_android_mock`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: serve self-contained mobile-mock.html for design review
  - allowed callers: design-review operator
  - related tests: android_mock_route_returns_design_preview
  - why shared: single preview route for all surfaces
- `UiAdpResponse / UiSubscriptionEvent ADP projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: daemon emits UiAdpResponse::SubscriptionEvent with UiSubscriptionEvent / UiProjection::Turn for any subscribing UI consumer
  - allowed callers: Android AdpEventStream, WebUI ADP client, any protocol-only UI consumer
  - related tests: app.android-client TimelineProjectorTest
  - why shared: Android and WebUI consume the same ADP event shape
- `UiCommand enum`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: protocol-owned command ingress shape shared by all UI consumers
  - allowed callers: Android CommandIngress, WebUI webui.js, CLI
  - related tests: CommandIngressProtocolTest
  - why shared: prevents a second command shape from being invented per UI surface

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | app shell entrypoint: load file-backed config, create controllers, start ADP connection | activity intent | app process | Android framework | ClientConfig::store, DaemonConnectionConfigStore::load, controller ctors | bound |
| 02 | `com.freehand.android.data.ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android Context to the app-owned daemon config file and bundled asset reader | Android Context | DaemonConnectionConfigStore | MainActivity::onCreate | app files dir plus asset reader | bound |
| 03 | `com.freehand.android.data.DaemonConnectionConfigStore::load / com.freehand.android.data.DaemonConnectionConfig::parse / com.freehand.android.data.DaemonConnectionConfig::activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read app-owned daemon JSON or bootstrap bundled config, validate required schema, active profile, Tailscale-only mode, endpoint paths, and relay-disabled state | config file plus bundled JSON reader | validated DaemonConnectionConfig or explicit config error | MainActivity::onCreate | Gson JsonParser, schema validator, file IO | bound |
| 04 | `com.freehand.android.data.HostConfig::adpUrl / com.freehand.android.data.HostConfig::healthUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | construct selected daemon endpoint URLs from the active profile | profile host plus port plus paths | ws://<host>:<port><adpPath> and health URL | Android app shell | active profile config | bound |
| 05 | `com.freehand.android.data.AdpEventStream::start` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | open OkHttp WebSocket to /adp, subscribe latest turn, and query latest turn | no | ADP WebSocket session | MainActivity::connectToDaemon | OkHttp newWebSocket | bound |
| 06 | `com.freehand.android.data.AdpEventStream::sendCommand` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | wrap UiCommand JSON in an ADP command frame and send it over the active socket | UiCommand JSON | immediate send result plus later command receipt or failure callback | CommandIngress | ADP WebSocket | bound |
| 07 | `com.freehand.android.data.CommandIngress::submit` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap user text in SubmitUserInput UiCommand and dispatch through injected ADP sender | user text | CommandResponse | InputBarController | AdpEventStream::sendCommand | bound |
| 08 | `com.freehand.android.data.CommandIngress::cancelLatest` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap CancelLatestActiveTurn UiCommand and dispatch through injected ADP sender | no | fire-and-forget | MainActivity::onKeyDown Escape | AdpEventStream::sendCommand | bound |
| 09 | `com.freehand.android.data.TimelineProjector::applyAdp` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | apply ADP query, subscription, and failure frames to internal turn/slave/error state; update latestRawTurnProjection | AdpEventStream.Event | updated projector state | AdpEventStream onEvent callback | applyAdpQueryResult, applyAdpProjection, applyTurnProjection | bound |
| 10 | `com.freehand.android.data.AdpEventStream::stop` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/AdpEventStream.kt` | close active ADP WebSocket | no | no | MainActivity::onPause, connectToDaemon | OkHttp WebSocket::close | bound |
| 11 | `com.freehand.android.data.TimelineProjector::snapshot` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit full UI state map including latest_turn for native controllers | no | Map<String, Any?> | MainActivity::pushSnapshotToWebView | internal state | bound |
| 12 | `com.freehand.android.data.TimelineProjector::latestTurnProjectionJson` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit canonical UiPublicTurnProjection JSON for JS bridge | no | String? | MainActivity::pushSnapshotToWebView | latestRawTurnProjection | bound |
| 13 | `com.freehand.android.ui.MainActivity::pushSnapshotToWebView` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | call window.__freehand.applySnapshot(json) on WebView via evaluateJavascript | projector snapshot | JS bridge invocation | SSE event callback, onPageFinished | TimelineProjector::latestTurnProjectionJson | bound |
| 14 | `applySnapshot` | `apps/freehand-android/app/src/main/assets/bridge.html` | render UiPublicTurnProjection.public_conversation items as DOM turn cards | JSON snapshot | DOM cards | native evaluateJavascript | DOM API | bound |
| 15 | `com.freehand.android.data.DaemonConnectionConfigStore::write` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | persist validated daemon config to the app-owned JSON file | DaemonConnectionConfig | normalized JSON file or explicit config error | MainActivity::saveHostConfig | file IO plus schema validator | bound |
| 16 | `com.freehand.android.ui.MainActivity::saveHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | update the active profile endpoint, write app-owned JSON, and reconnect only after write success | edited HostConfig | updated config or visible config error | DrawerController callback | DaemonConnectionConfigStore::write | bound |
| 17 | `freehand_server::handle_android_mock` | `apps/freehand-server/src/lib.rs` | serve self-contained mobile-mock.html for design review | HTTP GET /mock/android | HTML body | design-review operator | embedded mock asset | bound |
| 18 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | validate explicit-serial Android device UI foreground state and capture blocker or failure evidence | adb serial plus debug APK | passed, blocked, or failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap | bound |

## Sync Status Against Mainline Call

- all 18 call table rows bound to real Kotlin symbols or owner scripts in apps/freehand-android/
- Android daemon connection config bootstraps bundled assets/config/client.json into app-owned daemon-connection.json, then uses that JSON file as endpoint truth
- SharedPreferences no longer owns daemon host or port persistence
- Android live shell now defaults to AdpEventStream for status/control; ProtocolClient and SseEventStream remain compatibility transport classes but are not the default MainActivity live path
- unit tests exist for TimelineProjector, HostConfig, and CommandIngress protocol, including ADP URL/frame/projector coverage
- device UI verification script records pass/block/fail evidence for explicit-serial Android true-device validation
- mainline JSON generated from function map
- generated wiki must be regenerated from docs/mainline-calls/app.android-client.json when this function-map truth changes
