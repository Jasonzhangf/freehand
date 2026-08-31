# Freehand v2 Foundation, MVP, UI/Reason and Network Plan

Status: design-admitted
Branch: `v2`
Governance: AppSDK `0.1.5`
Baseline: `5c879c6155d8c3e6febc3d0a13b4716b9f544948`

## 1. Decision Summary

Freehand v2 is an independent project carried by the `v2` worktree and the
`origin/v2` branch. The v1 tree is a read-only reference during this plan.
Only v2 project source, contracts, documentation, tests, governance records,
and reviewed lifecycle evidence may be committed. Build outputs, generated
runtime state, external checkouts, credentials, and local AppSDK control state
must remain outside the committed change set.

The MVP is a single-machine, one-agent vertical slice. It must still expose
stable contracts for future multi-machine collaboration and network plugins.
The future network path is an extension boundary, not an MVP implementation.

The network extension is included in M0/M1 as typed contracts and in-memory
transport tests only. Production network execution remains out of scope.

```text
UI input
  -> UI protocol command
  -> Cordis orchestration
  -> Rust plugin capability
  -> typed control event
  -> Arc immutable payload handoff
  -> sessionlog-backed reasoning
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
| `v2-contracts` | IDs, typed commands/events, payload nodes, errors, capability declarations | protocol versioning, node identity, payload references |
| `v2-orchestration` | Cordis-facing node lifecycle, sequencing, continuation and stop wiring | distributed scheduling and remote node coordination |
| `v2-control-events` | typed control event creation, routing, ordering and terminal/error paths | event replication, acknowledgements and replay across nodes |
| `v2-reason-sessionlog` | reasoning adapter, session truth, cursor, append/replay and recovery | shared session views and remote session ownership |
| `v2-plugin-runtime` | Rust plugin trait, capability manifest, local registration and execution | remote plugin discovery, attestation and delegated execution |
| `v2-ui-protocol` | UI ingress, query/subscribe, public projections and redaction | multi-client and remote UI transport |
| `v2-network-extension` | MVP protocol skeleton and test transport boundary only | authenticated node transport, remote plugins and event/payload transfer |

Dependency direction:

```text
v2-contracts
  -> v2-control-events
  -> v2-reason-sessionlog
  -> v2-plugin-runtime
  -> v2-orchestration
  -> v2-ui-protocol
  -> v2-network-extension
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

### 2.2 Cordis orchestration

`v2-orchestration` owns the executable graph:

- node admission and adjacent-node transitions;
- lifecycle start, wait, continue, stop, retry and terminal handling;
- dispatch of plugin capabilities;
- consumption of control events;
- creation of UI projections from owner-backed truth.

Cordis is an orchestration owner only. Semantic validation remains in
`v2-contracts`, event decisions remain in `v2-control-events`, reasoning truth
remains in `v2-reason-sessionlog`, and plugin behavior remains in
`v2-plugin-runtime`.

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

### 2.4 Rust plugins

The first plugin is a local Rust capability with:

- stable plugin and capability IDs;
- declared input/output contract IDs;
- declared event emissions;
- explicit side effects and permissions;
- deterministic registration;
- typed failure returned to the error chain.

The plugin runtime must not contain reasoning policy, UI parsing, retry policy,
or network fallback. A remote plugin later uses the same capability contract
through a network adapter; it does not create a second plugin semantic model.

### 2.5 Arc payload sharing

`Arc<T>` is an in-process ownership optimization for immutable payloads only.
It is not a truth store, persistence mechanism, control channel, or global
context container.

```text
typed payload value
  -> Arc<ImmutablePayload>
  -> adjacent consumers

typed ControlEvent / ErrorEvent remain separate
```

Every shared type must state whether it is immutable, who creates it, who
owns persistence, and when the last consumer may release it. Mutable
coordination state uses its own typed owner and synchronization boundary.

### 2.6 Sessionlog reasoning

The reasoning owner exposes a storage-neutral sessionlog contract:

- append turn/event;
- read ordered session history;
- persist and restore cursor;
- replay a bounded session;
- recover an interrupted turn;
- report explicit corruption, lock, permission and version errors.

The runtime uses the contract, not a hard-coded directory. An external
`~/code/sessionlog` implementation is a future adapter and must not be read
directly by Cordis, UI, plugins or network code. There is no silent fallback
or double-write truth path.

### 2.7 UI and reasoning split

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

## 3. Multi-machine and Network Extension Contract

The MVP reserves, but does not implement, these network concepts:

| Contract | Required invariant |
| --- | --- |
| node identity | stable `NodeId`, declared role/capability set, no inferred peer identity |
| capability discovery | versioned manifest, explicit owner, permission and input/output contract |
| authentication | authenticated transport/session before control or payload admission |
| protocol version | explicit negotiation; unknown version fails closed |
| event transport | ordered event sequence, source identity, correlation ID, replay boundary |
| acknowledgement | control acknowledgement is separate from business result |
| payload transfer | payload reference/hash or explicit transfer frame; control data never embedded |
| disconnect recovery | explicit disconnected/reconnecting/failed truth; no silent local substitute |
| remote plugin | local contract validation before invocation; remote errors enter typed error chain |
| backpressure | bounded queues and explicit capacity/error semantics |

Network frames are divided into three physically separate classes:

```text
ControlFrame: identity, handshake, capability, event sequence, ack, cancel
PayloadFrame: immutable business payload or payload reference
ErrorFrame: correlated typed transport/plugin/error-chain failure
```

No network adapter may reconstruct control state from business payload. No
remote plugin may write local session or UI truth directly. Remote execution
must return through the same plugin result and error contracts as local
execution.

MVP network work is limited to:

1. contract definitions;
2. an in-memory transport test double;
3. positive and negative protocol tests;
4. capability/version rejection tests;
5. explicit disconnect and replay semantics.

It does not include sockets, TLS deployment, remote scheduling, production
Relay, cross-machine filesystem access, or distributed consensus.

## 4. MVP Scope

### In scope

1. One UI client and one local agent.
2. One Cordis orchestration graph with start, plugin call, reason turn and
   terminal projection.
3. One Rust plugin with deterministic local registration.
4. One typed event control path with positive and negative terminal tests.
5. One immutable `Arc` payload handoff with ownership tests.
6. One sessionlog adapter with append, restore and replay tests.
7. One UI protocol surface with command, query and projection tests.
8. Network contract and in-memory transport tests described above.
9. AppSDK records and maps for every implemented module and milestone.

### Out of scope

- multi-machine task scheduling;
- remote plugin execution over production sockets;
- Relay deployment and account federation;
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

Implement the minimal typed IDs, payload envelope, control event, error chain,
and in-memory event ledger. Prove physical separation from payload.

### M2: Sessionlog reason path

Implement the storage-neutral sessionlog trait/contract and one local adapter.
Close append -> reason turn -> replay -> restore without double-write.

### M3: Rust plugin and Cordis graph

Implement one local Rust plugin and the smallest Cordis graph that invokes it,
receives its result event, runs one reason turn, and emits terminal truth.

### M4: UI protocol

Implement the UI adaptor and the first compact operating console. Expose one
command, one query and one subscription/projection. Prove UI is only
ingress/read-only projection and cannot mutate reason or event truth. Use
`docs/v2/v2-ui-design.md` as the layout, state and adaptor contract.

### M5: Network extension skeleton

Implement only typed node/capability/version/frame contracts and an in-memory
transport test double. No production sockets or remote execution.

### M6: MVP closeout

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
- AppSDK 0.1.5 verifies the project and the exact v2 branch contains no
  build, external, runtime or secret artifact.
