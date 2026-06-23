# freehand-android (design skeleton, not yet a buildable Android project)

> ⚠️ This directory is the design skeleton for the planned native Android client of Freehand. It is **not** a buildable Android project yet — there is no `build.gradle`, no `AndroidManifest.xml`, no Kotlin/Java source. The on-disk resources are committed as the locked design skeleton that the future Android app crate will reuse.

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

## What is in this directory

- `res/values/colors.xml`, `dimens.xml`, `themes.xml`, `strings.xml` — design tokens (color, spacing, typography) for Android, aligned with `apps/freehand-server/assets/theme.css`.
- `res/drawable/` — pill, avatar, turn card, tool block shapes (with role-tinted left border and tool-status left border).
- `res/anim/` — pulse + spin + drawer enter/exit animations.
- `res/layout/` — layout structure: topbar + slave strip + conversation list + status banner + input bar + bottom nav + right-slide drawer.

## Reference

- `docs/design/multi-platform-ui-architecture.md` — locked multi-platform design rules.
- `docs/design/android-client-v1-execution.md` — Android v1 execution plan.
- `docs/function-maps/app.android-client.md` — owner function map.
- `docs/testing/app.android-client.md` — owner test design.

## Hard Constraints (must not be violated)

- the Android client is a `ui.protocol` consumer + command ingress source only; it does not own session, reason, debug, metadata, or provider truth
- the Android client does not import `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime` directly
- the Android client does not implement a second dispatch port, a second completion-schema validator, or a second projection layer
- `docs/function-maps/ui.protocol.md` — UI protocol truth consumed by the eventual WebView shell.
