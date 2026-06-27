# Android Client v1 Execution Plan

## Status

- **Status**: design-locked
- **Feature**: `app.android-client`
- **Owner**: `apps/freehand-android`
- **Reference design**: `docs/design/multi-platform-ui-architecture.md`
- **Reference protocol**: `docs/function-maps/ui.protocol.md`
- **Reference mock**: `apps/freehand-server/assets/mocks/android/mobile-mock.html`

## 1. Goal

Build Freehand Android as a protocol-consumer client, not a second truth source.

The Android client must:

- render `ui.protocol` projections
- submit command ingress into the existing protocol boundary
- show turn status, tool status, and slave status without mutating truth locally
- keep transient UI state separate from session truth
- support multiple agent/session switching without duplicating reason/debug semantics

## 2. Non-Goals

- No second reason engine
- No provider adapter inside Android
- No session truth ownership inside Android
- No fallback or duplicate projection logic
- No direct access to `freehand-reason` or provider crates from the client shell

## 3. Platform Choice

### v1 decision

Android v1 is already a native shell that consumes the same protocol truth and the same screen grammar as WebUI.

The shell uses a local WebView host for the rendered surface, but all truth still flows through `ui.protocol`:

- native Android app shell
- local `bridge.html` render host inside the APK
- shared UI protocol semantics
- protocol query/subscribe over HTTP + SSE
- command ingress over HTTP POST
- local drawer/state rendering for agent/session switching

## 4. Client Boundary

Android client responsibilities:

- input ingress
- snapshot query
- subscription rendering
- local presentation state
- native bridge for file pick / background / notifications

Android client forbidden responsibilities:

- writing session truth
- rendering provider raw payloads as primary UI
- inventing turn state outside `ui.protocol`
- duplicating debug or reason ledgers

## 5. Module Breakdown

### 5.1 Shell modules

| Module | Responsibility | Truth owner |
|---|---|---|
| `MainActivity` | activity, navigation, window, safe area, WebView host | Android framework |
| `ui/components/TopBarController` | current agent, slave summary, connection state | `ui.protocol` + runtime status |
| `ui/components/SlaveStripController` | collapsed slave summary strip | `ui.protocol` outputs |
| `ui/components/StatusBannerController` | transient / persistent connection banner | local presentation state |
| `ui/components/InputBarController` | bottom input bar + submit callback | `ui.protocol` command ingress |
| `ui/components/DrawerController` | agent/session switching, host selection | local UI state |
| `data/ProtocolClient` | HTTP query + command POST | `ui.protocol` |
| `data/SseEventStream` | SSE subscribe / disconnect | `ui.protocol` |
| `data/TimelineProjector` | turn/status/debug/session projection cache | `ui.protocol` outputs |
| `data/CommandIngress` | submit / cancel entry points | `ui.protocol` command ingress |
| `bridge.html` | render host for `UiPublicTurnProjection` | `ui.protocol` outputs |

### 5.2 Data flow modules

| Flow | Source | Sink | Notes |
|---|---|---|---|
| command ingress | user text / action | `ui.protocol` → runtime dispatch | mutation intent only; `CancelLatestActiveTurn` remains explicit |
| latest snapshot query | `ui.protocol` | projection store | first paint / refresh before SSE catch-up |
| incremental subscribe | SSE | projection store | no back-pressure semantics in v1 UI |
| agent/session switch | local selection | protocol query/subscribe selector | selection changes view, not truth |
| status animation | turn / connection changes | card / badge / banner | transient presentation only |

## 6. Mainline Call Skeleton

This is the Android client call skeleton already present in the scaffold.

### Request mainline

1. `MainActivity` receives input from `InputBarController`
2. `CommandIngress::submit` / `cancelLatest` wraps the protocol-owned command
3. `ProtocolClient::postCommand` sends `SubmitUserInput` or `CancelLatestActiveTurn`
4. `ui.protocol` validates ingress and emits an explicit dispatch receipt or error
5. Android client waits for `UiSubscriptionEvent` projections and updates the WebView snapshot

### Response mainline

1. client loads latest snapshot from `ui/query/latest-active-turn`
2. client subscribes to `ui/subscribe/turn/latest`
3. protocol emits incremental projections
4. `TimelineProjector` updates cards, badges, drawer, and debug views
5. `bridge.html` renders `UiPublicTurnProjection.public_conversation`

### Error mainline

1. invalid command -> explicit protocol error
2. missing turn / bad selector -> explicit query error
3. network / SSE drop -> explicit client-visible connection state
4. bridge failure -> explicit native bridge error
5. no fallback path hides the failure

## 7. State Semantics

### 7.1 Persistent truth

Persistent truth must stay in:

- `reason.turn`
- `debug.core`
- `metadata.core`
- `ui.protocol`
- `bridge.html` only as a read-only render host for `ui.protocol` outputs

### 7.2 Android local state

Local state may only cover:

- drawer open/close
- selected tab
- current visible filter
- scroll position
- temporary input draft
- transient connection banner
- cached host selection

### 7.3 Status mapping

| Protocol state | Android presentation |
|---|---|
| running | animated status dot + `thinking`/`running` pill |
| tool call active | tool block with spinner |
| success | green status pill and completed tool block |
| error | red status pill and failed tool block |
| blocked | amber status pill |
| cancelled | neutral status pill and terminal text preserved |
| done | final text only + completed badge |

### 7.4 Agent/session semantics

- current agent is shown in top bar
- other agents show in drawer and collapsed strip
- switching agent/session changes selection, not truth
- slave turns may appear as a substream card or detail page, but remain the same protocol truth

## 8. Implementation Order

1. lock Android shell routes and navigation surface
2. keep `bridge.html` as the render host and `mobile-mock.html` as the design preview
3. build protocol client and projection store
4. render top status strip, drawer, and timeline
5. bind command ingress and SSE subscribe
6. add debug / tool detail projection
7. add lifecycle reconnect and foreground/background handling
8. add native bridge only where system integration requires it

## 9. Testing Strategy

See `docs/testing/app.android-client.md` for the actual test plan.

The minimum validation stack before code closeout:

- projection mapping tests in `TimelineProjectorTest`
- component black-box tests in `HostConfigTest` and `CommandIngressProtocolTest`
- protocol replay tests against canned `UiSubscriptionEvent` fixtures
- route/render smoke tests for the self-contained mock
- workspace gates from `xtask`

## 10. Deliverables

- Android client feature entry
- Android client function map
- Android client test design
- self-contained mock preview
- native app shell skeleton with protocol client and projection store
- live WebView bridge host inside the APK
