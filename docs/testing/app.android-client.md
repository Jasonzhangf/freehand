# Test Design: `app.android-client`

- feature_id: `app.android-client`
- owner: `apps/freehand-android`
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- reference function map: `docs/function-maps/app.android-client.md`

## Lifecycle Path Under Test

1. `ClientConfig::load` parses bundled `assets/config/client.json` with Gson; falls back to hardcoded defaults on parse failure
2. `ClientConfig::load` merges SharedPreferences overrides (saved host/port) over bundled defaults
3. `HostStore::load` reads persisted host:port from SharedPreferences
4. `HostStore::save` writes host:port to SharedPreferences
5. `HostConfig` constructs correct URLs: `baseUrl`, `adpUrl`, `commandUrl`, `latestTurnUrl`, `latestTurnSseUrl`
6. `AdpEventStream::start` opens OkHttp WebSocket to `/adp`, sends `SubscribeLatestActiveTurn`, and sends `QueryLatestActiveTurn`
7. `AdpEventStream::sendCommand` wraps external-tag UiCommand JSON in a `kind=command` ADP frame
8. `CommandIngress::submit` wraps user text in `{"SubmitUserInput":{"text":"..."}}` JSON shape and dispatches through injected ADP sender by default
9. `CommandIngress::cancelLatest` wraps `{"CancelLatestActiveTurn":{}}` JSON shape and dispatches through injected ADP sender by default
10. `SseEventStream::start` opens OkHttp SSE to `ui/subscribe/turn/latest` only as compatibility transport
11. `SseEventStream::stop` cancels active EventSource compatibility transport
12. `TimelineProjector::applyAdp` routes ADP frames by `kind`: subscription_accepted, subscription_event, query_result, failure
13. `TimelineProjector::apply` still routes SSE compatibility events by `eventName`: turn, progress, node_status, error, terminal
13. `TimelineProjector::apply` turn event parses `UiTurnProjection` fields without crashing on JSON null values
14. `TimelineProjector::apply` turn event stores `latestRawTurnProjection` for bridge consumption
15. `TimelineProjector::apply` node_status populates slave map
16. `TimelineProjector::apply` error marks turn as error
17. `TimelineProjector::apply` terminal updates turn terminal status
18. `TimelineProjector::apply` progress updates turn state text
19. `TimelineProjector::snapshot` returns full state map
20. `TimelineProjector::latestTurnProjectionJson` returns canonical JSON for bridge
21. `TimelineProjector::fallbackTurnsJson` returns legacy flat turns array
22. `TimelineProjector::setConnectionState` updates connection field in snapshot
23. `MainActivity::selectPreferredHost` overrides legacy localhost/192.168.* with bundled config
24. `MainActivity::selectPreferredHost` overrides same host with legacy port 4040
25. `bridge.html` JS `applySnapshot` renders `public_conversation` items as DOM cards
26. `handle_android_mock` serves `mobile-mock.html` with HTTP 200

## White-Box Plan

- `TimelineProjectorTest`: covers turn event parsing (running/success/error/null terminal_status), ADP subscription_event projection including low-noise status-only tool summaries, ADP failure projection, progress, node_status (healthy/unhealthy), error, terminal, empty state, snapshot JSON, connection state, fallbackTurnsJson, latestTurnProjectionJson preservation
- `HostConfigTest`: covers URL construction for different hosts/ports, including `adpUrl`
- `CommandIngressProtocolTest`: covers SubmitUserInput shape, CancelLatestActiveTurn shape, ADP command frame shape, ADP subscribe frame shape, query-as-command negative frame shape, old type-field negative, special characters, empty text

### Protocol Replay Harness

- feed canned ADP `UiAdpResponse::SubscriptionEvent` JSON fixtures into `TimelineProjector::applyAdp`
- feed canned ADP failure JSON fixtures into `TimelineProjector::applyAdp`
- feed canned SSE compatibility `UiSubscriptionEvent` JSON fixtures into `TimelineProjector::apply`
- assert `snapshotJson()` and `latestTurnProjectionJson()` preserve the canonical `turn` + `public_conversation` shape
- replay both success and failure paths so null-safe parsing and terminal-state mapping stay locked

## Module Black-Box Plan

- `cd apps/freehand-android && ./gradlew testDebugUnitTest` runs the JVM-level Android owner tests
- `ClientConfig::load` parses real bundled `assets/config/client.json` and produces correct `ClientConfig` values
- `HostStore` round-trips host:port through SharedPreferences
- `AdpEventStream::buildFrame` produces correct ADP command/subscribe/query misuse frame shapes (verified by protocol shape tests)
- `ProtocolClient::postCommand` remains a compatibility HTTP request body path
- `handle_android_mock` returns HTTP 200 with `mock-mobile` class (existing server test)

## Project Black-Box Impact

- Android app boundary proves protocol-only consumption: submit/query/subscribe via ADP frames, compatibility SSE via `UiSubscriptionEvent`, compatibility query via HTTP GET
- Android app boundary proves no direct import of `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`
- Android app boundary proves Gson JsonNull safety in `TimelineProjector` (JSON null values do not crash)
- Android app boundary proves `bridge.html` renders the same public conversation projection as the server-side design preview

## Known Gaps

- no Espresso / instrumented tests yet (device-dependent; requires ADB-connected device)
- no integration smoke against live daemon from a real Android device yet (requires daemon running + device connected)
- `bridge.html` JS rendering is not unit-testable from JVM; requires WebView instrumented test
- `MainActivity` lifecycle (onCreate, onResume, onPause, onKeyDown) not unit-testable without Android framework; covered by design + future instrumented tests

## Sync Status Between Design and Implementation

- design: `docs/design/multi-platform-ui-architecture.md` (locked)
- design: `docs/design/android-client-v1-execution.md` (locked)
- function map: `docs/function-maps/app.android-client.md` (bound to real Kotlin symbols)
- feature map: `app.android-client` entry (locked)
- mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html` (self-contained)
- bridge: `apps/freehand-android/app/src/main/assets/bridge.html` (live WebView host)
- unit tests: `apps/freehand-android/app/src/test/java/com/freehand/android/data/` cover ADP URL/frame/projector behavior and compatibility protocol shapes
