# Function Map: `app.android-client`

- feature_id: `app.android-client`
- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- owner entry symbols:
  - `com.freehand.android.ui.MainActivity` — app shell entrypoint, WebView host, controller composition
  - `com.freehand.android.data.SseEventStream` — OkHttp SSE event stream consumer
  - `com.freehand.android.data.TimelineProjector` — ui.protocol event → UI state projection
  - `com.freehand.android.data.CommandIngress` — submit / cancel via protocol-owned HTTP command ingress
  - `com.freehand.android.data.ProtocolClient` — HTTP query + command POST against `freehand-ui-protocol`
  - `com.freehand.android.data.ClientConfig` — bundled config loading from `assets/config/client.json`
  - `com.freehand.android.data.HostStore` — host:port persistence in SharedPreferences
  - `com.freehand.android.data.HostConfig` — endpoint URL construction
- reference mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html`
- reference bridge: `apps/freehand-android/app/src/main/assets/bridge.html`

## Request Mainline

- Android client shell receives user input and forwards it as command ingress through the protocol-owned HTTP command ingress route
- Android client shell never mutates session, reason, debug, metadata, or provider truth locally
- Android client subscribes to `ui.protocol` turn / debug / status projections through HTTP query + SSE subscribe
- Android client submits user actions (submit / cancel) only through protocol-owned command ingress
- Android client reads the latest snapshot via `ui.protocol` HTTP query before any incremental SSE subscribe update is shown
- Android client does not import or directly call `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`; it only consumes `freehand-ui-protocol` projections via HTTP
- Android client does not define a second dispatch port, a second session store, or a second completion-schema validator

## Response Mainline

- Android client renders the latest active turn projection as turn cards via `bridge.html` JS bridge
- Android client renders terminal text as the final projected message and never as raw provider payload or raw completion schema
- Android client renders tool calls and tool results as protocol-projected tool blocks with status-driven color
- Android client renders the top status strip from protocol-projected current-agent and slave summary
- Android client renders the right-slide drawer from local UI selection without altering truth
- Android client surfaces the connection state (connecting / connected / offline) as a local banner
- Android client surfaces agent status and turn status through protocol-projected status pills
- Android client respects light and dark themes via `mobile-mock.css` tokens

## Error Mainline

- invalid command ingress returns explicit HTTP error to the user; the Android client does not invent success
- network or SSE drop returns explicit client-visible connection state; no silent re-render and no fallback projection
- provider / reason / debug error from `ui.protocol` is rendered as a red status pill; never re-projected as success
- cancel-without-active-turn clears only local input draft; does not invent a runtime mutation

## Shared Multi-Reference Functions

- `handle_android_mock`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: serve self-contained `mobile-mock.html` for design review
  - allowed callers: design-review operator
  - why shared: single preview route for all surfaces
- `crates/freehand-ui-protocol` SSE projection
  - purpose: daemon emits `UiSubscriptionEvent` with `UiProjection::Turn` for any subscribing UI consumer
  - why shared: Android and WebUI consume the same SSE event shape
- `crates/freehand-ui-protocol` `UiCommand` enum
  - purpose: protocol-owned command ingress shape shared by all UI consumers
  - why shared: prevents a second command shape from being invented per UI surface

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | app shell entrypoint: load config, create controllers, start discovery | activity intent | app process | Android framework | `ClientConfig::load`, `HostStore::load`, controller ctors | bound |
| 02 | `ClientConfig::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | load bundled daemon config from `assets/config/client.json` with SharedPreferences overrides | Android Context | `ClientConfig` | `MainActivity::onCreate` | Gson asset parser | bound |
| 03 | `HostStore::load` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | load persisted host:port from SharedPreferences | Android Context | `HostConfig` | `MainActivity::onCreate` | SharedPreferences | bound |
| 04 | `ProtocolClient::getLatestTurn` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` | HTTP GET to `ui/query/latest-active-turn` | no | `JsonObject?` | Android app shell | HTTP GET `HostConfig::latestTurnUrl` | bound |
| 05 | `ProtocolClient::postCommand` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` | HTTP POST to `ui/command` with UiCommand external-tag JSON | UiCommand JSON | `CommandResponse` | `CommandIngress` | HTTP POST `HostConfig::commandUrl` | bound |
| 06 | `CommandIngress::submit` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap user text in `{"SubmitUserInput":{"text":"..."}}` and dispatch | user text | `CommandResponse` | `InputBarController` | `ProtocolClient::postCommand` | bound |
| 07 | `CommandIngress::cancelLatest` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/CommandIngress.kt` | wrap `{"CancelLatestActiveTurn":{}}` and dispatch | no | fire-and-forget | `MainActivity::onKeyDown` Escape | `ProtocolClient::postCommand` | bound |
| 08 | `SseEventStream::start` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt` | OkHttp SSE subscribe to `ui/subscribe/turn/latest` | no | `Event` stream | `MainActivity::connectToDaemon` | OkHttp `EventSources.createFactory` | bound |
| 09 | `SseEventStream::stop` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt` | cancel active SSE EventSource | no | no | `MainActivity::onPause`, `connectToDaemon` | OkHttp `EventSource::cancel` | bound |
| 10 | `TimelineProjector::apply` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | apply SSE event to internal turn/slave state; update `latestRawTurnProjection` | `SseEventStream.Event` | updated projector state | `SseEventStream` onEvent callback | `applyTurnEnvelope`, `applyNodeStatus`, `applyError`, `applyTerminal`, `applyProgress` | bound |
| 11 | `TimelineProjector::snapshot` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit full UI state map including `latest_turn` for native controllers | no | `Map<String, Any?>` | `MainActivity::pushSnapshotToWebView` | internal state | bound |
| 12 | `TimelineProjector::latestTurnProjectionJson` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` | emit canonical `UiPublicTurnProjection` JSON for JS bridge | no | `String?` | `MainActivity::pushSnapshotToWebView` | `latestRawTurnProjection` | bound |
| 13 | `MainActivity::pushSnapshotToWebView` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | call `window.__freehand.applySnapshot(json)` on WebView via `evaluateJavascript` | projector snapshot | JS bridge invocation | SSE event callback, `onPageFinished` | `TimelineProjector::latestTurnProjectionJson` | bound |
| 14 | `bridge.html` JS `applySnapshot` | `apps/freehand-android/app/src/main/assets/bridge.html` | render `UiPublicTurnProjection.public_conversation` items as DOM turn cards | JSON snapshot | DOM cards | native `evaluateJavascript` | DOM API | bound |
| 15 | `HostStore::save` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostStore.kt` | persist host:port to SharedPreferences | `HostConfig` | no | `DrawerController` callback, `selectPreferredHost` | SharedPreferences | bound |
| 16 | `MainActivity::selectPreferredHost` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | override legacy localhost / 192.168.* / port 4040 saved values with bundled config | saved + bundled `HostConfig` | resolved `HostConfig` | `discoverDaemon` | `HostStore::save`, `ClientConfig::saveOverride` | bound |
| 17 | `handle_android_mock` | `apps/freehand-server/src/lib.rs` | serve self-contained `mobile-mock.html` for design review | HTTP GET `/mock/android` | HTML body | design-review operator | embedded mock asset | bound |

## Sync Status Against Code

- all 17 call table rows are bound to real file paths and symbol names
- steps 01-16 are code-bound to actual Kotlin classes in `apps/freehand-android/app/src/main/java/com/freehand/android/`
- step 17 is code-bound to `apps/freehand-server/src/lib.rs::handle_android_mock`
- mainline call JSON and generated wiki must be regenerated from this function map
- mainline call source: `docs/mainline-calls/app.android-client.json`
- generated wiki: `docs/wiki/app.android-client.md`
- the Android client is explicitly forbidden from owning a second copy of any projection, command ingress, debug surface, or theme truth
- shared functions (`UiCommand`, SSE projection, query route) are consumed via HTTP; the Android client never imports Rust crates directly
- unit tests exist under `apps/freehand-android/app/src/test/java/com/freehand/android/data/` for `TimelineProjector`, `HostConfig`, and `CommandIngress` protocol shape
