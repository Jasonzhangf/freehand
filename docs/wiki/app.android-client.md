# Wiki: `app.android-client`

Generated from `docs/mainline-calls/app.android-client.json`. Do not edit by hand.

- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- function map: `docs/function-maps/app.android-client.md`
- generated wiki: `docs/wiki/app.android-client.md`
- test design: `docs/testing/app.android-client.md`

## Request Mainline

- user input enters via InputBarController -> CommandIngress::submit wraps SubmitUserInput UiCommand -> ProtocolClient::postCommand POSTs to /ui/command
- cancel enters via MainActivity onKeyDown Escape -> CommandIngress::cancelLatest wraps CancelLatestActiveTurn -> ProtocolClient::postCommand POSTs to /ui/command
- Android client never mutates session/reason/debug/metadata/provider truth

## Response Mainline

- daemon SSE emits UiSubscriptionEvent with UiProjection::Turn -> SseEventStream receives Event -> TimelineProjector::apply updates state + latestRawTurnProjection
- MainActivity::pushSnapshotToWebView calls window.__freehand.applySnapshot(json) on bridge.html
- bridge.html JS renders public_conversation items as DOM turn cards

## Error Mainline

- SSE onError -> projector.setConnectionState('error') + statusBanner.showPersistent
- ProtocolClient::postCommand HTTP error -> CommandResponse(ok=false, code='http_NNN')
- TimelineProjector::applyError marks turn as error state

## Shared Multi-Reference Functions

- `handle_android_mock`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: serve self-contained mobile-mock.html for design review
  - allowed callers: design-review operator
  - related tests: android_mock_route_returns_design_preview
  - why shared: single preview route for all surfaces
- `UiSubscriptionEvent SSE projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: daemon emits UiSubscriptionEvent with UiProjection::Turn for any subscribing UI consumer
  - allowed callers: Android SseEventStream, WebUI EventSource, any protocol-only UI consumer
  - related tests: app.android-client TimelineProjectorTest
  - why shared: Android and WebUI consume the same SSE event shape
- `UiCommand enum`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: protocol-owned command ingress shape shared by all UI consumers
  - allowed callers: Android CommandIngress, WebUI webui.js, CLI
  - related tests: CommandIngressProtocolTest
  - why shared: prevents a second command shape from being invented per UI surface

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | app shell entrypoint: load config, create controllers, start discovery | activity intent | app process | Android framework | ClientConfig::load, HostStore::load, controller ctors | bound |
| 02 | `com.freehand.android.data.ClientConfig::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | load bundled daemon config from assets/config/client.json with SharedPreferences overrides | Android Context | ClientConfig | MainActivity::onCreate | Gson asset parser | bound |
| 03 | `com.freehand.android.data.HostStore::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | load persisted host:port from SharedPreferences | Android Context | HostConfig | MainActivity::onCreate | SharedPreferences | bound |
| 04 | `com.freehand.android.data.ProtocolClient::getLatestTurn` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` | HTTP GET to ui/query/latest-active-turn | no | JsonObject? | Android app shell | HTTP GET HostConfig::latestTurnUrl | bound |
| 05 | `com.freehand.android.data.ProtocolClient::postCommand` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` | HTTP POST to ui/command with UiCommand external-tag JSON | UiCommand JSON | CommandResponse | CommandIngress | HTTP POST HostConfig::commandUrl | bound |
| 06 | `com.freehand.android.data.CommandIngress::submit` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap user text in SubmitUserInput UiCommand and dispatch | user text | CommandResponse | InputBarController | ProtocolClient::postCommand | bound |
| 07 | `com.freehand.android.data.CommandIngress::cancelLatest` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap CancelLatestActiveTurn UiCommand and dispatch | no | fire-and-forget | MainActivity::onKeyDown Escape | ProtocolClient::postCommand | bound |
| 08 | `com.freehand.android.data.SseEventStream::start` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt` | OkHttp SSE subscribe to ui/subscribe/turn/latest | no | Event stream | MainActivity::connectToDaemon | OkHttp EventSources.createFactory | bound |
| 09 | `com.freehand.android.data.SseEventStream::stop` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt` | cancel active SSE EventSource | no | no | MainActivity::onPause, connectToDaemon | OkHttp EventSource::cancel | bound |
| 10 | `com.freehand.android.data.TimelineProjector::apply` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | apply SSE event to internal turn/slave state; update latestRawTurnProjection | SseEventStream.Event | updated projector state | SseEventStream onEvent callback | applyTurnEnvelope, applyNodeStatus, applyError, applyTerminal, applyProgress | bound |
| 11 | `com.freehand.android.data.TimelineProjector::snapshot` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit full UI state map including latest_turn for native controllers | no | Map<String, Any?> | MainActivity::pushSnapshotToWebView | internal state | bound |
| 12 | `com.freehand.android.data.TimelineProjector::latestTurnProjectionJson` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit canonical UiPublicTurnProjection JSON for JS bridge | no | String? | MainActivity::pushSnapshotToWebView | latestRawTurnProjection | bound |
| 13 | `com.freehand.android.ui.MainActivity::pushSnapshotToWebView` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | call window.__freehand.applySnapshot(json) on WebView via evaluateJavascript | projector snapshot | JS bridge invocation | SSE event callback, onPageFinished | TimelineProjector::latestTurnProjectionJson | bound |
| 14 | `applySnapshot` | `apps/freehand-android/app/src/main/assets/bridge.html` | render UiPublicTurnProjection.public_conversation items as DOM turn cards | JSON snapshot | DOM cards | native evaluateJavascript | DOM API | bound |
| 15 | `com.freehand.android.data.HostStore::save` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | persist host:port to SharedPreferences | HostConfig | no | DrawerController callback, selectPreferredHost | SharedPreferences | bound |
| 16 | `com.freehand.android.ui.MainActivity::selectPreferredHost` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | override legacy localhost / 192.168.* / port 4040 saved values with bundled config | saved + bundled HostConfig | resolved HostConfig | discoverDaemon | HostStore::save, ClientConfig::saveOverride | bound |
| 17 | `freehand_server::handle_android_mock` | `apps/freehand-server/src/lib.rs` | serve self-contained mobile-mock.html for design review | HTTP GET /mock/android | HTML body | design-review operator | embedded mock asset | bound |

## Sync Status Against Mainline Call

- all 17 call table rows bound to real Kotlin symbols in apps/freehand-android/app/src/main/java/com/freehand/android/
- unit tests exist for TimelineProjector (14 tests), HostConfig (5 tests), CommandIngress protocol (5 tests)
- mainline JSON generated from function map
- generated wiki must be regenerated from docs/mainline-calls/app.android-client.json when this function-map truth changes
