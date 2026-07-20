# Wiki: `app.android-client`

Generated from `docs/mainline-calls/app.android-client.json`. Do not edit by hand.

- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- function map: `docs/function-maps/app.android-client.md`
- generated wiki: `docs/wiki/app.android-client.md`
- test design: `docs/testing/app.android-client.md`

## Resource Operation Backlinks

- android_apk_update.check_manifest
- android_apk_update.download_apk
- android_apk_update.request_install

## Request Mainline

- MainActivity::onCreate loads app-owned daemon endpoint config
- DaemonConnectionConfig accepts legacy active Tailscale profile identity plus host and port, or config-owned remote_registry bootstrap imported from a freehand://daemon/import deep link
- removed top-level transport, relay, and alternate endpoint fields are explicit errors; relay is valid only as a remote_registry daemon endpoint with account relayUrl
- AndroidApkUpdater checks the selected daemon endpoint update manifest in the background and compares it with the installed package versionCode
- MainActivity::onCreate renders a native neutral startup overlay before WebView navigation and immediately loads the canonical daemon WebUI URL with client=android-webview; daemon HTML and no-store assets own WebUI versioning
- WebUI owns ADP query/subscribe/command, settings, lifecycle dashboard, transcript, composer, and error rendering
- Android physical Back asks the canonical WebUI hook to close focused fields, dialogs, sheets, or drawers before Activity exit or WebView history navigation
- Android only exposes the system file-picker bridge for WebUI attachment controls

## Response Mainline

- daemon root returns the canonical WebUI shell and mobile layout attributes
- remote_registry import persists scanned account, daemon, active endpoint, endpoint candidates, and one-time credential in app-owned config before WebView navigation
- a positive higher-version daemon APK manifest with a relative or http(s) APK URL downloads the APK into app cache and opens Android's system package installer with a FileProvider URI
- WebViewClient::onPageFinished logs canonical WebUI selector/layout/asset readiness evidence and removes the startup overlay only after WebUiStartupGate accepts the Android WebUI shell, applied stylesheet, and module-JavaScript probe
- MainActivity::handleAndroidBackPressed treats a true result from window.__freehandHandleAndroidBack as handled UI navigation and otherwise falls through to WebView history or Activity finish
- Android file-picker result returns URI metadata to window.__freehandAndroidAttachmentSelected

## Error Mainline

- config parse/load failure is an explicit Android startup failure
- unknown config fields, including removed ADP/SSE/command path or top-level relay fields, are rejected rather than ignored or migrated
- APK update manifest parse errors, non-positive versions, non-http absolute APK URLs, HTTP download failures, empty files, or installer handoff failures are logged as FreehandApkUpdate errors and do not become false upgrade success
- unsupported bootstrap kind/schema, malformed base64/JSON, expired bootstrap, missing credential, unknown daemon endpoint, relay endpoint without account relayUrl, or unsupported endpoint kind is an explicit startup/import error
- WebView network failure remains a WebView/network failure; Android does not render local fallback UI
- missing FreehandWebUiLayout evidence is a device-validation failure
- missing or false WebUI back-hook handling falls through to normal Android navigation or exit instead of fabricating native drawer truth
- missing, malformed, wrong-client, stylesheet-not-applied, or WebUI-JavaScript-not-ready startup probes leave the startup state visible instead of pretending the app is ready

## Shared Multi-Reference Functions

- `reportCanonicalWebUiLayout`
  - owner: `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt`
  - purpose: provide true-device evidence that the canonical WebUI rendered
  - allowed callers: WebViewClient::onPageFinished
  - related tests: apps/freehand-android/scripts/verify-device-ui.sh
  - why shared: device validation and runtime page-load evidence must use the same canonical WebUI selector probe

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | import supported bootstrap deep link when present, load config, render startup overlay, configure WebView, attach file picker bridge, and load the canonical daemon WebUI | activity intent | startup overlay plus WebView loading http://<host>:<port>/?client=android-webview | Android framework | ClientConfig::store, DaemonConnectionConfigStore::importBootstrapLink, DaemonConnectionConfigStore::load, DaemonConnectionConfig::activeHostConfig, WebView::loadUrl |  |  |  | bound |
| 02 | `com.freehand.android.data.ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android context to app-owned config and bundled first-run JSON | Android Context | DaemonConnectionConfigStore | MainActivity::onCreate | app files dir plus asset reader |  |  |  | bound |
| 03 | `com.freehand.android.data.DaemonConnectionConfigStore::load / com.freehand.android.data.DaemonConnectionConfigStore::importBootstrapLink / com.freehand.android.data.DaemonConnectionConfig::parse / com.freehand.android.data.DaemonConnectionConfig::parseBootstrapLink / com.freehand.android.data.DaemonConnectionConfig::activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read/bootstrap, strictly validate legacy Tailscale or config-owned remote_registry schema, import daemon bootstrap links, and select the already-declared active daemon endpoint | app-owned JSON, bundled daemon JSON, or freehand://daemon/import bootstrap link | HostConfig or explicit config error | MainActivity::onCreate | Gson JSON parser, bootstrap decoder, and schema validator |  |  |  | bound |
| 04 | `com.freehand.android.data.HostConfig::baseUrl / com.freehand.android.data.HostConfig::webUiUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | build daemon origin and canonical Android WebUI URL | host and port | http://<host>:<port> plus client=android-webview URL | MainActivity::onCreate | pure URL builder |  |  |  | bound |
| 05 | `com.freehand.android.ui.MainActivity.AndroidWebChromeClient::onShowFileChooser` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI input type=file to Android system picker | WebUI file chooser request | selected URI array or explicit picker failure | WebView | Android activity result API |  |  |  | bound |
| 06 | `com.freehand.android.ui.MainActivity.AndroidFilePickerBridge::request` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI attachment buttons to Android system picker | attachment kind | picker launch | daemon WebUI JavaScript | MainActivity::openAndroidAttachmentPicker |  |  |  | bound |
| 07 | `com.freehand.android.ui.MainActivity::injectAndroidAttachmentSelection` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | return selected Android URI metadata to the canonical WebUI attachment hook | Android picker result intent | window.__freehandAndroidAttachmentSelected(kind, files) | activity result callback | WebView JavaScript |  |  |  | bound |
| 08 | `com.freehand.android.ui.MainActivity::reportCanonicalWebUiLayout` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | log canonical WebUI selector/layout/style/script evidence and hide startup overlay only after canonical shell plus asset readiness | loaded page DOM | FreehandWebUiLayout logcat row plus startup overlay readiness decision | WebViewClient::onPageFinished | WebView JavaScript / WebUiStartupGate |  |  |  | bound |
| 08a | `com.freehand.android.ui.WebUiStartupGate::isCanonicalProbe / com.freehand.android.ui.WebUiStartupGate::evaluate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/WebUiStartupGate.kt` | accept only canonical Android WebUI shell, applied stylesheet, and module-JS-ready probe before native startup overlay removal | WebView DOM/CSS/JS readiness probe JSON | ready/not-ready decision plus status text | MainActivity::reportCanonicalWebUiLayout | pure JSON gate |  |  |  | bound |
| 08b | `com.freehand.android.ui.MainActivity::handleAndroidBackPressed` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | ask the canonical WebUI to handle focused field, dialog, Header tree, Agent sheet, or mobile drawer back intent before native exit or history navigation | Android physical Back | WebUI-handled no-op, WebView history navigation, or Activity finish | Android back dispatcher | window.__freehandHandleAndroidBack, WebView::canGoBack, WebView::goBack, Activity::finish |  |  |  | bound |
| 09 | `generate_launcher_icons` | `apps/freehand-android/scripts/generate-launcher-icons.sh` | derive launcher PNGs from assets/logo.png | source logo | density-specific launcher PNGs | Android asset maintainer | ImageMagick resize |  |  |  | bound |
| 10 | `verify_launcher_icons` | `apps/freehand-android/scripts/verify-launcher-icons.sh` | verify launcher dimensions and exact source-derived pixels | source logo plus launcher PNGs | success or explicit drift failure | Android asset maintainer | ImageMagick identify/compare |  |  |  | bound |
| 11 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | verify APK foreground state and canonical WebUI layout evidence on an explicit device | ADB serial and APK | passed, blocked, or failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap |  |  |  | bound |
| 12 | `com.freehand.android.ui.AndroidApkUpdater::checkForUpdateAsync` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | check selected daemon update manifest against installed version without blocking canonical WebUI startup, rejecting non-positive versions and non-http absolute APK URLs | HostConfig.updateManifestUrl plus BuildConfig.VERSION_CODE | current-version no-op log or valid higher-version update plan | MainActivity::onCreate | ApkUpdateManifest::parse, ApkUpdateManifest::updatePlan | android_apk_update | android_apk_update | android_apk_update.check_manifest | bound |
| 13 | `com.freehand.android.ui.AndroidApkUpdater::downloadApk` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | download the manifest-selected higher-version APK into app cache | resolved relative/http(s) APK URL and target versionCode | non-empty cached APK file | AndroidApkUpdater::checkForUpdateAsync | HttpURLConnection plus app cache | android_apk_update | android_apk_update | android_apk_update.download_apk | bound |
| 14 | `com.freehand.android.ui.AndroidApkUpdater::buildInstallIntent` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/AndroidApkUpdater.kt` | handoff the cached APK to Android's system package installer | cached APK file | ACTION_VIEW package installer intent with FileProvider URI | AndroidApkUpdater::checkForUpdateAsync | Android FileProvider and package installer | android_apk_update | android_apk_update | android_apk_update.request_install | bound |

## Sync Status Against Mainline Call

- Android native conversation/settings/update fallback files are physically deleted.
- bridge.html, Android ADP/SSE/HTTP UI transports, native projector, native controllers, and Android mock route/assets are removed.
- Android app-owned config contains legacy active Tailscale host/port or config-owned remote_registry bootstrap truth; removed transport and top-level relay fields fail explicitly.
- Android imports remote_registry bootstrap links and loads declared Tailscale/IPv4/IPv6/relay WebUI endpoints, but does not own account directory truth, route scoring, live health probing, Tailscale OS connection, or relay tunnel semantics.
- Android APK update routes remain daemon-owned release distribution endpoints; Android owns only manifest check, cache download, and system package-installer handoff, not a native update panel or silent package replacement.
