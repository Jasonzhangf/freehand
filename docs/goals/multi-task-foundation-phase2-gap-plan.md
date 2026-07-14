# Multi-Task Foundation Phase 2 Closeout And Production Gap

## Status

This document is no longer an active Phase 2 implementation plan. Phase 2A,
2B, 2C, and WebUI Phase 2D have been closed for their stated scopes.

Current marker: `production-master-worker-loop-gap-reconcile-20260709`.

Current remaining gap is production promotion: move from headless samples and a
deterministic provider fixture to a daemon-owned, non-smoke master/worker loop
with configured worker resources and a separate real-provider behavioral smoke.

## Verified Baseline

The following foundation is already implemented and verified:

- Phase 1: TaskBoard, AgentBoard, AgentLifecycle snapshot, ExecutionFact sync,
  SchedulerTick facts, and restart same-id proof.
- Phase 2A: master-worker execution lifecycle through owner truth: create
  worker, create task, assign, claim, progress, blocked, recovering,
  review_ready, reject, retry, approve, close, and restart same-id proof.
- Phase 2B: EventInbox and MasterPoll: four-part event cursors,
  replay-from-start full drain, persisted cursor, owner-backed cursor reread,
  classifications, and restart same-cursor proof.
- Phase 2C: worker-control foundation: `query_status`, queued safe-point
  requests, Task Center-backed pause/resume/cancel, control ledger/snapshot,
  and restart same-id proof.
- Phase 2D WebUI: browser projection of TaskBoard, AgentBoard, EventInbox,
  TaskHistory, and WorkerControl from ADP/runtime owner truth.
- Master-worker autonomy fixture: `master-worker-autonomy-sample` submits only
  `SubmitUserInput`; model/provider `task(op=...)` tool calls drive success,
  execution-error, and reject-retry scenarios; same task/execution/agent ids are
  verified after S-profile restart.

Android true-device Phase 2 projection is not implied by the WebUI closeout and
must be proved separately if mobile dashboard behavior changes.

## Current Gap

### G1: Production Master Scheduler Loop

Missing:

- daemon-owned loop activation from config, not a CLI sample command
- periodic or event-driven master poll execution
- durable cursor and task-state observation across daemon restarts
- explicit runtime status for loop enabled/disabled/error

Why it matters:

The framework can run `RunMasterPoll`, but production behavior still needs a
runtime owner to call it at the right time without a test harness.

### G2: Production Worker Runner

Missing:

- configured worker pool acquisition and release
- worker-visible queue runner that claims assigned tasks without scripted CLI
  task mutation
- resource lifecycle evidence: idle, busy, blocked, released, closed
- recovery behavior for worker crash, stale lease, and cancelled task

Why it matters:

Task Center can assign and claim, but a real worker process/runner must own the
claim/execute/report loop before Freehand can claim production autonomous work.

### G3: Non-Smoke Autonomy Loop

Missing:

- production command path that starts from user input and may spawn/assign work
  through the master loop rather than `master-worker-autonomy-sample`
- owner-backed transcript/task/lifecycle verification for production-created
  tasks
- restart proof for production-created task/execution/agent ids

Why it matters:

The fixture proves the protocol and tool loop are capable. It does not prove a
daemon-started production loop is continuously active.

### G4: Real-Provider Behavioral Smoke

Missing:

- a separate smoke against the configured real provider that asks for a bounded
  task dispatch and verifies whether the model chooses `task(op=...)`
- evidence that real-provider prompt behavior still follows the task tool
  contract at least in one bounded case
- a formal online E2E task, not a toy prompt: the agent must perform a real
  research/document workflow with current-source lookup, background synthesis,
  task progress, and a durable output artifact
- explicit documentation that deterministic fixtures remain the regression
  source of truth, while real-provider smoke is behavioral evidence

Why it matters:

Real model behavior can drift. Deterministic fixture tests lock framework
correctness; real-provider smoke catches prompt/contract drift without becoming
the only acceptance gate.

Formal E2E acceptance prompt:

```text
You are the Freehand master agent. Complete this formal task end to end.

Task:
Research one important AI/semiconductor/international-technology-policy news
item from the last 72 hours. Pick the most consequential item you can verify
from current sources, then produce a background briefing document.

Required behavior:
1. If the task needs multiple investigation tracks, create and manage worker
   tasks through task(op=...) rather than doing private, untracked work.
2. Search current sources. Use at least three independent sources when search is
   available. If current-source search is unavailable, fail explicitly with the
   missing capability instead of inventing facts.
3. Track task state through TaskBoard/AgentBoard/EventInbox/TaskHistory. Do not
   rely on prose-only status.
4. Write a markdown briefing document under a deterministic task output path.

Document requirements:
- title, date, selected news item, and why it matters
- verified facts with source links or source identifiers
- background timeline
- key actors/stakeholders
- uncertainty / unknowns / disputed claims
- implications for developers, product teams, or policy watchers
- next 3 follow-up questions for deeper research
- concise final executive summary

Final response requirements:
- output document path
- task ids, worker ids, and final task statuses
- what sources were used
- what could not be verified
```

Formal E2E pass criteria:

- real S-profile daemon and configured provider are used; no provider fixture
- prompt enters through normal user input path
- if workers are created, all task/execution/agent ids are queryable through
  owner truth and survive `restartS`
- output document exists and contains the required sections
- TaskBoard, AgentBoard, EventInbox, and TaskHistory match final visible state
- failure due to missing search/write capability is explicit and queryable, not
  a silent or fabricated success

### G5: UI Projection For Production Loop State

Missing only after G1/G2 land:

- WebUI/Android status showing production loop enabled/disabled/error
- worker runner status and last activity
- task execution progress sourced from owner truth, not UI-local state

Why it matters:

UI must remain a projection. It should not be built before the production loop
has owner truth to project.

## Current Production Loop Truth

Closed:

- `config.core` compiles ordered `paired_agents`; Master supports multiple
  explicit Worker peers while every Worker has exactly one Master
- `runtime.master-worker-loop` owns one runner per selected configured Worker
- Slave daemon mode claims only tasks assigned to its own agent id, renews heartbeats, runs the real
  provider/tool loop inside the task cwd, and reports `review_ready` or
  `blocked`
- Master guidance, TaskSpaceSnapshot, and task mutation boundary consume the
  full configured Worker set and reject historical/non-configured agents
- Worker launchd service/env/log naming is agent-specific
- Master prompt ownership is locked to create/assign/query/review; Worker owns
  claim/heartbeat/execution facts
- real-provider S-profile task `task-1783657707` produced
  `/tmp/freehand-worker-e2e-1783657707/result.md`, then Master approved/closed
- same task/execution/worker history survived explicit Worker restart

Still open:

- three independent Worker process online proof; the old verifier started one
  Worker process for three tasks and is not multi-agent evidence
- managed Worker process auto-start/restart and health projection
- concurrent configured Worker allocation/release proof
- background Master timeout/poll scheduler outside an active user turn
- production Master lifecycle runner and interrupted/rejected Worker requeue are
  implemented locally; real-provider blocked/reject/retry/reassignment proof
  remains open
- formal current-source research/document task and browser-visible WebUI proof

## Next Implementation Sequence

### P0: Owner Design And Maps — Completed

Deliverables:

- production Worker loop owner is `runtime.master-worker-loop` in
  `crates/freehand-runtime`; `apps/freehand-daemon` only selects Master UI host
  or Slave Worker host from configured agent mode
- update feature map, function map, test design, mainline manifest, and wiki
- first production slice uses existing explicit `agent.mode`; no enable fallback
  or duplicate startup flag
- define Worker idle/review-ready/blocked outcomes and failure behavior

Validation:

```bash
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

### P1: Configured Worker Runner — Completed

Deliverables:

- Slave mode starts the Worker runner from explicit agent config
- Worker claims assigned tasks from Task Center and renews the lease
- Worker uses task cwd plus Worker-only tool capability policy
- success writes `review_ready`; runtime/provider failure writes `blocked`
- synchronous Worker/provider work runs behind one async blocking boundary
- same task/execution/worker truth survives explicit Worker restart

Validation:

```bash
cargo test -p freehand-task -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-daemon -- --nocapture
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

### P2: Compiled Multi-Worker Topology — Controlled Online Closure Proven

Landed:

- only `paired_agents` is accepted; singular `paired_agent` is rejected
- Master has one-or-more reciprocal Slave peers; Slave has exactly one Master
- runtime Master assignment/create gate accepts any configured Worker and
  rejects historical/non-configured agents before Task Center mutation
- launchd Worker defaults use agent-specific label/env/log paths
- isolated verifier starts three separate Worker daemon processes and binds
  alpha/beta/gamma to `worker-alpha`/`worker-beta`/`worker-gamma`
- online TaskHistory proves distinct PIDs, agent ids, execution ids,
  heartbeat/review histories, and no cross-claim
- beta reject/rework uses a new execution; first parent evaluation creates an
  integration task; only the later evaluation completes the parent goal
- Master restart proof preserves exactly one final parent evaluation
- Task Center atomic JSON temp paths are process-unique, preventing concurrent
  Worker boot from stealing another process's index temp file

Validation:

```bash
cargo test -p freehand-config -- --nocapture
cargo test -p freehand-runtime master_assignment_gate -- --nocapture
bash -n scripts/install-launchd.sh
scripts/verify-launchd-worker-naming.sh
scripts/verify-master-three-worker-e2e-online.sh
```

### P3: Managed Worker Lifecycle And Pool

Deliverables:

- Worker processes auto-start from config and restart on failure
- Worker health/current task/last activity are queryable owner truth
- configured Worker pool allocation and release are deterministic
- no orphan Worker process or leaked lease after task completion/failure

Validation:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
# managed Worker lifecycle verifier remains a P3 deliverable
```

### P4: Background Master Scheduling And Recovery — In Progress

Deliverables:

- background Master poll/timeout scheduling outside an active model turn
- blocked/reject/retry/reassignment/takeover scenarios with typed owner truth
- same task/execution/agent ids across recovery and restart
- deterministic fixtures plus separate real-provider smoke

Validation:

```bash
scripts/install-launchd.sh restartS
freehand-cliS adp-config-query --url ws://127.0.0.1:4042/adp
# recovery-loop verifier to be added in P3
```

### P5: Formal Research And UI Evidence

Deliverables:

- formal current-source research/document task using the acceptance prompt above
- real browser WebUI submission, lifecycle projection, final result, and
  screenshot evidence for the same task
- WebUI projection for production-loop status and worker runner state
- Android proof only if mobile/release surface changes
- no raw ADP/runtime ids or debug plumbing in default user-facing chrome

Validation:

```bash
make verify-webui-online
# if Android/release changed:
scripts/install-global.sh
apps/freehand-android/scripts/verify-device-ui.sh <serial>
```

## Done Definition

Production master/worker autonomy is not complete until:

- the loop starts from daemon/runtime config, not a CLI sample
- worker tasks are claimed and advanced by a production runner
- success, execution-error, and reject-retry fixture scenarios pass through the
  production loop
- same task/execution/agent ids survive S-profile restart verification
- formal real-provider online E2E task is run and its result is documented
- function maps, test designs, mainline JSON, generated wiki, `CACHE.md`,
  `MEMORY.md`, `note.md`, and local skill rules are synchronized
