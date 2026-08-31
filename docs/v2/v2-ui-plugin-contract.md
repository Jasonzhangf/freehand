# Freehand v2 UI Plugin Contract

Status: freeze-candidate
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Parent architecture: `docs/v2/v2-cordis-reasoning-channel-architecture.md`
Plugin ecosystem contract: `docs/v2/v2-plugin-ecosystem-contract.md`
UI design: `docs/v2/v2-ui-design.md`
Information architecture: `docs/v2/v2-ui-plugin-information-architecture.md`

The UI plugin contract is one slice of the v2 plugin ecosystem. Full
ecosystem scope, kinds, typed ports, ownership and replacement rules live in
`docs/v2/v2-plugin-ecosystem-contract.md`.

## 1. Decision

The frontend is a Cordis plugin family. The UI is not one privileged
application layer and is not one replacement unit. It follows the v2
ecosystem rule that every executable or replaceable product part is a
Cordis plugin.

The fixed design orchestration plugin composes UI plugins through stable slots:

```text
Cordis root
└── design-orchestration-plugin
    ├── ui.shell
    ├── ui.navigation
    ├── ui.run
    ├── ui.sessions
    ├── ui.attention
    ├── ui.location
    ├── ui.more
    ├── ui.detail
    └── ui.adaptor
```

`ui.adaptor` is a bridge plugin. It is not a renderer. The other entries are
rendering or interaction plugins. All of them are independently loadable and
replaceable.

The UI plugin family does not weaken the domain plugin boundary. Search,
Memory, Timer, Tools, Notification, Topology and Session Canvas remain
independent domain or projection plugins; their UI consumers are separate
surface plugins connected through typed adaptor ports.

UI plugins own only:

- view composition;
- pointer, keyboard and focus state;
- temporary selection state;
- loading, unavailable and error presentation state;
- subscriptions owned by the slot.

UI plugins do not own:

- Session or Turn truth;
- Session Log persistence;
- Reasoning state or provider selection;
- Channel, Network or route truth;
- Search index/cache truth;
- Memory records or export truth;
- Timer or Tool truth;
- control, metadata, debug or error ledgers.

## 2. Identity

Every UI plugin definition declares:

| field | meaning | stability |
| --- | --- | --- |
| `plugin_id` | concrete implementation identity | changes when implementation changes |
| `slot_id` | replacement position in the UI graph | stable across implementations |
| `instance_id` | runtime-group plus UI endpoint instance | stable for one mounted instance |
| `contract_version` | typed port contract version | changes only with incompatible port semantics |
| `capabilities` | declared UI actions and states | validated before load |

`slot_id` is the replacement boundary. A new implementation may replace the
old implementation only when it claims the same `slot_id` and satisfies the
same contract version or an explicitly compatible version.

The first stable slots are:

| `slot_id` | responsibility | primary input projection |
| --- | --- | --- |
| `ui.shell` | page frame, mount regions, lifecycle and connection boundary | connection state and plugin availability |
| `ui.navigation` | one-row `Run / Sessions / Attention / Location / More` navigation | selected route and unread counts |
| `ui.run` | selected-session timeline, composer and run state | Session Log-derived UI projection |
| `ui.sessions` | active/recent/history session navigation | SessionCanvas projection |
| `ui.attention` | ranked notifications and owner actions | Notification projection |
| `ui.location` | machine/node/Agent/Channel navigation | Topology projection |
| `ui.more` | grouped secondary capability entry and availability | capability availability projection |
| `ui.detail` | progressive detail inspection and back/close | typed detail query result |
| `ui.adaptor` | transport and projection translation | owner-backed command/query/subscribe frames |

## 3. Typed Ports

The port families are separate. A command is not a query, a query is not a
subscription, and a UI projection is not a business payload.

```text
owner plugin
  -> typed projection/event
  -> ui.adaptor
  -> UiProjection / UiConnectionState
  -> UI plugin

UI plugin
  -> UiCommand
  -> ui.adaptor
  -> owner plugin

UI plugin
  -> UiQuery
  -> ui.adaptor
  -> owner plugin

UI plugin
  -> UiSubscribe
  -> ui.adaptor
  -> owner plugin
```

Each UI plugin may consume:

- `UiProjection`: read-only owner-backed surface data;
- `UiSelection`: stable selected resource identity;
- `UiCommandReceipt`: accepted, rejected or failed command result;
- `UiConnectionState`: local, remote, disconnected, unavailable or protocol
  error state;
- `UiCapabilityAvailability`: whether a declared UI action is available.

Each UI plugin may emit:

- `UiCommand`: an owner-directed action request;
- `UiQuery`: a read request;
- `UiSubscribe`: a stream request with a cursor;
- `UiSelection`: a navigation selection request.

The UI plugin must not emit raw Cordis events, raw Session Log records,
provider wire objects, control envelopes or metadata records.

## 4. Ownership And Dependency Direction

The dependency direction is:

```text
domain owner
  -> typed domain projection
  -> ui.adaptor
  -> UI plugin
  -> typed command/query/subscribe request
  -> ui.adaptor
  -> domain owner
```

The UI plugin graph is not allowed to call domain owners directly:

```text
forbidden:
ui.run -> freehand-reason
ui.more -> freehand-memory
ui.navigation -> freehand-task
ui.location -> freehand-node
```

The adaptor and runtime owner path must be used instead. This keeps UI
replacement independent from the domain implementation language and crate.

UI plugins may share `UiSelection` through the design orchestration plugin.
They may not share mutable view state, persistence, browser storage or
business payload objects.

## 5. Lifecycle

The lifecycle of one UI plugin instance is:

```text
discover
  -> validate definition and port contract
  -> load implementation
  -> mount into slot
  -> attach typed subscriptions
  -> render current projection
  -> accept commands and selections
  -> suspend or unmount
  -> release slot resources
```

The plugin must be able to rebuild its visible state from:

- the current `UiProjection`;
- the current `UiSelection`;
- the current `UiConnectionState`;
- unexpired `UiCommandReceipt` values relevant to the slot.

It must not require a hidden in-memory transcript or browser-local database to
recover after unmount, process restart or replacement.

Required explicit states:

```text
loading
ready
empty
unavailable
disconnected
error
```

`empty` means the owner returned a valid empty projection. `unavailable`,
`disconnected` and `error` are not empty success states.

## 6. Replacement Protocol

Replacement is scoped to one `slot_id`:

```text
replacement requested
  -> stop accepting new slot-local actions
  -> settle or cancel slot-owned view work
  -> detach subscriptions and view resources
  -> preserve selected identity and owner receipts
  -> unload old plugin
  -> validate replacement definition
  -> mount replacement in the same slot
  -> reconnect typed ports
  -> rebuild from current projection
  -> expose ready or explicit unavailable/error state
```

Replacement rules:

1. A UI replacement cannot rewrite owner truth.
2. A UI replacement cannot change the selected Session, Agent, Channel or
   other resource identity without an explicit selection command.
3. A failed replacement leaves the slot in explicit `unavailable` or `error`
   state; it must not show stale success.
4. Replacing `ui.run` does not restart or migrate the Reasoning backend.
5. Replacing `ui.adaptor` requires port compatibility validation before any
   surface plugin reconnects.
6. Replacing a domain plugin implementation does not require UI replacement
   when its typed projection contract remains compatible.

## 7. Mobile Composition

The mobile composition has exactly one first-level navigation row:

```text
ui.navigation
  -> Run | Sessions | Attention | Location | More
```

`ui.more` is the only entry point for secondary capabilities:

```text
操作: 新会话, 定时任务
信息: 搜索, 记忆
系统: 配置, 内置工具
```

The mobile slots remain separate even when they are rendered by one physical
page. Physical co-location is not permission to merge ownership or lifecycle.

## 8. MVP Implementation Order

The first implementation should minimize the number of active replacement
boundaries while proving the contract:

1. `ui.adaptor`: command/query/subscribe separation and typed projection
   delivery.
2. `ui.shell`: mount boundary, connection state and explicit unavailable/error
   rendering.
3. `ui.navigation`: one-row mobile route contract and shared selection.
4. `ui.run`: one selected session, ordered run projection and one composer
   command.
5. `ui.sessions`: active/recent/history selection.
6. `ui.attention`: importance/time projection and acknowledgement command.
7. `ui.location`: machine/node/Agent/Channel projection and focus.
8. `ui.more`: grouped secondary capability discovery.
9. `ui.detail`: progressive detail and back/close.
10. one replacement proof: replace `ui.navigation` or `ui.run` while keeping
    the same adaptor, selected identity and owner projection.

Search, Memory, Timer and Tools are domain capability plugins. Their first UI
surfaces are mounted through `ui.more`, `ui.run` or `ui.detail`; they are not
implemented as UI-owned stores. The domain plugins and their UI consumers can
therefore be replaced independently when their typed contracts remain
compatible.

## 9. Freeze Checklist

- [x] UI is a Cordis plugin family.
- [x] `slot_id` is the stable UI replacement boundary.
- [x] `ui.adaptor` is separate from rendering plugins.
- [x] UI plugins own presentation state only.
- [x] command, query and subscribe ports are distinct.
- [x] mobile first-level navigation is one row with five destinations.
- [x] secondary mobile capabilities are grouped under `ui.more`.
- [x] replacement preserves owner-backed selection and projection identity.
- [ ] exact Rust/Cordis implementation types;
- [ ] compiled UI plugin manifest schema;
- [ ] runtime registry and replacement gate;
- [ ] browser replacement verifier.

The unchecked items are implementation contracts, not permission to change the
ownership or replacement decisions above.

## 10. Machine-Readable Binding

The planned governance bindings for this contract are:

- module: `v2-ui-plugin-family`;
- resource: `v2_ui_plugin_slot`;
- lifecycle: `v2-ui-plugin-replacement`;
- gate: `v2_ui_plugin_slot_replacement`.

The business manifest is
`docs/v2/v2-governance-manifest.json`. These entries remain planned until
source symbols, tests and runtime evidence exist.
