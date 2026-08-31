# Freehand v2 UI Design Contract

Status: design-admitted
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Reference: `http://127.0.0.1:4173/docs/design/multi-agent-console-prototype.html`
Related plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
Test design: `docs/v2/v2-test-design.md`
Project black-box: `docs/v2/v2-project-blackbox-verification.md`

## 1. Product Job

Mode: `Operate`.

The v2 UI is a compact operating console for finding, operating and reviewing
work across multiple Agents. It must let a user:

1. submit or continue a conversation;
2. understand whether reasoning is running, waiting, blocked or complete;
3. inspect the current plugin/tool step and its result;
4. locate the owning Agent, node, machine and Channel by physical topology;
5. find important information quickly by notification importance and time;
6. review active, recent and historical sessions as a derived session graph;
7. search and load information through an independently addressable Search
   Plugin;
8. summarize, save, load, search and export session memory through an
   independently addressable Memory Plugin;
9. resolve an explicit user decision without confusing it with assistant text.

The first screen is an operating surface, not a marketing page. The UI
prioritizes current work and unresolved attention while preserving direct
access to physical location, session history, search and memory. Topology and
session drawing are navigation and review surfaces; neither becomes the
reasoning truth source.

## 2. Reference Adaptation

The local multi-agent console prototype contributes an interaction grammar:

| Reference pattern | v2 adaptation | Reason |
| --- | --- | --- |
| Fleet rail grouped by machine and Agent | Location/Topology surface grouped by machine, node, Agent and Channel | physical placement answers “where is this Agent?” |
| Topology board as the center | Run surface as the center for the selected session | the primary action remains operating one request |
| Activity rail with attention and recent sessions | Notifications/Attention surface with importance-first ordering | attention must lead to an actionable owner-backed state |
| Recent sessions list | Sessions/Canvas surface grouped by active, recent and history | drawing answers “what work is related and when?” |
| Agent/session click opens nested drawer | Agent, task, plugin and event details open a single typed detail panel with breadcrumb/back | progressive disclosure without changing the primary session |
| Filter buttons and search | Search Plugin entry with keyword, structured filters and result classification | search is a capability, not browser-local string filtering |
| Bottom mobile navigation and drawers | mobile Location / Run / Attention / Sessions navigation with sheets | preserve the same information model under narrow width |

Do not copy the reference's fake machine data, topology lines, emoji-like text
icons, or demo action wording. v2 uses actual typed projections and the
project's existing icon/component conventions when implementation begins.

## 3. Information Architecture

The product has four UI information surfaces. They are projections of plugin
outputs, not owners of the underlying resources:

| surface | user question | canonical input | primary output |
| --- | --- | --- | --- |
| `Location / Topology` | Where is the Agent, node or Channel? | `TopologyPlugin` projection | physical grouping and relationship graph |
| `Run` | What is happening in this session now? | Session Log and typed lifecycle projections | ordered live/terminal execution view |
| `Attention / Notifications` | What needs my attention first? | `NotificationPlugin` projection | importance/time ordered actionable items |
| `Sessions / Canvas` | What work is active, recent or historical? | `SessionCanvasPlugin` projection | session relationship graph and time bands |

Two capability entrypoints are global or session-scoped:

| entrypoint | scope | plugin output |
| --- | --- | --- |
| `Global Search` | workspace, Agent, node, session and plugin data | classified result set with source and cursor |
| `Session Memory` | one selected session | summary, memory records, load state and export reference |

The four surfaces share navigation identity, but they do not share truth.
`Run` reads Session Log-derived projections; `Location / Topology` reads
physical registry projections; `Attention / Notifications` reads notification
ranking projections; `Sessions / Canvas` reads session-relationship
projections. Search and Memory remain independently callable plugins.

### Desktop

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ Header: product · connection · current Agent · new session · settings      │
├──────────────────┬─────────────────────────────────────────┬──────────────┤
│ Location /       │ Current run                             │ Notifications│
│ Topology         │                                         │ / Attention  │
│                  │                                         │              │
│ machine          │ Session header                          │ Importance   │
│  └ node          │ User input                              │ then time   │
│    └ Agent        │ Reasoning / plugin timeline             │ decisions   │
│      └ Channel   │ Result / terminal state                 │ failures    │
│                  │ Composer                                │ reconnects   │
└──────────────────┴─────────────────────────────────────────┴──────────────┘
```

Regions have fixed responsibilities:

- `Location / Topology`: physical classification, location and relationship
  navigation only; it does not rank notifications or render session history.
- `Current run`: the only primary write surface; it sends a typed UI command.
- `Notifications / Attention`: read-only ranked notifications plus explicit
  control actions such as approve, retry, cancel or reconnect when the owner
  exposes them.
- `Sessions / Canvas`: active, recent and historical session navigation and
  relationship drawing; it is read-only and derived from Session Log facts.
- `Detail panel`: progressive read-only inspection, opened from a row or card.

The UI must never make a status decision from visible text. A status badge,
timeline phase or attention item is rendered from a typed projection field.

### Mobile

```text
┌─────────────────────────────┐
│ Header: session + status     │
├─────────────────────────────┤
│ Current run                  │
│ user / model / plugin rows  │
│ result or explicit wait      │
│ composer                     │
├─────────────────────────────┤
│ Location · Run · Attention   │
│ Sessions                     │
└─────────────────────────────┘
```

Mobile rules:

- the current run remains the default page;
- Location opens a full-height sheet containing machine, node, Agent and
  Channel groups;
- Attention opens a sheet containing unresolved ranked notifications;
- Sessions opens a sheet containing active, recent and history session bands;
- Search opens the Search Plugin surface, not a local browser filter;
- Memory opens the selected session's Memory Plugin surface;
- Agent/task/plugin details use a stacked detail sheet with back navigation;
- no desktop rail is squeezed into a horizontal scrolling strip;
- composer, status and primary actions remain reachable without hiding the
  current terminal state.

## 4. Current Run Composition

The current run is a chronological, typed timeline:

```text
User input
  -> model waiting / reasoning phase
  -> plugin invocation
  -> plugin result or typed failure
  -> control-event status
  -> reasoning continuation
  -> terminal projection
```

Each row has:

- semantic kind;
- owner/source identity;
- lifecycle status;
- timestamp or elapsed value when supplied by the projection;
- an optional detail action;
- a stable row key for incremental rendering.

The UI may collapse long detail payloads, but it may not remove actual
user-visible result data or change the ordering of the protocol projection.
Control events, error chains, metadata and debug observations are shown only
through explicit detail views designed for those projections. They are not
serialized into the assistant or user message body.

### Required states

| state | primary surface | allowed action |
| --- | --- | --- |
| `idle` | empty composer and last session context | submit new input |
| `dispatching` | accepted command receipt and current run placeholder | cancel if owner supports cancellation |
| `running` | model/plugin timeline with live phase | inspect current step, cancel |
| `waiting_user` | explicit decision panel, no fake assistant completion | choose one option, provide custom response, or keep waiting |
| `waiting_remote` | remote Agent/task ownership and reconnect state | inspect task, retry/reconnect only when typed owner action exists |
| `failed` | typed failure summary with source and retry boundary | inspect error, retry only when owner permits |
| `completed` | terminal result and stable transcript | continue session or inspect details |
| `disconnected` | transport state separated from session content | reconnect/inspect; never fabricate local success |
| `empty` | first-run explanation through labels and affordances | create session or submit |

Loading and streaming preserve geometry. A row changing from waiting to
complete must update in place by its stable key; historical rows must not
animate as if they were still live.

## 5. Plugin Surfaces

The following plugins are independent Cordis capabilities composed by the
fixed design orchestration plugin. They each have their own typed input,
output, lifecycle and owner. UI rendering is only one consumer of their
outputs.

### 5.1 Notification Plugin

```text
typed lifecycle/task/Agent/Channel/error/user-decision event
  -> NotificationPlugin
  -> importance + time ranking
  -> NotificationProjection
  -> Attention / Notifications surface
```

The ranking key is deterministic:

```text
importance descending
  -> occurred_at descending
  -> notification_id ascending
```

`importance` is supplied by the notification contract; the UI cannot infer it
from copy, color or event text. The plugin owns unread/read, acknowledgement,
snooze and deduplication state. It does not own Session Log, task, Agent,
Channel or business payload truth.

### 5.2 Topology Plugin

```text
Node + Machine + Agent + Channel + Capability + Registry projection
  -> TopologyPlugin
  -> physical grouping and relationship projection
  -> Location / Topology surface
```

The plugin answers physical location and relationship questions: which machine,
node, Agent and Channel are involved, and which capabilities are exposed. It
does not decide notification importance, session order or reasoning status.

### 5.3 Session Canvas Plugin

```text
Session Log-derived session/turn/plugin/event relationships
  -> SessionCanvasPlugin
  -> active/recent/history canvas projection
  -> Sessions / Canvas surface
```

The canvas is a derived navigation and review view. Its time bands are
`active`, `recent` and `history`; its nodes and edges retain stable source
identities and can focus the Run surface. It never becomes a second Session
Log, does not rewrite event sequence, and does not infer missing history from
browser state.

### 5.4 Search Plugin

```text
user keyword + structured filters
  -> SearchPlugin query input
  -> classified result + source + cursor
  -> SearchPlugin output
  -> Search surface or focused detail
```

The Search Plugin owns index construction, classification, database
maintenance, cache, invalidation and rebuild. It has two explicit input
families:

- interactive query input from the user;
- background index/maintenance input from lifecycle or storage owners.

It exposes query status, result classification, source identity, stable
cursor and explicit index errors. UI must not implement a parallel local
filter that can disagree with plugin results.

### 5.5 Memory Plugin

```text
Session Log + manual trigger + automatic trigger policy
  -> summarize
  -> record
  -> save
  -> load/search
  -> export
  -> MemoryPlugin projection
```

Memory is session-scoped but independently attachable and detachable at
runtime. It supports manual and automatic triggers at session close, turn
boundary or an owner-declared lifecycle point. The plugin owns summary and
memory-record persistence, load state, searchability and export references. It
must not mutate the original Session Log or present a summary as an original
session fact. A memory projection includes provenance, source session identity,
record state and export reference.

### 5.6 Composition Rule

The fixed Cordis design orchestration plugin composes these plugins and routes
their typed ports. It does not rank notifications, draw topology, index data,
summarize sessions or render UI. Search and Memory remain usable through
non-UI plugin consumers as well as through the UI adaptor.

## 6. Adaptor Boundary

The UI connects through a typed adaptor. The adaptor is a transport and
projection translator, not a truth owner.

```text
UI components
  -> UiAdaptor
  -> typed command/query/subscribe port
  -> v2 owner modules
  -> typed projection/event stream
  -> UiAdaptor
  -> UI components
```

### Adaptor responsibilities

- expose `send_command`, `query`, and `subscribe` as separate operations;
- carry correlation id and source identity in typed envelopes;
- map transport framing to v2 UI protocol types;
- preserve projection ordering and stable row identity;
- surface explicit connection, protocol and owner errors;
- close or reconnect only according to typed transport state;
- adapt the same UI model to local and future remote endpoints.

### Adaptor prohibitions

- no session, reason, event, plugin, task or node truth;
- no parsing raw assistant text to infer status;
- no retry/fallback policy;
- no metadata/debug/control fields in business command or result payloads;
- no direct filesystem access to sessionlog;
- no direct Cordis or Rust plugin imports;
- no browser-local persistence as recovery truth.

### Minimal adaptor shape

Names are design-level contracts until the v2 UI module is implemented:

```rust
trait UiAdaptor {
    fn send_command(&self, command: UiCommand) -> UiCommandReceipt;
    fn query(&self, query: UiQuery) -> UiQueryResult;
    fn subscribe(&self, request: UiSubscribeRequest) -> UiSubscription;
}
```

The concrete transport may be WebSocket, HTTP/SSE or another public protocol
only after its owner and verification binding are registered. The UI model
consumes `UiProjection`, `RunRow`, `AttentionItem`, `AgentSummary`,
`TaskSummary` and typed error states; it does not consume raw Cordis events or
sessionlog records.

### Local and remote adapters

The local adaptor is the MVP implementation boundary. A future remote adaptor
may add endpoint/session negotiation, authentication and reconnect state, but
must emit the same UI projection contract:

```text
local transport ─┐
                 ├─> UiAdaptor -> UiModel
remote transport ┘
```

Remote transport metadata is a typed side-channel. It must not change the
business payload shape to carry node, route, retry, capability or health data.

## 7. Interaction Rules

### Location and topology

- selecting a machine, node, Agent or Channel changes a typed topology query;
- topology selection may scope Session, Search or Attention queries but does not
  copy topology fields into business payloads;
- offline/disconnected state is rendered from the topology/transport projection;
- physical position is stable until a registry change projection is received.

### Notifications and attention

- the Attention surface consumes the Notification Plugin's deterministic order;
- importance is never recalculated in UI;
- time labels are display formatting only and cannot reorder records;
- acknowledgement and snooze send typed plugin commands;
- opening a notification preserves its source identity and owning detail
  context;
- a notification is not a substitute for the underlying task, session or
  error truth.

### Sessions and canvas

- Canvas selection focuses a Session or Run query through stable source IDs;
- active, recent and history bands are projection categories, not separate
  persistence stores;
- canvas edges must cite source Session Log relationship facts;
- reordering nodes visually must not mutate Session Log sequence;
- loading, partial and unavailable canvas states remain explicit.

### Search

- a search submit is a typed Search Plugin query, not a UI-only filter;
- keyword, structured filters, source scope and cursor remain separate fields;
- result classification and source identity are rendered from plugin output;
- index/cache maintenance status is visible as plugin state, not presented as
  a search result;
- failed, stale or rebuilding indexes remain explicit and never silently use a
  different local result path.

### Memory

- attach/detach, summarize, save, load, search and export are typed Memory
  Plugin commands or queries;
- manual and automatic trigger origin remains visible in the memory projection;
- loading memory does not overwrite the selected session transcript;
- memory records show source session and provenance;
- export returns an owner-provided artifact reference, not a browser-created
  copy of session truth.

### Session navigation

- selecting a session changes the selected-session query and subscription
  scope;
- the selected session is never reconstructed from browser-local state;
- running sessions are marked by owner-backed lifecycle fields;
- archived/history sessions remain queryable when the owner projection allows
  it;
- new session is an explicit command and requires the owner's accepted receipt.

### Attention

Attention items are actionable only when their owner provides a typed action.
Examples:

- user option required: render choices and custom answer;
- plugin failure: render the typed failure and retry boundary;
- remote Agent disconnected: render transport state and task/session owner;
- permission request: render the owner-provided approval action.

Clicking an attention item opens the owning detail context before an action is
sent. A command receipt is feedback, not a replacement for owner truth.

### Details

Use a single detail panel with a breadcrumb stack:

```text
session -> task -> Agent -> plugin/event/error
```

Back and close are icon buttons with accessible labels. Details are read-only
unless the selected owner exposes an explicit command. No detail panel may
become a second transcript or a second persistence path.

### Composer

The composer sends one `UiCommand` with the selected session and optional
workspace context. It must:

- disable duplicate submission while the same command is pending;
- show explicit dispatch failure;
- retain the user's input until the owner accepts or rejects it;
- never insert control status, debug text or raw event data into the user
  message;
- preserve focus and stable geometry through running/terminal transitions.

## 8. Visual System Direction

The visual direction is restrained operational UI:

- neutral surface hierarchy with one dark primary action color;
- blue for selection/info, green for healthy/completed, amber for waiting,
  red for failure, gray for disconnected;
- compact fixed-size status indicators paired with text;
- 8px-or-less radius for repeated cards and rows;
- one sans-serif family with fixed product-UI type steps;
- thin dividers and dense but readable rows;
- no decorative gradient/orb/glass treatment;
- no large hero section, marketing copy or topology-first composition.

Use semantic HTML, visible focus states, keyboard activation for rows, and
`prefers-reduced-motion`. Motion is limited to state change, detail reveal and
row updates; it must not conceal content or imply progress that the owner did
not report.

## 9. Component Inventory

The first implementation should be built from these owner-facing components:

| component | responsibility | source of truth |
| --- | --- | --- |
| `AppHeader` | current Agent, connection and session actions | typed config/connection projection |
| `TopologyRail` | machine/node/Agent/Channel location and selection | `TopologyPlugin` projection |
| `SessionRail` | active/recent/history session selection | `SessionCanvasPlugin` projection |
| `RunTimeline` | chronological user/model/plugin/result rows | UI projection |
| `RunRow` | one stable lifecycle row | row kind/status/source fields |
| `Composer` | user command ingress | typed `UiCommand` receipt |
| `AttentionRail` | importance/time ordered notifications and actions | `NotificationPlugin` projection |
| `SessionCanvas` | active/recent/history session relationship graph | `SessionCanvasPlugin` projection |
| `SearchSurface` | keyword/filter query and classified results | `SearchPlugin` projection |
| `MemorySurface` | summarize/record/save/load/search/export state | `MemoryPlugin` projection |
| `DetailPanel` | progressive inspection and owner actions | typed query result |
| `MobileNavigation` | Location/Run/Attention/Sessions route selection | transient UI route only |
| `ConnectionState` | local/remote/disconnected transport signal | adaptor connection state |

These are UI composition names, not permission to add runtime managers or
semantic service layers. Shared parsing, validation and projection helpers
remain in the owning contract/block module.

## 10. Verification Contract

The UI design is accepted only when the implementation proves:

### Positive

- one command enters through the adaptor and produces one owner-backed receipt;
- one selected session renders ordered user/model/plugin/result rows;
- running, waiting-user, waiting-remote, failed and completed states are
  distinguishable from typed fields;
- notifications render in importance/time order and acknowledgement changes
  only notification projection state;
- topology locates the selected Agent through machine/node/Channel groups;
- canvas renders active, recent and history sessions from stable source IDs;
- Search accepts keyword/filter input and returns classified plugin results;
- Memory completes summarize/record/save/load/search/export through typed plugin
  state, including manual and automatic trigger origin;
- Agent/task/plugin detail opens without replacing session truth;
- local adaptor and future remote adaptor map to the same UI model;
- desktop and mobile layouts preserve the same task order and actions;
- keyboard focus and reduced-motion behavior remain usable.

### Negative

- UI cannot write session, reason, control-event, plugin, task or node truth;
- UI cannot infer lifecycle status from assistant text;
- control/event/error/metadata/debug fields cannot enter business payload;
- query and subscribe cannot be routed through the same operation;
- disconnected remote state cannot render successful local completion;
- duplicate submit cannot create two accepted commands;
- detail panel cannot create a second transcript or persistence path;
- UI cannot reorder notifications, invent topology relationships or create
  canvas edges without plugin projection facts;
- UI cannot replace Search indexing/cache or Memory persistence/export with
  browser-local state;
- memory summary cannot be rendered as an original Session Log fact;
- no build output, generated output, runtime evidence or external checkout is
  part of the UI change set.

### Browser acceptance

When the concrete v2 UI exists, the public-entrypoint browser proof must cover:

- desktop wide and narrow layouts;
- mobile phone layout;
- topology location and session canvas navigation;
- Search Plugin keyword/filter/result flow;
- Memory Plugin attach, summarize, save, load and export states;
- notification importance/time ordering and acknowledgement;
- running -> waiting -> terminal transitions;
- plugin success and failure;
- explicit user decision;
- detail panel open/back/close;
- disconnected/reconnect state;
- keyboard focus and no horizontal overflow.

The browser proof must compare DOM projections with adaptor/query truth. A
visual screenshot alone cannot certify lifecycle or payload separation.

## 11. Non-goals

- no v1 WebUI rewrite in this design milestone;
- no direct reuse of v1 DOM state as v2 truth;
- no topology or session canvas as the primary Run write surface;
- no production multi-machine transport;
- no browser storage as session recovery;
- no UI-owned search index, cache or memory store;
- no fake reasoning transcript or hidden progress animation.
