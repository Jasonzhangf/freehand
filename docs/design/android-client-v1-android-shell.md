# Android Client v1 Android Shell — Execution Plan

## Status

- **Status**: in-progress
- **Feature**: `app.android-client`
- **Owner**: `apps/freehand-android`
- **Reference design**: `docs/design/multi-platform-ui-architecture.md`
- **Reference mock**: `apps/freehand-server/assets/mocks/android/mobile-mock.html` (locked static design)
- **Reference mobile-mock.css**: `apps/freehand-server/assets/mocks/android/mobile-mock.css`

## 1. Goal

Make the Android app render the locked mobile-mock design and connect to the real runtime daemon via the protocol transport (SSE subscribe + HTTP command ingress). Not a native rewrite — a WebView shell consuming the same `mobile-mock.html` assets served by `freehand-server`.

## 2. Non-Goals

- No native Android UI component rewrite in this slice
- No iOS work
- No second dispatch port or second projection layer
- Android does not own session/reason/debug/metadata/provider truth

## 3. What Exists vs What Is Missing

### Exists
- `mobile-mock.html` + `mobile-mock.css`: complete locked design (678 + 457 lines)
- `apps/freehand-android`: Gradle project scaffold (app shell, native components, assets)
- `MainActivity.kt`: WebView host + native input bar + controller composition
- `ProtocolClient.kt`: HTTP GET/POST against `ui.protocol` routes (query only, no SSE)
- `TimelineProjector.kt`: naive turn card state (not wired to real SSE events)
- `CommandIngress.kt`: submit/cancel via HTTP POST
- `TopBarController.kt`, `SlaveStripController.kt`, `StatusBannerController.kt`, `DrawerController.kt`, `InputBarController.kt`: individual native component controllers (not wired)
- `HostStore.kt`: SharedPreferences host:port persistence
- `mobile-shell.html` (old, crude): loaded by WebView but does not match the locked design

### Missing / Broken
1. WebView loads `mobile-shell.html` (crude) instead of `mobile-mock.html` (beautiful locked design)
2. `ProtocolClient` has no SSE subscription method — `TimelineProjector` never receives real events
3. `TimelineProjector` maps only a tiny subset of `ui.protocol` event types
4. Native controllers (topbar/slave-strip/drawer/status-banner) exist but are not wired to the WebView shell or to protocol state
5. SSE event stream is not consumed from the Android client
6. The Android APK would not compile against a real device (no `local.properties` / SDK path)

## 4. Execution Steps

### Step 1: Wire WebView to mobile-mock.html assets (static render)

**Action**: Change `MainActivity.kt` to load `mobile-mock.html` instead of `mobile-shell.html`.
**File**: `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt`
**Change**: `webView.loadUrl("file:///android_asset/mobile-shell.html")` → `webView.loadUrl("file:///android_asset/mobile-mock.html")`
**Verification**: Static render of the locked design in Android WebView (without live data).

Copy `mobile-mock.html` and `mobile-mock.css` into `app/src/main/assets/` so they are bundled in the APK.

**Files to add**:
- `apps/freehand-android/app/src/main/assets/mobile-mock.html`
- `apps/freehand-android/app/src/main/assets/mobile-mock.css`

### Step 2: Wire SSE subscription in ProtocolClient

**Action**: Add `subscribeLatestTurnSse()` method to `ProtocolClient` using OkHttp SSE or Android's built-in `HttpURLConnection` SSE reading.
**File**: `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt`
**Output**: `Flow<JsonObject>` or callback-based event stream matching `ui.protocol` SSE event types.
**Event types to handle**: `turn`, `progress`, `node_status`, `error`, `terminal`, `checkpoints`.

This is the core transport piece. Android must receive the same SSE stream that the WebUI consumes.

### Step 3: Update TimelineProjector to real ui.protocol types

**Action**: Expand `TimelineProjector` to handle all `ui.protocol` event types emitted by the SSE stream.
**File**: `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt`
**Changes**:
- Parse `type`, `turn_id`, `user_text`, `assistant_text`, `status`, `source_agent_id`, `source_node_id`
- Support `progress` events → update turn state
- Support `error` events → mark turn as failed
- Support `terminal` events → mark turn as done/failed/cancelled
- Support `node_status` events → update agent/slave status strip
- Maintain `current_agent`, `slave_states`, `turn_state`, `turns[]`
- Provide `applySnapshot(Map)` for the JS `window.__freehand.applySnapshot` bridge

### Step 4: JS bridge: wire applySnapshot to WebView

**Action**: Keep `window.__freehand.applySnapshot` in `mobile-mock.html` but drive it from `TimelineProjector.snapshotJson()` fed through `evaluateJavascript`.
**File**: `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt`
**Changes**:
- After each SSE event → `TimelineProjector.applyTurnEvent` → `webView.evaluateJavascript("window.__freehand.applySnapshot(" + snapshotJson + ")", null)`
- On `onPageFinished`: initial snapshot from `ProtocolClient.getLatestTurn()` before SSE opens
- This way the HTML/CSS shell is preserved and only the data-driving changes

### Step 5: Connect native controllers to protocol state

**Action**: Native controllers (TopBarController, SlaveStripController, StatusBannerController) read from `TimelineProjector` state instead of being passive.
**File**: `MainActivity.kt`
**Changes**:
- After each SSE event, read `TimelineProjector.snapshot()` and update native controller views
- `TopBarController.setAgent(name, status)` from `current_agent` + `turn_state`
- `SlaveStripController.render(slaves)` from `slave_states`
- `StatusBannerController.showTransient(message)` on `connection` state changes

### Step 6: Command ingress stays on native input bar

**Action**: `InputBarController` already wired to `CommandIngress.submit`. Keep it.
**Verification**: Submit button sends `POST /ui/command { type: SubmitUserInput, text: ... }` and shows success/error.

### Step 7: Escape / Cancel

**Action**: `MainActivity.onKeyDown` already handles `KEYCODE_ESCAPE` → `ingress.cancelLatest()`. Keep it.
**Verification**: Escape cancels the in-flight turn via `CancelLatestActiveTurn`.

### Step 8: Theme support (white + black)

**Action**: `mobile-mock.html` already supports `body.theme-dark` class. Android WebView sets initial theme from system preference.
**File**: `MainActivity.kt` or `HostConfig.kt`
**Changes**: Pass `?theme=dark` or `?theme=light` query param to the HTML, or call `evaluateJavascript("document.body.classList.toggle('theme-dark', true)")` based on Android night mode.

### Step 9: Add Android SDK path to local.properties

**File**: `apps/freehand-android/local.properties` (create if missing)
**Content**: `sdk.dir=<ANDROID_SDK_PATH>` (e.g. `/Users/fanzhang/Library/Android/sdk`)
**Note**: This file is gitignored and must be created per-machine.

### Step 10: Compile + verify

**Commands**:
```bash
cd apps/freehand-android
./gradlew :app:assembleDebug 2>&1 | tail -20
# If successful:
adb install -r app/build/outputs/apk/debug/app-debug.apk
# Or: open app/build/outputs/apk/debug/app-debug.apk in Android Studio
```

## 5. Module Map After This Slice

| Module | File | Responsibility |
|--------|------|----------------|
| Protocol transport | `ProtocolClient.kt` | HTTP query + SSE subscribe + POST command |
| Turn state | `TimelineProjector.kt` | ui.protocol event → UI state |
| Shell entry | `MainActivity.kt` | WebView host + controller composition |
| Native topbar | `TopBarController.kt` | Agent name + status pill (reads projector state) |
| Native slave strip | `SlaveStripController.kt` | Collapsed slave pills (reads projector state) |
| Native status banner | `StatusBannerController.kt` | Transient connection warnings |
| Native input | `InputBarController.kt` | User text submit (writes via CommandIngress) |
| Native drawer | `DrawerController.kt` | Agent/session switching (local UI only) |
| JS shell | `mobile-mock.html` | Turn card render + CSS design tokens |

## 6. Test Plan

### Static smoke (no daemon needed)
- `mobile-mock.html` opens via `file://` and renders locked design (open in desktop browser)
- `mobile-mock.html` served by `freehand-server webui-serve-smoke --bind 127.0.0.1:3501` at `http://127.0.0.1:3501/mock/android`
- `android_mock_route_returns_design_preview` test in `freehand-server` passes

### Integration smoke (daemon needed)
1. Start `freehand-daemon serve --agent master --bind 127.0.0.1:4040` (or any port)
2. Android device/emulator points to that host:port
3. App opens → WebView loads `mobile-mock.html` → static design visible
4. App submits text → `POST /ui/command` returns 202
5. App receives SSE stream → turn cards appear with real data
6. Escape cancels → `CancelLatestActiveTurn` dispatched

### White-box (local only)
- `ProtocolClient` SSE parsing unit tests (mock server responses)
- `TimelineProjector` event-mapping unit tests
- Theme toggle unit tests

### Reverse gates
- Android APK must not contain `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, `freehand-runtime` imports
- Android APK must not write to `~/.freehand/state/` or `~/.freehand/ledgers/`

## 7. Implemented (this session)

- Step 1 ✓: copied `mobile-mock.html` + `mobile-mock.css` into `app/src/main/assets/`
- Step 1 ✓: flipped `MainActivity.kt` loadUrl from `mobile-shell.html` → `mobile-mock.html`
- Step 2 ✓: added `SseEventStream.kt` using OkHttp 4.12 built-in SSE
- Step 3 ✓: rewrote `TimelineProjector.kt` to full `ui.protocol` event mapping
- Step 4 ✓: rewrote `MainActivity.kt` to SSE lifecycle + WebView snapshot push
- Step 5 + 6 ✓: native controllers wired via `pushSnapshotToWebView()`, command ingress kept
- Step 7: theme dark/light via Android night mode in `applyInitialTheme`
- Step 8: `local.properties` per-machine (gitignored)
- Step 9–10: compile + adb install (pending: no gradle in this environment)

## 8. File Changes Summary

| Action | File |
|--------|------|
| ADD | `apps/freehand-android/app/src/main/assets/mobile-mock.html` |
| ADD | `apps/freehand-android/app/src/main/assets/mobile-mock.css` |
| ADD | `apps/freehand-android/app/src/main/java/com/freehand/android/data/SseEventStream.kt` |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` (kept; SSE moved to dedicated class) |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` (full ui.protocol event mapping) |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` (SSE lifecycle + WebView snapshot push + native controller wiring) |
| CREATE | `apps/freehand-android/local.properties` (per-machine SDK path, gitignored) |
| ADD | `docs/design/android-client-v1-android-shell.md` |
| ADD | `note.md` |

| Action | File |
|--------|------|
| ADD | `apps/freehand-android/app/src/main/assets/mobile-mock.html` |
| ADD | `apps/freehand-android/app/src/main/assets/mobile-mock.css` |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/data/ProtocolClient.kt` (+SSE) |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/data/TimelineProjector.kt` (full protocol mapping) |
| UPDATE | `apps/freehand-android/app/src/main/java/com/freehand/android/ui/MainActivity.kt` (wire all) |
| CREATE | `apps/freehand-android/local.properties` (per-machine SDK path, gitignored) |

## 8. Dependencies

- `freehand-server` must serve `/mock/android` route (already done ✓)
- `freehand-daemon` must serve `/ui/subscribe/turn/latest` SSE (already done ✓)
- `freehand-daemon` must serve `/ui/command` POST (already done ✓)
- No new Rust changes required for this slice
