# Test Design: `app.android-client`

- feature_id: `app.android-client`
- owner: `apps/freehand-android` (future Android app crate / project shell)
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- reference function map: `docs/function-maps/app.android-client.md`
- reference mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html`

## Lifecycle Path Under Test

1. Android shell renders the static design preview from `mobile-mock.html` and matches the locked multi-platform screen grammar
2. Android shell opens `mobile-mock.html` directly via `file://` and renders without any external `/assets/...` dependency
3. Android shell serves the same design via the runtime daemon `/mock/android` route and returns HTTP 200 with the locked body
4. Android client consumes `ui.protocol` projection truth (latest active turn, debug snapshot, node status) without owning it
5. Android client submits user input through the protocol-owned HTTP command ingress route and waits for the protocol-owned dispatch receipt
6. Android client subscribes to the protocol-owned latest-turn SSE stream and renders incremental updates without back-pressure semantics in v1
7. Android client switches the visible agent / session through the drawer without altering truth
8. Android client surfaces provider / reason / debug errors as red status pills and failed tool blocks; it does not invent success
9. Android client surfaces connection drop and reconnect as a transient connection banner; it does not silently re-render
10. Android client cancels an in-flight turn through protocol-owned command ingress and only clears local input draft
11. Android client foreground / background lifecycle re-subscribes to the protocol-owned SSE stream after returning to foreground
12. Android client renders both white and black themes using the shared `theme.css` tokens
13. Android client never imports `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime` as a direct dep
14. Android client does not own session truth, debug ledger, provider payload, or turn-status truth
15. Android client does not implement a second dispatch port, a second completion-schema validator, or a second projection layer

## White-Box Plan

- `binding pending`: projection mapping helper unit tests (Android crate does not exist yet)
- `binding pending`: SSE subscribe state-machine helper unit tests
- `binding pending`: command ingress HTTP client unit tests
- `binding pending`: theme token binding unit tests against `theme.css`
- `binding pending`: drawer / tab / scroll / draft local state helper unit tests
- `bound`: `mobile-mock.html` is inlined-CSS and contains the locked `mock-mobile` class for design-review previews

## Module Black-Box Plan

- self-contained `mobile-mock.html` opens via `file://` and renders the locked multi-platform layout (smoke)
- `mobile-mock.html` does not depend on any external `/assets/...` CSS link (regression gate)
- `mobile-mock.html` route at `/mock/android` returns HTTP 200 and contains the locked `mock-mobile` class (bound via `android_mock_route_returns_design_preview`)
- `mobile-mock.html` matches the locked screen grammar: top agent strip, slave pill strip, turn cards, status banner, input bar, bottom nav
- `mobile-mock.html` honours white and black theme tokens
- reverse gate: Android client does not import `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime` as a direct dep
- reverse gate: Android client does not own a second copy of projection, command ingress, debug surface, or theme truth
- `binding pending`: Android app shell end-to-end smoke (when app crate lands)
- `binding pending`: Android SSE subscribe smoke against the runtime daemon
- `binding pending`: Android command ingress smoke against the runtime daemon
- `binding pending`: Android node-status query smoke against the runtime daemon
- `binding pending`: Android cancel command smoke
- `binding pending`: Android theme switch smoke (white + black)
- `binding pending`: Android foreground / background reconnect smoke

## Project Black-Box Impact

- Android app boundary proves it can consume `freehand-ui-protocol` without owning reason / provider / node / config / runtime semantics
- Android app boundary proves it does not need direct reason / provider / node / config / runtime imports
- Android app boundary must use the same shared transport / projection / theme / debug surface truth that the WebUI uses
- Android mock render matches the locked multi-platform screen grammar
- machine-readable mainline truth remains the only source for generated wiki artifacts; mainline call JSON must be regenerated when the Android app crate lands

## Fixtures / Replay Inputs / Runtime Evidence Paths

- `~/.freehand/state/android` (future)
- `~/.freehand/replays/android` (future)
- self-contained `mobile-mock.html` render screenshot
- `mobile-mock.html` HTTP route response body
-
## Known Gaps

- no Android app crate yet; binding-pending rows are explicit, not invented
- no generated wiki yet; wiki generator will emit `docs/wiki/app.android-client.md` once the mainline call JSON exists
- no native bridge yet; v1 plan explicitly defers system integration (file pick / background / notification) until app shell is in place
- v1 does not include a WebView fallback surface
- v1 SSE drop is surfaced as a transient banner; no auto-retry projection is implemented in the client (reconnect is a lifecycle handling step)
- command ingress currently uses the same protocol-owned HTTP command ingress as the WebUI; a second dispatch port is explicitly forbidden

## Sync Status Between Design and Implementation

- design: `docs/design/multi-platform-ui-architecture.md` (locked)
- design: `docs/design/android-client-v1-execution.md` (locked)
- function map: `docs/function-maps/app.android-client.md` (locked)
- feature map: `app.android-client` entry (locked)
- mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html` (self-contained, rendered through `/mock/android`)
- Android app crate: pending
- mainline call JSON: `binding pending`
- generated wiki: `binding pending`
