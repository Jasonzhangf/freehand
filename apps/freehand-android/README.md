# freehand-android (native Android client v1 scaffold)

> First buildable Android shell for Freehand. Native Activity hosts a WebView rendering layer + native input bar, and consumes `freehand-ui-protocol` through ADP WebSocket query / subscribe / command frames. HTTP query, POST command ingress, and SSE classes remain compatibility paths, not the default live shell.

## Owner and Truth

- feature_id: `app.android-client`
- owner: `apps/freehand-android`
- function map: `docs/function-maps/app.android-client.md`
- test design: `docs/testing/app.android-client.md`
- multi-platform design: `docs/design/multi-platform-ui-architecture.md`
- execution plan: `docs/design/android-client-v1-execution.md`

## Self-Contained Static Mock

The locked design preview lives in `apps/freehand-server/assets/mocks/android/mobile-mock.html`.
It is **self-contained** — CSS is inlined into the HTML — so it can be opened directly by double-clicking the file in a browser (file://) without any local web server.

It is also served by the `freehand-server` binary at:

- `http://127.0.0.1:<port>/mock/android`

To preview the same design through the runtime daemon port:

```bash
cargo run -p freehand-server -- webui-serve-smoke --bind 127.0.0.1:3501
# open http://127.0.0.1:3501/mock/android
```

To preview the same design by direct file open (no server needed):

```bash
open apps/freehand-server/assets/mocks/android/mobile-mock.html
```

`apps/freehand-server/assets/mocks/android/mobile-mock.css` is still kept in the repo as a style-token truth artifact, but the inlined HTML does not depend on it for design-review previews.

## Layout

- `app/src/main/java/com/freehand/android/ui/MainActivity.kt` — Activity entrypoint, WebView host, insets, key dispatch (Back closes drawer, Escape cancels latest turn).
- `app/src/main/java/com/freehand/android/ui/components/TopBarController.kt` — top bar: current agent name + status pill.
- `app/src/main/java/com/freehand/android/ui/components/SlaveStripController.kt` — collapsed horizontal strip of slave agent pills (hidden when none).
- `app/src/main/java/com/freehand/android/ui/components/StatusBannerController.kt` — transient + persistent status banner.
- `app/src/main/java/com/freehand/android/ui/components/InputBarController.kt` — bottom native input bar; submits via the protocol-owned command ingress.
- `app/src/main/java/com/freehand/android/ui/components/DrawerController.kt` — right-slide drawer for active daemon profile endpoint edits.
- `app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` — app-owned JSON daemon connection config parser/store; bootstraps bundled defaults and validates Tailscale-only profile truth.
- `app/src/main/java/com/freehand/android/data/ClientConfig.kt` — Android `Context` adapter for the app-owned daemon connection config file.
- `app/src/main/java/com/freehand/android/data/HostConfig.kt` — selected profile endpoint URL construction.
- `app/src/main/java/com/freehand/android/data/AdpEventStream.kt` — default ADP WebSocket query / subscribe / command transport.
- `app/src/main/java/com/freehand/android/data/ProtocolClient.kt` — HTTP query + command POST against `freehand-ui-protocol`.
- `app/src/main/java/com/freehand/android/data/CommandIngress.kt` — submit / cancel entry points with explicit success/failure callback.
- `app/src/main/java/com/freehand/android/data/TimelineProjector.kt` — single source of UI-local state; only consumes `ui.protocol` events.
- `app/src/main/assets/bridge.html` — WebView rendering layer; receives projection snapshots via `window.__freehand.applySnapshot(json)`.

## Reference

- `docs/design/multi-platform-ui-architecture.md` — locked multi-platform design rules.
- `docs/design/android-client-v1-execution.md` — Android v1 execution plan.
- `docs/function-maps/app.android-client.md` — owner function map.
- `docs/testing/app.android-client.md` — owner test design.
- `docs/function-maps/ui.protocol.md` — UI protocol truth consumed by the WebView shell.

## Build (when SDK is configured)

```bash
cd apps/freehand-android
./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

The bundled default profile is `tailscale-main` at `100.66.1.82:4041`. On first startup the app copies `assets/config/client.json` to its app-owned `daemon-connection.json`; drawer edits update that JSON file and persist across restarts. Relay fields are schema-reserved but disabled.

## Hard Constraints (must not be violated)

- the Android client is a `ui.protocol` consumer + command ingress source only; it does not own session, reason, debug, metadata, or provider truth
- the Android client does not import `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime` directly
- the Android client does not implement a second dispatch port, a second completion-schema validator, or a second projection layer
