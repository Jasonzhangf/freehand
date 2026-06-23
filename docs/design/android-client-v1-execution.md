# Android Client v1 Execution Plan

## Status

- **Status**: design-locked
- **Feature**: `app.android-client`
- **Owner**: `apps/freehand-android` (future app crate / project shell)
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

Android v1 is a native shell that consumes the same protocol truth and the same screen grammar as WebUI.

The shell can later host a WebView for browser-compatible surfaces if needed, but the first implementation plan assumes:

- native Android app shell
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
| `app shell` | activity, navigation, window, safe area | Android framework |
| `protocol client` | HTTP query + SSE subscribe + command ingress | `ui.protocol` |
| `projection store` | turn/status/debug/session projection cache | `ui.protocol` outputs |
| `turn timeline` | cards, tool blocks, collapsible details | `ui.protocol` outputs |
| `top status strip` | current agent, slave summary, connection state | `ui.protocol` + runtime status |
| `drawer` | agent/session switching, quick actions | local UI state |
| `debug surface` | current turn debug detail, read-only | `debug.core` projection via `ui.protocol` |

### 5.2 Data flow modules

| Flow | Source | Sink | Notes |
|---|---|---|---|
| command ingress | user text / action | `ui.protocol` → runtime dispatch | mutation intent only |
| latest snapshot query | `ui.protocol` | projection store | first paint / refresh |
| incremental subscribe | SSE | projection store | no back-pressure semantics in v1 UI |
| agent/session switch | local selection | protocol query/subscribe selector | selection changes view, not truth |
| status animation | turn status changes | card / badge / banner | transient presentation only |

## 6. Mainline Call Skeleton

This is the Android client call skeleton before code lands.

### Request mainline

1. user input enters shell
2. command ingress is validated by `ui.protocol`
3. accepted command is wrapped into owner-routing envelope
4. protocol/runtimes dispatch the command
5. Android client renders the ack and waits for projections

### Response mainline

1. client loads latest snapshot
2. client subscribes to turn/debug/status streams
3. protocol emits incremental projections
4. Android client updates cards, badges, drawer, and debug views
5. terminal state is shown as final projected text, not raw event payload

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

### 7.2 Android local state

Local state may only cover:

- drawer open/close
- selected tab
- current visible filter
- scroll position
- temporary input draft
- transient connection banner

### 7.3 Status mapping

| Protocol state | Android presentation |
|---|---|
| running | animated status dot + `thinking`/`running` pill |
| tool call active | tool block with spinner |
| success | green status pill and completed tool block |
| error | red status pill and failed tool block |
| blocked | amber status pill |
| done | final text only + completed badge |

### 7.4 Agent/session semantics

- current agent is shown in top bar
- other agents show in drawer and collapsed strip
- switching agent/session changes selection, not truth
- slave turns may appear as a substream card or detail page, but remain the same protocol truth

## 8. Implementation Order

1. lock Android shell routes and navigation surface
2. build protocol client and projection store
3. render top status strip, drawer, and timeline
4. bind command ingress and SSE subscribe
5. add debug / tool detail projection
6. add lifecycle reconnect and foreground/background handling
7. add native bridge only where system integration requires it

## 9. Testing Strategy

See `docs/testing/app.android-client.md` for the actual test plan.

The minimum validation stack before code closeout:

- projection mapping tests
- component black-box tests
- protocol replay tests
- route/render smoke tests for the self-contained mock
- workspace gates from `xtask`

## 10. Deliverables

- Android client feature entry
- Android client function map
- Android client test design
- self-contained mock preview
- later: app shell skeleton and protocol client skeleton

