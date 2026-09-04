# Freehand v2 Module Block and Skeleton

Status: frozen
Project: `freehand-v2`
Branch: `v2`
Governance: AppSDK `0.1.6`
Related docs:

- `docs/v2/v2-cordis-reasoning-channel-architecture.md`
- `docs/v2/v2-foundation-mvp-ui-reason-network-plan.md`
- `docs/v2/v2-plugin-ecosystem-contract.md`
- `docs/v2/v2-development-roadmap.md`
- `docs/v2/v2-test-design.md`
- `docs/design/v2-*-owner-binding.md`

## Purpose

This document freezes the v2 module partition and skeleton boundary that the
implementation worktrees must follow. It is the design-level map for module
blocks, plugin roles, payload/control separation and multi-machine extension
points. The runtime code is allowed to evolve inside these owner blocks; it
may not change the block graph without opening a new design freeze.

## Plugin Ecosystem Invariants

1. Cordis is the hot-pluggable orchestration ecosystem. The fixed design
   orchestration plugin is itself a Cordis plugin, not a privileged host.
2. Every executable, replaceable or externally connected v2 part is a plugin
   or a plugin family. This includes UI surfaces, reasoning backends, channel
   links, memory, search, canvas, topology, notifications and future network
   transport plugins.
3. Business payload and control data are physically separate. `Arc` sharing is
   the local immutable payload handoff contract; it is not a mutable global
   store and does not cross process/network boundaries as pointer identity.
4. Each module has one owner, one allowed path set, one mainline and one
   verification mapping. No module owns the truth of an adjacent module.

## Module Blocks

| block | owner | MVP scope | extension boundary |
| --- | --- | --- | --- |
| `v2-contracts` | contracts owner | typed IDs, immutable payload, UI command/wire frames, control/error events | versioned wire and payload reference extensions |
| `v2-control-events` | event owner | ordered event ledger, correlation, ack, replay, error chain | cross-node replication and durable event recovery |
| `v2-sessionlog` | session-log owner | canonical append/replay/surface/fork/recovery | DSH adaptor, persistent adapter, shared session views |
| `v2-reasoning-backend` | reasoning owner | provider-neutral service, native backend and OpenCode adaptor seam | additional backends and runtime-group switching |
| `v2-plugin-capabilities` | capability owner | typed manifest and local Rust leaf capability | remote plugin execution and attestation |
| `v2-cordis-ecosystem` | cordis owner | fixed design plugin, runtime-group scope, service injection, replacement | hot replacement and multi-runtime composition |
| `v2-ui-adaptor` | ui-adaptor owner | command/query/subscribe adaptor and owner-backed projections | multi-client and remote UI transport |
| `v2-ui-plugin-family` | ui-plugin owner | replaceable shell/navigation/run/session/attention/location/more/detail plugins | alternate UI implementations |
| `v2-notification-plugin` | notification owner | importance/time ordering, ack, snooze, archive | retention and cross-node delivery |
| `v2-topology-plugin` | topology owner | machine/node/agent/channel grouping and location projection | multi-machine topology and live route views |
| `v2-session-canvas-plugin` | canvas owner | session graph from Session Log facts, focus, replay | large-history and cross-node session graph |
| `v2-search-plugin` | search owner | typed query, classification, index, cache, invalidation | distributed indexing |
| `v2-memory-plugin` | memory owner | attach/summarize/record/save/load/search/export | cross-session memory policy and federation |
| `v2-channel-registry` | channel owner | token admission, endpoint registration, capability discovery, ChannelSession reconnect | local/cloud central Registry |
| `v2-network-link` | reserved network owner | no runtime implementation in MVP | network transport plugin, discovery/reconnect and multi-machine forwarding |
| `v2-public-vertical-slice` | integration owner | local public CLI/composed path proving the MVP mainline | browser/remote UI entrypoint integration |
| `v2-foundation-plan` | foundation owner | governance, design and module maps for the v2 project | none |

## Skeleton

```text
UI command
  -> v2-ui-adaptor ingress
  -> v2-cordis-ecosystem fixed design plugin
  -> v2-plugin-capabilities Rust leaf plugin
  -> v2-control-events ledger
  -> v2-sessionlog input/surface/result
  -> v2-reasoning-backend selected backend
  -> v2-sessionlog result
  -> v2-ui-adaptor projection
  -> UI plugin family rendering

Side boundary:
  v2-channel-registry provides endpoint/capability discovery and ChannelSession
  state. v2-network-link is reserved for future transport plugins and must never
  own session, reason, UI or payload truth.
```

## Allowed Edges

The MVP local mainline is:

```text
contracts -> control events -> sessionlog -> reasoning backend
  -> plugin capabilities -> cordis ecosystem -> ui adaptor -> UI plugin family
```

Information plugins (`notification`, `topology`, `session-canvas`, `search`,
`memory`) consume owner-backed projections and source facts without becoming a
second truth path. Channel/Registry consumes contracts and emits control frames
without controlling business payload. Future network transport consumes
channel/Registry contracts and never recreates control state from payload
bytes.

## Freeze Sign-off

- Module partition: frozen.
- Skeleton boundaries: frozen.
- Cordis plugin role: frozen.
- UI/reasoning split: frozen.
- Channel/Registry and reserved network plugin boundary: frozen.
- `Arc` local sharing and copy boundaries: frozen in `v2-contracts` contract.
