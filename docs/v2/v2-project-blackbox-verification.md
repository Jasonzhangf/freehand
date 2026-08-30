# Freehand v2 Project Black-Box Verification Contract

Status: planned
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.5`
Test design: `docs/v2/v2-test-design.md`

## Purpose

This is the project-level acceptance contract for the v2 MVP. It defines what
must be observed through the public command/query/subscribe entrypoints after
the owner modules exist. It does not authorize implementation, deployment or
promotion by itself.

The black-box harness must use a clean temporary runtime home and deterministic
in-memory or local test adapters. It must never read from or write to the
developer's live `~/.freehand` state, the v1 runtime, `~/code/sessionlog`,
`target/`, `artifacts/`, external checkouts or `.appsdk-control`.

## Public Entry Contract

The first implementation must expose three typed public operations:

| operation | input | observation |
| --- | --- | --- |
| command | one valid UI command with a correlation id | accepted/rejected receipt and owner-backed control events |
| query | session/event/projection query | current projection plus source identity |
| subscribe | subscription cursor or start position | ordered projection updates and explicit close/error |

The concrete transport is intentionally pending. The harness must not infer
the transport from a module name or silently substitute a second protocol.

## MVP Scenario

Input sentinel:

```text
v2-blackbox-payload-001
```

Expected local path:

```text
UI command
  -> UI protocol ingress
  -> Cordis orchestration
  -> local Rust plugin
  -> typed ControlEvent
  -> Arc immutable payload handoff
  -> sessionlog reasoning append
  -> typed UI projection
```

The harness must assert these observations independently:

1. The command receipt contains the same correlation identity as the emitted
   control events.
2. The plugin receives the typed payload value unchanged.
3. The control event identifies lifecycle/plugin/reasoning facts without being
   serialized inside the business payload.
4. The payload has one immutable handoff and remains equal for every consumer.
5. The sessionlog contains one ordered accepted input and one result turn.
6. A query returns the owner-backed session/projection truth.
7. A subscription receives the same projection in order and does not become a
   second query truth source.
8. The final projection contains user-visible result data only, not internal
   event envelopes, metadata, debug fields, network frames or raw storage
   paths.

## Negative Scenarios

The same harness must run the following failures:

| scenario | required result |
| --- | --- |
| malformed UI command | explicit protocol rejection; no plugin invocation |
| unknown plugin capability | explicit capability error; no reason append |
| plugin failure | typed failure event/error chain; no successful terminal projection |
| duplicate or out-of-order event | event rejection; existing ordered truth unchanged |
| sessionlog append failure | explicit persistence error; no second write path or success fallback |
| control field in payload | contract rejection before owner mutation |
| UI attempts direct session/event/plugin mutation | protocol/owner rejection |
| unsupported network version/capability | explicit negotiation rejection; no local fallback |
| network disconnect during replay | explicit disconnect/replay state; no fabricated completion |
| non-terminal reason result | running/waiting projection; no premature final state |

Every negative scenario must assert absence of the forbidden side effect. A
visible error string alone is insufficient evidence.

## Restart and Replay

After the successful scenario, the harness must restart only the test adapter
process or isolate its in-memory owner state according to the implementation
contract, then query the same session and replay from the recorded cursor.

Required assertions:

- accepted input and result order is unchanged;
- event sequence and correlation identities are unchanged;
- the payload is reconstructed from sessionlog truth, not from UI cache or
  metadata;
- replay emits no duplicate terminal event;
- a missing/corrupt record produces an explicit error and no success projection.

The restart proof is not a production deployment proof. Production install,
restart and online evidence become required only when a public runtime surface
is implemented.

## Architecture Boundary Assertions

The project harness and static gates must fail if:

- UI code writes session, event, plugin, reason or network truth;
- Cordis contains payload validation, event policy, reason persistence or
  plugin semantic logic;
- `Arc` is used as a mutable global context or truth store;
- control event, metadata, debug or error fields are embedded in a business
  request/response payload;
- sessionlog is double-written or silently replaced by another adapter;
- a network adapter owns task/reason/UI/payload truth;
- a production network path is used to satisfy the local MVP without an
  explicit contract;
- build outputs, generated outputs, external checkouts or runtime evidence are
  staged for commit.

## Evidence Shape

The future verifier must return a deterministic structured result with:

```text
project_id
candidate_commit
scenario_id
status
observed_command
observed_events
observed_payload_digest
observed_sessionlog_records
observed_projection
negative_assertions
restart_replay_assertions
boundary_assertions
```

This result is runtime evidence and remains outside the Git change set unless
the AppSDK lifecycle explicitly requires a project-owned summary document.
Raw logs, screenshots, temporary stores and generated reports stay ignored.

## Acceptance Gate

The v2 MVP is black-box accepted only when:

1. the positive scenario closes the full local vertical slice;
2. every negative scenario rejects the correct boundary without side effects;
3. restart/replay proves durable owner truth;
4. UI/reason and control/payload separation is observed through public outputs;
5. network extension tests prove explicit rejection without claiming distributed
   runtime;
6. the exact candidate source passes module and project gates;
7. AppSDK architecture review and unchanged-source effectiveness replay pass.

Until these conditions are met, the project status remains `draft` and no
Active/Protected artifact is published.
