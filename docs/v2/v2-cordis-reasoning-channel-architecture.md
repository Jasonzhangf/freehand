# Freehand v2 Cordis, Reasoning Backend And Channel Architecture

Status: design-admitted
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Related plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
Test design: `docs/v2/v2-test-design.md`
Project blackbox: `docs/v2/v2-project-blackbox-verification.md`

## 1. Frozen Direction

Freehand v2 uses Cordis as the architecture foundation and the complete
plugin ecosystem. Cordis owns plugin composition, Service dependency
activation, typed events, effects, child-plugin composition, scope isolation,
unload, replacement and hot-reload lifecycle.

Freehand does not create a second plugin runtime, Service registry, universal
event bus or hot-reload manager.

The fixed Freehand design component is a Cordis plugin:

```text
Cordis root
└── design-orchestration-plugin
    ├── UI plugin family
    ├── reasoning backend plugin family
    ├── sessionlog plugin family
    ├── channel and network plugin family
    └── capability endpoint plugin family
```

Every other product capability is also a Cordis plugin. A plugin may be:

- a leaf plugin implemented directly in Rust or TypeScript;
- a composition plugin that mounts child plugins;
- an adaptor plugin that translates an external runtime into a Freehand
  Service and typed event contract.

## 2. Four Product Zones, One Plugin Model

The product remains divided into four responsibility zones, but the zones are
not privileged runtime layers:

| zone | plugin family | owner boundary |
| --- | --- | --- |
| UI / UX | UI plugin and UI adaptor plugin | interaction and read-only projection |
| reasoning | reasoning backend, sessionlog and model plugins | model execution and reasoning facts |
| network / route | Registry, link, transport and route plugins | discovery, reachability and propagation |
| input / output channels | channel session and capability endpoint plugins | capability relationship and invocation |
| information surfaces | Notification, Topology, SessionCanvas, Search and Memory plugins | projection, navigation and session knowledge |

The fixed design orchestration plugin composes these families and connects
adjacent Services and typed events. It does not own their semantic truth.

The information-surface plugins are independently addressable Cordis plugins:

```text
design-orchestration-plugin
├── notification-plugin
├── topology-plugin
├── session-canvas-plugin
├── search-plugin
└── memory-plugin
```

Their outputs may be rendered by the UI adaptor, queried by headless clients,
or consumed by other plugins. UI rendering is not their lifecycle owner.

The ownership split is fixed:

- `NotificationPlugin` ranks typed source events by importance, time and stable
  identity, then owns acknowledgement/snooze projection state.
- `TopologyPlugin` derives physical machine/node/Agent/Channel relationships
  from Registry and Channel projections.
- `SessionCanvasPlugin` derives active/recent/history session relationships
  from Session Log facts without becoming a second log.
- `SearchPlugin` owns typed query, classification, index, cache, invalidation
  and rebuild behavior.
- `MemoryPlugin` owns session-scoped summarize, record, save, load, search and
  export behavior without rewriting Session Log facts.

## 3. Cordis Runtime Composition

### 3.1 Root context

The root context owns only ecosystem composition:

```text
RootContext
├── plugin composition configuration
├── registry of available plugin definitions
└── runtime groups
```

It must not expose one global reasoning implementation to all runtimes.

### 3.2 Runtime groups

Each independent Freehand runtime is a Cordis child composition with an
isolated scope:

```text
RootContext
├── runtime-group: local-freehand
│   ├── design-orchestration-plugin
│   ├── freehand-reasoning-backend
│   └── freehand-sessionlog
│
├── runtime-group: opencode-worker
│   ├── design-orchestration-plugin
│   ├── opencode-reasoning-adaptor
│   └── freehand-sessionlog
│
└── runtime-group: remote-agent
    ├── design-orchestration-plugin
    ├── channel-session-plugin
    └── remote-capability-endpoint
```

This permits different runtime groups to use different reasoning bases at the
same time. A backend replacement affects only the runtime group whose
dependency binding changed.

The implementation must use Cordis scope or child-context isolation instead of
one mutable process-global `ctx.reasoning` binding. A shared root-level
Service is allowed only when it is explicitly stateless and not
runtime-specific.

### 3.3 Hot replacement lifecycle

```text
provider plugin unload
  -> dependent plugin registrations/effects unwind
  -> runtime group enters explicit unavailable/rebinding state
  -> replacement provider loads
  -> dependent plugins reactivate through inject
```

An in-flight reasoning operation must not retain an unloaded provider object.
The backend contract therefore requires an operation-owned cancellation and
settlement boundary. A replacement is admitted only after the old provider's
owned operations have settled or have been explicitly interrupted.

## 4. Reasoning Backend Seam

The reasoning implementation is a replaceable Cordis capability seam with
three roles:

```text
Reasoning Service Definition
  -> Reasoning Backend Provider
  -> Reasoning Consumer / Orchestrator
```

The provider-neutral Service Definition is the only contract consumed by the
design orchestration plugin, UI adaptor and channel plugins.

### 4.1 Backend candidates

The first two backend implementations are:

| backend | role | integration |
| --- | --- | --- |
| Freehand reasoning backend | current project reasoning implementation | native plugin |
| OpenCode backend | pluggable reasoning base | OpenCode adaptor plugin |

OpenCode is not a second Freehand architecture. It is one backend provider
behind the same Reasoning Service Definition. The first adaptor must keep the
transport behind its plugin boundary; the initial implementation may use the
OpenCode SDK-next embedded server or an external OpenCode server, but neither
choice changes the Freehand contract or Session Log owner.

The OpenCode reference reviewed for this design is OpenCode `1.18.23`. Its
Effect-native V2 core exposes a Session service, Session Execution, durable
events, prompt admission, interruption, history, event subscription,
compaction and model/provider resolution. Its SDK-next can embed the server
router in-process, while its normal server/client path can remain a separate
transport option later.

### 4.2 Provider-neutral reasoning contract

The exact Rust types are implementation work, but the design contract has
these operations:

```text
reasoning.start(runtime_id, session_id, input)
reasoning.resume(runtime_id, session_id)
reasoning.interrupt(runtime_id, session_id)
reasoning.inspect(runtime_id, session_id)
reasoning.subscribe(runtime_id, session_id, cursor)
reasoning.capabilities(runtime_id)
```

The returned stream separates:

```text
ReasoningEvent       durable or replayable reasoning facts
ReasoningDelta       live-only non-durable rendering detail
ReasoningError       typed failure chain
ReasoningControl     cancellation, retry, replacement and lifecycle control
```

The backend may use a native internal protocol, but the adaptor must map it
into this common contract before the event reaches Freehand consumers.

### 4.3 Session Log ownership

The Freehand `sessionlog-plugin` is the canonical owner of Freehand reasoning
facts. It follows the DSH Session model:

```text
append-only typed SessionEvent log
  -> ordered surface fold
  -> model-visible message derivation
  -> transcript and UI projections
```

The log is immutable by sequence. Reorder, undo, replacement, retry and
recovery append new facts and produce a new derived surface. They do not edit
or delete historical events.

An OpenCode adaptor must not silently make OpenCode's native database a second
Freehand truth source. For the first integration:

1. Freehand Session Log supplies the request history to the backend;
2. the OpenCode adaptor invokes OpenCode execution through its public
   Session/SDK boundary;
3. returned durable facts are normalized into the Freehand Session Log;
4. OpenCode-native persistence is an adaptor implementation detail and cannot
   be used to reconstruct Freehand state independently;
5. if native OpenCode state is required for one execution, it is keyed by the
   Freehand `SessionId` and is rebuilt or validated from the canonical log.

This rule prevents dual-write and prevents a backend swap from losing or
silently changing the conversation history.

### 4.4 Backend switching

Backend selection is runtime-group configuration, not a business payload
field:

```text
runtime-group configuration
  -> backend binding
  -> injected Reasoning Service
```

The selected backend identity and route facts are control/session metadata
owned by the reasoning/sessionlog contract. They must not be inserted into
user messages, assistant content or provider business payloads.

A backend switch takes effect at a safe Session boundary:

```text
running operation
  -> explicit stop or terminal settlement
  -> append backend-switch control fact
  -> rebuild request from Session Log surface
  -> next operation uses the new backend
```

No in-flight model request is silently migrated between providers.

## 5. DSH Session Log Semantics Adopted By v2

### 5.1 Append-only event truth

Each accepted event receives a contiguous sequence. The event is validated,
snapshotted and committed before it is projected.

The Session Log contains both model-visible and log-only facts. A log-only
control fact is not automatically included in model history.

### 5.2 Surface operations

The first surface contract adopts the DSH concepts:

```text
append
replace
```

The v2 extension adds explicit operation families for:

```text
retract / undo
reorder
fork
```

These are separate typed event variants. They must cite their source event
sequences or surface nodes and must be validated against the current surface.

The operation rules are:

- `append` adds a new surface node at the tail;
- `replace` shadows a valid current range and inserts a replacement node;
- `retract/undo` appends a tombstone or exclusion fact for a declared target;
- `reorder` appends a new ordering fact and never changes historical `seq`;
- `fork` creates a child Session from a stable closed boundary;
- `retry` appends a new attempt after an explicit failed or interrupted
  attempt;
- `recovery` appends repair facts and does not convert failure into success.

The generic `reorder` shape remains a freeze item until its source coverage,
projection and conflict rules are specified. It must not be simulated by
mutating `seq` or by deleting old records.

### 5.3 Error recovery

The reasoning backend follows the DSH failed-request boundary:

```text
provider request
  -> terminal provider error
  -> append failure / request-error fact
  -> optional recovery plugin decision
  -> append replace/recovery facts if a new surface is ready
  -> append retry-started
  -> issue a new request from the derived surface
```

If recovery does not produce a valid surface advancement, the original error
remains authoritative. A failed recovery cannot become a successful terminal
projection.

An interrupted open turn is repaired with explicit synthetic closing facts
before a cold resume. Unsettled side effects are marked unknown; the system
must not blindly replay them.

## 6. Central Channel Registry

### 6.1 Registry role

Every channel endpoint is configured with a central Registry endpoint:

```text
registry_url
registry_token
endpoint_id
node_id
```

The Registry is a centralized control-plane plugin. It owns:

- endpoint registration;
- endpoint liveness and current route advertisement;
- capability manifest publication;
- endpoint version and change generation;
- discovery queries;
- channel relation lookup;
- route-change notifications.

The Registry does not own:

- reasoning Session Log truth;
- business payload truth;
- UI projection truth;
- plugin implementation state;
- authorization sessions.

The Registry may be local or cloud-hosted. The endpoint URL is configuration,
not a compiled assumption.

### 6.2 Registration lifecycle

```text
configured endpoint start
  -> authenticate with stateless token
  -> register identity, link address and capability manifest
  -> receive registry generation / endpoint lease
  -> renew registration while active
  -> publish capability or route changes
  -> unregister on clean shutdown
```

Registration state is control-plane state and may be persisted by the
Registry. Authentication remains stateless API-key/bearer-token validation;
the Registry does not create an authentication session table. ChannelSession
state is owned by the channel-session plugin, not by the Registry. The Registry
may publish a session lookup or generation, but it cannot become the session
truth store.

### 6.3 Discovery and change

Controllers resolve a channel through the Registry:

```text
controller
  -> discover endpoint
  -> read capability manifest
  -> compare desired capabilities
  -> reconcile against explicit compatibility policy
  -> open or rebind a logical channel
```

Capability and route changes advance an explicit generation. A channel session
must observe the generation before a new invocation. An incompatible change
enters an explicit `capability_changed` or `rebind_required` state; it cannot
silently select another capability or local implementation.

### 6.4 Registry, link and channel separation

```text
Registry Plugin
  answers: who is available and where is the current route?

Link / Transport Plugin
  answers: can bytes/control frames propagate through this route?

Channel Plugin
  answers: what logical controller/controlled relationship is active?

Capability Endpoint Plugin
  answers: what typed capability can be invoked?
```

The Registry is not a replacement for the Link or Channel plugin. It provides
the current control-plane facts that those plugins consume.

## 7. Channel Connection And Channel Session

The transport connection and the logical channel session are separate
resources.

```text
ConnectionId
  = one concrete link/transport connection; ephemeral

ChannelId
  = stable controller/controlled capability relationship

ChannelSessionId
  = durable logical invocation context; survives connection replacement

ReasoningSessionId
  = model/reasoning conversation identity; independent resource
```

One `ChannelSessionId` may use multiple `ConnectionId` values over its
lifetime:

```text
connection-1
  -> channel-session-A
connection-1 lost
  -> channel-session-A suspended
connection-2 discovered through Registry
  -> channel-session-A reattached
```

The channel session owns:

- controller and controlled endpoint identity;
- authorized capability set;
- session protocol version;
- invocation ordering/correlation;
- session-local continuation state;
- reconnect cursor;
- explicit state (`opening`, `active`, `suspended`, `reconnecting`,
  `rebind_required`, `closed`, `failed`).

The transport connection owns only:

- socket/link identity;
- transport state;
- frame sequencing and acknowledgement;
- connection-scoped cancellation.

Channel session state must not be reconstructed from payload text or UI state.
It is a typed control resource. Business payload travels separately through
the channel invocation contract.

The frozen mapping rule is one logical relationship per `ChannelId`, with one
`ChannelSessionId` for each active logical session of that relationship. A
ChannelSession may carry invocations for multiple ReasoningSession values; a
ReasoningSession does not own or contain ChannelSession state. Each invocation
may reference at most one ReasoningSessionId, and the channel remains valid when
that reference is absent.

## 8. Authentication And Security Boundary

The v2 channel authentication contract is stateless:

```text
Authorization: Bearer <channel-token>
```

The token authorizes a request or reattachment attempt. It is not a Session
Log event, channel business payload, or UI field.

The Registry and endpoint may maintain registration, lease and channel-session
state, but not server-side authentication sessions. Token rotation produces
an explicit reauthentication/rebind result; it does not silently preserve an
invalid connection.

## 9. Mainlines

### 9.1 Local reasoning path

```text
UI input
  -> UI adaptor command
  -> design-orchestration-plugin
  -> runtime-group Reasoning Service
  -> sessionlog append
  -> backend request from derived surface
  -> backend result/error
  -> sessionlog append
  -> typed UI projection
```

### 9.2 Channel path

```text
endpoint startup
  -> Registry registration
  -> controller discovery
  -> capability reconciliation
  -> logical ChannelSession open
  -> Connection attach
  -> typed capability invocation
  -> Payload result or Error chain
  -> ChannelSession state update
```

### 9.3 Backend replacement path

```text
backend replacement request
  -> Cordis unload old provider
  -> settle/interrupt old operations
  -> activate new provider
  -> read canonical Session Log
  -> derive current surface
  -> continue at a safe boundary
```

## 10. Payload Sharing Contract

`Arc<T>` is the default local Rust ownership and handoff contract for an
immutable business payload. It is not only a performance hint. The producer
constructs one `ImmutablePayload`, and adjacent local plugins, Services and
orchestrator edges pass cloned `Arc` handles to consumers:

```text
typed input
  -> one ImmutablePayload construction
  -> Arc<ImmutablePayload>
  -> Arc::clone across adjacent local nodes
  -> read-only consumers observe one allocation
```

The contract is:

- `Arc::clone` may increase the reference count but must not deep-copy the
  payload;
- `ImmutablePayload` must not expose interior mutation through
  `Arc<Mutex<_>>`, `Arc<RwLock<_>>` or an equivalent mutable shared container;
- every local consumer receives the same immutable value and may not rebuild
  it from control events, metadata or debug state;
- each shared type documents its creator, persistence owner, serialization
  boundary and release boundary.

Copying is allowed only at explicit boundaries:

1. initial typed payload construction;
2. serialization/deserialization across a process, persistence or network
   boundary;
3. an explicit schema transformation where the receiving contract differs.

An adapter at a remote boundary reconstructs its own immutable value after
wire transfer. `Arc` therefore does not claim zero-copy across processes or
machines. ControlEvent, ErrorEvent, metadata and debug observations remain
separate typed resources; they may carry a payload reference or digest, but
never embed business payload bytes.

The M1 contract test must prove `Arc::ptr_eq` for adjacent local consumers,
prove payload equality after every permitted explicit copy point, and reject
deep-copy and mutable-sharing implementations. The black-box test must also
prove that a remote-shaped serialization boundary creates a new allocation
without changing semantic payload content.

## 11. Explicit Non-goals

- Cordis is not reimplemented in Rust in this design milestone.
- OpenCode is not copied into Freehand and its database is not made a second
  source of truth.
- The central Registry is not a reasoning scheduler or business payload store.
- A transport disconnect does not create a local success result.
- Capability mismatch does not trigger hidden fallback.
- `Arc<T>` is not a channel session store, persistence mechanism, control
  channel, global context container or distributed memory.
- Production cloud Registry deployment, distributed consensus and remote
  execution remain implementation milestones after the contracts freeze.

## 12. Remaining Implementation Decisions

The following are intentionally not treated as decided:

1. exact Rust-to-Cordis bridge mechanism;
2. exact OpenCode adaptor transport (`sdk-next` embedded versus external
   OpenCode server);
3. generic `reorder` event schema and conflict semantics;
4. Registry lease and route-change wire schema;
5. concrete durable storage adapter for the channel-session plugin.
