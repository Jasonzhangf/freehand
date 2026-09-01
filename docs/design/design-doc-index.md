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
- `docs/design/control-error-center-refactor.md`
- `docs/design/instruction-capability-loader-design.md`
- `docs/design/task-orchestration-design.md`
- `docs/design/task-center-truth.md`
- `docs/design/agent-lifecycle-semantics.md`
- `docs/design/framework-mediated-agent-operations.md`
- `docs/design/master-worker-task-state-machine-phase1.md`
- `docs/design/master-worker-prompt-contract-phase1.md`
- `docs/design/master-worker-tool-action-contract-phase1.md`
- `docs/design/multi-task-foundation-implementation-plan.md`
- `docs/design/workspace-session-execution-taxonomy.md`
- `docs/design/multi-agent-dispatch-alignment.md`
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
- `docs/design/webui-layered-controls-and-attachments.md`
- `docs/design/mobile-webui-ui-tree.md`
- `docs/design/runtime-command-dispatch-design.md`
- `docs/design/runtime-checkpoint-rewind-design.md`
- `docs/design/runtime-daemon-design.md`
- `docs/design/acp-v1-agent-server-design.md`
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
- `task-orchestration-design.md`
  - task lifecycle, one-tool action surface, persistence, startup recovery, runtime memory state, and agent registry skeleton
- `task-center-truth.md`
  - global Task Center truth for BigTask, SubTask, Execution, Review, EventInbox, SchedulerTick, agent task indexes, task registration, sync, query, and recovery
- `agent-lifecycle-semantics.md`
  - per-agent runtime lifecycle semantics for master and worker agents, including live state, current/last activity, model/tool/error counters, runtime stats, control channel, and AgentBoard projections
- `framework-mediated-agent-operations.md`
  - durable boundary for framework-mediated Agent/Task operations, including Task Center mutations, Agent registry, Agent Lifecycle projections, worker-control queues, persistence, and current Phase 1 versus Phase 2 status
- `master-worker-task-state-machine-phase1.md`
  - first multi-task foundation state machine: one active BigTask, multiple SubTasks, MasterPollLoop, WorkerExecutionLoop, FrameworkSchedulerTick, timeout/block/review/retry/recovery acceptance
- `master-worker-prompt-contract-phase1.md`
  - master and worker prompt/tool behavior contract for Phase 1, including state handling tables, wait semantics, control-channel handling, and context admission rules
- `master-worker-tool-action-contract-phase1.md`
  - Phase 1 tool/action contract requiring a small owner-scoped tool surface with typed `op` parameters, semantic-action-to-op mapping, owner boundaries, and paired action validation errors
- `multi-task-foundation-implementation-plan.md`
  - staged plan for implementing Task Center, Agent Lifecycle, lifecycle-to-task sync, scheduler tick, runtime control channel, headless samples, and UI projection before full multi-agent scheduling
- `workspace-session-execution-taxonomy.md`
  - canonical vocabulary and ownership rules for master, worker resources, cwd-bound workspaces, workspace-owned sessions, and worker executions
- `multi-agent-dispatch-alignment.md`
  - Codex/Reasonix comparison and Freehand dispatch design for model-triggered task dispatch, worker execution tracking, subagent turn subscription, and compact status projection
- `node-master-slave-design.md`
  - local master/slave topology, pairing, node states, task delegation, turn subscription
- `ui-protocol-design.md`
  - CLI/WebUI scope, commands, projections, subscription model, black-box targets
- `webui-console-proposal.md`
  - proposal-only WebUI information architecture, visual direction, and binding matrix for a static review prototype
- `mobile-webui-ui-tree.md`
  - mobile WebUI route/page tree, Home versus selected-session split, phone-first Tools registry shape, and lifecycle UI closure rules
- `multi-platform-ui-architecture.md`
  - shared WebUI/mobile/Android responsive architecture, ADP-first transport, aspect-ratio layout matrix, and mobile daemon connection config direction
- `runtime-command-dispatch-design.md`
  - runtime-owned command dispatch wiring from UI protocol ingress to reason/node owner adapters
- `runtime-checkpoint-rewind-design.md`
  - runtime-owned writable-tool checkpoint snapshots, restore lifecycle, and rewind boundary
- `runtime-daemon-design.md`
  - runtime host process that injects `freehand-runtime` into shared protocol-only HTTP/SSE transport
- `docs/prototypes/README.md`
  - offline static prototype routing and review-only entry points
- `docs/v2/v2-ui-plugin-contract.md`
  - Cordis UI plugin slots, typed ports, lifecycle, replacement protocol and
    mobile composition
- `docs/v2/v2-plugin-ecosystem-contract.md`
  - all executable, replaceable and externally connected product parts as
    Cordis plugins, nested composition, typed ports and replacement boundaries
- `docs/v2/v2-sessionlog-test-vectors.md`
  - pre-implementation Session Log positive, negative, crash-recovery and DSH
    adaptor test vectors
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
- `instruction-capability-loader-design.md`
  - deterministic manifest compiler for global/local AGENTS.md and skills authoring surfaces
- `control-error-center-refactor.md`
  - next refactor design for four fixed request/response hook points, passive status schema, compact built-in task action tools, centralized error policy, metadata watermarks, task orchestration, and control/data isolation
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

## WebUI Architecture Review (2026-07-01)

| Field | Value |
|-------|-------|
| Title | WebUI 架构审核与改造建议 |
| Source | `docs/design/webui-architecture-review.md` |
| Author | Reasonix v2 codebase comparison |
| Scope | `webui.js` / `webui.css` / `page.rs` |
| Status | Review only; not yet planned |
