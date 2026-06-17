# Design Doc Index

Use this directory family for durable design truth.

## Current Design Sources

- `docs/architecture/workspace-layout.md`
- `docs/architecture/feature-map.md`
- `docs/architecture/function-map-spec.md`
- `docs/function-maps/README.md`
- `docs/architecture/dev-debug-workflow.md`
- `docs/architecture/test-strategy.md`
- `docs/design/system-architecture-overview.md`
- `docs/design/provider-and-reasoning-design.md`
- `docs/design/debug-and-observability-design.md`
- `docs/design/debug-core-design.md`
- `docs/design/metadata-core-design.md`
- `docs/design/ui-and-runtime-topology.md`
- `docs/design/config-core-design.md`
- `docs/design/contracts-core-design.md`
- `docs/design/provider-semantic-design.md`
- `docs/design/provider-adapter-design.md`
- `docs/design/reason-turn-design.md`
- `docs/design/reason-persistence-design.md`
- `docs/design/reason-context-planner-design.md`
- `docs/design/reason-rewrite-policy-design.md`
- `docs/design/tool-registry-design.md`
- `docs/design/tool-preview-design.md`
- `docs/design/node-master-slave-design.md`
- `docs/design/ui-protocol-design.md`
- `docs/design/webui-console-proposal.md`
- `docs/design/runtime-command-dispatch-design.md`
- `docs/design/runtime-checkpoint-rewind-design.md`
- `docs/design/runtime-daemon-design.md`
- `docs/prototypes/README.md`

## Rule

- design decisions that change owner, boundary, runtime path, or debug flow must be reflected in docs here or linked architecture docs
- chat discussion is not durable design truth

## Design Docs

- `system-architecture-overview.md`
  - high-level shape, layers, crate roles, confirmed boundaries
- `provider-and-reasoning-design.md`
  - provider abstraction, reasoning semantics, turn event model
- `provider-semantic-design.md`
  - provider scope, unified outputs, capability model, recovery model
- `reason-turn-design.md`
  - turn truth, event broadcast, tool re-entry, subscriber policy, terminal schema
- `reason-persistence-design.md`
  - authoritative snapshots, append-only ledgers, derived sidecars, restart recovery
- `reason-context-planner-design.md`
  - typed context segments, cache-stable prefix rules, subagent conclusion admission, metadata/request isolation
- `reason-rewrite-policy-design.md`
  - compaction thresholds, rollback/resume-rebuild triggers, and unexpected-case rewrite strategy
- `tool-registry-design.md`
  - built-in tool owner boundary, explicit implementation-state registry, runtime exposure gate, and Reasonix-aligned tool-surface policy
- `tool-preview-design.md`
  - writable-tool preview truth, preview/execute parity, and diff contract direction
- `node-master-slave-design.md`
  - local master/slave topology, pairing, node states, task delegation, turn subscription
- `ui-protocol-design.md`
  - CLI/WebUI scope, commands, projections, subscription model, black-box targets
- `webui-console-proposal.md`
  - proposal-only WebUI information architecture, visual direction, and binding matrix for a static review prototype
- `runtime-command-dispatch-design.md`
  - runtime-owned command dispatch wiring from UI protocol ingress to reason/node owner adapters
- `runtime-checkpoint-rewind-design.md`
  - runtime-owned writable-tool checkpoint snapshots, restore lifecycle, and rewind boundary
- `runtime-daemon-design.md`
  - runtime host process that injects `freehand-runtime` into shared protocol-only HTTP/SSE transport
- `docs/prototypes/README.md`
  - offline static prototype routing and review-only entry points
- `test-strategy.md`
  - white-box, module black-box, and project black-box validation policy
- `docs/function-maps/README.md`
  - code-bound function-map policy, mainline descriptions, multi-reference function registry



- `debug-and-observability-design.md`
  - semantic location, scene location, ledgers, replays, runtime evidence
- `debug-core-design.md`
  - debug module ownership, trace envelope, debug snapshot, and read-only observation boundaries
- `metadata-core-design.md`
  - internal metadata center, writer owner, write-node provenance, and metadata/request isolation boundary
- `ui-and-runtime-topology.md`
  - multi-UI access, runtime home, master/slave shape, UI protocol boundaries
- `config-core-design.md`
  - config source, multi-agent layout, required fields, startup semantics
- `contracts-core-design.md`
  - shared semantic contracts, pipeline node chains, IDs, error contracts, serialization rules
- `provider-semantic-design.md`
  - provider scope, unified provider semantics, capabilities, error recovery, raw-vs-semantic event policy
- `provider-adapter-design.md`
  - OpenAI responses/chat-completions and Anthropic messages adapter boundaries, request renderers, and stream parsers
