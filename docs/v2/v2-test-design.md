# Freehand v2 Test Design

Status: design-admitted
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
Project black-box contract: `docs/v2/v2-project-blackbox-verification.md`
UI design: `docs/v2/v2-ui-design.md`

## Test Contract

Every v2 module follows the same evidence order:

```text
white-box owner test
  -> module black-box contract test
  -> project black-box vertical-slice test
  -> architecture/boundary gate
  -> build and static validation
  -> public-entrypoint proof when a public entry exists
```

This document is a design contract. It does not claim that any v2 runtime
module is implemented or verified. A module remains `planned` until its source
symbols, tests, verification command and project black-box binding exist.

The test design must prove both directions for control-sensitive behavior:

- accepted/successful and rejected/failed;
- terminal and non-terminal;
- payload preserved and control/payload contamination rejected;
- local capability success and capability failure;
- append/replay success and corruption/lock/version failure;
- network admission and version/capability/disconnect rejection.

Test fixtures are source inputs or deterministic in-memory values. Runtime
logs, screenshots, `target/`, `artifacts/`, external checkouts and AppSDK local
control state are not test inputs and are never committed.

## Module Matrix

| module_id | owner | white-box scope | module black-box scope | project black-box scope | gate |
| --- | --- | --- | --- | --- | --- |
| `v2-contracts` | contracts owner | ID stability, typed command/event/error schemas, immutable payload envelope, Arc pointer sharing, frame-class separation | encode/decode public contract and unknown-field/version rejection | command and event values survive the complete local path without control fields entering payload and adjacent consumers share one immutable allocation | `v2_contract_boundary`, `v2_payload_control_isolation`, `v2_arc_payload_sharing` |
| `v2-control-events` | event owner | event creation, sequence ordering, terminal classification, explicit error chain, replay cursor, payload reference integrity | producer-to-ledger-to-orchestrator event flow, positive and negative event admission | plugin and lifecycle events drive the expected Cordis transition and terminal projection without copying or mutating the payload | `v2_control_event_positive_negative`, `v2_payload_control_isolation`, `v2_arc_payload_sharing` |
| `v2-sessionlog` | Session Log owner | append/read/replay/restore, surface derivation, replace/retract/reorder/fork, interrupted-turn recovery and explicit storage errors | canonical log contract with one deterministic local adapter and one DSH-shaped recovery fixture | one accepted input produces one durable reason surface, survives restore, and does not double-write | `v2_sessionlog_surface_recovery` |
| `v2-reasoning-backend` | reasoning backend owner | provider-neutral Service, backend capability declaration, normalized events/errors, safe interruption and replacement | native backend plus OpenCode adaptor conformance at the same Service boundary | two runtime groups can use different backends; a backend switch occurs only at a safe Session boundary | `v2_reasoning_backend_conformance`, `v2_reasoning_backend_switch` |
| `v2-plugin-capabilities` | capability plugin owner | typed capability manifest, deterministic registration, Rust leaf/composition plugin input/output, permission and failure behavior | Cordis plugin composition invokes one local Rust capability | declared plugin result/failure returns through typed events without semantic duplication | `v2_plugin_registration`, `v2_cordis_composition_boundary` |
| `v2-cordis-ecosystem` | Cordis ecosystem owner | fixed design plugin, runtime-group scope, Service injection, child-plugin lifecycle and hot replacement | Cordis composition consumes owner contracts and does not own payload/event/reason semantics | complete local vertical slice runs through the fixed design plugin and selected backend | `v2_cordis_composition_boundary`, `v2_plugin_hot_replace` |
| `v2-ui-adaptor` | UI adaptor owner | command validation, query/subscribe separation, projection and redaction | adaptor client/server contract with source identity fields | one UI command enters the local graph and receives owner-backed projection only; browser layout follows `docs/v2/v2-ui-design.md` | `v2_ui_projection_boundary` |
| `v2-channel-registry` | channel/registry owner | stateless token admission, endpoint registration, capability discovery/reconciliation, link frames, ChannelSession reconnect state | in-memory Registry and transport test double with accepted/rejected registration, generation and reattach paths | local MVP exercises contracts only; connection replacement retains ChannelSession state and never fabricates local completion | `v2_registry_registration`, `v2_channel_session_reconnect`, `v2_payload_control_isolation` |

## `v2-contracts`

### Lifecycle and logic

```text
typed value construction
  -> schema validation
  -> serialization boundary
  -> deserialization
  -> equality and semantic preservation
```

White-box tests must lock stable IDs, explicit enum variants, required fields,
unknown-field rejection, and the distinction between `Payload`,
`ControlEvent`, and `ErrorEvent`. The same bytes must not deserialize as more
than one frame class. The immutable payload test must pass `Arc` handles to at
least two adjacent consumers and assert `Arc::ptr_eq`; it must reject
`Arc<Mutex<_>>`, `Arc<RwLock<_>>` and equivalent mutable-sharing envelopes.

Positive cases cover valid local commands, immutable payloads, shared pointer
identity and valid error chains. Negative cases cover empty IDs, invalid
protocol versions, missing required fields, unknown variants, control fields
placed in payload values, deep-copying between adjacent consumers and mutable
shared payload containers.

### Black-box impact

The contract boundary is consumed by every other module. The module black-box
test must use only the public contract API. The project black-box test must
assert that a payload sentinel reaches the plugin/reason path unchanged while
an event sentinel remains visible only on the control path.

### Known gaps

- no production source crate exists yet;
- wire encoding format is not selected beyond the typed contract requirement;
- payload reference/transfer encoding is reserved for the network milestone;
  local `Arc` sharing does not cross a process or machine boundary.

## `v2-control-events`

### Lifecycle and logic

```text
producer
  -> validate event owner and adjacent predecessor
  -> assign stable EventId and sequence
  -> append event
  -> route to orchestration
  -> emit projection or explicit ErrorEvent
```

White-box tests cover ordered append, duplicate sequence rejection, unknown
event rejection, non-terminal preservation, terminal acceptance, explicit
error-chain routing and preservation of an associated immutable payload
reference. A failed event must not be converted into a successful terminal
fact or cause a payload deep copy.

### Black-box impact

The module black-box test supplies events through the public event port and
observes the public ledger/replay port. It must prove replay starts at the
requested cursor and that an event acknowledgement is not a business result.

### Known gaps

- event ledger persistence is not implemented;
- cross-node replication and acknowledgement durability are network scope.

## `v2-sessionlog`

### Lifecycle and logic

```text
accepted command
  -> append input fact
  -> derive model-visible surface
  -> append result/recovery fact
  -> persist cursor
  -> restore, replay or fork
```

Positive tests cover append, ordered surface derivation, replacement,
retraction, ordering, cursor restore, fork from a closed boundary and clean
interrupted-turn recovery. Reverse tests corrupt a record, use an invalid
cursor or target, simulate a lock/permission error, and provide a version
mismatch. Each failure must surface explicitly; no second storage path may
claim the turn.

The adapter contract must make the external `~/code/sessionlog` integration
replaceable without allowing Cordis, UI, plugin or network modules to open
that directory directly.

### Black-box impact

The project black-box test restarts the adapter boundary using the same
session identifier and proves the recovered history equals the pre-restart
history. It also proves a failed append leaves no successful terminal
projection.

### Known gaps

- the external sessionlog repository/API is not yet inspected or bound;
- local path, locking and migration semantics must be decided in M2;
- generic reorder conflict semantics remain a freeze item;
- provider-specific reasoning is covered by `v2-reasoning-backend`.

## `v2-reasoning-backend`

### Lifecycle and logic

```text
runtime-group binding
  -> injected Reasoning Service
  -> canonical Session Log surface
  -> backend execution
  -> normalized event/delta/error
  -> Session Log append or explicit recovery
```

Positive tests run the native backend and an OpenCode adaptor against the same
provider-neutral Service contract. They must prove normalized event identity,
session correlation, interruption and capability reporting. Negative tests
cover malformed backend output, provider failure, stale backend generation,
in-flight replacement and OpenCode state that cannot be reconciled with the
canonical Session Log.

The OpenCode adaptor may use its SDK or an external server only behind this
contract. It must not expose OpenCode-native persistence as Freehand truth.

### Black-box impact

The project black-box test binds two isolated runtime groups to different
backends, then proves that each uses its configured backend and that a switch
does not migrate an in-flight request or change historical Session Log facts.

### Known gaps

- exact Rust-to-Cordis bridge and OpenCode transport are implementation
  decisions;
- provider wire payloads remain adaptor-owned.

## `v2-plugin-capabilities`

### Lifecycle and logic

```text
Cordis capability plugin manifest
  -> schema and permission validation
  -> deterministic registration
  -> typed invocation
  -> typed result or ErrorEvent
```

Positive tests invoke one local Rust plugin with valid input and verify output
identity and declared event emissions. Negative tests reject duplicate plugin
IDs, undeclared capabilities, wrong input/output contract IDs, unauthorized
side effects and plugin failures without converting them to success.

### Black-box impact

The project black-box test registers exactly one plugin and invokes it through
the fixed Cordis design plugin. It must fail if Cordis bindings or the
orchestration plugin imports or duplicates capability semantic logic.

### Known gaps

- plugin ABI and process isolation are not part of the MVP;
- remote plugin attestation and delegated execution are reserved for the
  channel/registry extension.

## `v2-cordis-ecosystem`

### Lifecycle and logic

```text
Cordis root
  -> fixed design orchestration plugin
  -> runtime-group scope
  -> Service injection
  -> capability invocation and Reasoning Service
  -> owner-backed terminal or non-terminal projection
```

The Cordis ecosystem owner may compose adjacent plugins and carry correlation
identity. It may not validate payload semantics, implement event policy, write
Session Log truth, parse UI rendering data, or create network fallback.

Positive tests cover root -> fixed design plugin -> capability -> reasoning ->
terminal. Reverse tests cover rejected composition, missing Service injection,
plugin failure, non-terminal reasoning result, duplicate event and explicit
stop. Each case must end in an owner-backed state, not an orchestration-local
boolean.

### Black-box impact

The project black-box contract is the primary acceptance surface for this
module. It must inspect the command receipt, control events, plugin result,
sessionlog records and final UI projection as separate observations.

### Known gaps

- Cordis dependency/API is not yet selected or bound;
- no ecosystem binding source exists;
- retry policy remains an event/reason owner decision, not a generic loop.

## `v2-ui-adaptor`

### Lifecycle and logic

```text
UI ingress
  -> typed command validation
  -> design plugin command port
  -> owner-backed query/projection
  -> subscribe update
```

Positive tests cover one accepted command, one query and one subscription
update. Negative tests cover malformed commands, unknown command versions,
query/subscribe confusion, hidden control fields in public payloads, and UI
attempts to mutate session/event/plugin/network truth.

### Black-box impact

The project black-box test runs through the public protocol boundary and
asserts that the UI receives only a typed projection. The command receipt may
identify the accepted operation, but it must not expose internal payload,
metadata or control structures as user text.

### Known gaps

- the concrete UI transport is not selected;
- browser/mobile rendering is intentionally out of scope for M0/M1;
- public projection schemas are not implemented.

## `v2-channel-registry`

### Lifecycle and logic

```text
endpoint registration
  -> stateless token admission
  -> capability/route discovery
  -> generation reconciliation
  -> ChannelSession open and Connection attach
  -> ControlFrame or PayloadFrame
  -> ErrorFrame / ack / replay
```

The in-memory Registry/transport test double must preserve frame class and
correlation identity. Positive tests cover registration, discovery, matching
versions/capabilities, ordered delivery, acknowledgement, replay and
connection replacement with retained ChannelSession state. Negative tests
cover unknown endpoint, invalid token, unsupported version/capability,
generation mismatch, sequence gap, payload/control class confusion and
explicit disconnect.

Disconnect tests must expose `disconnected`, `reconnecting` or `failed`
truth according to the contract. They must not silently execute locally or
reconstruct control state from a business payload.

### Black-box impact

The project black-box test only exercises this module as an isolated contract
test. The MVP local path must not route through a production network adapter.
This prevents the network skeleton from becoming an accidental second runtime
truth path.

### Known gaps

- no sockets, TLS, Relay deployment or distributed scheduler;
- authentication mechanism is a contract placeholder;
- payload transfer/reference storage is not implemented.

## Verification Mapping

| gate_id | required evidence | positive lock | negative lock |
| --- | --- | --- | --- |
| `v2_contract_boundary` | public contract test result | valid IDs and schemas round-trip | invalid/unknown contract rejected |
| `v2_payload_control_isolation` | boundary test result plus source scan | payload remains unchanged | control/metadata/error fields cannot enter payload |
| `v2_arc_payload_sharing` | Arc ownership test plus local black-box handoff result | adjacent local consumers pass `Arc::clone` handles to the same immutable allocation; permitted boundary copies preserve value | deep-copy or mutable-shared payload rejected; process/network boundaries do not claim pointer identity |
| `v2_control_event_positive_negative` | event ledger contract result | ordered accepted event routes | rejected/duplicate event cannot terminalize |
| `v2_sessionlog_surface_recovery` | Session Log adapter result | append/surface/replay/restore/fork is stable | corruption/lock/version/invalid-operation failure is explicit |
| `v2_reasoning_backend_conformance` | two-backend conformance result | native and OpenCode adaptors emit one contract | malformed/stale backend output is rejected |
| `v2_reasoning_backend_switch` | runtime-group replacement result | switch occurs at safe Session boundary | in-flight request is not silently migrated |
| `v2_plugin_registration` | plugin registry result | one declared plugin invokes | undeclared/failed invocation is explicit |
| `v2_cordis_composition_boundary` | Cordis composition result | fixed design plugin connects adjacent owners | Cordis does not duplicate semantic owners |
| `v2_plugin_hot_replace` | plugin lifecycle result | unload/reactivation is scoped | stale effects/providers cannot remain active |
| `v2_ui_projection_boundary` | public protocol result | command/query/subscribe are distinct | UI cannot mutate owner truth or expose control |
| `v2_registry_registration` | Registry result | token-authenticated endpoint registers/discovers | invalid token or manifest is rejected |
| `v2_channel_session_reconnect` | channel reattach result | new Connection retains ChannelSession | stale generation/disconnect cannot fabricate completion |

## Completion Rule

No module is complete from a unit test alone. Completion requires the module
black-box test, the project black-box impact, architecture gates and the
AppSDK lifecycle records bound to the exact candidate source. M0 has no runtime
completion claim because all module statuses remain `planned`.
