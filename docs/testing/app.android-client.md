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
5. `HostConfig` constructs correct URLs: `baseUrl`, `commandUrl`, `latestTurnUrl`, `latestTurnSseUrl`
6. `ProtocolClient::postCommand` sends HTTP POST to `ui/command` with correct UiCommand external-tag JSON
7. `ProtocolClient::getLatestTurn` sends HTTP GET to `ui/query/latest-active-turn`
8. `CommandIngress::submit` wraps user text in `{"SubmitUserInput":{"text":"..."}}` JSON shape
9. `CommandIngress::cancelLatest` wraps `{"CancelLatestActiveTurn":{}}` JSON shape
10. `SseEventStream::start` opens OkHttp SSE to `ui/subscribe/turn/latest`
11. `SseEventStream::stop` cancels active EventSource
12. `TimelineProjector::apply` routes SSE events by `eventName`: turn, progress, node_status, error, terminal
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

- `TimelineProjectorTest`: 14 tests covering turn event parsing (running/success/error/null terminal_status), progress, node_status (healthy/unhealthy), error, terminal, empty state, snapshot JSON, connection state, fallbackTurnsJson, latestTurnProjectionJson preservation
- `HostConfigTest`: 5 tests covering URL construction for different hosts/ports
- `CommandIngressProtocolTest`: 5 tests covering SubmitUserInput shape, CancelLatestActiveTurn shape, negative (old type field), special characters, empty text

## Module Black-Box Plan

- `ClientConfig::load` parses real bundled `assets/config/client.json` and produces correct `ClientConfig` values
- `HostStore` round-trips host:port through SharedPreferences
- `ProtocolClient::postCommand` produces correct HTTP request body (verified by protocol shape tests)
- `handle_android_mock` returns HTTP 200 with `mock-mobile` class (existing server test)

## Project Black-Box Impact

- Android app boundary proves protocol-only consumption: submit via `UiCommand` external-tag, SSE via `UiSubscriptionEvent`, query via HTTP GET
- Android app boundary proves no direct import of `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`
- Android app boundary proves Gson JsonNull safety in `TimelineProjector` (JSON null values do not crash)

## Known Gaps

- no Espresso / instrumented tests yet (device-dependent; requires ADB-connected device)
- no integration smoke against live daemon (requires daemon running + device connected)
- `bridge.html` JS rendering is not unit-testable from JVM; requires WebView instrumented test
- `MainActivity` lifecycle (onCreate, onResume, onPause, onKeyDown) not unit-testable without Android framework; covered by design + future instrumented tests

## Sync Status Between Design and Implementation

- design: `docs/design/multi-platform-ui-architecture.md` (locked)
- design: `docs/design/android-client-v1-execution.md` (locked)
- function map: `docs/function-maps/app.android-client.md` (bound to real Kotlin symbols)
- feature map: `app.android-client` entry (locked)
- mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html` (self-contained)
- bridge: `apps/freehand-android/app/src/main/assets/bridge.html` (live WebView host)
- unit tests: `apps/freehand-android/app/src/test/java/com/freehand/android/data/` (24 tests, all pass)
