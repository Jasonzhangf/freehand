# Mobile WebUI UI Tree

## Status

- status: design baseline for the next mobile/WebUI implementation pass
- owner feature: `app.webui-smoke`
- protocol truth owners:
  - session and turn truth: `reason.persistence` / `ui.protocol`
  - task lifecycle truth: `task.orchestration`
  - agent lifecycle truth: `agent.lifecycle`
  - timer truth: `runtime.master-worker-loop`
  - tool registry truth: `tool.registry`
  - command/query dispatch: `runtime.ui-command-dispatch`
- machine manifest: `docs/design/mobile-webui-ui-tree.manifest.json`

This document is the UI-tree source for mobile WebUI/Android WebView work.
Implementation must update this document first when changing page ownership,
navigation, route state, or lifecycle display semantics.

## Non-negotiable Product Shape

Mobile has two mutually exclusive primary surfaces:

1. `Home`
   - session dashboard, navigation, and session CRUD management
   - shows global running-session monitor plus static session history
   - does not show one selected transcript as the main body
2. `SessionDetail(session_id)`
   - one selected session's transcript and scoped status
   - does not show the global `正在运行` / `历史会话` home dashboard

If a user taps any session from `Home`, the route changes to
`SessionDetail(session_id)`. The home dashboard must leave the primary body.
Current-session Agent/task status may appear only in the selected-session
Header / Agent sheet, never as the global home running/history dashboard above
the transcript.

## Page Tree

```mermaid
flowchart TD
  Root[Mobile WebUI Root]
  Chrome[Common mobile chrome]
  Home[Home / 会话首页]
  Session[SessionDetail(session_id) / 单会话]
  Settings[Settings / 设置]
  Timer[TimerDashboard / 定时任务]
  Tools[ToolsRegistry / 内置工具]
  Search[SessionSearch / 会话搜索]
  New[NewSession / 新建会话]
  AgentSheet[CurrentSessionAgentSheet / 当前会话 Agent]

  Root --> Chrome
  Root --> Home
  Root --> Session
  Chrome --> Settings
  Chrome --> Timer
  Chrome --> Tools
  Chrome --> Search
  Chrome --> New
  Home --> Session
  Session --> Home
  Session --> AgentSheet
  Search --> Session
  New --> Session
```

## Root Route State

| Route | Body owns | Body must not contain | Valid entry |
| --- | --- | --- | --- |
| `Home` | session dashboard | selected transcript as primary content | app launch, Home/Back from selected session, refresh-error exit |
| `SessionDetail(session_id)` | selected transcript and composer | global home `正在运行` / `历史会话` sections | tapping a session, creating a session, selecting search result, Worker child navigation |
| `Settings(page_id)` | settings stack page | transcript/home dashboard | settings corner entry |
| `TimerDashboard` | timer owner projection | session transcript/home dashboard | timer corner entry |
| `ToolsRegistry` | tool registry projection | session transcript/home dashboard | tools corner entry |
| `SessionSearch` | session search projection | transcript/home dashboard | search corner entry |
| `NewSession` | create-session/task dialog | transcript/home dashboard | new-session corner entry |

Route state is UI state only. It must not mutate session, reason, task, timer,
tool, config, or debug truth.

## Adaptive Layout Contract

Every WebUI surface must adapt from the same route tree by viewport width plus
height/width ratio. Do not treat mobile as "desktop width squeezed down", and
do not key layout only on device name or width.

Required layout inputs:

- viewport width
- viewport height
- height/width ratio
- orientation
- safe-area insets

Required shape classes:

| Shape | Required behavior |
| --- | --- |
| `phone_portrait` / `tall_phone` | one primary surface at a time, single-column body, fixed/safe-area composer, icon chrome, modal/sheet details |
| `tablet_portrait` | one primary surface with more spacing; secondary panes remain sheets/drawers unless explicitly opened |
| `phone_landscape` | compact horizontal layout only when height allows; no clipped modals or hidden close controls |
| `tablet_landscape` / `desktop_large` | multi-pane desktop layout may appear, but route ownership remains identical |

All routes must obey this contract:

- Home rows stay one-line in portrait.
- `SessionDetail` transcript/composer stay usable in portrait.
- Settings, Timer, Tools, Search, and New surfaces become portrait-safe pages
  or sheets with sticky close/back controls.
- Long code/schema/log/prose blocks scroll or wrap inside their own block; they
  must not widen the document.
- Switching layout shape must preserve selected session, draft input, route,
  scroll intent, pending submit, and owner query state.
- Layout state must never create fallback session/task/tool/timer/config truth.

## Common Mobile Chrome

Common chrome exists on all routes:

- top-left: Settings entry
- top-right: Timer entry and Tools entry
- center title: `Freehand`
- bottom-left: New Session entry
- bottom-right: Search entry
- Android/browser Back handling:
  1. blur focused form control
  2. close modal/sheet/drawer
  3. if in `SessionDetail`, return to `Home`
  4. otherwise allow app/browser exit

Common chrome must stay icon-first on phone. Text labels may be accessible names,
but not wide visible toolbar text.

## Home / 会话首页

### Purpose

Home answers: "What sessions exist and which ones are actually running now?"

Home is not a transcript page. It is the session navigation dashboard.
It must be concise enough for repeated daily use: scan by time, understand
status, manage sessions, then enter the selected session.

The Home dashboard is not a place to dump every session detail, every task
event, every Worker child, or every debug field. It is an index and control
surface.

### Structure

```text
Home
├── DashboardHeader / 总览
│   ├── compact counts: running, needs_user, blocked, total history
│   └── optional filter/sort controls
├── RunningSessionsPanel / 正在运行
│   ├── zero-state: 暂无正在运行的会话
│   └── RunningSessionItem* as one-line compact rows
└── SessionHistoryPanel / 历史会话
    ├── 今天
    │   └── SessionHistoryItem* as one-line compact rows ordered by latest activity time
    ├── 过去一周
    │   └── SessionHistoryItem* as one-line compact rows ordered by latest activity time
    └── 所有更早的
        └── SessionHistoryItem* as one-line compact rows ordered by latest activity time
```

### Home Information Density

Home uses one-line summary rows, not expanded content cards.

Each row may show only:

- title
- latest activity time or relative time
- status badge
- short summary clipped to the same line
- optional tiny counts for turns / child tasks / attention
- row actions menu

One session equals one visible row. A row must not wrap into a multi-line card
or expand inline by default. Long title/summary/cwd text is truncated with
ellipsis; full content is available only after entering `SessionDetail` or
opening an explicit action/detail sheet.

Each row must not show:

- full transcript content
- full Worker child list by default
- full task history or event inbox rows
- raw ids unless the row is in debug/detail mode
- long lifecycle prose
- schema, examples, or tool details

Worker child information on Home is summarized as counts/badges. Full child
details live in `SessionDetail` Header / Agent sheet.

### Running Sessions Contract

`正在运行` may include only sessions with live owner evidence:

- active provider/model request for that session
- running/waiting tool activity for that session
- provider retry/failover for that session
- open same-session child task from `TaskBoard`
- active/running timer owner truth for that source session
- active Master attention/retry owner truth that can wake without user input

`等待用户选择` is not running. It is a user-needed terminal or blocked state.
It belongs in `历史会话` with a clear badge such as `等待用户选择`, not in the
`正在运行` panel.

If owner projections have loaded and no live owner exists, `正在运行` must show
the zero-state and zero cards even if stale `active_turn_id` or historical
`ToolPending` text remains in cached session summary.

Running rows are compact. A running row can be tapped to enter
`SessionDetail(session_id)`, where the current turn, child tasks, transcript,
and detailed lifecycle state are shown.

### History Contract

`历史会话` contains persisted top-level Master/user sessions ordered by latest
display time and grouped under exactly three time buckets:

1. `今天`
2. `过去一周`
3. `所有更早的`

These bucket labels are the default Home history hierarchy. Do not add extra
date headings, calendar trees, nested month/year sections, or per-session
expanded group layers unless this design is explicitly revised.

History rows may show status badges:

- `已完成`
- `失败`
- `阻塞`
- `等待用户选择`
- `运行中` only when the same session is also present in `正在运行`

History and running item ids must be disjoint unless the UI intentionally
renders a history row in disabled/linked form; current implementation target is
disjoint lists.

Top-level history must not show internal runtime sessions:

- `worker-task-*`
- `master-lifecycle-*`
- `master-timer-*`

Worker child sessions may render only under their owning Master session through
`UiTaskSnapshotProjection.parent_session_id` and `worker_session_id`.

On Home, Worker child sessions should normally be summarized under the parent
row as counts, not expanded into the same visual hierarchy. Expanding every
child by default turns the dashboard into an unstructured list and is forbidden.

### Home CRUD Management

Home owns session management controls for persisted top-level sessions through
protocol owner paths.

Allowed Home actions:

- select one or more sessions
- open session
- archive session when the protocol owner supports it
- restore archived session when the protocol owner supports it
- delete/remove session through `DeleteSession`
- multi-select for batch archive/delete only when the owner command exists

CRUD controls must be compact:

- primary tap opens the session
- secondary actions live in a row action menu or selection mode
- destructive actions require explicit confirmation
- action receipts re-query `QuerySessionList` before the row changes state

Home must not keep browser-local session CRUD truth. It may keep only transient
selection/menu/dialog state.

Renaming is not a Home row action. A session can be renamed only after entering
`SessionDetail(session_id)`, where the current-session header owns the rename
control and routes it through `RenameSession`.

### Home Grouping And Sorting

Default order:

1. `正在运行` rows: live owner priority, then latest activity time descending
2. `历史会话` buckets in fixed order: `今天`, `过去一周`, `所有更早的`
3. rows inside each bucket: latest activity time descending

Allowed filters/search:

- all
- running
- needs user
- blocked/failed
- completed
- archived when supported

Filters must not replace the main tree with an unrelated search page. The
dedicated `SessionSearch` route remains for full-text owner-backed search.

### Home Simplicity Rules

- Default Home body has one summary header and two lists.
- Lists use rows, not nested cards inside cards.
- One session occupies exactly one row in Home.
- No row may wrap into two or more text lines before explicit detail.
- Details are one tap away in `SessionDetail`, not pre-expanded on Home.
- Dashboard copy must be terse; avoid explanatory paragraphs in the normal
  running/history lists.
- Empty states are one short line plus the relevant action.

## SessionDetail(session_id) / 单会话

### Purpose

SessionDetail answers: "What is happening inside this one session?"

This is where the user continues the conversation, continues drawing/work,
or inspects details. Home is only the dashboard that chooses this surface.
Session-specific management such as renaming the current session also belongs
here, not in the global Home list.

### Structure

```text
SessionDetail(session_id)
├── SessionHeader
│   ├── back/home affordance
│   ├── selected session title/status
│   ├── current-session Agent/Worker summary
│   └── WorkerStatusRail* as compact one-row Worker status entries
├── SessionRelationshipPanel (inline expandable, scoped to this session)
├── Transcript
│   └── TurnCycleCard* in protocol order
├── CurrentSessionAgentSheet (optional)
└── Composer
```

### SessionDetail Rules

- The global Home panels are hidden.
- The transcript is the dominant body.
- Header relationship panel is scoped to the selected session only.
- A failed Worker turn remains historical truth. Only when the same
  TaskBoard-projected Worker session has a later successful turn and the task
  owner status is `closed` may normal presentation relabel the older cycle
  `历史失败 · 后续已恢复`; debug details must retain the original failure. A latest
  failure, a success that precedes the failure, or a non-closed task must remain
  an unrecovered failure.
- Header Worker status rail is scoped to the selected parent session's
  TaskBoard/AgentBoard projections. It shows every current child Worker task as
  one compact row with Worker label, real task status, and duration derived from
  owner timestamps; clicking a row expands only that Worker's details in the
  Header.
- Nonterminal Worker rows refresh TaskBoard owner status while their duration
  clocks tick; the browser must not freeze status text from an old projection.
- Master/Worker dispatch waits are context-isolated. Worker transcripts stay
  separate from the Master transcript until TaskBoard/TaskHistory/EventInbox
  returns typed child result truth; success, failure, blocked, and interrupted
  child outcomes are rigid parent-visible notifications, not browser guesses.
- While a selected Master session is waiting on open child tasks or a source
  timer that can wake the lifecycle, the Composer remains available for new
  user input. The UI must not trap the user in a waiting state.
- Periodic waiting checks belong to runtime Timer/TaskBoard owner truth. The
  WebUI may render active source timers and refresh owner projections, but it
  must not synthesize timer status or child lifecycle state in the browser.
- Agent sheet is scoped to selected session TaskBoard/AgentBoard truth only.
- Selected transcript query is pinned to `session_id`; late responses for older
  selected sessions are discarded.
- No fallback to unrelated global active sessions.
- Worker task taps use `worker_session_id` from TaskBoard projection. The UI
  must not synthesize `worker-task-*` ids.
- Worker row expansion is presentation-only. Opening the Worker transcript is a
  separate explicit action that still uses the TaskBoard-projected
  `worker_session_id`.

## CurrentSessionAgentSheet

This sheet is not the home dashboard.

It answers: "For the selected session, what child Worker tasks / Agents exist?"

It may show:

- current child tasks for selected parent session
- Worker transcript navigation
- sibling Worker switching
- return to exact parent Master
- lifecycle buckets for selected-session tasks

It must not show:

- global TaskBoard history
- unrelated sessions' agents/tasks
- Worker capacity mutation controls; those belong in Settings / Agent Runtime

## ToolsRegistry / 内置工具

### Purpose

ToolsRegistry answers: "Which built-in tools exist, who can see them, and what
schema/examples/guidance are projected by the tool owner?"

It is read-only. It does not execute tools and does not render session-specific
tool turns.

### Phone-first Structure

```text
ToolsRegistry
├── sticky header
│   ├── title: 内置工具
│   ├── close/back
│   └── refresh
├── compact summary
│   └── registry version, total tools, master/worker counts
├── GuidanceOverview (collapsed or compact by default)
└── ToolCard*
    ├── name + short description
    ├── exposure badges: master / worker / hidden
    ├── permission badges: read-only / mutating / implemented
    ├── Examples (collapsed by default on phone)
    ├── Guidance (collapsed by default on phone)
    └── Schema (collapsed by default on phone)
```

### ToolsRegistry Layout Rules

- It must be a phone-first page/sheet, not a desktop modal scaled down.
- Close/back and refresh remain visible at the top while content scrolls.
- Long guidance, examples, and JSON schemas must not widen the page.
- Code/pre blocks either wrap or scroll inside their own block; the document
  itself must have no horizontal overflow.
- On phone, raw schema/example/guidance details are collapsed by default.
- Cards should prioritize tool name, short purpose, visibility, read-only/mutate,
  and implementation state.

## Settings / 设置

Settings is a stack, not a flat dump:

```text
SettingsRoot
├── 模型
│   ├── 模型服务配置
│   ├── 模型服务切换与策略
│   └── 模型组
├── 智能体运行时
│   ├── Worker capacity
│   └── runtime/agent status
├── 连接
│   ├── daemon connection
│   ├── Android APK update
│   └── Android permissions
├── 可观测性
│   ├── diagnostics
│   ├── logs
│   └── debug visibility
├── 外观
└── 关于
```

First-level Settings must show only these six coarse entries. Low-frequency
implementation details must remain child pages.

## TimerDashboard / 定时任务

TimerDashboard is not part of the home body.

It renders timer owner truth:

- active/running timers
- terminal timers
- timer ledger/history
- schedule/cancel commands through protocol owner path

Timers must not be encoded as browser-local session state or task truth.

## SessionSearch / 会话搜索

Search reads owner-backed session search/index projection.

It must not:

- scan browser-local DOM/session state as truth
- create sessions while searching
- promote Worker child sessions into top-level global results

Selecting a search result enters `SessionDetail(session_id)`.

## Lifecycle UI Closure Rules

The UI is closed only when visible state is derived from owner truth and has a
clear exit from stale/error states.

### Status Classification

| UI category | Required owner evidence | Home placement |
| --- | --- | --- |
| `running` | live provider/model/tool, open task, active timer, resumable Master attention/retry | `正在运行` |
| `needs_user` | ToolPending/user choice with no open lifecycle owner | `历史会话` badge |
| `blocked` | terminal blocked/failure truth | `历史会话` badge |
| `completed` | terminal success/closed truth | `历史会话` badge |
| `stale` | old active summary obsoleted by later terminal turn or closed owner projection | not running; normalize to terminal/needs_user |

### Invalidation Rules

Older active-looking UI state is invalid if any of these later owner facts exist:

- same-session later terminal turn
- TaskBoard says all same-session child tasks are terminal
- TimerList has no active/running timer for the source session
- Master loop pending attention/retry cursor is empty
- AgentBoard has no current binding for the selected session's tasks

Do not preserve `正在运行` from `active_turn_id` or visible text when owner truth
has closed.

## Forbidden UI Shapes

- selected-session transcript with global Home `正在运行` / `历史会话` panels still
  occupying the page
- floating global session list overlay covering the active selected transcript
- treating `等待用户选择` as `正在运行`
- using global TaskBoard/AgentBoard history when selected session has no scoped
  task/agent owner truth
- showing Worker `worker-task-*` sessions as top-level Home history
- rendering the Tools registry as a desktop-width modal with clipped text on
  phone
- hiding overflow by semantic trimming instead of fixing layout
- adding local browser fallback truth for session/task/tool/timer/config state

## Acceptance Gates For Implementation

Implementation is not complete until these are proven:

1. Home route:
   - phone screenshot shows exactly `正在运行` and `历史会话` as the body sections
   - no top-level internal runtime sessions
   - running/history ids disjoint
   - no horizontal overflow
2. Session route:
   - tapping any Home session hides Home panels
   - selected transcript dominates the body
   - Header/Agent sheet shows only selected-session scoped task/agent truth
   - Android Back or explicit Home returns to Home
3. Stale lifecycle:
   - session with no open owner truth is not listed under `正在运行`
   - `等待用户选择` appears as user-needed badge, not running
4. Tools route:
   - phone screenshot shows sticky header/close/refresh
   - cards fit phone width
   - examples/guidance/schema details collapsed or internally scroll/wrap
   - document/body no horizontal overflow
   - DOM rows still match `QueryToolRegistry`
5. Owner truth:
   - all visible route/status decisions are backed by ADP/query projections
   - no browser-local fallback truth is introduced
   - a closed Worker task with an older failed turn followed by a later success
     marks only the older cycle `historical_failure_recovered`; normal text does
     not present the duplicate raw provider error as current, while debug details
     still expose it
