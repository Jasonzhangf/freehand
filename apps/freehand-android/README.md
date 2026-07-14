# Freehand Android

Android is a thin host for the canonical daemon-hosted WebUI.

It must not render a second native conversation UI, settings drawer, update panel, status dashboard, local HTML bridge, or Android mock UI. Those surfaces belong to the daemon WebUI served at:

```text
http://<host>:<port>/?client=android-webview
```

Android-owned responsibilities:

- load `assets/config/client.json` into app-owned daemon endpoint config on first run;
- host a WebView with mobile viewport settings;
- expose `FreehandAndroidFilePicker` for WebUI attachment controls;
- package launcher icons generated from `assets/logo.png`;
- provide `scripts/verify-device-ui.sh` for true-device WebUI evidence.

Build and install:

```bash
cd apps/freehand-android
./gradlew testDebugUnitTest assembleDebug
adb -s <serial> install -r app/build/outputs/apk/debug/app-debug.apk
```

Device verification:

```bash
apps/freehand-android/scripts/verify-device-ui.sh <serial>
```

The verifier accepts only canonical WebUI layout evidence. Native fallback screenshots are failures.
