# Freehand v2 Test Design

Status: design-admitted
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.5`
Plan: `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
Project black-box contract: `docs/v2/v2-project-blackbox-verification.md`

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
| `v2-contracts` | contracts owner | ID stability, typed command/event/error schemas, immutable payload envelope, frame-class separation | encode/decode public contract and unknown-field/version rejection | command and event values survive the complete local path without control fields entering payload | `v2_contract_boundary`, `v2_payload_control_isolation` |
| `v2-control-events` | event owner | event creation, sequence ordering, terminal classification, explicit error chain, replay cursor | producer-to-ledger-to-orchestrator event flow, positive and negative event admission | plugin and lifecycle events drive the expected Cordis transition and terminal projection | `v2_control_event_positive_negative`, `v2_payload_control_isolation` |
| `v2-reason-sessionlog` | reason/sessionlog owner | append/read/replay/restore cursor, interrupted-turn recovery, corruption/lock/version errors | storage-neutral adapter contract with one deterministic local test adapter | one accepted input produces one reason turn, survives restore, and does not double-write | `v2_sessionlog_append_replay` |
| `v2-plugin-runtime` | plugin owner | capability declaration, deterministic registration, typed input/output, permission and failure behavior | registry-to-invocation contract with one local Rust plugin | Cordis invokes one plugin and routes result/failure through typed events and reason input | `v2_plugin_registration` |
| `v2-orchestration` | orchestration owner | adjacent-node transitions, continuation, stop, retry and terminal decisions | orchestration port consumes owner contracts without redefining semantics | complete local vertical slice from UI command to terminal projection | `v2_orchestration_adjacency` |
| `v2-ui-protocol` | UI protocol owner | command validation, query/subscribe separation, projection and redaction | protocol client/server contract with source identity fields | one UI command enters the local graph and receives owner-backed projection only | `v2_ui_projection_boundary` |
| `v2-network-extension` | network extension owner | node identity, capability/version negotiation, frame classes, sequence/ack/replay, disconnect state | in-memory transport test double with explicit accepted/rejected paths | network contracts remain unused by the single-machine MVP except for contract tests; no local fallback is fabricated | `v2_network_capability_version`, `v2_network_disconnect_explicit`, `v2_payload_control_isolation` |

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
than one frame class.

Positive cases cover valid local commands, immutable payloads and valid error
chains. Negative cases cover empty IDs, invalid protocol versions, missing
required fields, unknown variants and control fields placed in payload values.

### Black-box impact

The contract boundary is consumed by every other module. The module black-box
test must use only the public contract API. The project black-box test must
assert that a payload sentinel reaches the plugin/reason path unchanged while
an event sentinel remains visible only on the control path.

### Known gaps

- no production source crate exists yet;
- wire encoding format is not selected beyond the typed contract requirement;
- payload reference transfer is reserved for the network milestone.

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
event rejection, non-terminal preservation, terminal acceptance, and explicit
error-chain routing. A failed event must not be converted into a successful
terminal fact.

### Black-box impact

The module black-box test supplies events through the public event port and
observes the public ledger/replay port. It must prove replay starts at the
requested cursor and that an event acknowledgement is not a business result.

### Known gaps

- event ledger persistence is not implemented;
- cross-node replication and acknowledgement durability are network scope.

## `v2-reason-sessionlog`

### Lifecycle and logic

```text
accepted command
  -> append user/input truth
  -> reasoning adapter reads ordered session history
  -> append assistant/result truth
  -> persist cursor
  -> restore and replay
```

Positive tests cover one append, ordered replay, cursor restore and clean
interrupted-turn recovery. Reverse tests corrupt a record, use an invalid
cursor, simulate a lock/permission error, and provide a version mismatch. Each
failure must surface explicitly; no second storage path may claim the turn.

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
- provider-specific reasoning is outside this v2 foundation.

## `v2-plugin-runtime`

### Lifecycle and logic

```text
capability manifest
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

The project black-box test registers exactly one plugin and invokes it only
through the orchestration port. It must fail if the orchestrator imports or
duplicates plugin semantic logic.

### Known gaps

- plugin ABI and process isolation are not part of the MVP;
- remote plugin attestation and delegated execution are reserved for network
  extension.

## `v2-orchestration`

### Lifecycle and logic

```text
UI command
  -> admitted node
  -> plugin invocation
  -> result event
  -> reason/sessionlog append
  -> terminal or explicit non-terminal projection
```

The orchestration owner may connect adjacent nodes and carry correlation
identity. It may not validate payload semantics, implement event policy,
write session truth, parse UI rendering data, or create network fallback.

Positive tests cover start -> invoke -> reason -> terminal. Reverse tests cover
rejected command, plugin failure, non-terminal reason result, duplicate event,
and explicit stop. Each case must end in an owner-backed state, not an
orchestrator-local boolean.

### Black-box impact

The project black-box contract is the primary acceptance surface for this
module. It must inspect the command receipt, control events, plugin result,
sessionlog records and final UI projection as separate observations.

### Known gaps

- Cordis dependency/API is not yet selected or bound;
- no orchestration source exists;
- retry policy remains an event/reason owner decision, not a generic loop.

## `v2-ui-protocol`

### Lifecycle and logic

```text
UI ingress
  -> typed command validation
  -> orchestration port
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

## `v2-network-extension`

### Lifecycle and logic

```text
node identity
  -> authenticated session contract
  -> protocol/version negotiation
  -> capability admission
  -> ControlFrame or PayloadFrame
  -> ErrorFrame / ack / replay
```

The in-memory transport test double must preserve frame class and correlation
identity. Positive tests cover matching versions/capabilities, ordered event
delivery, acknowledgement and replay from a cursor. Negative tests cover
unknown node, unauthenticated admission, unsupported version/capability,
sequence gap, payload/control class confusion and explicit disconnect.

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
| `v2_control_event_positive_negative` | event ledger contract result | ordered accepted event routes | rejected/duplicate event cannot terminalize |
| `v2_sessionlog_append_replay` | adapter contract result | append/replay/restore is stable | corruption/lock/version failure is explicit |
| `v2_plugin_registration` | plugin registry result | one declared plugin invokes | undeclared/failed invocation is explicit |
| `v2_orchestration_adjacency` | mainline adjacency result | only adjacent transitions execute | shortcut/cross-owner semantic implementation fails |
| `v2_ui_projection_boundary` | public protocol result | command/query/subscribe are distinct | UI cannot mutate owner truth or expose control |
| `v2_network_capability_version` | in-memory transport result | matching identity/version/capability accepted | unsupported/unauthenticated admission rejected |
| `v2_network_disconnect_explicit` | disconnect/replay result | disconnect state and replay are explicit | silent local fallback is rejected |

## Completion Rule

No module is complete from a unit test alone. Completion requires the module
black-box test, the project black-box impact, architecture gates and the
AppSDK lifecycle records bound to the exact candidate source. M0 has no runtime
completion claim because all module statuses remain `planned`.
