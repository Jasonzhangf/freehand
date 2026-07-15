# Mobile Agent Dashboard Implementation Plan

## Goal

Implement the confirmed mobile WebUI direction from
`docs/prototypes/freehand-agent-dashboard/index.html` in the production WebUI,
while preserving Freehand's protocol-only browser boundary and owner-backed
runtime projections.

## Acceptance Criteria

1. Phone and tablet portrait layouts are conversation-first.
2. The mobile session list is a left drawer, opened by the header menu and the
   existing content-area swipe gesture.
3. The mobile agent dashboard is a compact top status strip plus an expandable
   bottom sheet.
4. Worker sessions render as child rows under their owning Master session using
   TaskBoard `parent_session_id` truth.
5. The compact Header shows current-session running Agent count, delegated task
   count, and active task title.
6. The first tap opens only the current session's delegated child-task list;
   tapping one task selects the protocol-projected Worker session and opens its
   persisted conversation. Master evaluation/history/control remain in runtime
   truth and desktop inspector surfaces, not the first mobile task sheet.
7. Visual system follows the confirmed prototype direction:
   - black / white / gray base
   - minimal blue for active/running/evaluation cues
   - minimal green for accepted/OK cues
   - gray for neutral/review/rework states unless existing app semantics require
     an error color
8. Existing WebUI, ADP, session, task, turn, and runtime truth owners remain
   unchanged.
9. No fallback behavior is added. Missing owner projection should surface a
   visible unavailable/empty state, not browser-invented truth.
10. Online verification proves the layout and projection against the real
    daemon-backed WebUI, not only static HTML.

## Scope

In scope:

- `apps/freehand-server/assets/webui.css`
- `apps/freehand-server/assets/webui.js`
- `apps/freehand-server/src/lib.rs` only for existing asset/smoke coverage
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/design/webui-console-proposal.md` or a new targeted design note if the
  production implementation changes durable UI design truth
- `MEMORY.md` and `note.md` after verified implementation

Out of scope:

- Changing `freehand-ui-protocol` unless the required mobile surface cannot be
  expressed through existing TaskBoard, AgentBoard, EventInbox, TaskHistory, and
  WorkerControl projections.
- Changing runtime Master/Worker evaluation semantics.
- Creating browser-local task/session/agent truth.
- Rewriting WebUI in a new framework.
- Changing Android native shell behavior except where WebView viewport
  verification requires it.
- Implementing desktop redesign beyond the mobile Agent Dashboard integration
  needed for shared component structure.

## Required Source Truth

Read before implementation:

- `docs/prototypes/freehand-agent-dashboard/index.html`
- `MEMORY.md`
- `note.md`
- `docs/resource-maps/core.json`
- `docs/architecture/feature-map.md`
- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/design/multi-platform-ui-architecture.md`
- `docs/design/webui-console-proposal.md`
- `.agents/skills/freehand-dev/SKILL.md`

## Design Principles

1. Browser UI rearranges owner-backed projections; it does not create semantic
   truth.
2. Mobile layout changes must preserve selected session, transcript, composer
   draft, pending submit, scroll anchor, and lifecycle timers.
3. The conversation remains the primary surface on phone.
4. The compact agent strip is summary-only; the bottom sheet is a current-session
   delegated-task navigator, and the next level is the selected Worker transcript.
5. Session drawer and agent sheet are presentation controls. Opening/closing
   them must not mutate ADP/session/task truth.
6. Internal protocol labels, raw task ids, runtime turn ids, and ADP terms stay
   hidden from user-facing mobile text unless debug details are explicitly
   enabled.
7. Styling should reuse existing WebUI state classes where possible; add tokens
   only where the confirmed grayscale direction requires them.

## Technical Plan

### Slice 1: Locate Existing Mobile Surfaces

- Trace current mobile drawer, session hierarchy, lifecycle observer,
  AgentBoard, TaskBoard, TaskHistory, and WorkerControl render paths.
- Identify existing render functions and CSS selectors that already satisfy part
  of the prototype.
- Confirm no implementation needs a new protocol owner before editing.

### Slice 2: Agent Summary Strip

- Add or adapt a mobile-only compact strip below the app bar.
- Strip content should summarize:
  - current-session running Agent count
  - current-session delegated task count
  - active task title
- Strip opens the agent bottom sheet.
- Strip must be absent or low-emphasis on desktop if existing desktop lifecycle
  observer already owns the richer display.

### Slice 3: Delegated Task Sheet

- Implement a mobile/tablet portrait bottom sheet containing only the selected
  session's delegated child tasks.
- Each task row derives from owner-backed TaskBoard projection and consumes the
  projected canonical Worker session id.
- Task tap closes the sheet, selects the Worker session, immediately refreshes
  `QuerySessionTurns`, and renders its conversation.
- Missing projection data should render as explicit unavailable/empty rows.

### Slice 4: Session Drawer Alignment

- Preserve existing mobile session drawer behavior and right-swipe opening.
- Ensure Worker child sessions remain indented under the parent Master session.
- Ensure drawer open/close does not affect transcript, composer, or lifecycle
  timers.
- Keep user-facing labels free of raw ADP/runtime/internal ids.

### Slice 5: Visual System

- Shift mobile surfaces toward the confirmed black/white/gray design.
- Use blue only for active/running/evaluating cues.
- Use green only for accepted/OK/completed cues.
- Keep review/rework neutral gray unless the existing product uses red for a
  true failure/blocker distinction.
- Remove mobile left-edge state strips or thick color bars where they consume
  horizontal space.

### Slice 6: Verification + Documentation

- Update `docs/function-maps/app.webui-smoke.md` and
  `docs/testing/app.webui-smoke.md` if render behavior, tests, or owner-backed
  projection expectations change.
- Update local project memory after verified implementation.
- Do not claim complete until online WebUI evidence passes.

## Verification Matrix

Static / local:

- `node --check apps/freehand-server/assets/webui.js`
- relevant `cargo test -p freehand-server ...` asset/smoke tests
- `cargo fmt --check`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- `git diff --check`

Online WebUI:

- Restart/use S-profile daemon as appropriate.
- `curl -4fsS http://127.0.0.1:4042/health`
- `freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp`
- Run the WebUI online verifier or extend it to assert:
  - phone portrait
  - tall phone
  - tablet portrait
  - phone landscape if affected
  - desktop regression
  - compact agent strip visible on phone/tablet portrait
  - agent bottom sheet opens/closes
  - session drawer opens/closes
  - Worker child sessions are nested under parent Master session
  - no raw ADP/runtime ids in normal mobile UI
  - composer remains visible and not clipped
  - selected session/transcript/composer state survive drawer/sheet toggles
  - TaskBoard/AgentBoard/TaskHistory/WorkerControl visible text matches service
    truth where those projections are available

Visual evidence:

- Capture screenshots for:
  - phone main conversation
  - phone session drawer
  - phone agent bottom sheet
  - tablet portrait main / sheet
  - desktop regression

## Risks

| Risk | Mitigation |
| --- | --- |
| Browser invents task/agent semantics | Use only owner-backed projection fields; render unavailable if missing |
| Mobile UI becomes dashboard-first | Keep only compact strip on main surface; detailed state goes to sheet |
| Existing drawer/session logic regresses | Preserve current drawer contract and extend tests before visual polish |
| Mobile sheet becomes a dense inspector | Keep it to current-session delegated tasks; task click opens the Worker transcript |
| Styling breaks current accessibility/readability | Keep high contrast grayscale base and minimal semantic accents |
| Static prototype proof is mistaken for product proof | Require online daemon-backed WebUI verifier before completion |

## Implementation Steps

1. Read required source truth and confirm `app.webui-smoke` remains the owner.
2. Map current mobile drawer and lifecycle observer functions/classes.
3. Add tests or verifier assertions for the mobile Agent Dashboard target shape.
4. Implement compact mobile agent strip.
5. Implement delegated-task bottom sheet backed by selected-session TaskBoard projection.
6. Align mobile session drawer and Worker-child hierarchy with the prototype.
7. Apply confirmed grayscale visual tokens.
8. Run local/static checks.
9. Run online WebUI verification and capture screenshots.
10. Update docs and memory with verified evidence and remaining gaps.

## Done Definition

- Production WebUI mobile layout matches the confirmed prototype direction.
- Phone/tablet portrait are conversation-first with drawer + agent sheet.
- Agent Dashboard expresses current-session running/delegated progress and
  navigates Header -> delegated tasks -> Worker conversation -> exact parent
  Master, while sibling Worker tasks remain directly switchable.
- The Agents runtime sheet is navigation/progress only. Configured Worker
  capacity is mutated only from the system Config surface.
- Verified online against daemon-backed WebUI with screenshots and service truth.
- Existing desktop WebUI remains usable and regression-checked.
- Runtime/protocol changes are limited to owner-backed Worker transcript query
  and canonical Worker session projection required by the navigation contract.
