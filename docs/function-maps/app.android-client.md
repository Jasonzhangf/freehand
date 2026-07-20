# Function Map: `app.android-client`

- feature_id: `app.android-client`
- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- product UI owner: daemon-hosted WebUI from `app.webui-smoke`
- Android scope: thin WebView host plus platform file-picker bridge plus APK update system-installer handoff
- reference design: `docs/design/multi-platform-ui-architecture.md`
- resource map: `docs/resource-maps/core.json`
- test design: `docs/testing/app.android-client.md`
- mainline call source: `docs/mainline-calls/app.android-client.json`
- generated wiki: `docs/wiki/app.android-client.md`

## Resource Map Binding

- owned resources: Android app bootstrap/config, platform WebView host, and `android_apk_update`
- touched resources: daemon WebUI URL, daemon APK update manifest URL, app-owned daemon endpoint bootstrap config, Android launcher assets
- resource operations: `android_apk_update.check_manifest` (`android_apk_update` -> `android_apk_update`), `android_apk_update.download_apk` (`android_apk_update` -> `android_apk_update`), `android_apk_update.request_install` (`android_apk_update` -> `android_apk_update`)
- forbidden shortcuts: Android must not own a second conversation UI, settings UI, native task/status projector, ADP command transport, SSE transport, local HTML bridge, Android mock page, native update panel, remote daemon account directory truth, route scoring, or relay protocol/tunnel semantics

## Request Mainline

- `MainActivity` loads app-owned daemon endpoint config.
- Config admits legacy active Tailscale profile identity plus host/port, or config-owned `remote_registry` bootstrap imported from a `freehand://daemon/import?payload=...` deep link.
- Removed top-level transport/relay/alternate-endpoint fields fail explicitly; relay is allowed only as a daemon endpoint inside `remote_registry` with an account relay URL.
- `MainActivity` immediately loads the canonical daemon WebUI URL from `HostConfig.webUiUrl`: `http://<host>:<port>/?client=android-webview`; daemon HTML and no-store asset URLs own WebUI versioning, not a hardcoded Android query parameter.
- If startup intent is `ACTION_VIEW` with a supported daemon import deep link, `MainActivity` imports the bootstrap bundle into app-owned config before WebView navigation.
- `AndroidApkUpdater::checkForUpdateAsync` checks `HostConfig.updateManifestUrl` in the background against the installed `BuildConfig.VERSION_CODE`; current or missing updates do not block canonical WebUI loading.
- `MainActivity` renders a neutral native startup overlay before WebView navigation and removes it only after `WebUiStartupGate` accepts the canonical Android WebUI shell plus stylesheet and module-JavaScript readiness probe.
- WebUI itself owns ADP query/subscribe/command, settings, lifecycle dashboard, transcript, composer, and error rendering.
- Android physical Back invokes the daemon WebUI-owned `window.__freehandHandleAndroidBack` hook first, so focused fields and WebUI dialogs/drawers close before the Activity exits or WebView history is navigated; Android must not implement native settings/session fallback behavior.
- Android exposes only `FreehandAndroidFilePicker` so WebUI attachment controls can invoke Android's system picker.

## Response Mainline

- Successful page load renders the same daemon WebUI shell as browser WebUI, with `client=android-webview` layout hints.
- Remote registry import persists the scanned account, daemon, active endpoint, endpoint candidates, and one-time credential in the app-owned config file for subsequent launches.
- A daemon manifest with positive higher `versionCode` and a relative/http(s) APK URL downloads its APK into app cache and opens Android's system package installer through the app FileProvider URI; install remains Android/user-confirmed rather than silent.
- Android logs `FreehandWebUiLayout` from canonical WebUI DOM selectors, applied stylesheet state, and WebUI module-JavaScript readiness for true-device validation.
- The startup overlay shows loading/error state until the canonical Android WebUI shell is verified; page-finished alone is not success.
- Android Back returns a WebUI-handled result when the canonical page closes a focused form, dialog, Header tree, Agent sheet, or mobile drawer; otherwise Android may navigate WebView history or exit.
- File picker selections are returned to WebUI through `window.__freehandAndroidAttachmentSelected`.

## Error Mainline

- Config parse/load errors are fatal Android startup errors.
- Unknown or removed config fields are errors; Android does not ignore or migrate them.
- APK update manifest parse errors, non-positive versions, non-http absolute APK URLs, HTTP failures, empty APK downloads, or installer handoff errors are explicit `FreehandApkUpdate` logcat failures and do not pretend the app upgraded.
- Unsupported bootstrap kind/schema, malformed base64/JSON, expired bootstrap, missing credential, unknown daemon endpoint, relay endpoint without account relay URL, or unsupported endpoint kind is an explicit Android startup/import error.
- Network/WebView load failures remain WebView failures. Android does not render native fallback UI.
- Missing WebUI layout probe is a failed/blocked device validation result, not acceptance evidence.
- Missing, malformed, wrong-client, stylesheet-not-applied, or WebUI-JavaScript-not-ready startup probe leaves the native startup state visible instead of pretending the app is ready.
- Missing or false WebUI back-hook handling falls through to normal Android exit/navigation; Android must not fabricate local drawer state.

## Function Call Table

| Step | Symbol | File | Responsibility | Input | Output | Caller | Callee | Status |
|---|---|---|---|---|---|---|---|---|
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | import supported bootstrap deep link when present, load config, render startup overlay, configure WebView, attach Android file picker bridge, and load daemon WebUI URL | activity intent | startup overlay plus WebView loading canonical WebUI | Android framework | `ClientConfig::store`, `DaemonConnectionConfigStore::importBootstrapLink`, `DaemonConnectionConfigStore::load`, `DaemonConnectionConfig::activeHostConfig`, `WebView::loadUrl` | bound |
| 02 | `com.freehand.android.data.ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android context to app-owned daemon config file and bundled first-run config | Android context | `DaemonConnectionConfigStore` | `MainActivity::onCreate` | app files dir + asset reader | bound |
| 03 | `com.freehand.android.data.DaemonConnectionConfigStore::load` / `importBootstrapLink` / `com.freehand.android.data.DaemonConnectionConfig::parse` / `parseBootstrapLink` / `activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read/bootstrap, validate legacy Tailscale or config-owned remote_registry schema, import daemon bootstrap links, and select the already-declared active daemon endpoint | app-owned JSON, bundled JSON, or `freehand://daemon/import?payload=...` | `HostConfig` or explicit config/import error | `MainActivity::onCreate` | JSON parser + bootstrap decoder + schema validator | bound |
| 04 | `com.freehand.android.data.HostConfig::baseUrl` / `com.freehand.android.data.HostConfig::webUiUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | build daemon origin and canonical Android WebUI URL | host + port | `http://<host>:<port>` plus `client=android-webview` URL | `MainActivity::onCreate` | pure URL builder | bound |
| 05 | `com.freehand.android.ui.MainActivity.AndroidWebChromeClient::onShowFileChooser` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI `<input type=file>` requests to Android system picker | WebUI file chooser request | selected URI array or explicit picker failure | WebView | Android activity result API | bound |
| 06 | `com.freehand.android.ui.MainActivity.AndroidFilePickerBridge::request` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI Android attachment buttons to Android system picker | `kind` string | picker launch | daemon WebUI JS bridge | `MainActivity::openAndroidAttachmentPicker` | bound |
| 07 | `com.freehand.android.ui.MainActivity::injectAndroidAttachmentSelection` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | return selected Android URIs to canonical WebUI attachment draft hook | Android result intent | `window.__freehandAndroidAttachmentSelected(kind, files)` | activity result callback | WebView JS | bound |
| 08 | `com.freehand.android.ui.MainActivity::reportCanonicalWebUiLayout` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | log canonical WebUI selector/layout/style/script evidence for device validation | loaded page DOM | `FreehandWebUiLayout` logcat row plus startup overlay readiness decision | `WebViewClient::onPageFinished` | WebView JS / `WebUiStartupGate` | bound |
| 08a | `com.freehand.android.ui.WebUiStartupGate::isCanonicalProbe` / `evaluate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/WebUiStartupGate.kt` | accept only canonical Android WebUI shell, stylesheet-applied, and module-JS-ready probe before native startup overlay removal | WebView DOM/CSS/JS readiness probe JSON | ready/not-ready decision plus status text | `MainActivity::reportCanonicalWebUiLayout` | pure JSON gate | bound |
| 08b | `com.freehand.android.ui.MainActivity::handleAndroidBackPressed` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | ask canonical WebUI to handle focused field/dialog/drawer back intent before native exit/navigation | Android physical Back | WebUI-handled no-op, WebView history navigation, or Activity finish | Android back dispatcher | `window.__freehandHandleAndroidBack`, `WebView::canGoBack`, `WebView::goBack`, `Activity::finish` | bound |
| 09 | `generate_launcher_icons` | `apps/freehand-android/scripts/generate-launcher-icons.sh` | derive Android launcher mipmaps from `assets/logo.png` | source logo | launcher PNGs | maintainer | ImageMagick | bound |
| 10 | `verify_launcher_icons` | `apps/freehand-android/scripts/verify-launcher-icons.sh` | verify launcher dimensions and source pixels | source logo + launcher PNGs | success or explicit drift failure | maintainer | ImageMagick | bound |
| 11 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | install/start APK and verify canonical WebUI is foreground with layout evidence | adb serial + APK | passed/blocked/failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap | bound |
| 12 | `com.freehand.android.ui.AndroidApkUpdater::checkForUpdateAsync` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | check selected daemon update manifest against installed version and reject non-positive versions or non-http absolute APK URLs | `HostConfig.updateManifestUrl` + `BuildConfig.VERSION_CODE` | no-op current log or valid higher-version update plan | `MainActivity::onCreate` | `ApkUpdateManifest::parse`, `ApkUpdateManifest::updatePlan` | bound |
| 13 | `com.freehand.android.ui.AndroidApkUpdater::downloadApk` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | download manifest-selected higher-version APK into app cache | resolved relative/http(s) APK URL + target versionCode | non-empty cached APK file | `AndroidApkUpdater::checkForUpdateAsync` | `HttpURLConnection` + app cache | bound |
| 14 | `com.freehand.android.ui.AndroidApkUpdater::buildInstallIntent` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | hand cached APK to Android system package installer | cached APK file | `ACTION_VIEW` package-installer intent with FileProvider URI | `AndroidApkUpdater::checkForUpdateAsync` | Android `FileProvider` / package installer | bound |

## Sync Status Against Code

- Android native conversation/settings/update fallback files are physically deleted.
- `bridge.html`, Android ADP/SSE/HTTP command transports, native projector, native drawer/composer/topbar/status components, and Android mock route/assets are removed.
- App-owned config no longer carries ADP/query/command/subscription paths or top-level relay routing.
- Android can import `remote_registry` bootstrap links and load a declared Tailscale/IPv4/IPv6/relay WebUI endpoint, but it does not own account directory truth, route scoring, live health probing, Tailscale OS connection, or relay tunnel semantics.
- Android APK update route remains daemon release distribution truth; Android now owns only manifest check, APK cache download, and system package-installer handoff, not a native update panel or silent install path.
