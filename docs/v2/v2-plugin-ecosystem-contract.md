# Freehand v2 Plugin Ecosystem Contract

Status: freeze-candidate
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Parent architecture: `docs/v2/v2-cordis-reasoning-channel-architecture.md`
UI contract: `docs/v2/v2-ui-plugin-contract.md`
MVP plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`

## 1. Frozen Decision

Freehand v2 follows one top-level rule:

> Every executable, replaceable or externally connected product part is a
> Cordis plugin.

Cordis is the only plugin substrate and lifecycle owner. Freehand does not
create a second loader, Service registry, event bus or hot-reload manager.

The fixed design orchestration component is itself a Cordis plugin. It is
fixed as the composition contract and root entry, not as a privileged runtime
layer. Its child implementations remain independently loadable, replaceable
and unloadable.

The rule includes:

- design orchestration;
- control-event, error and policy components;
- reasoning backends and provider adaptors;
- Session Log and storage adaptors;
- local Rust capabilities and composed capabilities;
- remote capability endpoint adaptors;
- Registry, link, transport and ChannelSession components;
- Notification, Topology, Session Canvas, Search and Memory;
- UI adaptor, UI Shell and every replaceable UI surface;
- external-runtime adaptors and test doubles when loaded by the runtime.

Pure contracts, IDs and immutable shared data types are not runtime plugins by
themselves. They are the typed port contracts consumed by plugins. This keeps
the type layer from becoming a second runtime while keeping all behavior
under plugin ownership.

## 2. Root Composition

```text
Cordis root
└── design-orchestration-plugin
    ├── control-plane
    │   ├── control-event-plugin
    │   ├── error-chain-plugin
    │   └── metadata/debug observation plugins
    ├── reasoning
    │   ├── sessionlog-plugin
    │   │   └── sessionlog-storage-adaptor
    │   ├── reasoning-service-plugin
    │   │   ├── freehand-reasoning-backend
    │   │   └── opencode-reasoning-adaptor
    │   └── context/rewrite policy plugins
    ├── capabilities
    │   ├── capability-registry-plugin
    │   ├── local-rust-capability-plugin
    │   └── remote-capability-endpoint-plugin
    ├── channels
    │   ├── registry-plugin
    │   ├── link-plugin
    │   ├── transport-plugin
    │   └── channel-session-plugin
    ├── information
    │   ├── notification-plugin
    │   ├── topology-plugin
    │   ├── session-canvas-plugin
    │   ├── search-plugin
    │   └── memory-plugin
    └── ui
        ├── ui-adaptor-plugin
        ├── ui.shell
        ├── ui.navigation
        ├── ui.run
        ├── ui.sessions
        ├── ui.attention
        ├── ui.location
        ├── ui.more
        └── ui.detail
```

The tree describes composition, not shared ownership. The orchestration
plugin connects declared typed ports and lifecycle edges. It does not own
Session Log records, reasoning decisions, channel state, search indexes,
memory records or UI presentation state.

## 3. Plugin Kinds

Each plugin declares one primary kind. A plugin may contain child plugins, but
each child remains independently addressable.

| kind | responsibility | example |
| --- | --- | --- |
| `composition` | mount children and connect adjacent ports | design orchestration |
| `service` | provide a typed runtime service | Reasoning Service |
| `capability` | execute one typed capability | local Rust CLI adapter |
| `adaptor` | translate an external runtime or transport | OpenCode, Session Log, UI |
| `projection` | derive an owner-backed read surface | Search, Memory, Topology |
| `transport` | propagate frames and connection state | WebSocket link |
| `persistence` | store and restore one owner resource | Session Log storage |
| `policy` | make one declared semantic decision | rewrite policy |
| `surface` | render and interact with one UI slot | `ui.run` |

`plugin_kind` describes behavior. It does not grant resource ownership,
permission to bypass a port or permission to mutate another plugin's truth.

## 4. Plugin Identity And Compiled Definition

Runtime loading consumes a compiled plugin manifest, not an arbitrary source
directory. Each definition declares:

| field | meaning |
| --- | --- |
| `plugin_id` | concrete implementation identity |
| `plugin_kind` | primary plugin kind |
| `instance_id` | mounted runtime instance |
| `scope_id` | Cordis scope or runtime group |
| `contract_versions` | consumed and provided port versions |
| `requires` | typed services and ports required before activation |
| `provides` | typed services, projections or capabilities exposed |
| `owns_resources` | resources this plugin may mutate or persist |
| `projects_resources` | resources it may derive read-only |
| `emits_events` | typed event families it may emit |
| `accepts_commands` | typed command families it may accept |
| `replacement_policy` | settlement and safe-boundary requirements |
| `permissions` | declared side effects and external access |
| `build_identity` | implementation and dependency identity |

Unknown fields, duplicate identities, missing owner bindings, undeclared
permissions, incompatible ports and forbidden resource edges fail admission.

`plugin_id` identifies an implementation and may change during replacement.
Stable replacement uses a separate boundary such as `slot_id`,
`capability_id`, `service_id` or `resource_id`.

## 5. Typed Port Rules

Plugins connect only through declared typed ports:

```text
provider plugin
  -> provided typed port
  -> adjacent consumer plugin
```

These port families are distinct:

| port | purpose | content |
| --- | --- | --- |
| `service` | request/response behavior | typed service calls |
| `event` | ordered control facts | typed control events |
| `projection` | read-only surface | owner-backed projections |
| `command` | mutation request | command plus receipt |
| `query` | read request | query plus result |
| `subscribe` | stream request | cursor plus typed updates |
| `payload` | business data | immutable payload or reference |
| `error` | failure chain | typed error facts |
| `transport-control` | connection lifecycle | handshake, ack, cancel, route |

The following substitutions are invalid:

- command for query;
- query for subscription;
- control event for business payload;
- metadata/debug for business payload;
- error for success;
- transport frame for Session Log record.

Every edge is adjacent in the declared mainline. A consumer may not bypass an
adaptor, owner or required intermediate resource by importing another plugin's
implementation directly.

## 6. Ownership And Nested Composition

One resource has one truth owner. A plugin may be:

- the sole mutating owner;
- a read-only projector;
- a translating adaptor;
- a composition owner for lifecycle wiring.

Nested composition does not merge ownership. Every child retains its own:

- identity and Cordis scope;
- resource owner;
- event cursor;
- cancellation and settlement boundary;
- persistence boundary;
- replacement lifecycle.

The parent may start, stop, replace or unload a child through the declared
lifecycle contract. It may not inspect or mutate child-private truth.

Valid forms include both:

```text
leaf Rust plugin

composition plugin
  └── adaptor plugin
      └── external runtime
```

## 7. Forbidden Shortcuts

The following paths are architecturally invalid:

```text
UI plugin -> Session Log/private reasoning implementation
UI plugin -> provider or network implementation
orchestration plugin -> capability semantic implementation
transport plugin -> local Session/Turn/UI truth
projection plugin -> source-resource mutation
adaptor plugin -> second persistence truth
plugin -> control state reconstructed from payload text
```

All of them must route through the registered owner, typed port and required
intermediate resource. A failed port or owner operation is an explicit error;
it is not silently replaced by a local implementation.

## 8. Lifecycle And Replacement

All plugin kinds follow the common lifecycle, with kind-specific gates:

```text
discover
  -> validate definition and ports
  -> create Cordis scope
  -> load
  -> activate dependencies
  -> mount/register
  -> attach typed subscriptions
  -> serve requests
  -> settle or explicitly cancel owned work
  -> detach subscriptions
  -> unload
```

Replacement is scoped to a stable contract boundary:

```text
replacement requested
  -> stop new work at the boundary
  -> settle or interrupt owned operations
  -> flush required cursors and receipts
  -> preserve owner truth and stable identity
  -> unload old implementation
  -> validate replacement
  -> mount replacement in the same scope/boundary
  -> reconnect typed ports
  -> rebuild from owner-backed state
  -> ready or explicit unavailable/error
```

Replacement must not rewrite payloads, delete Session Log history, silently
migrate an in-flight request, change selected resource identity, convert
failure to success, retain an unloaded plugin reference or display stale
success after a failed replacement.

## 9. Data And Control Separation

The plugin rule does not relax the physical data boundary:

```text
ControlEvent / ErrorEvent / Metadata / Debug
  !=
Immutable business payload
```

Local Rust payload handoff is:

```text
one ImmutablePayload construction
  -> Arc<ImmutablePayload>
  -> Arc::clone across adjacent consumers
```

`Arc<T>` is a local immutable ownership handoff. It is not a global context,
truth store, mutable shared state or cross-machine zero-copy mechanism.

Serialization occurs only at persistence, process, network or explicit schema
conversion boundaries. Control frames may carry a payload reference or digest,
but never hidden business payload bytes.

## 10. Runtime Groups And UI Replacement

Each runtime group is a Cordis child scope. Different groups may select
different reasoning backends and UI implementations concurrently:

```text
Cordis root
├── local-freehand runtime group
│   ├── Freehand reasoning backend
│   └── UI plugin family
├── opencode runtime group
│   ├── OpenCode adaptor
│   └── UI or headless surface plugins
└── remote-agent runtime group
    ├── channel-session plugin
    ├── transport plugin
    └── remote capability endpoint
```

Replacing `ui.run`, `ui.navigation` or another UI slot does not replace the
domain plugin that supplies its projection. The replacement rebuilds from the
current projection and selected identity through `ui.adaptor`.

## 11. MVP Consequence

The first MVP proves the plugin rule with one local vertical slice:

```text
ui.run surface plugin
  -> ui.adaptor plugin
  -> design-orchestration plugin
  -> local Rust capability plugin
  -> control-event plugin
  -> Arc immutable payload
  -> Session Log adaptor plugin
  -> one Reasoning Service backend plugin
  -> UI projection
```

The MVP does not require production dynamic-library loading or process
isolation. It does require compiled-definition validation, typed ports, owner
bindings and replacement tests using in-memory plugin implementations.

The next design work is therefore:

1. freeze the plugin manifest and port schemas;
2. bind plugin definitions to resource/function/mainline/verification maps;
3. define the Cordis root and runtime-group bootstrap contract;
4. define one UI slot replacement test contract;
5. only then implement the first Rust plugin and adaptor.

## 12. Freeze Checklist

- [x] Cordis is the only plugin substrate.
- [x] The fixed design orchestration component is a Cordis plugin.
- [x] Executable, replaceable and externally connected product parts are
  plugins.
- [x] UI is a replaceable plugin family, not one monolith.
- [x] Nested composition and leaf Rust plugins are supported.
- [x] Plugin identity is separated from stable replacement boundaries.
- [x] Typed service, event, projection, command, query, subscribe, payload,
  error and transport-control ports are distinct.
- [x] Each resource retains one truth owner.
- [x] Replacement preserves owner truth and stable identity.
- [x] Local immutable payload handoff uses `Arc<T>`.
- [ ] full Rust manifest and typed port schemas (MVP covers plugin role/registration manifest only);
- [x] compiled plugin registry: MVP in-memory typed `PluginRole`/`PluginRegistration` registry in `v2-cordis-ecosystem`;
- [ ] runtime replacement gate;
- [ ] dynamic loading/process-isolation policy.

The unchecked items are the next design phase. They do not reopen the
decisions above.
