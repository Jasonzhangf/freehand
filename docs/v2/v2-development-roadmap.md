# Freehand v2 Development Roadmap

Status: executable roadmap
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Date: 2026-09-01

This document is the short operational route from the frozen v2 design to the
single-machine MVP. The architecture decisions remain in the v2 design
documents. This file answers four practical questions:

1. What is already real?
2. What is being implemented now?
3. What may run in parallel?
4. What evidence is required before the next milestone can start?

The long-form architecture and milestone contract remain:

- `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `docs/v2/v2-plugin-ecosystem-contract.md`
- `docs/v2/v2-test-design.md`
- `docs/v2/v2-project-blackbox-verification.md`

## 1. Current State

The current remote v2 mainline is `0c9f6fe7835288884d14491fcd02766a465cccd4`.

Integrated:

- AppSDK `0.1.6` governance surface;
- v2 design contracts for Cordis, plugins, channels, UI/reasoning
  separation, central Registry, ChannelSession, Session Log ownership and
  Arc payload sharing;
- `v2-contracts` M1a source and boundary tests;
- typed immutable IDs and payload contracts;
- local `Arc` sharing at the command boundary;
- explicit wire-boundary copying;
- payload/control/error physical separation.

In progress in another owner worktree:

- `v2-control-events` M1b;
- ordered control-event ledger;
- event admission, replay cursor, acknowledgement and error-chain behavior;
- positive and negative event-boundary tests;
- AppSDK lifecycle binding for the M1b dependency graph.

Current dependency rule:

`v2-sessionlog` cannot start implementation until `v2-control-events` has a
remote mainline receipt and the dependency module is in an AppSDK stage accepted
by the compiler. A source-only M1b worktree or a local generated artifact is
not sufficient.

## 2. Execution Order

```text
M0 governance and design
  -> M1a contracts
  -> M1b control events
  -> M2 canonical Session Log
  -> M3 reasoning backend seam
  -> M4 capability plugin + Cordis composition
  -> M5 UI adaptor + UI plugin slots
  -> M6 notification/topology/canvas/search/memory
  -> M7 Registry + ChannelSession
  -> M8 public vertical slice and MVP closeout
```

The dependency-critical path is:

```text
contracts
  -> control events
  -> Session Log
  -> reasoning backend
  -> Cordis composition
  -> UI adaptor
  -> public vertical slice
```

The following work may be prepared in parallel once its source owner and
dependency contracts are available:

- UI slot and surface contract review;
- OpenCode adaptor conformance fixture design;
- Registry and ChannelSession negative-test design;
- information-surface projection design;
- public black-box harness shape.

Parallel preparation must remain documentation or isolated test design until
the required predecessor module has a remote receipt. It must not create a
second implementation of the predecessor's semantics.

## 3. Milestones

### M1b: Typed Control Events

Owner: `v2-control-events`

Inputs:

- `v2-contracts` IDs, `ControlEvent`, `ErrorEvent`, `PayloadRef`;
- fixed plugin and channel contracts.

Implementation:

- typed event catalog;
- deterministic event sequence and correlation identity;
- ordered in-memory ledger;
- accepted/rejected admission;
- acknowledgement and replay cursor;
- explicit terminal, non-terminal and already-terminal states;
- error-event chain.

Required evidence:

- accepted event reaches the declared owner;
- rejected or duplicate event cannot mutate owner state;
- replay is deterministic;
- control events contain no business payload bytes;
- `Arc` payload pointer sharing remains intact at adjacent local boundaries;
- M1b AppSDK lifecycle records bind the actual candidate and artifact.

Start condition for M2:

- candidate merged into remote `v2`;
- remote mainline receipt exists;
- dependency module is accepted by AppSDK compile/admission;
- no unresolved M1b source or contract drift.

### M2: Canonical Session Log

Owner: `v2-sessionlog`

M2 is the next implementation milestone after M1b. It owns one canonical
Session Log and one local persistence adapter. It does not own provider
execution, UI state, or network transport.

Minimum source surface:

```text
playground/experiments/v2/sessionlog/
tests/v2/sessionlog/
```

Minimum behavior:

```text
accepted input
  -> append immutable input fact
  -> derive ordered model-visible surface
  -> append result or recovery fact
  -> persist cursor
  -> restore / replay / fork
```

Required operations:

- append;
- read;
- derive surface;
- replace or supersede a surface range without deleting history;
- represent retry and recovery explicitly;
- represent undo and reorder as new operations, never in-place mutation;
- fork from a stable closed boundary;
- restore from a cursor;
- bounded replay;
- interrupted-turn recovery.

Required failure behavior:

- corrupt record;
- invalid cursor or fork target;
- lock failure;
- permission failure;
- version mismatch;
- duplicate append;
- non-terminal or already-terminal operation misuse.

Each failure must return an explicit typed error and must not produce a
successful terminal projection.

M2 does not integrate `~/code/sessionlog` directly. The external repository is
an adaptor target only. The local adapter first proves the owner contract; the
external adaptor is a later implementation behind the same port.

### M3: Reasoning Backend Seam

Owner: `v2-reasoning-backend`

Depends on M2.

Implement:

- provider-neutral Reasoning Service;
- native Freehand backend adaptor;
- OpenCode adaptor boundary;
- normalized reasoning events and errors;
- start/resume/interrupt/inspect/subscribe operations;
- runtime-group backend selection;
- safe replacement boundary.

Acceptance:

- native and OpenCode fixtures satisfy the same contract;
- one runtime group has one active backend;
- two groups may select different backends;
- replacement never silently migrates an in-flight operation;
- backend identity and replacement state stay on the control path;
- no provider wire DTO enters v2 contracts, Session Log or UI projections.

### M4: Capability Plugin and Cordis Composition

Owners: `v2-plugin-capabilities`, `v2-cordis-ecosystem`

Depends on M1b, M2 and M3.

Implement:

- compiled capability manifest;
- one local Rust leaf capability;
- fixed design-orchestration Cordis plugin;
- runtime-group child scope;
- Service injection and activation;
- invoke/result/failure event path;
- safe unload and hot replacement.

The design-orchestration plugin connects adjacent typed ports. It does not
implement payload validation, event policy, Session Log persistence, reasoning
decisions or capability semantics.

### M5: UI Adaptor and Replaceable UI Slots

Owners: `v2-ui-adaptor`, `v2-ui-plugin-family`

Depends on M4.

Implement:

- typed command/query/subscribe adaptor;
- one compact operating-console entry;
- stable UI slot registry;
- Shell, Navigation, Run, Sessions, Attention, Location, More and Detail
  slots;
- mount/unmount/reconnect/replace behavior;
- loading, unavailable and error projections.

Acceptance:

- the UI sends typed commands only;
- the UI consumes owner-backed projections only;
- replacing a UI plugin keeps selected Session/Run identity;
- desktop and mobile use the same semantic operations;
- one-line mobile navigation remains the first-level entry;
- no browser-local session, event, search or memory truth exists.

### M6: Information Surface Plugins

Owners:

- `v2-notification-plugin`;
- `v2-topology-plugin`;
- `v2-session-canvas-plugin`;
- `v2-search-plugin`;
- `v2-memory-plugin`.

Depends on M2, M4 and M5 where the UI surface is involved.

Implement the smallest executable lifecycle for each plugin:

- Notification: importance/time ranking, acknowledgement and snooze;
- Topology: machine -> node -> Agent -> Channel location projection;
- Session Canvas: active/recent/history bands from Session Log facts;
- Search: typed query, classification, index, cache and invalidation;
- Memory: attach, summarize, record, save, load, search and export.

These are domain plugins with typed input/output ports. Their UI consumers are
replaceable surfaces. None may become a UI-only filter or a second truth store.

### M7: Central Registry and ChannelSession

Owner: `v2-channel-registry`

Depends on M1b, M4 and the channel contracts.

Implement local deterministic doubles for:

- stateless bearer-token admission;
- endpoint registration;
- capability manifest discovery;
- protocol/version reconciliation;
- route-change publication;
- logical ChannelSession state;
- transport connection attach/replace;
- suspend/reattach;
- ordered event replay;
- separate control, payload and error frames.

The MVP proves the logical contract without claiming production distributed
execution. Replacing a transport connection must preserve the logical
ChannelSession identity and state. A stale generation must fail closed.

### M8: Public Vertical Slice and MVP Closeout

Owner: `v2-foundation-plan` integration owner with all module owners.

The public black-box path must be observable as:

```text
UI command
  -> UI adaptor
  -> design-orchestration plugin
  -> Rust capability plugin
  -> typed control event
  -> Arc immutable payload handoff
  -> Session Log surface
  -> selected reasoning backend
  -> Session Log result
  -> UI projection
```

The closeout must include:

- successful turn;
- explicit capability failure;
- waiting/non-terminal turn;
- already-terminal rejection;
- Session Log restart and replay;
- UI plugin replacement;
- Registry connection replacement retaining ChannelSession;
- Search query;
- Memory save/load/export;
- positive and negative control/payload isolation;
- no forbidden build artifacts or runtime state in the commit.

## 4. Development Protocol

Every milestone uses one semantic claim, one owner, one branch and one clean
worktree below `v2/playground/`.

Required order:

1. Read the current resource, function, mainline and verification maps.
2. Confirm information sufficiency, closed logic and lifecycle completeness.
3. Confirm the predecessor's remote receipt and dependency stage.
4. Update the milestone test design before source edits.
5. Create a focused red test or baseline reproduction.
6. Implement only in the declared owner paths.
7. Run module-boundary self-check.
8. Run focused tests, module black-box tests, build, format, clippy and gates.
9. Run the real public entrypoint when that milestone has one.
10. Run AppSDK review admission only after all required evidence exists.
11. Run AGY architecture review against the exact candidate.
12. Replay positive, negative and original inputs with unchanged source.
13. Deliver the exact candidate to the merge queue.
14. Integrate only the tested candidate into `v2`.
15. Re-run affected validation on integrated `v2`.
16. Verify local and remote mainline identity.

A source, test, build or runtime-configuration change after review invalidates
the dependent evidence and requires a new validation cycle.

## 5. Current Next Action

The immediate execution route is:

```text
M1b owner completes control-event lifecycle
  -> remote v2 receipt
  -> new clean M2 worktree
  -> write M2 test design and red append/replay test
  -> implement canonical local Session Log adapter
  -> prove restore/replay/recovery and failure closure
```

Until the M1b receipt exists, the only valid work on this route is:

- review the Session Log contract;
- prepare M2 test vectors and failure matrix;
- inspect the external `~/code/sessionlog` API without binding it as truth;
- prepare the M2 owner worktree only after the predecessor receipt is real.

No M2 implementation may be started from a stale `v2` base or by importing
M1b's unmerged source.

## 6. Artifact and Boundary Policy

Committed:

- v2 source;
- v2 tests;
- v2 design and test documents;
- `.appsdk` contracts and lifecycle records created by the official SDK.

Forbidden in commits:

- `target/**`;
- `generated/**`;
- `active/lib/**`;
- `protected/**` projections unless the official lifecycle requires them;
- `artifacts/**`;
- `external/**`;
- `.appsdk-control/**`;
- runtime logs, credentials, temporary stores and build outputs.

The local `Arc` contract is valid only inside one process. Persistence and
network boundaries serialize and reconstruct values explicitly; they must not
claim cross-process pointer identity.

Control events, network control frames, metadata, debug observations and error
chains never enter business payload fields.

## 7. Stop Conditions

Stop the current milestone and report the first failing gate when:

- the owner or allowed path is ambiguous;
- a dependency has no remote mainline receipt;
- an AppSDK record is missing or bound to another candidate;
- a producer would overwrite immutable evidence;
- a test would need to weaken a negative assertion;
- a failure would require fallback or silent success;
- a new implementation would duplicate another owner's semantics;
- an artifact, runtime state or credential would enter the change set.

Each stop report must include:

- milestone and attempt identity;
- candidate/source commit;
- failed node or gate;
- preserved state;
- retry permission;
- exactly one next action.

## 8. Completion Definition

The v2 MVP is complete only when M1b through M8 have real source, tests,
owner-bound maps, public black-box evidence and valid AppSDK lifecycle records.
Design documents, placeholder modules, generated artifacts, local-only tests
or an unmerged worktree do not count as completion.
