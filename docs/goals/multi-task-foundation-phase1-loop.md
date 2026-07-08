# Multi-Task Foundation Phase 1 Loop

## Status

Execution target for the first implementation loop after single-agent closeout.

This is the current working contract for starting Phase 1. It summarizes the
existing durable design docs, current gaps, first-loop objective, deliverables,
verification gates, and stop conditions.

## Source Truth

Canonical design docs:

- `docs/design/task-center-truth.md`
- `docs/design/agent-lifecycle-semantics.md`
- `docs/design/master-worker-task-state-machine-phase1.md`
- `docs/design/master-worker-prompt-contract-phase1.md`
- `docs/design/master-worker-tool-action-contract-phase1.md`
- `docs/design/multi-task-foundation-implementation-plan.md`
- `docs/design/task-orchestration-design.md`
- `docs/design/workspace-session-execution-taxonomy.md`
- `docs/design/multi-agent-dispatch-alignment.md`

Owner maps and validation truth:

- `docs/architecture/feature-map.md`
- `docs/function-maps/task.orchestration.md`
- `docs/testing/task.orchestration.md`
- `docs/function-maps/tool.registry.md`
- `docs/testing/tool.registry.md`
- `docs/architecture/architecture-gaps.md`

Recent commits locking the baseline:

- `3e7ce4b docs: define multi-task foundation`
- `0a81e1c docs: define phase1 tool action contract`
- `9ea754b test: lock task tool op surface`
- `3502491 docs: clarify lifecycle and worker control`

## Current Baseline

Already closed:

- single-agent reasoning lifecycle
- failed-tool result pairing and multi-round repair
- schema polishing distinct from failure
- provider retry distinction and provider-domain evidence
- same-session continuation
- WebUI/ADP lifecycle proof
- deterministic single-agent task lifecycle commands
- `task` tool op-dispatched surface
- task runtime skeleton with task/agent registry, assignment, claim, heartbeat,
  review, close, and restart recovery
- red lock preventing task-management semantic action names from becoming
  standalone exposed tools

Important current truth:

- Agent Lifecycle is an intrinsic agent property, not a standalone model-facing
  tool.
- Task Center owns durable work state.
- Agent Lifecycle owns live agent state.
- Worker Control owns safe-point control of already running worker executions.
- Task mutation remains in Task Center.
- Phase 1 does not implement multiple independent BigTasks or cross-session
  context switching.

## Current Gaps

From the design docs and gap registry:

1. Task Center is not yet a first-class global board beyond the existing task
   runtime skeleton.
2. Execution identity/query/subscription is incomplete.
3. Agent Lifecycle reducer and AgentBoard projection do not yet exist as a
   durable owner-backed surface.
4. Worker progress/block/submission facts do not yet synchronize through a
   typed ExecutionFact pipeline into Task Center.
5. Scheduler tick/timer event truth is missing.
6. Master poll loop over TaskBoard and AgentBoard is not wired.
7. Worker Control channel is not implemented.
8. Headless ADP/CLI samples for the Phase 1 multi-task foundation are missing.
9. UI task/agent projection is not in Phase 1 loop scope unless the headless
   truth surface is complete.

## Phase 1 Loop Objective

Implement the minimum owner-backed multi-task foundation that can be tested
without UI:

```text
Task Center board truth
  + Agent Lifecycle truth
  + Execution binding/facts
  + Scheduler tick/timer facts
  + headless ADP/CLI query samples
```

The loop is successful only when a black-box headless path can prove:

- a task exists in Task Center truth
- an execution is bound to an agent
- the agent has lifecycle state
- execution facts update Task Center state
- scheduler tick can mark stale/timeout/review-ready style facts without making
  business decisions
- restart recovery can query the same ids after daemon restart

## First Loop Scope

In scope:

- owner-map and function-map updates for the implemented slice
- test-design updates before code changes
- TaskBoard query skeleton
- AgentLifecycleSnapshot and AgentBoard query skeleton
- ExecutionBinding / ExecutionFact contract
- lifecycle reducer for typed runtime/provider/tool/error/task events
- scheduler tick contract for elapsed/stale/timeout facts
- ADP/CLI headless query or sample commands for the new truth surfaces
- restart proof for the same task/execution/agent ids

Out of scope:

- WebUI task dashboard
- Android UI task dashboard
- worker pool autoscaling
- cross-machine worker transport
- multiple independent BigTasks
- cross-session master context switching
- autonomous business decisions by framework
- standalone model-facing `agent` lifecycle tool
- broad tool-surface redesign for general code tools

## Owner Decisions For First Implementation

Default owner direction:

- Task Center expansion starts under `task.orchestration` unless the first code
  slice proves a separate `task.center` feature id is required.
- Agent Lifecycle should start as an owner-backed state/projection surface,
  likely under a new `agent.lifecycle` feature id if the function map cannot fit
  cleanly under `task.orchestration` or `node.master-slave`.
- Worker Control should not be implemented until Agent Lifecycle and execution
  binding exist. It needs a separate owner decision because it controls running
  worker execution but must not mutate Task Center directly.
- Tool registry remains small for task-management actions: `task(op=...)` is
  the baseline; no standalone semantic action tool names.

Before code edits, the implementer must decide and document:

- final `feature_id`
- owner crate/module
- function map entry
- test-design entry
- mainline call map entry if migrated
- required tests and headless samples

## Required Deliverables

### D1: Owner And Map Closeout

- Feature map updated for the chosen owner(s).
- Function map updated with real or pending symbols.
- Test design updated with white-box, module black-box, and project black-box
  coverage.
- Mainline call map generated or explicitly marked pending according to current
  project rules.

Acceptance:

- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`

### D2: Task Center Board Skeleton

- TaskBoard snapshot/query contract exists.
- TaskBoard can answer task list, task detail, blocked/review/stale filtered
  views, and agent/task binding query direction.
- Query path is owner-backed; no UI-local task state.

Acceptance:

- white-box reducer/query tests
- runtime/query black-box test
- no standalone semantic action tool names added

### D3: Agent Lifecycle Skeleton

- AgentLifecycleSnapshot exists.
- AgentBoard projection exists.
- Lifecycle reducer consumes typed events, not raw prose.
- Snapshot includes state, current activity, elapsed time, task/execution/turn
  binding, model/tool/error counters, and last activity.

Acceptance:

- tests for `model_thinking`
- tests for `tool_running`
- tests for `recovering`
- tests for `blocked`
- tests proving raw assistant prose is not parsed as lifecycle truth

### D4: Execution Fact Sync

- ExecutionFact contract exists.
- Running/recovering/blocked/review_ready facts update Task Center truth.
- Agent-to-task binding index is queryable.
- Recovering does not fail the task.

Acceptance:

- worker recovering does not terminalize task
- worker blocked creates master-visible event
- review_ready enters review queue
- same agent query returns active task and task list

### D5: Scheduler Tick Skeleton

- Scheduler tick event contract exists.
- Tick computes elapsed/stale/soft-timeout/hard-timeout facts.
- Tick emits recommendations/facts only; it does not make business decisions.

Acceptance:

- soft timeout does not fail task
- stale requires no heartbeat/progress past threshold
- hard timeout requires master decision, not automatic failure
- scheduler events are durable/replayable

### D6: Headless Proof

At least one no-UI sample proves the slice end to end.

Required sample direction:

```text
task-center-foundation-sample
agent-lifecycle-foundation-sample
execution-fact-sync-sample
scheduler-tick-foundation-sample
restart-query-same-ids-sample
```

The exact CLI names may change, but the evidence must prove the same semantics.

Acceptance:

- S-profile fixed `127.0.0.1:4042`
- ADP/CLI can query TaskBoard and AgentBoard truth
- same ids survive `scripts/install-launchd.sh restartS`
- no model prose is accepted as proof

## Verification Gates

Minimum local gates for the first loop:

```bash
cargo fmt --check
cargo test -p freehand-task -- --nocapture
cargo test -p freehand-tools -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

Online/headless gate when runtime/ADP/CLI surface changes:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
```

Then run the new Phase 1 samples against `ws://127.0.0.1:4042/adp`.

## Stop Conditions

Stop and report instead of continuing when:

- owner cannot be mapped uniquely
- task mutation starts leaking into Agent Lifecycle or Worker Control
- Agent Lifecycle is implemented as raw prose parsing
- `worker_control` tries to create/assign/approve/close tasks
- a semantic action is added as a standalone tool name
- same failure repeats twice without changing approach
- required headless proof cannot be made deterministic
- restart proof cannot query the same ids

## Completion Standard

The first loop is complete only when all are true:

- D1-D6 are implemented or explicitly marked out-of-scope with Jason approval.
- Owner docs, function maps, test designs, and generated mainline/wiki artifacts
  are synchronized.
- Local gates pass.
- S-profile headless proof passes on fixed `127.0.0.1:4042`.
- Restart proof re-queries the same TaskBoard/AgentBoard/execution ids.
- `MEMORY.md` and `note.md` record the verified durable truth.
- Work is committed in scoped commits.

## First Execution Order

1. Update owner maps and test-design records.
2. Implement TaskBoard query skeleton.
3. Implement AgentLifecycleSnapshot and AgentBoard projection.
4. Implement ExecutionFact contract and Task Center sync.
5. Implement scheduler tick facts.
6. Add CLI/ADP samples.
7. Run local gates.
8. Run S-profile headless proof and restart proof.
9. Commit and report evidence.
