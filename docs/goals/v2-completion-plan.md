# Freehand v2 Completion Plan

Status: execution plan
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`

## Goal and Acceptance

Deliver the Freehand v2 MVP as a Rust-first, Cordis-based, plugin-composed
system. UI, reasoning, network/link, channel, search, memory, notification,
topology and canvas functions must remain replaceable plugins around one
canonical Session Log and one explicit control-event path.

Acceptance requires:

- M1a contracts and M1b control events are accepted on remote `v2`;
- M2 canonical Session Log passes append, replay, restore, recovery, replace,
  reorder, undo and fork positive/negative contracts;
- native reasoning and OpenCode adaptors satisfy one provider-neutral seam;
- Cordis design orchestration composes a local Rust capability plugin;
- UI adaptor and replaceable UI slots work on desktop and mobile semantics;
- Notification, Topology, Canvas, Search and Memory plugins have typed ports;
- Registry and ChannelSession prove token admission, discovery, reconnect and
  stale-generation rejection;
- the local public vertical slice passes restart/replay and negative boundary
  checks;
- only project source, tests and documents enter Git; no build/runtime output;
- required AppSDK, build, live-entrypoint and selected review gates pass.

## Scope and Boundaries

In scope: v2 only, under `/Volumes/extension/code/freehand/v2`, with all
development worktrees below `v2/playground/`.

Out of scope until explicitly scheduled: changing v1 behavior, claiming
production multi-machine execution from local doubles, copying DSH source into
v2, using UI/cache/provider raw logs as truth, and deploying or publishing
without the required lifecycle gates.

Canonical ownership:

- Cordis owns plugin composition and hot replacement;
- contracts own shared typed values and frame separation;
- control events own control-path ordering and error events;
- Session Log owns durable session facts and replay truth;
- reasoning owns provider-neutral execution and backend adaptors;
- channel/Registry owns link, discovery, capability negotiation and
  ChannelSession state;
- UI consumes adaptor projections and never writes domain truth;
- Search and Memory own their own plugin input/output and derived stores.

Control, metadata, debug, error, network frames and business payload remain
physically separate. Local immutable payload sharing may use `Arc`; process,
storage and network boundaries serialize explicitly and must not claim zero-copy.

## Design Sources

- `docs/v2/v2-development-roadmap.md`
- `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `docs/v2/v2-plugin-ecosystem-contract.md`
- `docs/v2/v2-ui-plugin-contract.md`
- `docs/v2/v2-test-design.md`
- `docs/v2/v2-sessionlog-test-vectors.md`
- `docs/v2/v2-project-blackbox-verification.md`

DSH references are read-only compatibility references under
`/Users/fanzhang/code/dsh/packages/session`. The future DSH adaptor must
consume the v2 Session Log contract and may not create a second truth store.

## Technical Plan and File Ownership

Each milestone gets one semantic claim, one owner, one branch and one clean
worktree. Before source work, update resource, function, mainline and
verification bindings plus the milestone test design.

| milestone | owner surface | minimum deliverable |
| --- | --- | --- |
| M1a | `v2-contracts` | typed IDs, payload envelope, frame separation and immutable local sharing |
| M1b | `v2-control-events` | ordered ledger, correlation, acknowledgement, replay and error chain |
| M2 | `v2-sessionlog` | canonical local log, durable append, deterministic surface, cursor restore, recovery and operation facts |
| M3 | `v2-reasoning-backend` | provider-neutral reasoning service, native backend and OpenCode adaptor seam |
| M4 | `v2-plugin-capabilities`, `v2-cordis-ecosystem` | compiled manifest, local Rust capability, fixed design orchestration plugin and hot replacement |
| M5 | `v2-ui-adaptor`, `v2-ui-plugin-family` | typed UI command/query/subscribe adaptor and compact replaceable slots |
| M6 | notification/topology/canvas/search/memory owners | typed information plugins with independent lifecycle and projections |
| M7 | `v2-channel-registry` | stateless token, endpoint/capability discovery, ChannelSession reconnect and frame separation |
| M8 | integration owner | local public vertical slice, restart/replay, negative boundaries and MVP evidence |

Expected source/test locations are declared by each owner map. Do not add
source to this plan worktree as a substitute for an owner worktree.

## Risk Controls

- No predecessor receipt, no next milestone implementation.
- No AppSDK governance preflight, no business-code debugging or delivery.
- No fallback, silent strip, duplicate persistence path or UI-side truth.
- No overwrite of immutable evidence or historical records.
- No cross-boundary `Arc` claim.
- Failed, non-terminal, interrupted and already-terminal states remain
  explicit and distinct.
- A change after validation or review invalidates dependent evidence and
  requires the affected gates to run again.
- Build outputs, generated projections, runtime state, credentials, external
  checkouts and temporary evidence remain untracked and unstaged.

## Verification Matrix

Every milestone must complete, in order:

1. module boundary self-check against maps and owner paths;
2. focused white-box positive/negative tests;
3. module black-box contract test;
4. build, format, clippy and architecture gates;
5. project black-box or public-entrypoint test when available;
6. required install/restart/live verification for runtime milestones;
7. exact candidate/artifact/evidence binding;
8. AppSDK admission and the explicitly selected read-only review tool;
9. unchanged-source effectiveness replay;
10. precise handoff, integration and remote receipt verification.

The M2 vector contract is the minimum test matrix for Session Log work. The
M8 harness must independently observe UI command receipt, control events,
Arc-local sharing, Session Log records, reasoning result, plugin projections,
ChannelSession identity and restart/replay truth.

## Execution Order

1. Complete M1b governance and control-event lifecycle on its existing owner
   branch; preserve all historical evidence.
2. Verify the remote `v2` receipt and accepted dependency stage.
3. Create a new clean M2 owner worktree below `v2/playground/`.
4. Bind M2 maps and write append/replay red tests before implementation.
5. Implement and verify the canonical local Session Log, then deliver it.
6. Implement M3 through M7 sequentially, opening parallel preparation only as
   documentation or isolated tests whose dependencies have receipts.
7. Integrate M8 through the public local vertical slice.
8. Run final project gates, selected review, effectiveness replay and remote
   receipt verification.

## Definition of Done

The task is complete only when the acceptance list is backed by real source,
tests, maps, runtime evidence and lifecycle records for the exact candidate.
The final branch contains project code and documentation only. A design-only
document, local-only test, unmerged branch, generated artifact or verbal
claim is not completion evidence.
