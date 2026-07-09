# Multi-Task Foundation Phase 2 Gap Plan

## Status

Gap and execution plan after Phase 1 headless foundation closeout.

Phase 1 proved owner-backed TaskBoard, AgentBoard, ExecutionFact,
SchedulerTick, and restart same-id recovery. Phase 2 must turn those truth
surfaces into a framework-mediated master/worker execution loop before UI
dashboard work.

## Baseline Already Closed

Current verified foundation:

- TaskBoard owner-backed query skeleton
- AgentLifecycleSnapshot and AgentBoard projection skeleton
- ExecutionFact sync for running/recovering/blocked/review_ready
- SchedulerTick durable facts and recommendations only
- `freehand-cliS phase1-foundation-sample`
- S-profile `127.0.0.1:4042` restart same-id proof
- docs/function-map/test-design/mainline/wiki sync for Phase 1 surfaces

## Current Gap Summary

### G1: Worker Task Queue And Claim Loop

Missing:

- worker-visible assigned-task queue
- automatic or explicit worker claim loop around `claim_next`
- notification that a worker should check its queue
- durable claim/lease evidence tied to execution id

Why it matters:

Phase 1 can prove task assignment and execution facts, but it does not prove a
real worker receives work and starts execution through the framework.

### G2: Master Poll Loop

Missing:

- master loop that loads TaskBoard + AgentBoard + EventInbox cursor
- board-state classification for review_ready, blocked, stale, idle worker,
  ready subtask, and timeout
- master action admission back through `task(op=...)`
- durable processed-event cursor

Why it matters:

Without this, TaskBoard exists but no master behavior is attached to it. UI
would only display facts, not prove task management.

### G3: Worker Execution Review/Retry Closure

Missing:

- headless proof that a worker reaches review_ready
- master reject feedback linked to retry
- worker retry creates a new execution attempt or retry state
- second review_ready can be approved
- close only after approved review

Why it matters:

This is the first real "work was delegated, corrected, and accepted" loop.

### G4: Worker Control Inbox

Missing:

- `worker_control(op=...)` owner decision and function map
- control inbox persistence
- safe-point checks
- framework-answerable status query
- worker-model-answerable question/answer
- accepted/deferred/answered/rejected events

Why it matters:

Agent-to-agent runtime communication must happen through framework queues, not
private prompt mutation or direct state edits.

### G5: Event Inbox And Subscription Surface

Missing:

- master-visible event inbox cursor backed by Task Center truth
- query/subscription for task/execution events beyond board snapshots
- same-id recovery proof for event cursor and inbox contents

Why it matters:

Polling only board state loses "what happened since last check" semantics.

### G6: Agent Lifecycle Event Coverage

Missing:

- live bridge events wired into lifecycle reducer for real model/tool/provider
  activity
- worker blocked/recovering/review states projected from real execution path
- lifecycle counters proved in master/worker loop samples

Why it matters:

Phase 1 has lifecycle skeleton/projection; Phase 2 must prove real execution
events feed it.

### G7: UI Projection

Missing:

- WebUI/Android TaskBoard and AgentBoard dashboard
- task/execution event timeline
- worker status and control affordances

Why it is not first:

UI should project owner truth after the master/worker loop works. Building UI
first would risk another fake state surface.

## Recommended Execution Order

### Phase 2A: Worker Execution Loop, No UI

Goal:

Prove one BigTask/SubTask master-worker lifecycle through framework truth.

Deliverables:

1. worker queue/claim loop around assigned tasks
2. execution id binding on claim/start
3. worker progress/block/recover/review facts through Task Center
4. review reject -> retry -> review_ready -> approve -> close
5. headless CLI sample
6. restart same-id proof

Required sample:

```text
master-worker-foundation-sample
```

Sample path:

```text
create task
assign worker
worker claim_next
execution running
record progress
record blocked
master query blocked
record recovering
submit review
master reject
worker retry
submit review again
master approve
close task
restart
verify same task/execution/agent/history/lifecycle ids
```

Why first:

It proves the core delegation lifecycle without UI or worker-control complexity.

### Phase 2B: Master Poll Loop And Event Inbox

Goal:

Make master behavior consume TaskBoard + AgentBoard + EventInbox instead of
manual sample sequencing.

Deliverables:

1. EventInbox cursor model
2. master board classification helper
3. master poll-loop headless sample
4. processed cursor persistence and restart proof
5. no framework business decisions; only model/tool-admitted actions mutate
   task truth

Why second:

The poll loop needs a proven worker execution lifecycle to manage.

### Phase 2C: Worker Control Channel

Goal:

Implement safe-point runtime communication for already-running worker
executions.

Deliverables:

1. `worker_control` owner map/function map/test design
2. control inbox ledger/snapshot
3. `query_status` framework-answerable path
4. `ask_at_safe_point` worker-model-answerable path
5. pause/resume/cancel events that write Task Center consequences
6. restart same-id proof for control events

Why third:

It is runtime coordination, not task mutation. It should not block the basic
task execution/review/retry loop.

### Phase 2D: UI Projection

Goal:

Render the already-proven TaskBoard, AgentBoard, execution history, and worker
control surfaces.

Deliverables:

1. WebUI task/agent dashboard
2. Android projection alignment
3. compact task/execution timeline
4. debug-only raw ids/details
5. browser and device evidence against ADP truth

Why last:

UI becomes a reliable projection only after the owner truth and queue behavior
exist.

## Non-Goals For Phase 2A

- multi BigTask context switching
- cross-machine workers
- worker autoscaling
- full UI dashboard
- direct Agent-to-Agent private messaging
- direct prompt-history mutation as worker control
- framework choosing business actions without master/model admission

## Required Owner Work Before Phase 2A Code

- update `task.orchestration` function map/test design for worker queue/claim
  sample if existing entries are insufficient
- update `agent.lifecycle` function map/test design for real execution-event
  coverage
- update `runtime.ui-command-dispatch` only if new ADP commands are required
- update `app.cli-runtime-smoke` for `master-worker-foundation-sample`
- add `worker_control` feature only when Phase 2C starts, not in Phase 2A

## Required Validation For Phase 2A

Local:

```bash
cargo fmt --check
cargo test -p freehand-task -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

Online:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp
scripts/install-launchd.sh restartS
freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp --verify ...
```

## Completion Standard

Phase 2A is complete only when:

- the sample proves worker claim, progress, blocked, recovering, review_ready,
  reject, retry, approve, and close
- TaskBoard and AgentBoard both show the expected states
- task history proves ordered durable events
- restart verify queries the same ids
- no UI-local state or model prose is used as proof
- docs/function-map/test-design/mainline/wiki/MEMORY/note are synchronized

## Phase 2A Closeout Status

Phase 2A is implemented and verified for the current no-UI scope. Current
S-profile proof closed task
`task-cli-master-worker-FHPHASE2A1783515402294813000` with execution
`exec-cli-master-worker-FHPHASE2A1783515402294813000` and worker
`worker-cli-master-worker-FHPHASE2A1783515402294813000`; after `restartS`,
verify mode queried the same ids and returned closed task history plus closed
agent lifecycle truth.

## Phase 2B Implementation Target

Phase 2B is the next no-UI slice. It must make master behavior consume
owner-backed TaskBoard, AgentBoard, and EventInbox truth instead of relying on
manual sample sequencing.

Deliverables:

1. EventInbox projection built from Task Center ledger truth, with a stable
   master-visible cursor and compact event kinds.
2. Durable master processed cursor stored under Task Center runtime state and
   recovered after restart.
3. Master poll outcome that reads TaskBoard, AgentBoard, and EventInbox, then
   classifies review_ready, blocked, stale, soft_timeout, hard_timeout, idle
   worker, ready task, and all-closed states.
4. Headless ADP/CLI sample `master-poll-foundation-sample` proving inbox,
   classifications, cursor persistence, and restart same-id cursor recovery.
5. No framework business action mutation. The poll output may recommend
   semantic actions, but approve/reject/assign/close still require explicit
   task mutation commands admitted through the owner path.

Required validation:

```bash
cargo test -p freehand-task -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
freehand-cliS master-poll-foundation-sample --url ws://127.0.0.1:4042/adp
scripts/install-launchd.sh restartS
freehand-cliS master-poll-foundation-sample --url ws://127.0.0.1:4042/adp --verify-cursor ...
```

Phase 2B remains no-UI. WebUI/Android dashboards, worker_control, multi
BigTask, cross-machine workers, and framework-owned business decisions stay out
of scope.

## Phase 2 Current Closeout Status

Phase 2A, 2B, and 2C are implemented and verified for their no-UI foundation
scope.

- Phase 2A closed the master-worker execution lifecycle sample: worker
  creation, assignment, claim, progress, blocked, recovering, review, reject,
  retry, approve, close, and restart same-id proof.
- Phase 2B closed EventInbox and MasterPoll truth: four-part event cursors,
  replay-from-start full drain, persisted cursor, owner-backed cursor reread,
  and restart same-cursor proof.
- Phase 2C closed worker-control foundation: `query_status`, safe-point queued
  requests, Task Center-backed pause/resume/cancel consequences, control
  ledger/snapshot, and restart same-id proof.

The remaining Phase 2 work is Phase 2D plus final Phase 2 audit. Phase 2D is
not allowed to invent new task, agent, event, or worker-control truth. It must
project already-proven owner truth through shared protocol surfaces, then prove
WebUI/Android-visible state matches ADP/headless truth.

## Phase 2 Full Closeout Plan

### Objective

Complete the Phase 2 master/worker foundation by turning the proven no-UI
TaskBoard, AgentBoard, EventInbox, execution history, and worker-control truth
into reliable UI projections, then run a full Phase 2 regression and restart
recovery audit.

### In Scope

- Revalidate Phase 2A/2B/2C headless owner truth before UI work.
- Fill any missing `ui.protocol` query/subscription projection needed by UI for
  TaskBoard, AgentBoard, EventInbox, execution history, and worker-control
  timeline/status.
- Implement WebUI Phase 2D projection with compact task/agent surfaces, event
  timeline, worker status, and worker-control affordances.
- Keep Android aligned with the WebUI responsive surface when the shared WebUI
  assets change; validate on browser mobile layout and true Android device when
  APK/release surface is touched.
- Hide raw internal transport names, raw ids, and debug-only fields by default;
  expose them only through explicit debug/details surfaces.
- Update function maps, test designs, mainline-call manifests, generated wiki,
  `CACHE.md`, `MEMORY.md`, `note.md`, and local skill rules when truth or
  verification workflow changes.

### Out Of Scope

- Multiple independent BigTasks and cross-session master context switching.
- Cross-machine workers and worker autoscaling.
- Framework-owned business decisions without model/tool admission.
- Direct private Agent-to-Agent mutation or prompt-history mutation.
- New provider/model reasoning semantics beyond what is required to project
  Phase 2 owner truth.
- Replacing the current worker-control foundation with real preemptive runtime
  interruption unless a separate owner design and tests are added first.

### Execution Order

1. Baseline audit: run source-only search through feature maps/function maps,
   then revalidate Phase 2A, 2B, and 2C samples on S-profile `127.0.0.1:4042`
   including restart same-id/same-cursor proofs.
2. Projection contract audit: inspect `ui.protocol`,
   `runtime.ui-command-dispatch`, and CLI samples to identify the exact missing
   query/subscribe shapes for Phase 2D UI. Add only owner-routed projections;
   do not let app code infer task or control truth.
3. WebUI implementation: render TaskBoard/AgentBoard/EventInbox/worker-control
   from protocol truth, keep conversation primary on phone layouts, use
   drawers/details for low-frequency surfaces, and keep debug fields hidden by
   default.
4. Android alignment: if shared WebUI assets or APK packaging change, install
   the APK on the online phone and verify the same Phase 2 surfaces through the
   real WebView.
5. Final audit: run local gates, online S-profile proof, browser proof, Android
   proof when applicable, source-only MemoryPalace mine/search, and commit the
   complete Phase 2 closeout.

### Verification Matrix

Local required checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p freehand-task -- --nocapture
cargo test -p freehand-ui-protocol -- --nocapture
cargo test -p freehand-runtime -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo test -p freehand-server -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

Online S-profile required checks:

```bash
scripts/install-launchd.sh restartS
curl -4fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp
scripts/install-launchd.sh restartS
freehand-cliS master-worker-foundation-sample --url ws://127.0.0.1:4042/adp --verify ...
freehand-cliS master-poll-foundation-sample --url ws://127.0.0.1:4042/adp
scripts/install-launchd.sh restartS
freehand-cliS master-poll-foundation-sample --url ws://127.0.0.1:4042/adp --verify-cursor ...
freehand-cliS worker-control-foundation-sample --url ws://127.0.0.1:4042/adp
scripts/install-launchd.sh restartS
freehand-cliS worker-control-foundation-sample --url ws://127.0.0.1:4042/adp --verify-task ...
```

UI required checks:

- Real browser proof against fixed S-profile `127.0.0.1:4042`.
- Browser-visible TaskBoard/AgentBoard/EventInbox/worker-control state must
  match ADP/headless query truth for the same task/execution/agent/control ids.
- Historical task/control rows must not show fake live animation after terminal
  state.
- User-facing UI must not show raw `ADP`, raw runtime turn ids, or debug-only
  transport plumbing by default.
- If Android/release assets are changed, run release asset hash proof and true
  Android device verification with the online phone.

Memory and closeout checks:

- Update `note.md`, promote durable truth to `MEMORY.md`, update `CACHE.md`.
- Mine a source-only curated MemoryPalace corpus into `wing=freehand`.
- Verify the new Phase 2 closeout marker is searchable.
- Commit only after validation evidence is current.

### Done Definition

Phase 2 is complete only when:

- Phase 2A/2B/2C headless samples still pass with restart same-id/same-cursor
  recovery evidence.
- Phase 2D UI is a projection of owner truth, not UI-local task/control state.
- WebUI and Android-visible Phase 2 state can be traced to ADP/headless truth.
- Function maps, test designs, mainline manifests, generated wiki, memory, and
  local skill rules are synchronized.
- Final local gates, online S-profile proof, browser proof, source-only
  MemoryPalace proof, and required device proof have current evidence.
