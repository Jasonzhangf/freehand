# Wiki: `app.android-client`

Generated from `docs/mainline-calls/app.android-client.json`. Do not edit by hand.

- owner crate: `apps/freehand-android`
- owner module: `apps/freehand-android/app/src/main/java/com/freehand/android/`
- function map: `docs/function-maps/app.android-client.md`
- generated wiki: `docs/wiki/app.android-client.md`
- test design: `docs/testing/app.android-client.md`

## Request Mainline

- MainActivity::onCreate loads app-owned daemon endpoint config
- DaemonConnectionConfig accepts only active Tailscale profile identity plus host and port; removed transport, relay, and alternate endpoint fields are explicit errors
- MainActivity::onCreate immediately loads the canonical daemon WebUI URL with client=android-webview
- WebUI owns ADP query/subscribe/command, settings, lifecycle dashboard, transcript, composer, and error rendering
- Android only exposes the system file-picker bridge for WebUI attachment controls

## Response Mainline

- daemon root returns the canonical WebUI shell and mobile layout attributes
- WebViewClient::onPageFinished logs canonical WebUI selector/layout evidence
- Android file-picker result returns URI metadata to window.__freehandAndroidAttachmentSelected

## Error Mainline

- config parse/load failure is an explicit Android startup failure
- unknown config fields, including removed ADP/SSE/command path or relay fields, are rejected rather than ignored or migrated
- WebView network failure remains a WebView/network failure; Android does not render local fallback UI
- missing FreehandWebUiLayout evidence is a device-validation failure

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
| 01 | `com.freehand.android.ui.MainActivity::onCreate` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | load config, configure WebView, attach file picker bridge, and load canonical daemon WebUI | activity intent | WebView loading http://<host>:<port>/?client=android-webview | Android framework | ClientConfig::store, DaemonConnectionConfigStore::load, DaemonConnectionConfig::activeHostConfig, WebView::loadUrl |  |  |  | bound |
| 02 | `com.freehand.android.data.ClientConfig::store` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ClientConfig.kt` | adapt Android context to app-owned config and bundled first-run JSON | Android Context | DaemonConnectionConfigStore | MainActivity::onCreate | app files dir plus asset reader |  |  |  | bound |
| 03 | `com.freehand.android.data.DaemonConnectionConfigStore::load / com.freehand.android.data.DaemonConnectionConfig::parse / com.freehand.android.data.DaemonConnectionConfig::activeHostConfig` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/DaemonConnectionConfig.kt` | read/bootstrap, strictly validate host/port-only profile schema, and select the active daemon endpoint | app-owned or bundled daemon JSON | HostConfig or explicit config error | MainActivity::onCreate | Gson JSON parser and schema validator |  |  |  | bound |
| 04 | `com.freehand.android.data.HostConfig::baseUrl` | `apps/freehand-android/app/src/main/java/com/freehand/android/data/HostConfig.kt` | build daemon WebUI origin | host and port | http://<host>:<port> | MainActivity::onCreate | pure URL builder |  |  |  | bound |
| 05 | `com.freehand.android.ui.MainActivity.AndroidWebChromeClient::onShowFileChooser` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI input type=file to Android system picker | WebUI file chooser request | selected URI array or explicit picker failure | WebView | Android activity result API |  |  |  | bound |
| 06 | `com.freehand.android.ui.MainActivity.AndroidFilePickerBridge::request` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | route WebUI attachment buttons to Android system picker | attachment kind | picker launch | daemon WebUI JavaScript | MainActivity::openAndroidAttachmentPicker |  |  |  | bound |
| 07 | `com.freehand.android.ui.MainActivity::injectAndroidAttachmentSelection` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | return selected Android URI metadata to the canonical WebUI attachment hook | Android picker result intent | window.__freehandAndroidAttachmentSelected(kind, files) | activity result callback | WebView JavaScript |  |  |  | bound |
| 08 | `com.freehand.android.ui.MainActivity::reportCanonicalWebUiLayout` | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` | log canonical WebUI selector/layout evidence | loaded page DOM | FreehandWebUiLayout logcat row | WebViewClient::onPageFinished | WebView JavaScript |  |  |  | bound |
| 09 | `generate_launcher_icons` | `apps/freehand-android/scripts/generate-launcher-icons.sh` | derive launcher PNGs from assets/logo.png | source logo | density-specific launcher PNGs | Android asset maintainer | ImageMagick resize |  |  |  | bound |
| 10 | `verify_launcher_icons` | `apps/freehand-android/scripts/verify-launcher-icons.sh` | verify launcher dimensions and exact source-derived pixels | source logo plus launcher PNGs | success or explicit drift failure | Android asset maintainer | ImageMagick identify/compare |  |  |  | bound |
| 11 | `verify_device_ui` | `apps/freehand-android/scripts/verify-device-ui.sh` | verify APK foreground state and canonical WebUI layout evidence on an explicit device | ADB serial and APK | passed, blocked, or failed artifact directory | operator | adb install/start/dumpsys/logcat/screencap |  |  |  | bound |

## Sync Status Against Mainline Call

- Android native conversation/settings/update fallback files are physically deleted.
- bridge.html, Android ADP/SSE/HTTP UI transports, native projector, native controllers, and Android mock route/assets are removed.
- Android app-owned config contains only active Tailscale profile identity plus host and port; removed transport and relay fields fail explicitly.
- Android APK update routes remain daemon-owned release distribution endpoints; Android does not render a native update panel.
