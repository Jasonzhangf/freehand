# Test Design: `app.android-client`

- feature_id: `app.android-client`
- owner: `apps/freehand-android`
- reference function map: `docs/function-maps/app.android-client.md`
- resource map: `docs/resource-maps/core.json`

## Lifecycle Path Under Test

1. `ClientConfig::store` adapts Android `Context` to the app-owned config file and bundled first-run JSON reader.
2. `DaemonConnectionConfigStore::load` bootstraps or reads app-owned daemon config, preferring the remote_registry sidecar when present and keeping the legacy compatibility projection in sync for older APKs.
3. `DaemonConnectionConfig::parse` validates legacy Tailscale profile schema or config-owned `remote_registry` bootstrap schema and rejects unknown fields.
4. `DaemonConnectionConfig::parseBootstrapLink` accepts only versioned `freehand://daemon/import?payload=...` daemon bootstrap payloads, checks expiry, and writes imported config through `DaemonConnectionConfigStore::importBootstrapLink`, which now writes both the remote_registry sidecar and the legacy compatibility config.
5. `DaemonConnectionConfig::activeHostConfig` selects the already-declared active daemon endpoint; Android does not score routes.
6. `MainActivity::onCreate` renders a native neutral startup overlay before WebView navigation and loads canonical `HostConfig.webUiUrl` immediately; daemon HTML and no-store asset URLs own WebUI versioning.
7. Canonical daemon WebUI owns transcript, composer, settings, status, lifecycle dashboard, ADP, command dispatch, and WebUI errors.
8. `MainActivity::requestInstallFileAccessIfNeeded` runs once per package install/update marker during startup, requests supported runtime file/media permissions, opens Android 11+ all-files-access settings when needed, and logs `FreehandFileAccess` status instead of waiting for later file actions.
9. Android `WebChromeClient::onShowFileChooser` and `FreehandAndroidFilePicker.request` open Android system picker for WebUI attachment controls.
10. `MainActivity::injectAndroidAttachmentSelection` returns selected Android URI metadata to WebUI.
11. `MainActivity::reportCanonicalWebUiLayout` logs canonical WebUI DOM, stylesheet, and module-JavaScript evidence, retries during the bounded ES-module bootstrap window, and removes the startup overlay only after the canonical Android WebUI readiness probe succeeds; the true-device verifier waits for the positive readiness probe instead of stopping on the first non-ready sample, while redacted console level/line and HTTP asset failures remain separate diagnostics.
12. `MainActivity::handleAndroidBackPressed` routes physical Back into canonical WebUI `window.__freehandHandleAndroidBack` first; only an unhandled result may navigate WebView history or finish the Activity.
13. `generate-launcher-icons.sh` and `verify-launcher-icons.sh` keep launcher assets source-derived from `assets/logo.png`.
14. `verify-device-ui.sh <adb-serial>` installs/starts APK and accepts only canonical WebUI layout evidence; local `apkanalyzer` failures are blocker evidence (`apkanalyzer_failed`), not proof that the APK is missing the launcher activity.
15. `AndroidApkUpdater::checkForUpdateAsync` checks the selected daemon endpoint's APK update manifest, requires SHA-256, byte size, and signer certificate identity before admitting a positive higher `versionCode` with a relative/http(s) APK URL, permits current/older legacy manifests without integrity fields because they create no update plan, emits stable status phases, verifies downloaded APK size/hash/package/signer in app cache, and hands only the verified APK to Android's system installer through FileProvider.
16. `MainActivity.AndroidApkUpdateBridge::check` lets daemon WebUI Settings trigger the same updater path, while `MainActivity::recordAndroidApkUpdateStatus` replays startup or manual status to `window.__freehandAndroidApkUpdateStatus`.
17. `MainActivity::requestInstallNotificationPermissionIfNeeded` requests Android 13+ notification permission at startup, and `FreehandAndroidNotifications.turnFinished` posts one tappable notification for each distinct terminal turn.

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `android_apk_update.check_manifest` | bound | `./gradlew testDebugUnitTest` covers manifest parsing, unknown-field rejection, positive version validation, non-http absolute URL rejection, current-version legacy-manifest no-op, required higher-version SHA-256/size/signer metadata, higher-version plan creation, and stable WebUI bridge status phases | `./gradlew assembleDebug` compiles the startup/manual caller and updater wiring into the APK, including no-cache manifest requests | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` remains the device entrypoint; update proof uses daemon-served compiled `dist/android/update.json`, a staged higher-version APK, signer/hash/size evidence, Settings click evidence, and manual Android installer confirmation |
| `android_apk_update.download_apk` | bound | `./gradlew testDebugUnitTest` covers daemon/relay APK URL resolution from manifest truth and rejects non-http absolute APK URLs before download | `./gradlew assembleDebug` compiles the cache download owner and FileProvider authority | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` is the device smoke; full staged old-to-new APK replacement proof uses a release artifact newer than the installed build |
| `android_apk_update.request_install` | bound | `./gradlew testDebugUnitTest` covers higher-version plan admission before installer handoff | `./gradlew assembleRelease` plus `apksigner verify --verbose --print-certs` verifies `REQUEST_INSTALL_PACKAGES`, FileProvider metadata, updater code, and a signed APK package together | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` is the device smoke; Android system package installer confirmation is user-controlled and cannot be silently completed by the app |
| `android_file_access.request_startup_permissions` | bound | `./gradlew testDebugUnitTest` covers SDK-specific runtime file/media permission lists and once-per-install-marker prompt admission, including same-version reinstall markers | `./gradlew assembleDebug` compiles `READ_EXTERNAL_STORAGE`, `READ_MEDIA_*`, and `MANAGE_EXTERNAL_STORAGE` manifest wiring plus the ActivityResult permission launcher | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` must include `FreehandFileAccess` logcat rows showing granted, requested, or restricted startup status |
| `android_file_access.open_all_files_settings` | bound | `./gradlew testDebugUnitTest` covers Android 11+ all-files settings eligibility and pre-Android 11 exclusion | `./gradlew assembleDebug` compiles the package-scoped `ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION` handoff | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` must observe Android settings handoff when all-files access is not already granted; a normal app cannot silently grant it |
| `android_file_access.project_status` | bound | `./gradlew testDebugUnitTest` covers stable policy decisions used by `FreehandFileAccess` status logging | `./gradlew assembleDebug` compiles startup status logging | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` reads `FreehandFileAccess` logcat rows; denial remains restricted evidence, not success |
| `android_notification.request_startup_permission` | bound | `./gradlew testDebugUnitTest` covers SDK/permission prompt policy | `./gradlew assembleDebug` compiles `POST_NOTIFICATIONS` manifest and ActivityResult launcher wiring | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` captures startup permission requested, granted, denied, or not-required truth in `notification-logcat.txt` |
| `android_notification.post_turn_finished` | bound | `./gradlew testDebugUnitTest` covers notification prompt policy and duplicate turn identity behavior | `./gradlew assembleDebug` compiles the WebUI JavaScript bridge, notification channel, tap-return intent, and notification post path | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` captures `FreehandNotification` post truth and Android notification-manager truth after a distinct terminal WebUI turn |
| `android_notification.project_status` | bound | `./gradlew testDebugUnitTest` locks policy inputs used by notification status projection | `./gradlew assembleDebug` compiles stable `FreehandNotification` status logging | `bash apps/freehand-android/scripts/verify-device-ui.sh <adb-serial>` writes notification permission/post/dedupe/failure status to `notification-logcat.txt` instead of allowing a silent bridge failure |

## Negative Contract

- Android must not load `bridge.html` or any local HTML fallback.
- Android must not render a native conversation timeline, native settings drawer, native update panel, native composer, native agent strip, or native status banner.
- Android must not own ADP/SSE/HTTP command/query/subscription transports; daemon WebUI owns them.
- Android config must not carry ADP/query/command/subscription paths, top-level relay routing, or alternate UI endpoints.
- Android may accept `kind=relay` only inside a `remote_registry` daemon endpoint with account `relayUrl`; it must not own account directory truth, route scoring, live health probing, Tailscale OS connection, or relay tunnel protocol.
- Android mock `/mock/android` must remain removed.
- Network/config failure remains a failure; there is no local replacement UI, and WebView console or HTTP asset failures are logged rather than converted into readiness.
- Unsupported bootstrap kind/schema, malformed base64/JSON, expired bootstrap, missing one-time credential, relay endpoint without account relay URL, or unknown active endpoint remains an explicit startup/import failure.
- Page-finished alone must not hide startup state; a missing, malformed, wrong-client, stylesheet-not-applied, or WebUI-JavaScript-not-ready shell probe remains visible while the bounded retry is running and after retry exhaustion.

## White-Box Plan

- Source scan rejects old fallback symbols/files: `bridge.html`, `showNativeShell`, `DrawerController`, `InputBarController`, `TopBarController`, `StatusBannerController`, `SlaveStripController`, `TimelineProjector`, `CommandIngress`, `AdpEventStream`, `SseEventStream`, `ProtocolClient`, `/mock/android`, and `mobile-mock`; the only allowed update code is the contract-owned `android_apk_update` system-installer handoff.
- `HostConfigTest` covers daemon origin plus canonical Android WebUI URL construction without an APK-hardcoded asset version query.
- `ApkUpdateManifestTest` covers APK update manifest strict parsing, positive version validation, non-http absolute APK URL rejection, version comparison, current-version legacy manifests without integrity metadata, higher-version SHA-256/size/signer requirements, invalid signer rejection, direct endpoint URL resolution, and relay namespace URL resolution.
- `ApkUpdateStatusTest` covers the stable status phase vocabulary consumed by the WebUI Settings bridge.
- `FileAccessPermissionPolicyTest` covers runtime permission selection, all-files settings eligibility, and once-per-install-marker prompt admission.
- `NotificationPermissionPolicyTest` covers Android 13+ prompt eligibility and pre-Android 13 no-prompt behavior.
- `WebUiStartupGateTest` positively accepts only `webuiShell=true` plus `layoutClient=android-webview` plus `webuiCssApplied=true` plus `webuiJsReady=true`, and negatively rejects false, malformed, null, wrong-client, missing-stylesheet, and missing-JavaScript probes; true-device verification proves the Activity retries past the initial page-finished race until JavaScript readiness is observed.
- `DaemonConnectionConfigTest` covers bundled config bootstrap, strict schema validation, app-owned persistence, rejection of removed transport/top-level relay fields, remote registry Tailscale/relay endpoint selection, bootstrap deep-link import, expiry rejection, relay endpoint account binding, and sidecar/legacy compatibility projection.
- Server asset smoke locks that WebUI exposes `window.__freehandHandleAndroidBack`; Android compile verifies `MainActivity::handleAndroidBackPressed` calls that hook instead of owning native drawer/session/settings state.
- Server asset smoke locks that WebUI exposes the Android-only APK update Settings card and `window.__freehandAndroidApkUpdateStatus`, while Android compile verifies `MainActivity.AndroidApkUpdateBridge::check` is packaged.
- `verify-launcher-icons.sh` covers launcher dimensions and source-derived pixels.

## Module Black-Box Plan

- `cd apps/freehand-android && JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug`
- APK inspection must show `com.freehand.android.ui.MainActivity` is packaged and `assets/bridge.html` is absent.
- APK inspection must show `REQUEST_INSTALL_PACKAGES`, `POST_NOTIFICATIONS`, file/media storage permissions, `MANAGE_EXTERNAL_STORAGE`, and `${applicationId}.apkupdate.fileprovider` are packaged for the system installer handoff and startup permission prompts.
- Server test must prove `/mock/android` and `/assets/mocks/android/mobile-mock.css` return 404 while `/?client=android-webview` returns the canonical WebUI shell.

## Project Black-Box Impact

- True-device closure requires a connected/unlocked explicit ADB serial.
- Turn-finish notification closure requires notification permission accepted on Android 13+, a real WebUI turn reaching terminal state, `FreehandNotification` posted evidence, `dumpsys notification` package evidence, and tapping the notification back into `MainActivity`.
- File-access closure requires a fresh install/update marker not yet recorded in app preferences, launch evidence for `FreehandFileAccess` including that marker, runtime permission dialog/settings handoff when permissions are missing, and final granted/restricted status after returning to the app.
- Full auto-upgrade closure requires installing an older APK, staging a signed higher-version APK plus matching compiled `dist/android/update.json` at the selected daemon endpoint, observing `FreehandApkUpdate` logcat update-plan/download/install-intent evidence, manually confirming Android's system installer if prompted, and reading back the upgraded package `versionCode` plus compatible signer evidence.
- Manual Settings closure requires opening Config inside the Android WebView, tapping `Check APK update`, observing the status card move through check/download/install phases or current-version/no-update, and preserving the daemon WebUI as the only visible product UI.
- Remote daemon bootstrap closure requires opening or scanning a `freehand://daemon/import?payload=...` link on the device, then reading back app-owned `files/daemon-connection.json` plus the remote_registry sidecar before WebUI acceptance.
- Acceptance evidence must include a screenshot and logcat showing daemon WebUI selectors/layout/assets (`data-webui-shell=true`, `layoutClient=android-webview`, stylesheet applied, WebUI JavaScript ready), not native Android chrome or unstyled HTML.
- True-device settings/back proof must open Config, scroll to the long provider form, verify the sticky WebUI drawer header/close path remains accessible, press Android Back to blur the focused field, press Back again to close the drawer rather than exit the app, and capture screenshot/logcat evidence.
- A blocker summary from `verify-device-ui.sh` is evidence of non-closure, not success.

## Known Gaps

- No Espresso/instrumented tests yet.
- Attachment picker still needs true-device proof after the WebUI-only shell change.
- The Android slice does not implement relay signaling/tunnel IO or Tailscale OS auto-connect; it imports config-owned route/bootstrap truth and loads the selected daemon-hosted WebUI endpoint only.
- Silent background package replacement is not implemented because Android package installation remains system/user controlled for a normal non-device-owner app.
- Silent all-files permission grant is not implemented because Android 11+ requires user confirmation in system settings for a normal non-device-owner app.
