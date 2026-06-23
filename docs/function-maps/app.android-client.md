# Function Map: `app.android-client`

- feature_id: `app.android-client`
- owner crate: `apps/freehand-android` (future Android app crate / project shell)
- owner module: `apps/freehand-android/app/src/main/**` (binding pending)
- reference design: `docs/design/multi-platform-ui-architecture.md`
- reference execution plan: `docs/design/android-client-v1-execution.md`
- owner entry symbols:
  - `binding pending` (no Android app crate exists yet; symbols deliberately not invented)
- current in-tree design artifacts (locked, not yet app source):
  - `apps/freehand-android/app/src/main/res/values/colors.xml`
  - `apps/freehand-android/app/src/main/res/values/dimens.xml`
  - `apps/freehand-android/app/src/main/res/values/themes.xml`
  - `apps/freehand-android/app/src/main/res/values/strings.xml`
  - `apps/freehand-android/app/src/main/res/drawable/**` (pill, avatar, turn card, tool block shapes)
  - `apps/freehand-android/app/src/main/res/anim/**` (pulse + spin + drawer enter/exit)
  - `apps/freehand-android/app/src/main/res/layout/**` (topbar + slave strip + conversation list + status banner + input bar + bottom nav + right-slide drawer)
- reference mock: `apps/freehand-server/assets/mocks/android/mobile-mock.html` (self-contained, opens via `file://` and via `/mock/android`)

## Request Mainline

- Android client shell receives user input and forwards it as command ingress through the protocol-owned HTTP command ingress route
- Android client shell never mutates session, reason, debug, metadata, or provider truth locally
- Android client subscribes to `ui.protocol` turn / debug / status projections through HTTP query + SSE subscribe
- Android client submits user actions (submit / cancel / switch agent / switch session) only through protocol-owned command ingress
- Android client reads the latest snapshot via `ui.protocol` HTTP query before any incremental SSE subscribe update is shown
- Android client does not import or directly call `freehand-reason`, `freehand-provider-*`, `freehand-node`, `freehand-config`, or `freehand-runtime`; it only consumes `freehand-ui-protocol` projections
- Android client does not define a second dispatch port, a second session store, or a second completion-schema validator

## Response Mainline

- Android client renders the latest active turn projection as turn cards
- Android client renders terminal text as the final projected message and never as raw provider payload or raw completion schema
- Android client renders tool calls and tool results as protocol-projected tool blocks with status-driven color and a collapsible details section
- Android client renders the top status strip from protocol-projected current-agent and slave summary
- Android client renders the right-slide drawer from local UI selection (agents / sessions / quick actions) without altering truth
- Android client renders debug detail through the protocol-owned debug surface and never via a local debug ledger
- Android client surfaces the connection state (running / dropped / reconnecting) as a transient connection banner; it is local UI state, not protocol truth
- Android client surfaces agent status (`busy` / `idle`) and turn status (`thinking` / `running tool` / `success` / `error` / `blocked` / `done`) through protocol-projected status pills and animated indicators, never through client-invented status
- Android client respects light and dark themes through the same theme module used by the WebUI; theme selection is local UI state

## Error Mainline

- invalid command ingress returns explicit protocol error to the user; the Android client does not invent success
- missing or unknown agent / session / turn selector returns explicit protocol error
- network or SSE drop returns explicit client-visible connection state; no silent re-render and no fallback projection
- provider / reason / debug error from `ui.protocol` is rendered as a red status pill and a failed tool block; it is never re-projected as success
- cancel-without-active-turn clears only local input draft; it does not invent a runtime mutation
- native bridge failures (file pick / background / notification) return explicit bridge error and never silently fall back to a local-only mutation
- the Android client must not implement a fallback / second projection path; any rendering failure is reported as explicit client-visible error and routed back to the protocol owner for the only true fix

## Shared Multi-Reference Functions

- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/**` (binding pending)
  - purpose: project latest active turn into a UI-safe public projection
  - allowed callers: any UI consumer including Android, WebUI, future surfaces
  - why shared: prevents a second projection logic from being forked into the Android client
- `latest_turn_subscribe_payload`
  - owner: `crates/freehand-ui-protocol/src/**` (binding pending)
  - purpose: emit incremental SSE projection updates for the latest active turn
  - allowed callers: any UI consumer including Android, WebUI, future surfaces
  - why shared: avoids a second SSE event-shape contract
- `command_ingress_dispatch`
  - owner: `crates/freehand-ui-protocol/src/**` (binding pending)
  - purpose: validate and dispatch a UI command into the protocol-owned dispatch envelope
  - allowed callers: any UI consumer including Android, WebUI, future surfaces
  - why shared: keeps command ingress shape and validation in a single owner
- `debug_query_payload`
  - owner: `crates/freehand-ui-protocol/src/**` (binding pending)
  - purpose: project a debug snapshot into a UI-safe debug surface payload
  - allowed callers: any UI consumer including Android, WebUI, future surfaces
  - why shared: keeps debug projection in a single owner and prevents client-side debug truth
- `theme.css` design tokens
  - owner: `apps/freehand-server/assets/theme.css`
  - purpose: shared white/black theme tokens reused by the Android mock and future Android client
  - allowed callers: WebUI shell, Android mock, future Android client
  - why shared: keeps one source of theme truth across surfaces

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `binding pending` | `apps/freehand-android/app/src/main/**` | Android app shell entrypoint | activity intent | app process | Android framework | app shell | binding pending |
| 02 | `binding pending` | `apps/freehand-android/app/src/main/**` | HTTP query call to `ui.protocol` latest-turn endpoint | selector (agent, session, turn) | `UiPublicTurnProjection` | Android app shell | `freehand-ui-protocol` query route | binding pending |
| 03 | `binding pending` | `apps/freehand-android/app/src/main/**` | SSE subscribe call to `ui.protocol` latest-turn stream | selector | incremental `UiPublicTurnProjection` | Android app shell | `freehand-ui-protocol` SSE route | binding pending |
| 04 | `binding pending` | `apps/freehand-android/app/src/main/**` | HTTP POST command ingress call | user text / action | dispatch receipt | Android app shell | `freehand-ui-protocol` command ingress | binding pending |
| 05 | `binding pending` | `apps/freehand-android/app/src/main/**` | HTTP query call to `ui.protocol` debug snapshot endpoint | turn id | `DebugStateSnapshot` | Android app shell | `freehand-ui-protocol` debug query | binding pending |
| 06 | `binding pending` | `apps/freehand-android/app/src/main/**` | HTTP query call to `ui.protocol` node-status endpoint | agent id | node status projection | Android app shell | `freehand-ui-protocol` node status query | binding pending |
| 07 | `binding pending` | `apps/freehand-android/app/src/main/**` | Native bridge: file pick / background / notification | OS API request | OS result | Android app shell | Android framework | binding pending |
| 08 | `binding pending` | `apps/freehand-android/app/src/main/**` | Drawer / tab / scroll / draft local state update | user UI action | local UI state | Android app shell | local UI state holder | binding pending |
| 09 | `binding pending` | `apps/freehand-android/app/src/main/**` | Reconnect + foreground/background lifecycle handling | lifecycle event | subscribe state | Android app shell | SSE subscribe + query refresh | binding pending |
| 10 | `handle_android_mock` | `apps/freehand-server/src/lib.rs` | Serve self-contained `mobile-mock.html` for design review | HTTP GET `/mock/android` | HTML body | design-review operator | embedded mock asset | bound (mock preview) |

## Sync Status Against Code

- no Android app crate exists yet; the function map, test design, and feature map must land before any Android source code
- the static design preview `apps/freehand-server/assets/mocks/android/mobile-mock.html` is self-contained and rendered through `handle_android_mock` for review only
- generated wiki and mainline call JSON must be regenerated from `docs/mainline-calls/app.android-client.json` and the wiki generator once the Android app crate is created
- the Android client is explicitly forbidden from owning a second copy of any projection, command ingress, debug surface, or theme truth; shared functions stay in `freehand-ui-protocol`, `freehand-server`, and the shared theme assets
