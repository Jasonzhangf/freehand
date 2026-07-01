# WebUI Architecture Review

**Date**: 2026-07-01
**Scope**: `apps/freehand-server/assets/webui.js` (2249 lines), `webui.css` (1253 lines), `apps/freehand-server/src/page.rs` (154 lines)
**Comparand**: Reasonix v2 Wails Desktop + React TSX
**Related docs**: `docs/design/multi-platform-ui-architecture.md`, `docs/design/ui-protocol-design.md`, `docs/architecture/feature-map.md`

## Current State

| Component | Lines | Technology |
|-----------|-------|-----------|
| HTML template | Rust `concat!()` in `page.rs` | Hardcoded string macro |
| Client JS | `webui.js` 2249 | Pure JS, no framework |
| Client CSS | `webui.css` 1253 | Flat CSS + custom properties |
| Theme | `theme.css` | Light/dark CSS class toggle |

## Issue 1: Rendering Layer Has No Separation

All JS lives in one flat file. No DOM diff, no virtual DOM, no view templates. Every state change calls `messageList.replaceChildren()` — a full rebuild.

**Evidence**:

- `renderMessages()` is triggered by ~8 different event sources (setInterval 1s poll, submit result, query return, bulk operations, debug toggle, session select, conversation switch, sample buttons). No traceability for which source caused the re-render.
- `renderAll()` iterates 8 render functions sequentially, all DOM operations.
- The 1s `setInterval(() => { renderMessages(); renderTurnMeta(); renderCommandStatus(); }, 1000)` causes flickering on every poll cycle because `replaceChildren()` destroys and recreates all DOM nodes.

**Consequence**: No incremental update. Each turn card, tool call card, and status indicator is rebuilt from scratch every render cycle. On mobile WebView this translates to wasted battery and jank.

## Issue 2: State Layer Leaks Into DOM

`state` is a flat global object mixing protocol data, display state, local preferences, and timing tracking:

```
state = {
  sessions: [],         // protocol
  sessionTurns: [],     // protocol
  selectedSessionId: null,  // display
  selectedSessionIds: new Set(),  // display
  turn: null,               // protocol
  debug: null,              // protocol
  checkpoints: [],          // protocol
  submittedUserText: null,  // input
  pendingUserInput: null,   // input
  submitInFlight: false,    // input
  modelRequestStartedAt: null,  // timing (derived)
  toolTimings: new Map(),       // timing (derived)
  debugDetailsVisible: false,   // UI preference
  draftSessionId: null,         // local
  forceScrollToBottom: false,   // local
  selectedCwd: "",              // persistent
  adpStatus: "",                // connection
  adpFailure: null,             // connection
}
```

No domain boundaries. No Action-Dispatch mapping. Multiple event sources write to the same fields without coordination.

## Issue 3: Inconsistent Message DOM Structure

CSS classes don't form a clean hierarchy:

| Component | CSS class(es) | Notes |
|-----------|---------------|-------|
| Assistant messge | `dialog-block block-body text-kind` | Uses `.dialog-block` + `.block-body` |
| User message | `user-block` within `.dialog-block` | Different structure |
| Turn execution | `execution-block execution-body` | Different container entirely |
| Tool call | `execution-row-tool` within `.execution-body` | No standalone `.tool-call` |
| Tool result | `tool-primary-line` / `tool-secondary-line` | Inline within `execution-row-body` |
| Empty state | `chat-empty-state` | Separate layout, not a card |
| Slave cards | `slave-card` | Outside message list entirely |

In contrast, Reasonix v2 uses a consistent three-layer semantic: `turn-card` → `tool-call` / `tool-result` → detail collapse. Freehand's `.execution-row-tool` mixes tool calls, tool results, and errors in the same DOM level, differentiated only by background color (`.execution-row-tool` vs `.execution-row-final` vs `.execution-row-error`).

## Issue 4: Design Token Mismatch

`docs/design/multi-platform-ui-architecture.md` defines a full token system:

```css
--color-bg-primary: #fafafa;
--color-text-primary: #1a1a1a;
--color-accent: #2563eb;
--radius-md: 8px;
--z-rail: 100;
--z-inspector: 200;
```

But the actual CSS uses a different variable set:

```css
--panel / --canvas / --bg / --rail / --master
--text / --text-soft / --text-strong / --text-faint
--line / --assistant / --user / --tool / --fail
--ok / --warning / --accent / --running / --running-soft
--shadow / --card-head-bg
```

The design doc tokens were **never implemented in CSS**. Dark mode uses hardcoded `rgba()` values per section instead of token overrides. Inspector, sidebar, and conversation region each have their own dark background formula:

```css
body.theme-dark .inspector { background: rgba(15, 19, 18, 0.76); }
body.theme-dark .sidebar { background: rgba(14, 18, 17, 0.72); }
body.theme-dark .conversation-region { background: linear-gradient(...); }
```

## Issue 5: No True SSE Subscription

The design doc specifies:

> Subscribe latest turn: GET `/ui/subscribe/turn/latest` → SSE stream

But the actual implementation uses HTTP polling:

```js
setInterval(() => {
  // ... checks pending states ...
  renderMessages();
  renderTurnMeta();
  renderCommandStatus();
}, 1000);
```

The ADP stack already has `adp_subscribe("subscription", ...)` backend path (used in daemon tests). The JS client simply never implemented `EventSource` — every "live" update is a 1-second poll cycle.

## Issue 6: Not Responsive

Current layout is a fixed desktop grid:

```css
.app-shell {
  grid-template-columns: 56px 254px minmax(0, 1fr) 244px;
}
```

Only one breakpoint exists. The design doc defines three tiers:
- Desktop ≥1180px: Rail + Conversation + Inspector
- Tablet 880-1180px: Collapsed Rail, Inspector as bottom drawer
- Mobile <880px: Bottom nav, no rail, no persistent inspector

None of the mobile/tablet layouts are implemented. On a 375px Android WebView viewport, all four columns render simultaneously, making the UI unusable.

## Issue 7: Rust HTML Template Hard-Coded

`page.rs` (154 lines) generates the full HTML shell in a Rust `concat!()` macro:

```rust
pub fn render_webui_smoke() -> String {
    concat!(
        "<!DOCTYPE html>",
        "<html lang=\"zh-CN\">",
        // ... full DOM in string ...
        "</main>",
        "<script type=\"module\" src=\"/assets/webui.js\"></script>",
        "</body>",
        "</html>"
    ).to_owned()
}
```

All endpoint URLs (`data-adp-endpoint`, `data-turn-query`, `data-turn-subscribe`, etc.) are hardcoded. Sidebar buttons (WS/ND/RP/DG) are hardcoded. Changing any button text, adding a page element, or adjusting layout requires a Rust recompile. No template engine, no server-side rendering path for dynamic data.

## Issue 8: No Accessibility Baseline

- Composer textarea uses `placeholder` as its only label — no `aria-label` or associated `<label>`.
- Session list items use `aria-label="Select session ${session_id}"` on checkboxes, but the session title is conveyed only visually.
- Tool call cards have no ARIA live regions for streaming status changes.
- Theme toggle buttons use `aria-label="Theme switch"` but without `aria-pressed` state.
- No keyboard navigation for session selection, tool card expansion, or checkpoint rewind buttons.

## Comparison: Reasonix v2 Desktop

| Dimension | Freehand WebUI | Reasonix v2 Desktop |
|-----------|---------------|---------------------|
| Frontend stack | Pure JS (no framework) | Wails + React + TSX components |
| Rendering | `replaceChildren()` full rebuild | React virtual-DOM diff |
| State management | Flat global object | Component state + props |
| Real-time updates | setInterval 1s HTTP poll | Wails runtime event channel (`agent:event`) |
| HTML template | Rust `concat!()` macro | JSX components |
| Responsive | Fixed desktop grid (one breakpoint) | CSS responsive + mobile/tablet/desktop |
| Design tokens | Partial CSS vars, mismatched from design doc | CSS Modules + theme system |
| Tool preview | `plan_*` in freehand-tools (Rust) + backend API | Wails Previewer interface → diff card in frontend |
| Accessibility | Minimal | Wails + standard web patterns |

## Recommendations (Priority Order)

### P0 — Split Rendering, Protocol, and State Layers

Refactor `webui.js` into three directories:

```
webui/
  data/
    state.js       — State domain definitions + mutations
    projection.js  — Protocol data → UI projection helpers
  view/
    render.js      — Pure functions: state → DOM
    components.js  — Reusable element creators (turn-card, tool-call, etc.)
  controller/
    events.js      — Event bindings + dispatchers
    adp.js         — ADP command/query/subscribe wrappers
```

Goal: `renderMessages()` is a pure function `(projection) → DocumentFragment`, never reads global `state` directly.

### P1 — Move HTML Template to Static File

Replace `page.rs` `concat!()` with a static `index.html` served by `freehand-server`. Rust only proxies API, never generates HTML. No recompile for layout changes.

### P2 — Implement True SSE

Remove the 1-second `setInterval` poll. Add `EventSource` listener to `/ui/subscribe/turn/latest`. The ADP backend already has the subscription path — the JS side just needs to consume it.

### P3 — Domain-Group the State Object

Split `state` into clear domains:

```js
state = {
  connection: { adpStatus, adpFailure },
  session: { list: [], selectedId: null, ... },
  turn: { current: null, timeline: [], ... },
  ui: { debugVisible: false, theme: 'light', ... },
  sessionUI: { selection: new Set(), bulkMode: false, ... },
}
```

Write rule: `state.session.*` only written by `applyAdpQueryResult()`; `state.ui.*` only written by user interaction.

### P4 — Align CSS Variables With Design Doc

Decide whether the design doc token system or the actual CSS variable set is the source of truth. Implement whichever wins. Must have a single `--color-bg-primary` path that dark mode overrides at one point, not per-section hardcoded `rgba()`.

### P5 — Pipeline Message Rendering

Current chain: `derivePublicConversation()` → `normalizePublicConversation()` → `conversationItemsForTurn()` → `turnExecutionCard()` → `executionRow()`.

Add a typed intermediate model:

```js
// Single projection item for rendering
{
  kind: "user" | "assistant" | "tool_call" | "tool_result" | "terminal" | "error",
  role: "user" | "assistant" | "system",
  text: string,
  tool?: ToolDisplayProjection,
  status: "pending" | "running" | "success" | "failed",
  timestamp: number,
  metadata: { turn_id, tool_call_id, ... }
}
```

All render functions consume this single model. Never build DOM directly from backend API responses.

### P6 — Gradual Component Extraction

Not a framework migration (no React/Vue). Start with pure-function element creators:

```
view/create-turn-card.js     — createTurnCard(projection) → HTMLElement
view/create-tool-card.js     — createToolCard(toolProjection) → HTMLElement
view/create-status-bar.js    — createStatusBar(status) → HTMLElement
```

Each in its own file. No side effects — input = projection, output = DOM subtree.

### P7 — Implement Responsive Breakpoints

Three-tier layout from `docs/design/multi-platform-ui-architecture.md`:
- ≥1180px: current four-column grid
- 880-1180px: collapsed rail, inspector as bottom drawer
- <880px: bottom nav, no rail/inspecor, stacked conversation

CSS `@media` queries only. No JS resize listeners for layout.

## Implementation Effort Estimate

| Phase | Scope | Days |
|-------|-------|------|
| 1 | `page.rs` → `index.html` | 1-2 |
| 2 | State domain separation + proxy getters | 2-3 |
| 3 | EventSource subscription | 1-2 |
| 4 | Render pipeline pure functions | 3-4 |
| 5 | Responsive breakpoints | 2-3 |
| 6 | CSS token alignment | 1-2 |
| Total | | 10-16 |

## Files Referenced

- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/assets/theme.css`
- `apps/freehand-server/src/page.rs`
- `docs/design/multi-platform-ui-architecture.md`
- `docs/design/ui-protocol-design.md`
- `crates/freehand-tools/src/lib.rs` (tool display projection)
- `crates/freehand-blocks/src/tool_display.rs` (ToolDisplayKind classification)
