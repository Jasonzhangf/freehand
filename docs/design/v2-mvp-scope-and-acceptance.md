# Freehand v2 MVP Scope and Acceptance

Status: frozen
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Related docs:

- `docs/goals/v2-completion-plan.md`
- `docs/design/v2-module-block-and-skeleton.md`
- `docs/v2/v2-development-roadmap.md`
- `docs/v2/v2-test-design.md`
- `docs/v2/v2-project-blackbox-verification.md`

## Purpose

This document freezes what the first v2 MVP must do, what it does not do, and
how acceptance is proven. It prevents later worktrees from redefining the MVP
as a design-only document, a UI mock, a generated artifact or a network double.

## MVP Scope

The first v2 MVP is a single-machine, one-agent public vertical slice that can
be run with local mock/typed adapters. It must prove:

- UI input enters through a typed UI adaptor and returns an owner-backed UI
  projection;
- Cordis orchestrates the fixed design plugin and a local Rust capability
  plugin;
- control events route on a separate control path;
- immutable payloads flow through `Arc` sharing at adjacent local boundaries;
- Session Log captures input, derived surface and result in order;
- a selected reasoning backend produces normalized result facts;
- channel/Registry contracts and ChannelSession state can be replaced without
  losing logical session state;
- Search and Memory are useable plugin surfaces, not browser-local stores;
- restart/replay and negative boundary checks pass in the project black-box
  harness.

## Out of Scope for MVP

- production distributed multi-machine execution;
- cloud central Registry availability;
- production network transport plugin implementation;
- DSH source ingestion or copying;
- browser-bound truth or UI-local persistence;
- remote plugin attestation;
- promotion/freeze/publish of v2 as a consumable runtime.

The reserved `v2-network-link` plugin block keeps the extension boundary
documented and admits future transport adapters without changing the MVP
mainline shape.

## Acceptance

The v2 MVP is accepted only when:

1. `origin/v2` carries project code, tests, docs and AppSDK records only.
2. AppSDK `0.1.6` `verify` and project gates pass on the exact branch.
3. M1b through M8 have real source, maps, tests and boundary evidence.
4. The M8 public entrypoint passes whitebox, install, restart and deployed
   blackbox evidence with one candidate-bound evidence identity.
5. `appsdk verify --review-admission` passes before review.
6. A selected read-only review tool returns PASS against the exact candidate.
7. Remote `v2` head matches local head after delivery.

## Verification Matrix

| gate | proof |
| --- | --- |
| typed UI/reason split | public vertical slice test observes UI projection without owner truth mutation and reasoning result in Session Log |
| plugin composition | local Rust capability runs through fixed Cordis design plugin |
| control/payload isolation | positive and negative wire tests prove control fields never enter payload |
| Arc sharing | tests prove adjacent local consumers share one immutable allocation |
| Session Log | append/surface/result, restart/replay and explicit failure behavior |
| channel/Registry | token admission, capability reconciliation, ChannelSession reconnect and stale generation rejection |
| search/memory | plugin lifecycle/projection tests and M8 vertical slice boundary test |
| network extension | reserved `v2-network-link` block is documented; no production transport is claimed |

## Freeze Sign-off

- MVP scope: frozen.
- Out-of-scope boundaries: frozen.
- Acceptance evidence order: frozen.
- Extension boundary for multi-machine collaboration and network plugins:
  frozen at design level and reserved for future owner worktrees.
