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
- protocol query/subscribe/command over ADP WebSocket `/adp`
- HTTP query, SSE subscribe, and HTTP POST remain compatibility paths
- local drawer/state rendering for agent/session switching
- daemon connection profile loaded from file-backed client config, defaulting to Tailscale

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
| `data/AdpEventStream` | ADP WebSocket query / subscribe / command | `ui.protocol` |
| `data/SseEventStream` | compatibility SSE subscribe / disconnect | `ui.protocol` |
| `data/TimelineProjector` | turn/status/debug/session projection cache | `ui.protocol` outputs |
| `data/CommandIngress` | submit / cancel entry points | `ui.protocol` command ingress |
| `bridge.html` | render host for `UiPublicTurnProjection` | `ui.protocol` outputs |
| file-backed client config | daemon connection profile and endpoint shape | local client config only |

### 5.2 Data flow modules

| Flow | Source | Sink | Notes |
|---|---|---|---|
| command ingress | user text / action | `ui.protocol` → runtime dispatch | mutation intent only; `CancelLatestActiveTurn` remains explicit |
| latest snapshot query | ADP query | projection store | first paint / refresh before subscribe catch-up |
| incremental subscribe | ADP subscribe | projection store | no second truth source |
| agent/session switch | local selection | protocol query/subscribe selector | selection changes view, not truth |
| status animation | turn / connection changes | card / badge / banner | transient presentation only |
| daemon connection config | app-owned config file | `HostConfig` / ADP URL | default Tailscale profile; relay reserved |

## 6. Mainline Call Skeleton

This is the Android client call skeleton already present in the scaffold.

### Request mainline

1. `MainActivity` receives input from `InputBarController`
2. `CommandIngress::submit` / `cancelLatest` wraps the protocol-owned command
3. `AdpEventStream::sendCommand` sends `SubmitUserInput` or `CancelLatestActiveTurn` in an ADP command frame
4. `ui.protocol` validates ingress and emits an explicit command receipt or failure frame
5. Android client waits for ADP `UiSubscriptionEvent` projections and updates the WebView snapshot

### Response mainline

1. client loads latest snapshot through ADP query
2. client subscribes through ADP subscribe
3. protocol emits incremental projections
4. `TimelineProjector` updates cards, badges, drawer, and debug views
5. `bridge.html` renders `UiPublicTurnProjection.public_conversation`

### Error mainline

1. invalid command -> explicit protocol error
2. missing turn / bad selector -> explicit query error
3. network / ADP drop -> explicit client-visible connection state
4. bridge failure -> explicit native bridge error
5. no fallback path hides the failure

### Connection config mainline

1. bundled `assets/config/client.json` provides the default Tailscale profile
2. first app startup copies the bundled config into an app-owned editable config file
3. subsequent startups read the app-owned file as the active source
4. user edits update that file and then reconnect ADP from the selected profile
5. relay profile fields may exist in schema but stay disabled until relay protocol/auth is designed

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
- file-backed daemon connection profile

Host/endpoint selection is local client state, but it must be file-backed. `SharedPreferences` may cache UI hints, but it is not the authoritative long-term daemon connection config.

### 7.2.1 Daemon connection config

Required direction:

- `assets/config/client.json` remains the bundled bootstrap default.
- The app must persist user-edited daemon connection config into an app-owned JSON file.
- Default connection mode is `tailscale` with a fixed daemon port, normally release `4041` unless the user selects a development profile.
- The config schema keeps an explicit relay section for future server-mediated access, but relay is disabled by default and must not silently activate.
- Connection failure must show the active profile, endpoint, and error class.

Current implementation:

- `ClientConfig::store` adapts Android `Context` to the app-owned config file path.
- `DaemonConnectionConfigStore::load` bootstraps `assets/config/client.json` into `filesDir/daemon-connection.json` on first run.
- Later startups read only the app-owned JSON file as endpoint truth.
- `DaemonConnectionConfig::parse` validates required fields, active profile, Tailscale-only mode, endpoint paths, and relay-disabled state explicitly.
- `DrawerController` edits the active profile endpoint, `MainActivity::saveHostConfig` writes the app-owned JSON file, and ADP reconnects from the selected profile only after the write succeeds.

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
9. keep file-backed daemon connection config covered by JVM tests and explicit UI errors
10. add aspect-ratio aware WebView/WebUI layout verification for phone, foldable, and tablet shapes

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
