# Test Design: `app.android-client`

- feature_id: `app.android-client`
- owner: `apps/freehand-android`
- reference function map: `docs/function-maps/app.android-client.md`

## Lifecycle Path Under Test

1. `ClientConfig::store` adapts Android `Context` to the app-owned config file and bundled first-run JSON reader.
2. `DaemonConnectionConfigStore::load` bootstraps or reads app-owned daemon config.
3. `DaemonConnectionConfig::parse` validates the single supported Tailscale profile schema and rejects unknown fields; Android config owns only profile id, mode, host, and port.
4. `DaemonConnectionConfig::activeHostConfig` selects the single active daemon endpoint.
5. `MainActivity::onCreate` renders a native neutral startup overlay before WebView navigation and loads version-addressed `HostConfig.webUiUrl` immediately.
6. Canonical daemon WebUI owns transcript, composer, settings, status, lifecycle dashboard, ADP, command dispatch, and WebUI errors.
7. Android `WebChromeClient::onShowFileChooser` and `FreehandAndroidFilePicker.request` open Android system picker for WebUI attachment controls.
8. `MainActivity::injectAndroidAttachmentSelection` returns selected Android URI metadata to WebUI.
9. `MainActivity::reportCanonicalWebUiLayout` logs canonical WebUI DOM, stylesheet, and module-JavaScript evidence and removes the startup overlay only after the canonical Android WebUI readiness probe succeeds.
10. `generate-launcher-icons.sh` and `verify-launcher-icons.sh` keep launcher assets source-derived from `assets/logo.png`.
11. `verify-device-ui.sh <adb-serial>` installs/starts APK and accepts only canonical WebUI layout evidence.

## Negative Contract

- Android must not load `bridge.html` or any local HTML fallback.
- Android must not render a native conversation timeline, native settings drawer, native update panel, native composer, native agent strip, or native status banner.
- Android must not own ADP/SSE/HTTP command/query/subscription transports; daemon WebUI owns them.
- Android config must not carry ADP/query/command/subscription paths, relay routing, or alternate UI endpoints.
- Android mock `/mock/android` must remain removed.
- Network/config failure remains a failure; there is no local replacement UI.
- Page-finished alone must not hide startup state; a missing, malformed, wrong-client, stylesheet-not-applied, or WebUI-JavaScript-not-ready shell probe remains visible as explicit loading/error state.

## White-Box Plan

- Source scan rejects old fallback symbols/files: `bridge.html`, `showNativeShell`, `DrawerController`, `InputBarController`, `TopBarController`, `StatusBannerController`, `SlaveStripController`, `TimelineProjector`, `CommandIngress`, `AdpEventStream`, `SseEventStream`, `ProtocolClient`, `ApkUpdate`, `/mock/android`, and `mobile-mock`.
- `HostConfigTest` covers daemon origin plus version-addressed Android WebUI URL construction.
- `WebUiStartupGateTest` positively accepts only `webuiShell=true` plus `layoutClient=android-webview` plus `webuiCssApplied=true` plus `webuiJsReady=true`, and negatively rejects false, malformed, null, wrong-client, missing-stylesheet, and missing-JavaScript probes.
- `DaemonConnectionConfigTest` covers bundled config bootstrap, strict schema validation, app-owned persistence, and rejection of removed transport/relay fields.
- `verify-launcher-icons.sh` covers launcher dimensions and source-derived pixels.

## Module Black-Box Plan

- `cd apps/freehand-android && ./gradlew testDebugUnitTest assembleDebug`
- APK inspection must show `com.freehand.android.ui.MainActivity` is packaged and `assets/bridge.html` is absent.
- Server test must prove `/mock/android` and `/assets/mocks/android/mobile-mock.css` return 404 while `/?client=android-webview` returns the canonical WebUI shell.

## Project Black-Box Impact

- True-device closure requires a connected/unlocked explicit ADB serial.
- Acceptance evidence must include a screenshot and logcat showing daemon WebUI selectors/layout/assets (`data-webui-shell=true`, `layoutClient=android-webview`, stylesheet applied, WebUI JavaScript ready), not native Android chrome or unstyled HTML.
- A blocker summary from `verify-device-ui.sh` is evidence of non-closure, not success.

## Known Gaps

- No Espresso/instrumented tests yet.
- Attachment picker still needs true-device proof after the WebUI-only shell change.
