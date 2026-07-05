# Test Design: `app.android-client`

- feature_id: `app.android-client`
- owner: `apps/freehand-android`
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- reference function map: `docs/function-maps/app.android-client.md`

## Lifecycle Path Under Test

1. `ClientConfig::store` adapts Android `Context` into an app-owned daemon config file path plus bundled config reader
2. `DaemonConnectionConfigStore::load` reads the app-owned JSON file when present
3. `DaemonConnectionConfigStore::load` bootstraps bundled `assets/config/client.json` into the app-owned JSON file on first run
4. `DaemonConnectionConfigStore::write` persists validated daemon connection config to the app-owned JSON file
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
23. `DaemonConnectionConfig::parse` rejects malformed JSON, missing required fields, missing active profile, unsupported connection modes, invalid paths, and relay-enabled config explicitly
24. `MainActivity::saveHostConfig` writes the edited active profile endpoint and reconnects only after the app-owned JSON write succeeds
25. `bridge.html` JS `applySnapshot` renders `public_conversation` items as DOM cards
26. `handle_android_mock` serves `mobile-mock.html` with HTTP 200
27. target mobile closeout: file-backed daemon config bootstraps from bundled `assets/config/client.json` on first run
28. target mobile closeout: file-backed daemon config persists user-edited active profile to an app-owned JSON file, not only SharedPreferences
29. target mobile closeout: default active profile is `tailscale` and builds health + ADP endpoints from configured host/port/path
30. target mobile closeout: relay config fields are parsed but remain disabled unless explicitly selected by a future relay design
31. target mobile closeout: connection failure projection includes active profile, endpoint, and concrete failure class
32. target mobile closeout: aspect-ratio shape selection maps phone portrait, phone landscape, tablet portrait, tablet landscape, foldable unfolded, and desktop-like WebView into the expected layout mode without changing session/protocol truth
33. `apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` validates true-device foreground state and captures explicit blocker/failure evidence when ADB/device/UI is not usable

## White-Box Plan

- `TimelineProjectorTest`: covers turn event parsing (running/success/error/null terminal_status), ADP subscription_event projection including low-noise status-only tool summaries, ADP failure projection, progress, node_status (healthy/unhealthy), error, terminal, empty state, snapshot JSON, connection state, fallbackTurnsJson, latestTurnProjectionJson preservation
- `HostConfigTest`: covers URL construction for different hosts/ports and endpoint paths, including `adpUrl` and `healthUrl`
- `DaemonConnectionConfigTest`: bootstrap bundled Tailscale profile, first-run copy to app-owned JSON, edited profile write/read, malformed existing file explicit failure, missing active profile explicit failure, relay enabled rejection
- aspect-ratio layout classifier tests: map viewport width/height pairs to mobile/foldable/tablet/desktop layout modes without mutating selected session or draft state
- `CommandIngressProtocolTest`: covers SubmitUserInput shape, CancelLatestActiveTurn shape, ADP command frame shape, ADP subscribe frame shape, query-as-command negative frame shape, old type-field negative, special characters, empty text
- Android device validation script static checks: explicit serial required, no broad process kill, records `adb devices`, foreground/window dumps, logcat, screenshot, and summary status

### Protocol Replay Harness

- feed canned ADP `UiAdpResponse::SubscriptionEvent` JSON fixtures into `TimelineProjector::applyAdp`
- feed canned ADP failure JSON fixtures into `TimelineProjector::applyAdp`
- feed canned SSE compatibility `UiSubscriptionEvent` JSON fixtures into `TimelineProjector::apply`
- assert `snapshotJson()` and `latestTurnProjectionJson()` preserve the canonical `turn` + `public_conversation` shape
- replay both success and failure paths so null-safe parsing and terminal-state mapping stay locked

## Module Black-Box Plan

- `cd apps/freehand-android && ./gradlew testDebugUnitTest` runs the JVM-level Android owner tests
- `DaemonConnectionConfig::parse` parses real bundled `assets/config/client.json` and produces the active Tailscale `HostConfig`
- `DaemonConnectionConfigStore` round-trips host/port/path through an app-owned JSON file
- file-backed daemon config round-trips an edited Tailscale profile through an app-owned JSON file and rebuilds `HostConfig.adpUrl`
- relay profile remains inert unless explicitly selected by the future relay flow
- `AdpEventStream::buildFrame` produces correct ADP command/subscribe/query misuse frame shapes (verified by protocol shape tests)
- `ProtocolClient::postCommand` remains a compatibility HTTP request body path
- `handle_android_mock` returns HTTP 200 with `mock-mobile` class (existing server test)
- `apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` exits with `passed`, `blocked`, or `failed` summary JSON and never treats offline/locked/not-foreground states as success

## Project Black-Box Impact

- Android app boundary proves protocol-only consumption: submit/query/subscribe via ADP frames, compatibility SSE via `UiSubscriptionEvent`, compatibility query via HTTP GET
- Android app boundary proves no direct import of `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`
- Android app boundary proves Gson JsonNull safety in `TimelineProjector` (JSON null values do not crash)
- Android app boundary proves `bridge.html` renders the same public conversation projection as the server-side design preview
- Android app boundary proves daemon connection setup is file-backed, Tailscale-first, explicit on failure, and does not silently fall back to LAN scan/localhost/relay
- Android/WebUI visual evidence must cover at least phone portrait, phone landscape, tablet portrait, tablet landscape, and foldable-like aspect ratios
- Android true-device closure requires `apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` to pass with Freehand foreground screenshot and no fatal logcat; blocker summaries are evidence of non-closure, not acceptance evidence

## Known Gaps

- no Espresso / instrumented tests yet (device-dependent; requires ADB-connected device)
- no passing integration smoke against live daemon from a real Android device yet (requires daemon running + connected/unlocked device)
- no Espresso / instrumented coverage for the drawer edit flow yet; JVM tests cover the file config owner and MainActivity is still device/framework-scoped
- aspect-ratio layout classifier and visual verification are not implemented yet
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
