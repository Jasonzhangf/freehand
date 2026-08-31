# Freehand v2 Foundation, MVP, UI/Reason and Network Plan

Status: design-admitted
Branch: `v2`
Governance: AppSDK `0.1.6`
Baseline: `5c879c6155d8c3e6febc3d0a13b4716b9f544948`

## 1. Decision Summary

Freehand v2 is an independent project carried by the `v2` worktree and the
`origin/v2` branch. The v1 tree is a read-only reference during this plan.
Only v2 project source, contracts, documentation, tests, governance records,
and reviewed lifecycle evidence may be committed. Build outputs, generated
runtime state, external checkouts, credentials, and local AppSDK control state
must remain outside the committed change set.

Cordis is the architecture foundation and the complete plugin ecosystem.
Every Freehand capability is a Cordis plugin. The fixed design orchestration
component is itself a Cordis plugin; it is not a privileged host outside
Cordis. Cordis provides plugin composition, Service dependency activation,
typed events, effects, child plugins, scope isolation, unload, replacement
and hot-reload lifecycle.

The MVP is a single-machine, one-agent vertical slice. It must still expose
stable contracts for multiple runtime groups, multiple reasoning backends,
centralized channel discovery, reconnectable channel sessions, future
multi-machine collaboration and network plugins.

The reasoning implementation is a replaceable Cordis backend seam. The first
backend candidates are the native Freehand reasoning backend and an OpenCode
adaptor backend. They use one provider-neutral Reasoning Service contract, so
different runtime groups may use different bases at the same time.

Every channel endpoint registers with a configured central Registry endpoint at
startup. The Registry owns discovery, endpoint route advertisements,
capability generations and change notifications. A transport connection is
independent from the logical ChannelSession, so a connection can be replaced
without losing the channel session state.

The central Registry, channel session and network transport are included in
the design contracts. Production cloud deployment and distributed execution
remain out of scope for the first local MVP.

```text
UI input
  -> UI protocol command
  -> design-orchestration-plugin
  -> runtime-group Reasoning Service
  -> Cordis-selected reasoning backend
  -> typed control event
  -> Arc immutable payload handoff
  -> canonical sessionlog
  -> derived model surface
  -> reasoning result
  -> typed UI projection
```

Control events, metadata, debug observations, network control frames, and
business payloads are physically separate types and paths.

## 2. Module Partition

Each module has one owner, one resource boundary, one function map, one
mainline call map, and one verification map. The module names below are the
v2 semantic IDs used by AppSDK records.

The formal test design and project black-box contract are:

- `docs/v2/v2-test-design.md`
- `docs/v2/v2-project-blackbox-verification.md`
- `docs/v2/v2-ui-design.md`

| Module | MVP responsibility | Future extension |
| --- | --- | --- |
| `v2-contracts` | provider-neutral IDs, commands/events, payloads, errors, capability and session contracts | protocol versioning and payload references |
| `v2-cordis-ecosystem` | fixed design orchestration plugin, runtime-group composition and Cordis bindings | hot replacement and multi-runtime composition |
| `v2-control-events` | Cordis typed control events, ordering, replay and error-chain boundary | event replication and acknowledgements |
| `v2-sessionlog` | canonical append/replay/surface/fork/recovery Session Log | persistence adapters and shared session views |
| `v2-reasoning-backend` | provider-neutral Reasoning Service and Freehand/OpenCode backend adaptors | additional reasoning bases and runtime-specific bindings |
| `v2-plugin-capabilities` | Rust leaf/composition capability plugins and manifests | remote plugin execution and attestation |
| `v2-ui-adaptor` | UI ingress, query/subscribe, public projections and backend-neutral adaptor | multi-client and remote UI transport |
| `v2-notification-plugin` | importance/time ordered notification projection and acknowledgement lifecycle | retention, cross-node notification delivery and policy updates |
| `v2-topology-plugin` | physical machine/node/Agent/Channel grouping and relationship projection | multi-machine topology views and live route visualization |
| `v2-session-canvas-plugin` | Session Log-derived active/recent/history session graph | large-history layout, cross-node session graph and replay navigation |
| `v2-search-plugin` | typed keyword/filter search, classification, index and cache lifecycle | distributed indexing and cross-node search |
| `v2-memory-plugin` | session summarize/record/save/load/search/export lifecycle | shared memory policy, retention and cross-session federation |
| `v2-channel-registry` | central Registry, link/transport/channel session contracts and capability reconciliation | cloud deployment, relay and multi-machine execution |

Dependency direction:

```text
v2-contracts
  -> v2-control-events
  -> v2-sessionlog
  -> v2-reasoning-backend
  -> v2-plugin-capabilities
  -> v2-cordis-ecosystem
  -> v2-ui-adaptor
  -> v2-notification-plugin
  -> v2-topology-plugin
  -> v2-session-canvas-plugin
  -> v2-search-plugin
  -> v2-memory-plugin
  -> v2-channel-registry
```

The arrows are dependency permission, not a promise that every module must
depend directly on every predecessor. Network code must never become the
owner of reasoning, task, UI, or business payload truth.

### 2.1 Contracts

`v2-contracts` owns semantic types only:

- `NodeId`, `SessionId`, `TurnId`, `EventId`, `PluginId`, `CapabilityId`;
- `UiCommand`, `UiProjection`, `ControlEvent`, `ErrorEvent`;
- immutable payload envelopes and payload-reference contracts;
- protocol version and capability negotiation records;
- explicit error-chain contracts.

It does not own provider wire DTOs, UI rendering, storage implementation,
network sockets, debug envelopes, or metadata.

### 2.2 Cordis ecosystem and fixed design orchestration plugin

`v2-cordis-ecosystem` owns only the Freehand bindings around Cordis:

- runtime-group composition;
- Service Definition and `inject` bindings;
- fixed design orchestration plugin;
- child-plugin and scope boundaries;
- unload/replacement lifecycle wiring;
- adjacent Service/event connections.

Cordis itself owns plugin loading, dependency activation, typed event dispatch,
effects and hot replacement. Freehand does not duplicate those mechanisms.
Semantic validation remains in `v2-contracts`, Session Log truth remains in
`v2-sessionlog`, reasoning decisions remain in `v2-reasoning-backend`, and
capability behavior remains in `v2-plugin-capabilities`.

### 2.3 Control events

The event path is typed and append-oriented:

```text
producer -> ControlEvent -> event owner -> orchestrator decision
                                      -> projection / ledger / error chain
```

MVP event families:

- lifecycle: start, waiting, resumed, stopped, terminal;
- plugin: admitted, invoked, result, failed;
- reasoning: turn started, response received, turn closed;
- UI: command accepted, rejected, projection updated.

The families share catalog rules, IDs, ordering and replay contracts, but do
not collapse into one universal event payload. Control fields cannot be
mirrored into business payload or metadata.

### 2.4 Rust capability plugins

The first plugin is a local Rust capability with:

- stable plugin and capability IDs;
- declared input/output contract IDs;
- declared event emissions;
- explicit side effects and permissions;
- deterministic registration;
- typed failure returned to the error chain.

Rust is one plugin implementation language, not a separate runtime model. A
Rust plugin may be a leaf implementation or a composition plugin. Cordis owns
its registration and lifecycle; the capability contract owns its typed input,
output and error semantics. No plugin may contain hidden reasoning policy, UI
parsing or network fallback.

### 2.5 Arc payload sharing

`Arc<T>` is the default local Rust transport for immutable business payloads.
It is an ownership and handoff contract, not merely a performance hint:

```text
typed input
  -> construct exactly one ImmutablePayload
  -> Arc<ImmutablePayload>
  -> clone Arc handle across adjacent local consumers
  -> each consumer reads the same allocation
```

An `Arc::clone` only increments the reference count. Adjacent local plugins,
Services and orchestrator edges must not deep-copy the payload. `ImmutablePayload`
must not expose interior mutation through `Arc<Mutex<_>>`, `Arc<RwLock<_>>` or
an equivalent shared mutable container.

The contract has explicit copy points:

1. initial typed payload construction;
2. serialization at a process, persistence or network boundary;
3. an explicit schema transformation where the receiving contract differs.

No additional deep copy is allowed between local adjacent nodes. A remote
endpoint reconstructs its own immutable value after wire transfer; `Arc` does
not imply zero-copy across processes or machines. ControlEvent and ErrorEvent
remain separate and never embed payload bytes. A typed payload reference may
associate a control fact with a payload without carrying its business content.

Every shared type must state whether it is immutable, who creates it, who
owns persistence, which boundary serializes it, and when the last consumer may
release it. `Arc<T>` is not a truth store, persistence mechanism, control
channel, global context container or distributed memory.

### 2.6 Canonical Session Log and reasoning

`v2-sessionlog` exposes the canonical DSH-inspired Session Log contract:

- append turn/event;
- derive the ordered model-visible surface;
- append surface replacement/recovery facts;
- read ordered session history;
- persist and restore cursor;
- replay a bounded session;
- fork from a stable closed boundary;
- recover an interrupted turn;
- report explicit corruption, lock, permission and version errors.

The runtime uses the contract, not a hard-coded directory. The DSH Session Log
model is the design reference. The external `~/code/sessionlog` integration is
an adapter milestone and must not be opened directly by Cordis, UI, plugins or
network code. There is no silent fallback or double-write truth path.

The reasoning backend reads the derived surface and appends normalized results
through this owner. OpenCode's native storage cannot become a second Freehand
truth source.

### 2.7 Reasoning backend seam

The reasoning contract is provider-neutral. At least two backends must fit it:

- native Freehand reasoning;
- OpenCode adaptor reasoning.

Each runtime group binds exactly one active backend at a time, while multiple
runtime groups may use different backends concurrently. Backend replacement
is a Cordis unload/reactivation operation at a safe Session boundary. The
backend identity is control/session state and never business payload.

### 2.8 UI and reasoning split

```text
UI app
  -> ui.protocol ingress
  -> orchestration command port
  -> owner modules
  -> ui.protocol projection
  -> UI app
```

UI may submit commands and render projections. It cannot write session truth,
reasoning state, event truth, plugin state, or network state.

Reasoning consumes typed input and emits owner-backed facts. It does not import
UI code or depend on a browser/mobile rendering model.

## 3. Central Registry, ChannelSession and Network Contract

The v2 channel system reserves, but does not implement production distributed
execution, these concepts:

| Contract | Required invariant |
| --- | --- |
| node identity | stable `NodeId`, declared role/capability set, no inferred peer identity |
| capability discovery | versioned manifest, explicit owner, permission and input/output contract |
| authentication | stateless bearer/API-key admission before control or payload admission |
| protocol version | explicit negotiation; unknown version fails closed |
| central registry | configured local/cloud Registry for registration, discovery and route changes |
| channel session | logical controller/controlled session independent of transport connection |
| capability generation | explicit change generation checked before invocation |
| event transport | ordered event sequence, source identity, correlation ID, replay boundary |
| acknowledgement | control acknowledgement is separate from business result |
| payload transfer | payload reference/hash or explicit transfer frame; control data never embedded |
| disconnect recovery | explicit disconnected/reconnecting/failed truth; no silent local substitute |
| remote plugin | local contract validation before invocation; remote errors enter typed error chain |
| backpressure | bounded queues and explicit capacity/error semantics |

Network frames are divided into three physically separate classes:

```text
RegistryControl: registration, discovery, route/capability generation
ChannelControl: handshake, session attach, event sequence, ack, cancel
PayloadFrame: immutable business payload or payload reference
ErrorFrame: correlated typed transport/plugin/error-chain failure
```

No network adapter may reconstruct control state from business payload. No
remote plugin may write local session or UI truth directly. Remote execution
must return through the same plugin result and error contracts as local
execution.

MVP network work is limited to:

1. Registry, channel and ChannelSession contract definitions;
2. an in-memory Registry and transport test double;
3. positive and negative registration/discovery/reconciliation tests;
4. capability/version rejection tests;
5. connection replacement while ChannelSession state is retained;
6. explicit disconnect and replay semantics.

It does not include sockets, TLS deployment, remote scheduling, production
Relay, cross-machine filesystem access, or distributed consensus.

## 4. MVP Scope

### In scope

1. One UI client and one local agent.
2. One Cordis composition with start, plugin call, reason turn and terminal
   projection.
3. One Rust capability plugin with deterministic local registration.
4. One typed event control path with positive and negative terminal tests.
5. One immutable `Arc` payload handoff with ownership tests.
6. One canonical Session Log adapter with append, surface, restore and replay
   tests.
7. One UI protocol surface with command, query and projection tests.
8. Registry, ChannelSession and in-memory transport tests described above.
9. AppSDK records and maps for every implemented module and milestone.

### Out of scope

- multi-machine task scheduling;
- remote plugin execution over production sockets;
- central cloud Registry deployment;
- distributed locks or consensus;
- full v1 UI/Android migration;
- provider-specific wire protocol in contracts;
- runtime scanning of Playground, Protected source or `.appsdk-control`;
- build output, external checkout or runtime state in Git.

## 5. Development Flow

```text
goal confirmed
  -> AppSDK project/map validation
  -> one semantic claim and clean v2 Playground worktree
  -> red test / baseline evidence
  -> smallest owner-local implementation
  -> module boundary self-check
  -> focused white-box tests
  -> module black-box tests
  -> build, fmt, clippy and architecture gates
  -> install/restart/public-entrypoint proof when applicable
  -> AppSDK pre-review validation
  -> architecture review
  -> unchanged-source effectiveness replay
  -> exact commit and serial merge
  -> promotion / Active / Protected only after lifecycle evidence
```

Each milestone changes one semantic owner. A documentation-only milestone
must not claim runtime behavior. A design contract is not an implemented
module until source symbols, tests, and verification entries exist.

Required paired tests for control-sensitive work:

- start/terminal and non-terminal/still-running;
- event accepted and event rejected;
- plugin success and plugin failure;
- payload preserved and control/payload contamination rejected;
- session append/replay success and corruption/lock failure;
- network version/capability accepted and rejected;
- disconnect recovery explicit and silent fallback rejected.

## 6. Milestone Plan

### M0: Governance and maps

Create the v2 project contract, goal confirmation, module registry, resource
map, function map, mainline call map, verification map, test design, project
black-box contract and this design. No product runtime code.

### M1: Contracts and control path

Implement the minimal typed IDs, immutable payload envelope, `Arc` handoff
contract, control event, error chain and in-memory event ledger. Prove both
physical control/payload separation and pointer-sharing across adjacent local
consumers.

### M2: Canonical Session Log

Implement the DSH-inspired canonical Session Log contract and one local
adapter. Close append -> surface derivation -> replay -> restore without
double-write, including explicit replacement and interrupted-turn repair.

### M3: Reasoning backend seam

Implement the provider-neutral Reasoning Service and one native backend
adaptor. Add the OpenCode adaptor contract and an isolated conformance
fixture; do not claim OpenCode runtime integration until its transport and
session-state ownership are verified.

### M4: Rust capability plugin and Cordis composition

Implement one local Rust capability plugin and the smallest Cordis composition
that invokes it, receives its result event, runs one selected reasoning
backend, and emits terminal truth.

### M5: UI adaptor

Implement the UI adaptor and the first compact operating console. Expose one
command, one query and one subscription/projection. Prove UI is only
ingress/read-only projection and cannot mutate reason or event truth. Use
`docs/v2/v2-ui-design.md` as the layout, state and adaptor contract.

### M6: Central Registry and ChannelSession skeleton

Implement only typed Registry registration/discovery, ChannelSession attach and
in-memory transport contracts. Prove connection replacement retains
ChannelSession state. No production cloud deployment or remote execution.

### M7: MVP closeout

Run full module verification, public-entrypoint black-box proof, AppSDK review
and effectiveness replay. Promote only the exact reviewed source artifact.

## 7. Commit and Artifact Policy

Allowed in `origin/v2`:

- v2 source code;
- v2 tests;
- `.appsdk` project contracts, maps and lifecycle records;
- `docs/v2/**`;
- reviewed project design and verification documents.

Forbidden in `origin/v2`:

- `target/**`, `generated/**`, `active/lib/**` before governed promotion;
- `external/**` checkouts;
- `.appsdk-control/**`;
- runtime logs, caches, artifacts and temporary files;
- credentials, tokens, `.env*` and private keys.

Before every commit:

```text
git diff --cached --stat
git diff --cached --name-status
git diff --cached --check
git ls-tree -r --name-only HEAD
```

The staged paths must equal the declared milestone change set. A build may
produce local outputs for verification, but those outputs are never part of
the project commit or remote branch.

## 8. Acceptance

The design is accepted when:

- every MVP module has one owner and a registered resource;
- every mainline edge is adjacent and machine-bound;
- UI/reason, control/payload, local/remote plugin and source/artifact
  boundaries are executable gates;
- the single-machine vertical slice passes positive and negative tests;
- network extension contracts are present without pretending distributed
  runtime exists;
- AppSDK 0.1.6 verifies the project and the exact v2 branch contains no
  build, external, runtime or secret artifact.
