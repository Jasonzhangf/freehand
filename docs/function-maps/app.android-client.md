# Function Map: `app.android-client`

- feature_id: `app.android-client`
- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- product UI owner: daemon-hosted WebUI from `app.webui-smoke`
- Android scope: thin WebView host plus platform file-picker bridge only
- reference design: `docs/design/multi-platform-ui-architecture.md`
- test design: `docs/testing/app.android-client.md`
- mainline call source: `docs/mainline-calls/app.android-client.json`
- generated wiki: `docs/wiki/app.android-client.md`

## Resource Map Binding

- owned resources: Android app bootstrap/config and platform WebView host
- touched resources: daemon WebUI URL, Android launcher assets
- forbidden shortcuts: Android must not own a second conversation UI, settings UI, native task/status projector, ADP command transport, SSE transport, local HTML bridge, Android mock page, or in-app native APK updater

## Request Mainline

- `MainActivity` loads app-owned daemon endpoint config.
- Config admits only active Tailscale profile identity plus host and port; removed transport/relay/alternate-endpoint fields fail explicitly.
- `MainActivity` immediately loads the canonical daemon WebUI URL from `HostConfig.webUiUrl`: `http://<host>:<port>/?client=android-webview&v=<bootstrap-version>`, so an APK upgrade cannot reuse an older cached root shell.
- `MainActivity` renders a neutral native startup overlay before WebView navigation and removes it only after `WebUiStartupGate` accepts the canonical Android WebUI shell plus stylesheet and module-JavaScript readiness probe.
- WebUI itself owns ADP query/subscribe/command, settings, lifecycle dashboard, transcript, composer, and error rendering.
- Android exposes only `FreehandAndroidFilePicker` so WebUI attachment controls can invoke Android's system picker.

## Response Mainline

- Successful page load renders the same daemon WebUI shell as browser WebUI, with `client=android-webview` layout hints.
- Android logs `FreehandWebUiLayout` from canonical WebUI DOM selectors, applied stylesheet state, and WebUI module-JavaScript readiness for true-device validation.
- The startup overlay shows loading/error state until the canonical Android WebUI shell is verified; page-finished alone is not success.
- File picker selections are returned to WebUI through `window.__freehandAndroidAttachmentSelected`.

## Error Mainline

- Config parse/load errors are fatal Android startup errors.
- Unknown or removed config fields are errors; Android does not ignore or migrate them.
- Network/WebView load failures remain WebView failures. Android does not render native fallback UI.
- Missing WebUI layout probe is a failed/blocked device validation result, not acceptance evidence.
- Missing, malformed, wrong-client, stylesheet-not-applied, or WebUI-JavaScript-not-ready startup probe leaves the native startup state visible instead of pretending the app is ready.

## Function Call Table

| Step | Symbol | File | Responsibility | Input | Output | Caller | Callee | Status |
|---|---|---|---|---|---|---|---|---|
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | load config, render startup overlay, configure WebView, attach Android file picker bridge, and load daemon WebUI URL | activity intent | startup overlay plus WebView loading canonical WebUI | Android framework | `ClientConfig::store`, `DaemonConnectionConfigStore::load`, `DaemonConnectionConfig::activeHostConfig`, `WebView::loadUrl` | bound |
| 02 | `com.freehand.android.data.ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android context to app-owned daemon config file and bundled first-run config | Android context | `DaemonConnectionConfigStore` | `MainActivity::onCreate` | app files dir + asset reader | bound |
| 03 | `com.freehand.android.data.DaemonConnectionConfigStore::load` / `com.freehand.android.data.DaemonConnectionConfig::parse` / `com.freehand.android.data.DaemonConnectionConfig::activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read/bootstrap, validate, and select active daemon endpoint | app-owned JSON or bundled JSON | `HostConfig` or explicit config error | `MainActivity::onCreate` | JSON parser + schema validator | bound |
| 04 | `com.freehand.android.data.HostConfig::baseUrl` / `com.freehand.android.data.HostConfig::webUiUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | build daemon origin and version-addressed Android WebUI URL | host + port | `http://<host>:<port>` plus versioned `client=android-webview` URL | `MainActivity::onCreate` | pure URL builder | bound |
| 05 | `com.freehand.android.ui.MainActivity.AndroidWebChromeClient::onShowFileChooser` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI `<input type=file>` requests to Android system picker | WebUI file chooser request | selected URI array or explicit picker failure | WebView | Android activity result API | bound |
| 06 | `com.freehand.android.ui.MainActivity.AndroidFilePickerBridge::request` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI Android attachment buttons to Android system picker | `kind` string | picker launch | daemon WebUI JS bridge | `MainActivity::openAndroidAttachmentPicker` | bound |
| 07 | `com.freehand.android.ui.MainActivity::injectAndroidAttachmentSelection` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | return selected Android URIs to canonical WebUI attachment draft hook | Android result intent | `window.__freehandAndroidAttachmentSelected(kind, files)` | activity result callback | WebView JS | bound |
| 08 | `com.freehand.android.ui.MainActivity::reportCanonicalWebUiLayout` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | log canonical WebUI selector/layout/style/script evidence for device validation | loaded page DOM | `FreehandWebUiLayout` logcat row plus startup overlay readiness decision | `WebViewClient::onPageFinished` | WebView JS / `WebUiStartupGate` | bound |
| 08a | `com.freehand.android.ui.WebUiStartupGate::isCanonicalProbe` / `evaluate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/WebUiStartupGate.kt` | accept only canonical Android WebUI shell, stylesheet-applied, and module-JS-ready probe before native startup overlay removal | WebView DOM/CSS/JS readiness probe JSON | ready/not-ready decision plus status text | `MainActivity::reportCanonicalWebUiLayout` | pure JSON gate | bound |
| 09 | `generate_launcher_icons` | `apps/freehand-android/scripts/generate-launcher-icons.sh` | derive Android launcher mipmaps from `assets/logo.png` | source logo | launcher PNGs | maintainer | ImageMagick | bound |
| 10 | `verify_launcher_icons` | `apps/freehand-android/scripts/verify-launcher-icons.sh` | verify launcher dimensions and source pixels | source logo + launcher PNGs | success or explicit drift failure | maintainer | ImageMagick | bound |
| 11 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | install/start APK and verify canonical WebUI is foreground with layout evidence | adb serial + APK | passed/blocked/failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap | bound |

## Sync Status Against Code

- Android native conversation/settings/update fallback files are physically deleted.
- `bridge.html`, Android ADP/SSE/HTTP command transports, native projector, native drawer/composer/topbar/status components, and Android mock route/assets are removed.
- App-owned config no longer carries ADP/query/command/subscription paths or relay routing.
- Android APK update route still exists on the daemon for release distribution, but Android no longer renders or owns a native update UI.
