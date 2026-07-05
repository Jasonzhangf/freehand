# Mobile WebUI Responsive Implementation Plan

## L1 Audit Status

Status: scoped L1 report-only audit completed.

Evidence:

- Commit `3a5b461 docs(ui): define mobile layout direction` recorded the report-only design findings.
- `docs/design/multi-platform-ui-architecture.md` now defines ADP-first transport, aspect-ratio layout switching, mobile daemon connection config, Tailscale default, relay-disabled placeholder, and no silent fallback.
- `docs/design/android-client-v1-execution.md` now records Android ADP default transport and file-backed daemon config as the required next implementation slice.
- `docs/testing/app.android-client.md` now lists file-backed config and aspect-ratio verification targets.
- `docs/function-maps/app.android-client.md` marks current `SharedPreferences` host/port persistence as scaffold-only.
- Validation already run for the L1 doc slice: `cargo run -p xtask -- mainlines check`, `cargo run -p xtask -- gates check`, and `git diff --check`.

Boundary:

- This was a scoped feature audit, not a recurring unattended loop.
- No `LOOP.md`, `STATE.md`, `loop-budget.md`, or recurring loop files were created because this is a one-shot implementation stream, not a scheduled automation loop.
- The next step is L2 assisted implementation: one owner-scoped slice at a time, with tests and online evidence before claiming completion.

## Goal

Implement Freehand mobile WebUI and Android WebView closeout by reusing the existing WebUI/ADP/session rendering truth, adding aspect-ratio aware layout switching, and replacing mobile daemon connection persistence with a file-backed Tailscale-first config path.

## Acceptance Criteria

1. WebUI and Android WebView continue to consume `ui.protocol` / ADP truth only; no second session, turn, tool, or provider truth is introduced.
2. Layout switching uses width plus aspect ratio, not width-only breakpoints.
3. Phone portrait, phone landscape, tablet portrait, tablet landscape, foldable unfolded, and desktop-like viewports each map to explicit layout modes.
4. Layout switches preserve selected session, composer draft, scroll anchor, pending submit visibility, and current live lifecycle timers.
5. Mobile UI keeps the conversation first: bottom composer, safe-area handling, bottom nav or compact rail, sessions/tools/settings in sheets or secondary panes.
6. Foldable and tablet layouts use available aspect ratio to promote session/task list into a durable pane while keeping conversation primary.
7. Android daemon connection config bootstraps from bundled `assets/config/client.json`, then persists edits to an app-owned JSON file.
8. Default mobile connection mode is Tailscale, using a configured Tailscale host/IP and fixed daemon port.
9. Relay config is schema-reserved but disabled unless explicitly selected by a future relay design.
10. Connection failures show active profile, endpoint, and concrete failure class; no silent fallback to localhost, LAN scan, relay, or another profile.
11. Browser and ADP evidence prove real multi-turn conversation, failed-tool continuation, refresh persistence, no historical fake animation, and correct layout at required viewports.
12. Android JVM tests prove config and ADP protocol behavior; WebUI online proof uses S profile `127.0.0.1:4042`.

## Scope

In scope:

- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/theme.css` if token additions are needed
- `apps/freehand-server/src/lib.rs` only for asset/smoke tests or static route coverage
- `apps/freehand-android/app/src/main/java/com/freehand/android/data/**`
- `apps/freehand-android/app/src/main/java/com/freehand/android/ui/**`
- `apps/freehand-android/app/src/main/assets/config/client.json`
- `apps/freehand-android/app/src/main/assets/bridge.html` only for rendering/layout compatibility
- `apps/freehand-android/app/src/test/**`
- `docs/design/multi-platform-ui-architecture.md`
- `docs/design/android-client-v1-execution.md`
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/function-maps/app.android-client.md`
- `docs/testing/app.android-client.md`
- `docs/mainline-calls/app.android-client.json` and generated wiki if call bindings change
- `docs/mainline-calls/app.webui-smoke.json` and generated wiki if WebUI call bindings change
- `CACHE.md`, `MEMORY.md`, `note.md` after verified changes

Out of scope:

- Changing ADP frame semantics or `freehand-ui-protocol` projection truth unless tests prove the existing projection cannot express the required state.
- Rewriting WebUI in React/Vue/Svelte.
- Creating native Android UI as a replacement for WebView.
- Adding relay server implementation or relay auth flow.
- Adding LAN scan fallback as the default mobile connection behavior.
- Changing provider, reason, runtime, or session truth owners for UI convenience.
- Treating screenshots, static DOM, or local unit tests as online closure without real fixed-port WebUI/ADP evidence.

## Design Principles

1. ADP is the default command/query/subscribe truth for WebUI, Android, CLI, and headless validation.
2. HTTP/SSE paths are compatibility surfaces or display refresh mirrors only.
3. Mobile layout is a presentation decision; protocol state must not change when viewport shape changes.
4. UI consumes protocol-projected tool display semantics; UI must not classify tools from raw names, args, or result strings.
5. File-backed daemon config is local client config, not session truth or runtime truth.
6. Config errors are explicit. No fallback, no hidden profile switching, no silent localhost/LAN/relay substitution.
7. The implementation proceeds one owner slice at a time and verifies after each material behavior change.

## Technical Plan

### Slice 1: WebUI Shape Classifier

Owner: `app.webui-smoke` / `ui.platform-architecture`.

Implement a small layout classifier in WebUI assets:

- Input: viewport width, viewport height, orientation/aspect ratio.
- Output: explicit layout mode such as `phone_portrait`, `phone_landscape`, `tablet_portrait`, `tablet_landscape`, `foldable_unfolded`, `desktop_large`.
- Keep classifier pure and testable.
- Apply layout mode as a body/data attribute such as `data-layout-shape`.
- Do not mutate selected session, transcript, ADP state, or composer state inside classifier.

Required tests:

- JS/static asset smoke proves classifier function exists and maps required viewport pairs.
- WebUI CSS smoke proves each layout mode has a deterministic shell arrangement.
- Regression proves shape changes do not reset selected session or composer draft.

### Slice 2: Responsive WebUI Shell

Owner: `app.webui-smoke`.

Implement layout behavior over existing WebUI render model:

- Phone portrait: full-width chat, fixed bottom composer, safe-area padding, bottom nav.
- Phone landscape: compact two-pane mode when height is constrained; avoid hiding composer behind keyboard/safe area.
- Tablet portrait: conversation primary, sessions as drawer/sheet, inspector as bottom sheet.
- Tablet landscape: conversation plus rail; inspector as collapsible right drawer or bottom sheet.
- Foldable unfolded: durable session/task pane plus conversation; inspector remains sheet unless large enough.
- Desktop large: preserve current desktop shell behavior.

Required tests:

- Browser screenshots for required viewports.
- Assertions for visible composer, visible current session, no text overlap, no hidden bottom controls, and stable scroll-to-bottom behavior.
- Real WebUI submit at at least phone portrait and foldable/tablet shape.

### Slice 3: Mobile Connection Config Schema

Owner: `app.android-client`.

Introduce file-backed daemon connection config:

- Keep bundled `assets/config/client.json` as bootstrap default.
- Add app-owned JSON config file path for user edits.
- Define schema with `connectionMode`, `activeProfile`, `profiles[]`, and `relay` disabled block.
- Default profile mode is `tailscale`.
- Build `HostConfig` / `adpUrl` / `healthUrl` from selected profile.
- Reject malformed config explicitly and surface an actionable connection config error.
- Do not silently fallback to SharedPreferences, localhost, LAN scan, relay, or hardcoded host after a file config exists.

Implementation note:

- Existing `ClientConfig::load` and `HostStore` use bundled JSON plus `SharedPreferences`; preserve compatibility only as a migration source if needed, then write normalized file config.
- `SharedPreferences` may cache UI hints, but must not be final authoritative daemon connection config.

Required tests:

- Bootstrap from bundled config.
- First-run copy to app-owned JSON file.
- Edited Tailscale profile persists and rebuilds ADP/health URLs.
- Malformed JSON fails explicitly with a visible config error.
- Relay remains disabled by default.
- No silent fallback when active profile endpoint is unreachable.

### Slice 4: Android WebView Integration

Owner: `app.android-client`.

Wire file-backed config into Android live shell:

- `MainActivity` loads file-backed config before connecting.
- `AdpEventStream` connects to selected profile ADP URL.
- Connection banner includes profile and endpoint on failures.
- WebView layout receives viewport/safe-area/keyboard changes without duplicating render truth.
- Bridge still renders `UiPublicTurnProjection` from protocol truth only.

Required tests:

- `./gradlew testDebugUnitTest` covers config, URL construction, ADP frame shape, projector behavior.
- Existing protocol replay tests remain green.
- If no device is available, report Android WebView live-device validation as not run; do not claim device closure.

### Slice 5: Online WebUI Evidence

Owner: `app.webui-smoke` with S profile.

Run fixed-port online proof:

- `scripts/install-launchd.sh restartS`
- `curl -4fsS http://127.0.0.1:4042/health`
- `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- Browser automation against `http://127.0.0.1:4042/`.

Evidence must cover:

- `375x812` phone portrait.
- `430x932` large/tall phone.
- `844x390` phone landscape.
- `768x1024` tablet portrait.
- `1024x768` tablet landscape.
- foldable-like viewport, for example `853x1024` or another square-ish inner display shape.
- desktop control viewport.

At least one real flow must prove:

- submit text remains visible after send.
- composer clears correctly.
- failed tool result returns to the model and final turn succeeds.
- refresh preserves session history.
- historical completed turns have no live animation.
- ADP query for the same session matches visible turn ids/status.

## Risk And Mitigation

| Risk | Mitigation |
|------|------------|
| Width-only breakpoints regress foldables | Use explicit aspect-ratio classifier and viewport matrix tests |
| Mobile UI duplicates WebUI semantics | Reuse existing render model and ADP projections; UI only rearranges components |
| Config path becomes hidden fallback chain | Enforce single active file config after bootstrap; failures visible |
| Relay support leaks in too early | Parse relay block but keep disabled and unselected until relay design lands |
| Android tests pass but WebView/device behavior is unknown | Report device validation separately; do not claim device closure without evidence |
| Online browser proof misses mobile regressions | Capture screenshots and DOM assertions per required viewport |
| Touch/keyboard overlaps composer | Validate safe-area and visual viewport behavior in phone portrait/landscape |

## Verification Matrix

Static and owner tests:

- `node --check apps/freehand-server/assets/webui.js`
- `cargo test -p freehand-server -- --nocapture`
- `cd apps/freehand-android && JAVA_HOME=/opt/homebrew/opt/openjdk@17 PATH=/opt/homebrew/opt/openjdk@17/bin:$PATH ./gradlew testDebugUnitTest`
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Docs and gates:

- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `make ci`

Online S-profile:

- `scripts/install-launchd.sh restartS`
- `curl -4fsS http://127.0.0.1:4042/health`
- `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- WebUI browser screenshots and ADP query evidence under `artifacts/webui-online/`.

Android device validation:

- Required only when an Android device/emulator is available and explicitly in scope.
- If unavailable, mark as not run and keep the claim limited to JVM/WebUI/WebView asset proof.

## Implementation Order

1. Re-read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`, `docs/architecture/feature-map.md`, this goal plan, and the referenced design/function/test docs.
2. Run MemoryPalace search for mobile WebUI, Android config, aspect-ratio layout, and ADP mobile evidence.
3. Confirm owners: `app.webui-smoke`, `app.android-client`, and embedded `ui.platform-architecture` doc owner.
4. Add or update test design before implementing each slice.
5. Implement WebUI pure layout classifier and static tests.
6. Implement responsive CSS/JS shell changes without touching protocol truth.
7. Validate WebUI at static/unit level.
8. Implement Android file-backed daemon config and tests.
9. Wire Android config into ADP connection path and connection error display.
10. Run owner tests and workspace gates.
11. Run S-profile online WebUI evidence across viewport matrix.
12. Update function maps, test docs, generated mainline/wiki if touched symbols or call edges changed.
13. Update `note.md`, promote verified durable conclusions to `MEMORY.md`, update local skill only if a reusable workflow changed.
14. Re-mine MemoryPalace if the lock is available; if not, report the exact blocking PID.
15. Commit only scoped changes and evidence that belong to this task; leave unrelated untracked files untouched.

## Definition Of Done

The task is complete only when:

- WebUI uses aspect-ratio aware layout modes without changing protocol/session truth.
- Required viewport screenshots and DOM assertions are saved.
- Android daemon config is file-backed, Tailscale-first, and tested.
- Relay is present only as disabled/reserved config surface.
- ADP remains the default control/status path.
- Fixed-port S-profile online proof validates real WebUI multi-turn behavior and ADP truth.
- Owner tests, workspace gates, and docs/mainline checks pass.
- Function maps and test designs match implementation.
- Final report includes changed files, commits, verification commands/results, evidence paths, and any unverified Android-device gap.
