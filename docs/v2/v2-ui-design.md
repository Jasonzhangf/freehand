# Freehand v2 UI Design Contract

Status: design-admitted
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.5`
Reference: `http://127.0.0.1:4173/docs/design/multi-agent-console-prototype.html`
Related plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
Test design: `docs/v2/v2-test-design.md`
Project black-box: `docs/v2/v2-project-blackbox-verification.md`

## 1. Product Job

Mode: `Operate`.

The v2 UI is a compact operating console for one active reasoning session. It
must let a user:

1. submit or continue a conversation;
2. understand whether reasoning is running, waiting, blocked or complete;
3. inspect the current plugin/tool step and its result;
4. inspect the owning Agent, task and node when the work is delegated;
5. resolve an explicit user decision without confusing it with assistant text.

The first screen is the working session, not a marketing page and not a
topology diagram. Multi-machine collaboration remains visible through
owner-backed status and drill-down surfaces, but it does not displace the
conversation and current execution from the primary viewport.

## 2. Reference Adaptation

The local multi-agent console prototype contributes an interaction grammar:

| Reference pattern | v2 adaptation | Reason |
| --- | --- | --- |
| Fleet rail grouped by machine and Agent | Workspace/session rail grouped by workspace, session and current Agent | v2 starts with a local session; remote nodes are a later projection |
| Topology board as the center | Current run and reasoning timeline as the center | the primary v2 job is operating one request, not surveying a fleet |
| Activity rail with attention and recent sessions | Attention rail with user decisions, failures and current task context | attention must lead to an actionable owner-backed state |
| Agent/session click opens nested drawer | Agent, task, plugin and event details open a single typed detail panel with breadcrumb/back | progressive disclosure without changing the primary session |
| Filter buttons and search | session search plus status filters | scanability for repeated operation |
| Bottom mobile navigation and drawers | mobile Session / Run / Attention navigation with bottom sheets | preserve the same information model under narrow width |

Do not copy the reference's fake machine data, topology lines, emoji-like text
icons, or demo action wording. v2 uses actual typed projections and the
project's existing icon/component conventions when implementation begins.

## 3. Information Architecture

### Desktop

```text
┌───────────────────────────────────────────────────────────────────────────┐
│ Header: product · connection · current Agent · new session · settings      │
├──────────────────┬─────────────────────────────────────────┬──────────────┤
│ Session rail     │ Current run                             │ Attention    │
│                  │                                         │              │
│ Workspace        │ Session header                          │ Needs action │
│  └ sessions      │ User input                              │ Current task │
│  └ running       │ Reasoning / plugin timeline             │ Agent status │
│  └ history       │ Result / terminal state                 │ Context      │
│                  │ Composer                                │              │
└──────────────────┴─────────────────────────────────────────┴──────────────┘
```

Regions have fixed responsibilities:

- `Session rail`: navigation and session selection only.
- `Current run`: the only primary write surface; it sends a typed UI command.
- `Attention rail`: read-only owner projections plus explicit control actions
  such as approve, retry, cancel or reconnect when the owner exposes them.
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
│ Session · Run · Attention    │
└─────────────────────────────┘
```

Mobile rules:

- the current run remains the default page;
- Session opens a full-height sheet containing search and session rows;
- Attention opens a sheet containing only unresolved owner-backed items;
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

## 5. Adaptor Boundary

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

## 6. Interaction Rules

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

## 7. Visual System Direction

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

## 8. Component Inventory

The first implementation should be built from these owner-facing components:

| component | responsibility | source of truth |
| --- | --- | --- |
| `AppHeader` | current Agent, connection and session actions | typed config/connection projection |
| `SessionRail` | search, filters and session selection | session query/subscription projection |
| `RunTimeline` | chronological user/model/plugin/result rows | UI projection |
| `RunRow` | one stable lifecycle row | row kind/status/source fields |
| `Composer` | user command ingress | typed `UiCommand` receipt |
| `AttentionRail` | unresolved decisions and failures | task/Agent/transport projections |
| `DetailPanel` | progressive inspection and owner actions | typed query result |
| `MobileNavigation` | Session/Run/Attention route selection | transient UI route only |
| `ConnectionState` | local/remote/disconnected transport signal | adaptor connection state |

These are UI composition names, not permission to add runtime managers or
semantic service layers. Shared parsing, validation and projection helpers
remain in the owning contract/block module.

## 9. Verification Contract

The UI design is accepted only when the implementation proves:

### Positive

- one command enters through the adaptor and produces one owner-backed receipt;
- one selected session renders ordered user/model/plugin/result rows;
- running, waiting-user, waiting-remote, failed and completed states are
  distinguishable from typed fields;
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
- no build output, generated output, runtime evidence or external checkout is
  part of the UI change set.

### Browser acceptance

When the concrete v2 UI exists, the public-entrypoint browser proof must cover:

- desktop wide and narrow layouts;
- mobile phone layout;
- session selection and search;
- running -> waiting -> terminal transitions;
- plugin success and failure;
- explicit user decision;
- detail panel open/back/close;
- disconnected/reconnect state;
- keyboard focus and no horizontal overflow.

The browser proof must compare DOM projections with adaptor/query truth. A
visual screenshot alone cannot certify lifecycle or payload separation.

## 10. Non-goals

- no v1 WebUI rewrite in this design milestone;
- no direct reuse of v1 DOM state as v2 truth;
- no topology canvas as the primary v2 home screen;
- no production multi-machine transport;
- no browser storage as session recovery;
- no fake reasoning transcript or hidden progress animation.
