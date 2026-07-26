# Mobile WebUI Modular Rewrite Plan

## Objective

Rewrite the mobile WebUI implementation from a giant coupled script into an explicit route/surface/module architecture, then implement the locked mobile Home dashboard and portrait-adaptive surfaces from `docs/design/mobile-webui-ui-tree.md`.

The outcome is not another patch on `apps/freehand-server/assets/webui.js`. The outcome is a modular WebUI where each entry owns its own UI surface, controls, route edges, verification contract, and owner-truth boundary.

## Acceptance Standards

A build is acceptable only when all of these are true:

1. `webui.js` is reduced to a bootstrap/thin shell or a temporary compatibility entry that delegates to modules.
2. Home dashboard is concise and closed:
   - four corner entries stay outside the center dashboard.
   - center body shows only dashboard summary, `正在运行`, and `历史会话`.
   - history buckets are exactly `今天`, `过去一周`, `所有更早的`.
   - one session occupies exactly one visible row.
   - each row uses clipped/ellipsis text and an action menu, not multi-line cards.
   - Home supports owner-backed session CRUD controls without browser-local session truth.
3. Selecting any session enters `SessionDetail(session_id)` and hides Home dashboard panels.
4. Each UI surface is implemented as a separate module with explicit controls and edges.
5. All route transitions go through a route/edge controller; controls do not mutate arbitrary global route/session state directly.
6. Every route adapts to portrait by viewport width plus height/width ratio, not by device name or width only.
7. Tools/Timer/Settings/Search/New are portrait-safe pages/sheets with sticky close/back and no document-level horizontal overflow.
8. Runtime/session/task/tool/timer/config truth remains owner-backed through ADP/protocol projections and commands. No browser-local fallback truth is introduced.
9. Online browser proof on S-profile passes for phone portrait, tall phone, phone landscape, tablet portrait, and desktop regression.
10. Android WebView proof is run if ADB is reachable; otherwise the missing true-device proof is reported as an explicit blocker/gap.

## Scope And Boundary

### In Scope

- WebUI front-end module architecture under `apps/freehand-server/assets/`.
- WebUI HTML shell changes in `apps/freehand-server/src/page.rs` only when required by module loading or stable anchors.
- CSS split or scoped component CSS if needed to prevent portrait overflow and giant cross-surface rules.
- Home dashboard route and surface.
- SessionDetail route and selected-session route isolation.
- ToolsRegistry portrait-safe surface.
- TimerDashboard / Settings / SessionSearch / NewSession route boundaries, even if the first pass keeps some internal rendering delegated during migration.
- Explicit UI edge manifest and verifier coverage.
- Docs/function-map/mainline/test-design updates for changed architecture.

### Out Of Scope

- New runtime semantics.
- New provider semantics.
- New task lifecycle semantics.
- New timer semantics beyond consuming existing owner projections/commands.
- New tool execution semantics.
- Native Android UI rewrite.
- Release/production deployment unless separately requested.
- Cleaning unrelated dirty files.

## Canonical Inputs

Read these before implementation:

1. `AGENTS.md`
2. `CACHE.md`
3. `MEMORY.md`
4. `note.md`
5. `docs/resource-maps/core.json`
6. `docs/architecture/feature-map.md`
7. `docs/function-maps/app.webui-smoke.md`
8. `docs/mainline-calls/app.webui-smoke.json`
9. `docs/testing/app.webui-smoke.md`
10. `docs/design/mobile-webui-ui-tree.md`
11. `docs/design/mobile-webui-ui-tree.manifest.json`
12. `.agents/skills/freehand-dev/SKILL.md`

## Design Principles

1. Route first, component second.
   - Home, SessionDetail, Tools, Timer, Settings, Search, and New are separate surfaces.
   - A surface never directly reaches into another surface's private state.

2. Explicit edge graph.
   - Every transition has `from`, `event`, `to`, `required payload`, `allowed state effects`, and `forbidden side effects`.
   - Buttons dispatch edge events; they do not rewrite route/session state directly.

3. Owner truth only.
   - UI reads ADP/protocol owner projections.
   - UI submits owner commands.
   - UI may store transient UI state only: open menu, selected filter, pending dialog state, route intent.

4. One primary surface in portrait.
   - Portrait cannot show Home dashboard plus selected transcript at the same time.
   - Modals/sheets must have sticky close/back.

5. Home is a dashboard.
   - One session equals one row.
   - No default expanded transcript, Worker list, task history, debug data, schema, or raw ids.

6. Rewrite by strangler migration, not big-bang breakage.
   - Introduce module loader and route controller first.
   - Move one surface at a time behind tests.
   - Keep behavior owner-backed during migration.

7. No fallback.
   - If a projection/command is missing, expose explicit unsupported/error state and fix owner path if in scope.
   - Do not synthesize truth in browser.

## Target Module Architecture

Target structure:

```text
apps/freehand-server/assets/webui/
├── bootstrap.js
├── app-shell/
│   ├── adp-client.js
│   ├── app-state.js
│   ├── route-controller.js
│   ├── edge-registry.js
│   ├── layout-shape.js
│   └── render-root.js
├── surfaces/
│   ├── home-dashboard/
│   │   ├── index.js
│   │   ├── view.js
│   │   ├── model.js
│   │   ├── controls.js
│   │   ├── edges.js
│   │   └── contract.js
│   ├── session-detail/
│   │   ├── index.js
│   │   ├── view.js
│   │   ├── transcript.js
│   │   ├── composer.js
│   │   ├── agent-sheet.js
│   │   ├── controls.js
│   │   ├── edges.js
│   │   └── contract.js
│   ├── tools-registry/
│   ├── timer-dashboard/
│   ├── settings/
│   ├── session-search/
│   └── new-session/
└── shared/
    ├── dom.js
    ├── ui-kit.js
    ├── time-buckets.js
    ├── status-labels.js
    ├── owner-projections.js
    ├── text-overflow.js
    └── assertions.js
```

Compatibility path during migration:

```text
apps/freehand-server/assets/webui.js
```

may temporarily import `webui/bootstrap.js` and expose required legacy test hooks, but must not continue growing as the implementation owner.

## Edge Contract

Create a machine-readable edge registry, preferably:

```text
apps/freehand-server/assets/webui/app-shell/edge-registry.js
```

and mirror the stable route graph in docs/test expectations.

Minimum edge shape:

```js
{
  id: 'home.open_session',
  from: 'home_dashboard',
  event: 'session.open',
  to: 'session_detail',
  requires: ['session_id'],
  allowedEffects: ['set_route', 'pin_selected_session', 'query_session_turns'],
  forbiddenEffects: ['mutate_session_truth', 'clear_unrelated_surface_state', 'synthesize_worker_session_id'],
}
```

Required edges:

| Edge | Trigger | Required payload | Result |
| --- | --- | --- | --- |
| `root.open_home` | launch/back/home | none | Home route visible |
| `home.open_session` | tap session row | `session_id` | SessionDetail route, Home hidden |
| `home.rename_session` | row action | `session_id`, `title` | owner command, then QuerySessionList |
| `home.delete_session` | row action | `session_id` | confirmation -> owner command -> QuerySessionList |
| `home.archive_session` | row action | `session_id` | owner command if supported |
| `home.open_search` | corner search | none/query | Search surface |
| `home.open_new` | corner new | optional kind/cwd | New surface |
| `root.open_tools` | corner tools | none | ToolsRegistry surface |
| `root.open_timer` | corner timer | none | TimerDashboard surface |
| `root.open_settings` | corner settings | optional page | Settings surface |
| `session.back_home` | back/home | none | Home route visible, selected session preserved |
| `session.submit` | composer submit | text/attachments/session_id | owner command, pending receipt in session scope |
| `session.open_agent_sheet` | agent/status tap | session_id | scoped Agent sheet only |
| `session.open_worker_session` | Worker task tap | `worker_session_id` from TaskBoard | SessionDetail for Worker session |
| `search.open_result` | search result tap | parent `session_id` | SessionDetail route |
| `new.created` | CreateSession receipt | `session_id` | SessionDetail route after QuerySessionList |
| `tools.refresh` | refresh | none | QueryToolRegistry only |
| `timer.refresh` | refresh | none | QueryTimerList only |
| `settings.navigate` | settings card/back | `page_id` | Settings stack page only |

## Surface Contracts

### HomeDashboard

Owner input:

- `QuerySessionList`
- scoped lifecycle projections when already loaded:
  - TaskBoard
  - AgentBoard
  - TimerList
  - Master attention/retry projection if available

Private transient state:

- selected filter
- row action menu open id
- multi-select ids, if batch action is active
- confirmation dialog state

Render contract:

```text
HomeDashboard
├── compact summary header
├── Running list
│   └── one row per running session
└── History list
    ├── 今天
    ├── 过去一周
    └── 所有更早的
```

Row contract:

- one session = one visible row
- no wrapping
- title/time/status/summary/counts/actions all fit the same row
- overflow ellipsis
- tap row -> `home.open_session`
- action menu -> CRUD edges

Forbidden:

- transcript snippets longer than one clipped summary
- full Worker child list
- full task/event/debug/tool data
- raw ids outside explicit debug/detail mode
- local CRUD truth

### SessionDetail

Owner input:

- pinned `QuerySessionTurns(session_id)`
- selected session summary
- selected-session scoped TaskBoard/AgentBoard/Timer truth

Private transient state:

- composer draft
- scroll lock/intent
- pending submit id
- opened agent sheet
- opened relationship panel

Render contract:

- Home dashboard hidden
- transcript and composer dominate
- scoped Agent sheet only
- no global running/history lists in body

### ToolsRegistry

Owner input:

- `QueryToolRegistry`

Private transient state:

- expanded tool detail ids
- refresh in-flight

Render contract:

- portrait-safe page/sheet
- sticky close/back and refresh
- compact summary
- one card/row per tool
- examples/guidance/schema collapsed by default on phone
- long code/schema blocks scroll or wrap internally

Forbidden:

- executing tools
- rendering session-specific tool turns
- desktop modal clipped on phone
- document-level horizontal overflow

### TimerDashboard

Owner input:

- `QueryTimerList`
- `ScheduleTimer` / `CancelTimer` receipts

Render contract:

- route/sheet independent from Home
- active/running/terminal timers from owner projection
- no browser-local timer truth

### Settings

Owner input:

- `QueryConfigStatus`
- config command receipts
- Android bridge status only where relevant

Render contract:

- stack navigation
- first level only: `模型`, `智能体运行时`, `连接`, `可观测性`, `外观`, `关于`
- child details under owner group

### SessionSearch

Owner input:

- `QuerySessionSearch`

Render contract:

- owner-backed full-text/session search
- Worker hits nested under parent sessions only
- result tap -> `search.open_result`

### NewSession

Owner input:

- `CreateSession`
- `QuerySessionList` after receipt

Render contract:

- create conversation
- create cwd-bound task session where supported
- no random verifier spam; test hooks must be explicit and production-inert

## Implementation Phases

### Phase 0 — Design and Gate Setup

1. Confirm `docs/design/mobile-webui-ui-tree.md` and manifest include:
   - Home dashboard rules
   - one-session-one-row
   - `今天` / `过去一周` / `所有更早的`
   - adaptive portrait layout
   - module/edge requirement
2. Add or update function map/test design to reference this plan and the edge registry.
3. Add static tests/gates that reject:
   - direct surface-to-surface private imports
   - route mutation outside route controller
   - new giant `webui.js` logic growth if possible
   - missing edge entry for surface controls

### Phase 1 — Shell Extraction

1. Create `assets/webui/bootstrap.js`.
2. Create `app-shell/adp-client.js` and move ADP request/subscribe primitives.
3. Create `app-shell/app-state.js` for shared app state boundaries.
4. Create `app-shell/layout-shape.js` for width + height/width ratio classification.
5. Create `app-shell/route-controller.js` and `edge-registry.js`.
6. Keep legacy `webui.js` importing bootstrap or delegating to new shell.
7. Preserve existing production behavior before surface migration.

Validation:

- JS syntax checks for new modules.
- Existing mobile UI online verifier still passes or records only expected not-yet-migrated gaps.
- No route behavior regression.

### Phase 2 — HomeDashboard Rewrite

1. Implement `surfaces/home-dashboard`.
2. Build Home model from owner projections:
   - running sessions
   - history buckets
   - needs-user / terminal / stale classification
3. Implement one-row session row component.
4. Implement row action menu and owner-backed CRUD edges.
5. Ensure selecting row enters SessionDetail and hides Home.
6. Update verifier to assert:
   - one row per session
   - fixed history buckets
   - no multi-line cards
   - CRUD controls visible and owner-backed
   - no browser-local CRUD state

Validation:

- unit/static DOM tests if available.
- browser online Home verifier against S-profile.
- no horizontal overflow matrix.

### Phase 3 — SessionDetail Rewrite

1. Implement `surfaces/session-detail`.
2. Pin selected session query to route param.
3. Move transcript rendering and composer into SessionDetail module.
4. Move current-session Agent sheet / relationship panel into scoped module.
5. Implement SessionDetail edges:
   - back home
   - submit
   - open agent sheet
   - open Worker session via projected `worker_session_id`
6. Ensure late transcript responses for previous sessions are discarded.

Validation:

- selecting every Home row hides Home and shows correct transcript.
- Android/back simulation returns Home.
- Worker child navigation uses projected ids only.

### Phase 4 — ToolsRegistry Rewrite

1. Implement `surfaces/tools-registry`.
2. Render from `QueryToolRegistry` only.
3. Convert desktop modal into portrait-safe page/sheet.
4. Sticky close/back/refresh.
5. Collapse schema/examples/guidance by default on phone.
6. Internal scrolling/wrapping for long code/schema/prose.

Validation:

- DOM rows match ADP registry.
- phone portrait screenshot has no clipped desktop modal.
- no document-level horizontal overflow.
- no sessions/tasks/tools are created.

### Phase 5 — Timer / Settings / Search / New Boundaries

1. Move each entry into its own surface module.
2. Route controls through edge registry.
3. Preserve owner-backed behavior already implemented.
4. Remove cross-surface private state coupling.
5. Add smoke checks for each surface open/close/back/refresh path.

Validation:

- existing online verifiers for Timer, Settings/model groups/provider registry, Search, New, Diagnostics still pass or are updated to new module anchors without semantic weakening.

### Phase 6 — CSS / Layout Split

1. Split CSS by shell/shared/surface if practical.
2. Add portrait rules for every surface.
3. Add overflow rules for one-row Home and Tools schema/code blocks.
4. Keep desktop regression intact.

Validation:

- viewport matrix:
  - phone portrait
  - tall phone
  - phone landscape
  - tablet portrait
  - tablet landscape
  - desktop large
- no horizontal overflow in all routes.

### Phase 7 — Remove Dead Legacy

1. Remove migrated legacy functions from giant `webui.js`.
2. Ensure `webui.js` is bootstrap-only or delete it if route imports are updated.
3. Remove duplicate CSS and obsolete selectors.
4. Update server asset smoke tests to require modular assets.
5. Update maps/docs/tests/memory.

Validation:

- static import/gate checks.
- no stale legacy function symbols unless intentionally wrapped.
- full focused WebUI test matrix.

## File Plan

Likely touched files:

- `apps/freehand-server/src/page.rs`
- `apps/freehand-server/src/assets.rs`
- `apps/freehand-server/src/lib.rs`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/assets/webui.css`
- new `apps/freehand-server/assets/webui/**`
- `scripts/verify-webui-mobile-ui-tree-online.mjs`
- `scripts/verify-webui-tools-registry-online.mjs`
- possibly new `scripts/verify-webui-modular-edges-online.mjs`
- `docs/design/mobile-webui-ui-tree.md`
- `docs/design/mobile-webui-ui-tree.manifest.json`
- `docs/function-maps/app.webui-smoke.md`
- `docs/mainline-calls/app.webui-smoke.json`
- `docs/testing/app.webui-smoke.md`
- `docs/wiki/app.webui-smoke.md`
- `.agents/skills/freehand-dev/SKILL.md`
- `MEMORY.md`
- `note.md`

Do not touch unrelated Android/config/runtime dirty files unless a later phase explicitly requires Android WebView packaging/proof.

## Risk Matrix

| Risk | Cause | Mitigation |
| --- | --- | --- |
| Big-bang rewrite breaks live UI | moving too much at once | strangler migration; route shell first; one surface per phase |
| module split becomes fake while globals remain | new files call old globals freely | edge registry + import/static checks + owner boundaries |
| Home becomes a dump again | rows include transcript/task/debug details | one-session-one-row gate and Home forbidden list |
| stale running state persists | active_turn_id or ToolPending copied into Home running | owner-projection classification; waiting-user not running |
| Tools still clips in phone portrait | modal width and pre blocks | portrait page/sheet; sticky header; internal scroll/wrap |
| verifiers weaken semantics | selector-only checks | compare DOM to ADP owner projection and route edge state |
| Android WebView differs from browser | WebView sizing/safe area | true-device proof when ADB reachable; otherwise explicit gap |
| unrelated dirty files get staged | existing workspace dirt | targeted add/commit only owned files |

## Verification Plan

### Static / Local

- `node --check` for all touched/new JS verifier and asset modules.
- `python3 -m json.tool` for any JSON manifest if added/changed.
- `cargo fmt --check`.
- `cargo test -p freehand-server --lib -- --nocapture --test-threads=1`.
- focused UI protocol/runtime tests only if contracts change.
- `cargo run -p xtask -- mainlines check`.
- `cargo run -p xtask -- gates check`.
- `git diff --check`.

### Browser Online

Run against S-profile / production daemon after building and service-scoped restart when implementation changes are served:

- mobile UI tree verifier:
  - Home dashboard one-row/buckets/no-overflow/session route split.
- tools registry verifier:
  - owner projection rows match DOM, portrait no overflow.
- session detail verifier:
  - selecting each session hides Home and pins transcript.
- modular edge verifier:
  - each control dispatches an allowed edge and forbidden direct state mutations are absent from observable behavior.
- existing regressions:
  - Search
  - New session
  - Timer
  - Settings/provider/model group/diagnostics

### Android

If ADB device is reachable/unlocked:

- install current APK only if WebUI asset packaging or Android bridge changed.
- run `apps/freehand-android/scripts/verify-device-ui.sh <serial>`.
- capture screenshots for Home, SessionDetail, Tools, Back behavior.

If ADB is unavailable:

- report Android true-device proof as explicit gap.
- do not claim Android acceptance.

## Implementation Order

1. Refresh MemoryPalace/resource map/function map/test design before coding.
2. Add UI module/edge architecture docs and map updates.
3. Create shell modules and edge registry.
4. Migrate HomeDashboard.
5. Migrate SessionDetail.
6. Migrate ToolsRegistry.
7. Migrate remaining surfaces and CSS/layout.
8. Remove dead legacy from `webui.js`.
9. Run local tests/gates.
10. Run S-profile online browser proof.
11. Run Android true-device proof if available.
12. Update memory/skill/docs and commit targeted files only.

## Definition Of Done

- `webui.js` is no longer the giant owner of route/surface/control logic.
- All major surfaces are separate modules with explicit edges.
- Home matches Jason's locked dashboard contract:
  - four corner entries
  - running sessions
  - history buckets `今天` / `过去一周` / `所有更早的`
  - one session per row
  - CRUD through owner commands
- Selecting a session enters one-session detail and hides Home dashboard.
- Every route adapts to portrait by height/width ratio.
- Tools registry is phone-safe and owner-backed.
- Verifiers prove owner projection, route edges, no overflow, and no browser-local fallback truth.
- Docs/function maps/mainline/test design/wiki/memory are synchronized.
- Only owned files are committed; unrelated dirty files remain untouched.
