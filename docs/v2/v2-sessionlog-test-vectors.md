# Freehand v2 Session Log Test Vectors

Status: pre-implementation contract
Project: `freehand-v2`
Module: `v2-sessionlog`
Owner: `v2-sessionlog`
Governance: AppSDK `0.1.6`
Related roadmap: `docs/v2/v2-development-roadmap.md`
Related test design: `docs/v2/v2-test-design.md`
DSH reference: `/Users/fanzhang/code/dsh/packages/session`

This document freezes the first executable test contract for the canonical
Session Log. It does not claim that the module exists, that its storage format
is selected, or that any test has passed. The first M2 implementation must
bind every vector below to a real owner symbol and a real verification gate.

## Scope

M2 owns one provider-neutral Session Log contract and one local durable
adapter. It owns:

- immutable session facts and contiguous sequence assignment;
- append and read;
- cursor-based replay;
- deterministic model-visible surface derivation;
- append-only replacement, reorder and undo facts;
- stable fork boundaries;
- interrupted-turn recovery;
- explicit storage, format and integrity errors.

M2 does not own:

- provider request or response DTOs;
- reasoning decisions or retry policy;
- UI state, browser cache or rendered surface state;
- Cordis composition;
- network transport, Registry or ChannelSession;
- a second provider-native persistence truth store.

The external DSH implementation is an adaptor target. It is evidence for
semantic compatibility, not a dependency and not a source of v2 truth.

## Canonical Model

The minimum provider-neutral model is:

```text
SessionHeader
  session_id
  format_version
  created_at
  optional parent_session_id
  optional seed_length
  optional runtime_identity

SessionEvent
  seq
  event_id
  timestamp
  typed event kind
  typed data
  optional surface operation
  optional source event references
  optional ignorable marker

SessionCursor
  session_id
  last_applied_seq
  optional active_turn_id
  log format version

SurfaceProjection
  session_id
  source event sequence
  stable surface node identity
  current order
  replacement generation
```

The exact Rust names may differ, but the semantic fields and ownership must
remain explicit. Control events, metadata, debug data, provider state and
network frames are not `SessionEvent.data`.

## Lifecycle Contract

The primary lifecycle is:

```text
create header
  -> append accepted input fact
  -> append model-visible surface fact
  -> append result or recovery fact
  -> persist the next cursor
  -> inspect / read / replay
  -> restart adapter
  -> restore the same session
```

The implementation must expose one canonical append path. A query or UI
projection may derive data from the log, but it may not write a replacement
history or repair a missing event.

Every append must be:

- associated with exactly one session;
- assigned a contiguous `seq`;
- immutable after acceptance;
- idempotent only when the same event identity and exact event content are
  presented according to the frozen contract;
- rejected explicitly when it conflicts with the existing log.

Durability means that a successful append is visible after adapter restart.
An in-memory success before the durability boundary is not a terminal success
for the module.

## Positive Vectors

Each vector must produce a structured result containing `vector_id`,
`session_id`, candidate/source identity, observed event sequences, projection
digest and storage adapter identity.

| vector_id | Scenario | Required assertions |
| --- | --- | --- |
| `SL-P01` | append input | One input fact is accepted at the next contiguous sequence; the event is readable from the same canonical log. |
| `SL-P02` | append surface | A model-visible surface fact is derived in stable order and references its source event without copying control state into business data. |
| `SL-P03` | append result | A result fact closes the expected turn; the final projection is terminal only when the typed result says it is terminal. |
| `SL-P04` | cursor replay | Replay from `last_applied_seq` returns only the ordered suffix and does not emit a duplicate terminal fact. |
| `SL-P05` | restart restore | A fresh adapter instance restores the same header, event sequence, correlation identities and projection digest. |
| `SL-P06` | surface replace | A replacement appends a new fact referencing the replaced surface nodes; old events remain byte/logically unchanged. |
| `SL-P07` | surface reorder | A reorder is represented as a new operation with source references; replay derives the same order deterministically. |
| `SL-P08` | undo | Undo records an explicit operation and source relation; it does not delete or mutate the undone event. |
| `SL-P09` | closed fork | A child session can fork only from a stable closed boundary; the header records parent identity and seed length. |
| `SL-P10` | interrupted recovery | A complete but open turn is preserved and closed with an explicit interrupted/recovery fact; prior events remain present. |
| `SL-P11` | read-only fanout | Multiple projections consume one canonical log snapshot without creating divergent histories or writes. |
| `SL-P12` | export | Export reads the canonical log and preserves event order, source references and header lineage; it does not construct a second history. |

## Negative Vectors

Every negative vector must assert both the returned typed error and absence of
the forbidden side effect. A visible error without an unchanged-log assertion
is insufficient.

| vector_id | Failure | Required result |
| --- | --- | --- |
| `SL-N01` | duplicate sequence | Reject before owner mutation; the stored next sequence and projection remain unchanged. |
| `SL-N02` | sequence gap | Reject a batch whose first or internal sequence is not contiguous; no partial batch is committed. |
| `SL-N03` | duplicate event identity with changed content | Fail closed; do not treat the changed event as an idempotent retry. |
| `SL-N04` | invalid cursor | Reject a cursor from another session, a negative position or a position beyond the durable boundary. |
| `SL-N05` | fork inside open turn | Reject the fork target; no child session or inherited history is published. |
| `SL-N06` | fork from another session | Reject a source session that is not the declared parent; no cross-session replacement or seed is accepted. |
| `SL-N07` | malformed event envelope | Reject missing sequence, event kind, timestamp or typed data before persistence. |
| `SL-N08` | corrupt committed record | Return a corruption error; do not silently skip the record or produce a successful projection. |
| `SL-N09` | torn tail | Apply only the documented recovery policy; preserve the committed prefix and expose the repair/recovery result. |
| `SL-N10` | unknown required event | Refuse the log as unsupported; only an explicitly ignorable unknown event may be skipped. |
| `SL-N11` | unsupported format version | Return a direction-aware version error; do not interpret the record as the current format. |
| `SL-N12` | lock failure | Surface the storage lock error; do not report durable append success. |
| `SL-N13` | permission failure | Surface the permission error; do not switch to another root or adapter. |
| `SL-N14` | snapshot/log mismatch | Reject the snapshot or rebuild from the canonical log; never let a sidecar become truth. |
| `SL-N15` | failed append | No successful terminal projection, no second write path and no fabricated recovery success. |
| `SL-N16` | failed recovery | Keep the session in an explicit error/recovery state; never project failure as success. |
| `SL-N17` | UI/cache-only restore | Reject any restore attempt that does not use the canonical Session Log. |
| `SL-N18` | provider raw-log restore | Reject use of provider-native raw history as the v2 Session Log truth. |
| `SL-N19` | in-place replace | Reject physical mutation of an existing event; require a new replacement fact. |
| `SL-N20` | cross-session replacement | Reject source references that name another session without an explicit valid lineage boundary. |

## Crash and Recovery Matrix

The local adapter must test recovery at each durable boundary. The expected
state is explicit and must not be inferred from a UI cache.

| boundary | durable prefix | expected recovery |
| --- | --- | --- |
| before header commit | none | session is absent or reports an explicit incomplete-create result according to the adapter contract; no successful session projection exists |
| after header, before first event | header only | header-only state is recoverable only if the contract explicitly materializes empty sessions |
| after input append | input fact | input remains readable after restart |
| after surface append | input plus surface | surface remains ordered and source-bound after restart |
| after result append | closed result | terminal projection is reproducible after restart |
| during open turn | complete prefix with no closer | preserve the prefix and append explicit interrupted/recovery facts |
| torn final record | committed prefix plus partial tail | reject or truncate only the torn tail according to the selected format; never rewrite the committed prefix |
| repair commit failure | unchanged or partially repaired physical state | surface repair failure; do not claim recovered success or silently retry through another store |

The adapter must distinguish:

- an uncommitted physical tail;
- a committed but interrupted logical turn;
- a committed corruption in the prefix;
- a valid non-terminal turn;
- an already-terminal session.

These cases must not share one generic success or error projection.

## DSH Semantic Mapping

The following mapping is the compatibility target for a future adaptor:

| DSH semantic | v2 contract treatment |
| --- | --- |
| `SessionPersistence` service | v2 `SessionLog` provider-neutral service boundary |
| `SessionHeader` | v2 header resource kept beside the event log |
| `SessionEvent` | v2 immutable event fact after provider-neutral typing |
| contiguous `seq` | mandatory v2 append invariant |
| `append` durability barrier | v2 durable append completion boundary |
| `load` / `inspect` | v2 restore and read operations with explicit mutation differences |
| `readFrom` | v2 cursor/suffix replay |
| `parentSession` / `seedLength` | v2 fork lineage and stable seed boundary |
| `surfaceOp: replace` | v2 append-only replacement operation |
| synthetic interrupted closers | v2 explicit interrupted/recovery facts |
| format refusal | v2 unsupported-version/unknown-required-event errors |
| JSONL or SQLite backend | future adaptor/storage implementation, not v2 truth |
| session-log delivery watermark | a provider/network side-channel, not a Session Log event payload unless v2 explicitly types it as a fact |
| session-log export | v2 export projection over the canonical log |

The adaptor must preserve semantic event order and source references. It may
change physical encoding, batching, compression and indexing, but it may not
change the logical event stream or make the provider-native database
authoritative.

## Ownership and Boundary Checks

The M2 owner must add a machine-checkable binding for:

- `resource_id: v2_session_log`;
- `feature_id: v2-sessionlog`;
- one canonical append owner;
- one canonical restore/replay owner;
- one local persistence adapter;
- one test owner for the vectors in this document;
- all allowed source and test paths;
- forbidden direct edges from UI, Cordis, Network, Search and Memory.

The implementation fails architecture review if:

- a second append or recovery path exists;
- UI, search or memory writes the Session Log directly;
- a projection or cache is used for authoritative restore;
- provider wire fields become v2 event fields without a typed contract;
- control, metadata, debug or error data is embedded in business payload;
- a replacement or undo mutates an earlier event;
- an `Arc` handle is mistaken for cross-process or durable zero-copy;
- a failed append is projected as a successful terminal result.

## Required Verification Sequence

After M1b has a remote mainline receipt and AppSDK accepts the dependency
graph, the M2 owner runs:

```text
1. read resource/function/mainline/verification maps
2. update the M2 module binding and test design
3. write SL-P01 and SL-P04 as red tests
4. implement the minimum local Session Log contract
5. make the positive vectors pass
6. add the paired negative vectors and recovery matrix
7. run module boundary and architecture gates
8. run the local module black-box test
9. run the project black-box restart/replay test
10. validate the exact candidate and artifact
11. run the required install/restart/public entrypoint checks
12. only then run AppSDK review admission and AGY review
```

The initial M2 source worktree must be created only after the predecessor
receipt exists:

```text
<v2-project-root>/playground/v2-sessionlog-<run_id>
```

No DSH source is copied into that worktree. No external checkout is added to
the commit.

## Completion Signal

M2 is ready for handoff only when:

- every positive vector has a real test binding and passes;
- every negative vector has a real rejection and no-side-effect assertion;
- crash/restart/replay evidence uses the same candidate and adapter;
- the canonical log remains the only restore truth;
- DSH compatibility is demonstrated through an adaptor-shaped fixture;
- owner maps and verification maps bind real symbols and paths;
- no build artifact, runtime state, external checkout or credential is staged.

Until then, M2 remains `planned` or `in_progress`; a design document alone is
not implementation evidence.
