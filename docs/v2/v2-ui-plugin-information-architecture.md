# Freehand v2 UI Plugin Information Architecture

Status: freeze-candidate
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Primary contract: `docs/v2/v2-ui-design.md`
Architecture: `docs/v2/v2-cordis-reasoning-channel-architecture.md`

## 1. Decision

The frontend is validated as an operating console with four information
surfaces:

```text
Location / Topology
Run
Attention / Notifications
Sessions / Canvas
```

Search and Memory are not UI-only features. They are independently addressable
Cordis plugins with typed inputs, typed outputs, lifecycle state and owners.
The fixed Cordis design orchestration plugin composes them; `UiAdaptor` exposes
their projections and commands to the frontend.

This follows the v2 ecosystem rule: UI renderers, UI adaptor, information
surfaces, reasoning, channels and capabilities are all plugins. The UI is a
replaceable plugin family, not a privileged layer above the plugin ecosystem.

The design preserves the existing reference prototype's compact desktop/mobile
grammar while changing the source of truth:

```text
reference visual grammar
  -> typed plugin projections
  -> UiAdaptor
  -> UI surfaces
```

## 2. Findability Model

Users locate information through two orthogonal axes:

| axis | surface | answer |
| --- | --- | --- |
| importance + time | Notifications / Attention | what needs action first |
| physical location | Location / Topology | where the Agent or Channel is |
| work time | Sessions / Canvas | what is active, recent or historical |
| semantic query | Search Plugin | which records match a keyword/filter |
| session knowledge | Memory Plugin | what was summarized, saved or loaded |

The axes must not be collapsed into one list. A notification can point to a
session and a topology node, but it does not own either resource. A canvas node
can focus a session, but it does not become a notification. Search can return
all of them through typed result categories without changing their ownership.

## 3. Plugin Contracts

### 3.1 NotificationPlugin

Input:

- typed lifecycle, task, Agent, Channel, error and user-decision events;
- notification policy and source identity;
- acknowledgement/snooze commands.

Output:

- `NotificationProjection`;
- deterministic order key `importance DESC, occurred_at DESC,
  notification_id ASC`;
- unread/read, acknowledged and snoozed state;
- source and owning detail identity.

Lifecycle:

```text
admit -> rank -> publish -> acknowledge/snooze -> expire/archive
```

Forbidden:

- status inference from UI strings;
- mutation of task/session/Agent truth;
- control data embedded in a notification business payload.

### 3.2 TopologyPlugin

Input:

- Machine, Node, Agent, Channel and Capability registry projections;
- registry change and transport status events.

Output:

- physical grouping;
- relationship edges;
- location, role, capability and connection status;
- stable focus identity.

Lifecycle:

```text
load registry projection -> reconcile generation -> publish -> focus
```

Forbidden:

- notification ranking;
- session ordering;
- inferred relationships not present in Registry/Channel truth.

### 3.3 SessionCanvasPlugin

Input:

- Session Log-derived session, turn, plugin and event relationships;
- time-band query `active | recent | history`;
- focus and filter commands.

Output:

- stable canvas nodes and edges;
- time-band classification;
- source Session/Turn identity;
- focus target for Run and Detail surfaces.

Lifecycle:

```text
derive -> publish -> focus/filter -> refresh/replay
```

Forbidden:

- second session store;
- mutation of Session Log sequence;
- browser-local reconstruction of historical edges.

### 3.4 SearchPlugin

Input:

- keyword query;
- structured filters and source scope;
- index, cache, invalidation and rebuild commands.

Output:

- classified result records;
- source identity and location;
- stable cursor and query status;
- explicit stale/rebuilding/error state.

Lifecycle:

```text
configure -> index -> query -> classify -> cache/invalidate/rebuild
```

Forbidden:

- UI-only string filtering as a second truth path;
- silently serving stale results after an explicit invalidation;
- exposing raw control/metadata/debug fields as business matches.

### 3.5 MemoryPlugin

Input:

- Session Log facts through the Session Log owner;
- manual trigger;
- automatic trigger policy at session close, turn boundary or another
  declared lifecycle point;
- attach/detach, load, search and export commands.

Output:

- summary and memory records;
- record provenance and source Session identity;
- save/load/search status;
- export artifact reference;
- attach state and trigger origin.

Lifecycle:

```text
attach -> summarize -> record -> save -> load/search -> export -> detach
```

Forbidden:

- rewriting original Session Log facts;
- treating generated summaries as original user/model content;
- browser-local persistence as recovery truth;
- silent loss of a failed save/export.

## 4. UI Surface Contract

```text
Location / Topology
  -> select machine/node/Agent/Channel
  -> scope other queries

Run
  -> operate selected session
  -> inspect live and terminal rows

Attention / Notifications
  -> rank by importance then time
  -> open owner detail
  -> send typed acknowledgement/action

Sessions / Canvas
  -> choose active/recent/history session
  -> focus Run or Detail

Global Search
  -> SearchPlugin query
  -> classified results

Session Memory
  -> MemoryPlugin command/query
  -> summary/record/load/export projection
```

The surfaces may share selection identity and navigation, but they do not
share persistence. All writes go through typed commands to the owning plugin
or owner module.

## 4.1 UI plugin decomposition

The information surfaces above are domain projections. Their frontend
renderers are separate Cordis UI plugins:

```text
UiAdaptor
  -> ui.shell
  -> ui.navigation
  -> ui.run
  -> ui.sessions
  -> ui.attention
  -> ui.location
  -> ui.more
  -> ui.detail
```

The domain plugin and UI plugin are different owners:

```text
SearchPlugin        -> ui.more / ui.detail
MemoryPlugin        -> ui.run / ui.more / ui.detail
SessionCanvasPlugin -> ui.sessions
NotificationPlugin  -> ui.attention
TopologyPlugin      -> ui.location
Session Log         -> ui.run
```

Every UI plugin has one stable `slot_id`, typed input projections, typed
command/query/subscribe output ports and an explicit mount/unmount lifecycle.
Its local state is disposable: after unload and replacement, the new
implementation rebuilds from the current projection and selected identity.

Replacement rules:

1. Replacing a UI plugin does not replace the domain plugin that supplies its
   projection.
2. Replacing a domain plugin implementation does not require replacing its UI
   consumer when the typed projection contract remains valid.
3. UI plugins never import truth-owner modules directly when the adaptor port
   exists.
4. Missing, incompatible or disconnected plugins render an explicit
   unavailable/error state; they do not silently render empty success.
5. The fixed design orchestration plugin composes and reconnects UI plugins,
   but does not render their surfaces or own their presentation state.

### Mobile entry hierarchy

Mobile has one primary navigation row only:

```text
Run | Sessions | Attention | Location | More
```

`More` is a second-level capability surface, not a peer information surface.
It is the only mobile entry for:

```text
操作: 新会话, 定时任务
信息: 搜索, 记忆
系统: 配置, 内置工具
```

This keeps information surfaces at level one and keeps operational, knowledge
and system capabilities at level two. No separate corner bar or parallel
mobile shortcut row is allowed.

## 5. Frontend Validation

The reference prototype was inspected at desktop and mobile widths. The
validated interaction grammar is:

- desktop: grouped navigation, central work surface, right-side attention;
- mobile: central work surface remains primary, five first-level destinations
  stay in one bottom row, and secondary capabilities open from grouped `More`;
- progressive detail: one detail panel with breadcrumb/back;
- compact rows: stable geometry for live updates and long content.

Required validation questions:

1. Can a user identify the highest-importance unresolved item without reading
   every session?
2. Can a user locate an Agent by machine, node, Agent and Channel without
   searching transcript text?
3. Can a user distinguish active, recent and historical work without confusing
   time bands with lifecycle truth?
4. Can a user search via Search Plugin and inspect source/classification without
   treating browser filtering as the result?
5. Can a user complete the Memory Plugin flow from trigger to saved record,
   load, search and export without replacing the original transcript?
6. Are desktop and mobile actions semantically identical even when surfaces
   move into sheets?

The browser acceptance in `docs/v2/v2-ui-design.md` must compare DOM output
with adaptor/query truth. Screenshots validate layout only; they do not prove
plugin lifecycle or ownership.

## 6. Freeze Checklist

- [x] four UI information surfaces are named and non-overlapping;
- [x] notification order is deterministic by importance and time;
- [x] topology is physical location, not session history;
- [x] canvas is Session Log-derived navigation, not a second store;
- [x] Search is an index/query/cache plugin;
- [x] Memory is a session summarize/record/save/load/search/export plugin;
- [x] manual and automatic Memory triggers are represented;
- [x] UI consumes typed projections through `UiAdaptor`;
- [x] control, metadata, debug and error data stay outside business payloads;
- [ ] concrete runtime symbols and browser verifier remain implementation
  work.
