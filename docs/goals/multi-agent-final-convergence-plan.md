# Multi-Agent Final Convergence Plan

## Objective

Close Freehand's production multi-agent loop end to end:

```text
user objective
→ Master decomposes and dispatches child work
→ Workers execute, submit review-ready truth, pause/block/recover when needed
→ Master evaluates work quality against the total objective and child contracts
→ Master rejects/reassigns/creates next-round work when incomplete
→ Master finalizes only when the total objective is actually satisfied
→ WebUI and Android show current-session task/agent progress and every child session
```

The final behavior is a quality-evaluation loop, not a Worker-result aggregation
summary. Worker results are evidence for Master decisions; Master may require
rework or more tasks before user-visible completion.

## Acceptance Criteria

1. Worker lifecycle is independently closed:
   - assigned → running → review submitted → waiting review
   - rejected → new execution → review submitted
   - blocked/interrupted → Master-visible decision → same task recovers or is reassigned
   - pause/resume has deterministic safe-point acknowledgement and no stale success
   - completed Worker tasks close only after Master approval
2. Master lifecycle is independently closed:
   - EventInbox admission remains source-ordered
   - attention dequeue uses deterministic weighted aging, not wall-clock guessing
   - blocked and critical/high semantic-change attention has large weight
   - retryable failures preserve the same pending attention identity
   - busy Master can suspend only at safe points, run an isolated decision, inject typed resolution, and continue the original logical turn
3. Integration is closed:
   - Master evaluates original user objective + child task contracts + accepted Worker review truth
   - Master creates rework/improvement/next-round tasks when needed
   - no new task is created just because a prior one blocked; same task is recovered/reassigned when semantically same
   - final user-visible completion is accepted only when objective truth is closed
4. Observability is closed:
   - WebUI header/dashboard shows current-session running agents and delegated tasks
   - first tap opens current child-task list only
   - task tap opens the canonical Worker session
   - every child task can be inspected one by one
   - transient provider retry/failover is status, not durable error card after recovery
   - task/timer/tool cards expose semantic operation details, not raw generic labels
5. Android is product-aligned:
   - Android remains a thin WebView over daemon-hosted WebUI
   - no native fallback UI or mock local shell returns
   - APK is rebuilt and installed to the online ADB device when Android/native assets change
   - true-device verification proves the same WebUI behavior on phone layout

## Scope

### In Scope

- `runtime.master-worker-loop`
- `task.orchestration`
- `worker.control`
- `agent.lifecycle`
- `reason.context-planner`
- `contracts.core`
- `ui.protocol`
- `app.webui-smoke`
- `app.android-client` only when phone/WebView/native packaging changes
- S-profile and isolated online verifiers required for production claims

### Out of Scope

- fallback/compatibility paths
- result aggregation as a completion substitute
- direct edits to Task Center JSON truth outside owner APIs
- broad cleanup of unrelated dirty/untracked workspace content
- cross-machine transport unless it becomes required by a failing acceptance gate

## Current Known State

- `master_work` active-work truth exists and is focused-test bound.
- Typed `AttentionResolution` continuation is focused-test bound through
  `master_work.admit_resolution_context`.
- Stale tools are invalidated with paired failed tool results and no side
  effects.
- Stale terminal candidates are discarded before terminal persistence.
- `master.edge.continue_original` is bound by focused tests.
- `master.edge.handle_attention` is focused-test bound: a suspended active user
  turn invokes an event/attempt-scoped isolated control turn, and raw control
  transcript/provider payload text does not enter the foreground session or
  typed resolution.
- Worker production pause/resume is focused-test bound: the runner monitors
  persisted pause control truth, wires it into the live cancel token, stops at
  existing live-bridge safe points without stale review/block publication, and
  re-enters the same task/execution after persisted resume.
- Parent-goal recovery is code-bound and online-proven for the controlled
  three-Worker convergence verifier. `ReasonPersistence::restore_turn_start_snapshots`
  reads authoritative reason-ledger `TurnStarted` rows and honors rollback
  markers, so parent evaluation recovers the original first-round operator
  objective even when the effective/closed snapshot is a later repaired round.
  Process-mode session `online-master-three-worker-evaluation-1784187343` and
  launchd-mode session `online-launchd-three-worker-evaluation-1784187532-2390`
  both reached beta reject/rework, gamma interrupted same-task takeover,
  first evaluation next-round integration, second evaluation final Success,
  and restart-idempotent `final_evaluation_count=1`.
- The controlled three-Worker verifier now treats initial dispatch as a
  foreground waiting turn: after the three initial tasks are created/assigned,
  `SubmitUserInput` must return a completed waiting receipt and the script then
  observes background Worker/Master lifecycle through ADP truth. The verifier
  fails if the foreground receipt is an error. Current launchd proof
  `online-launchd-three-worker-evaluation-1784190586-60111` returned
  `reason_live_turn_completed`, then reached beta reject/rework, gamma
  interrupted same-task takeover by worker-alpha, integration next-round
  closure, final `runtime-turn-3` Success, and restart-idempotent
  `final_evaluation_count=1`.
- Full daemon/WebUI/Android online proof for busy-Master preemption remains
  unclaimed.

## Design Principles

1. Truth source first:
   - Task/agent state comes from Task Center / AgentBoard owner truth.
   - Reason transcript truth comes from reason persistence.
   - UI is projection only.
2. No fallback:
   - recover by owner truth and explicit typed state transitions.
   - do not hide provider/tool/framework failures behind alternate UI or silent retries.
3. Same semantic task stays same task:
   - blocked/interrupted/rejected work recovers or reassigns the existing task unless Master deliberately creates a new child for a new subgoal.
4. Master evaluates, not aggregates:
   - child review truth is input to a quality decision.
   - final answer requires total objective closure.
5. Deterministic contracts:
   - no time guessing for priority or recovery.
   - use source event sequence, typed safe points, task/execution ids, and persisted markers.
6. Online proof before product claims:
   - local unit/focused tests can bind code paths.
   - product closure requires daemon/WebUI/ADP/Android evidence where applicable.

## Technical Plan

### 1. Bind isolated Master attention decision for busy work — focused-test complete

Target docs:

- `docs/lifecycles/master-worker-lifecycle.json`
- `docs/wiki/master-worker-lifecycle.md`
- `docs/function-maps/runtime.master-worker-loop.md`
- `docs/mainline-calls/runtime.master-worker-loop.json`
- `docs/testing/runtime.master-worker-loop.md`

Bound evidence:

- `production_master_attention_uses_isolated_control_turn` proves a suspended
  active user turn invokes an event/attempt-scoped lifecycle/control decision
  with a session, turn, and trace distinct from foreground work.
- `production_master_attention_raw_transcript_never_enters_user_session`
  proves raw control/provider sentinel text cannot enter foreground
  ReasonPersistence, `master_work`, or typed resolution constraints.
- Existing exact-identity, mismatched-identity, safe-point, stale-tool, and
  stale-terminal tests remain green.

Remaining boundary:

- S-profile daemon/WebUI proof must still demonstrate the full live
  suspend → isolated decision → typed continuation sequence before product
  closure is claimed.

### 2. Close Worker pause/resume lifecycle — focused-test complete

Target docs:

- `docs/function-maps/worker.control.md`
- `docs/testing/worker.control.md`
- `docs/lifecycles/master-worker-lifecycle.json`
- `docs/wiki/master-worker-lifecycle.md`

Bound evidence:

- `production_worker_runner_pause_stops_before_submission` now applies pause
  while Worker execution is in flight, observes the live cancel token, and
  verifies no review/block truth is written.
- `production_worker_runner_resume_reenters_reasoning_and_submits_review`
  proves persisted resume re-enters the same task/execution.
- `production_worker_runner_paused_execution_cannot_publish_stale_success` and
  `production_worker_runner_paused_without_resume_stays_idle` lock the negative
  stale-success and no-resume paths.

Remaining boundary:

- Product closure still requires the full S-profile multi-Worker/WebUI online
  convergence proof.

### 3. Close blocked/interrupted/rejected same-task recovery policy

Target docs:

- `docs/function-maps/runtime.master-worker-loop.md`
- `docs/testing/runtime.master-worker-loop.md`
- `docs/function-maps/task.orchestration.md`
- `docs/testing/task.orchestration.md`

Implementation direction:

- Same semantic task must not duplicate into a new blocked task.
- Master must append blocked decision, reassign, or adjust the same task when
  the objective is unchanged.
- New task creation is allowed only for a new subgoal or next-round work found
  by Master evaluation.
- TaskBoard and WebUI should not show historical/unrelated blocked tasks as the
  current session's progress.

Expected tests:

- positive: blocked same task is reassigned/recovered without duplicate task id
- positive: next-round new task is created only after Master evaluation discovers new work
- negative: repeated blocking does not create an unbounded series of replacement tasks
- negative: current-session dashboard does not display global historical blocked tasks

### 4. Close full Master quality-evaluation loop

Target docs:

- `docs/lifecycles/master-worker-lifecycle.json`
- `docs/wiki/master-worker-lifecycle.md`
- `docs/function-maps/runtime.master-worker-loop.md`
- `docs/testing/runtime.master-worker-loop.md`

Implementation direction:

- Parent evaluation must always compare:
  - original user objective from authoritative reason `TurnStarted` ledger
    truth via `ReasonPersistence::restore_turn_start_snapshots`
  - decomposed child task goal/deliverables/acceptance
  - accepted Worker review summary/deliverables/evidence
  - current TaskBoard/AgentBoard/EventInbox truth
- It may approve, reject, reassign, create next-round work, or block.
- It must not claim complete while open required children exist.
- It must not treat all children closed as sufficient completion.

Expected tests:

- positive: rejected Worker review triggers rework and then approval/close
- positive: all first-round children closed triggers evaluation that creates next-round work
- positive: final completion only after next-round work closes and objective is verified
- negative: aggregation-only final answer is rejected
- negative: open sibling child rejects `claim="complete"`

### 5. Close WebUI observability and control UX

Target docs:

- `docs/function-maps/app.webui-smoke.md`
- `docs/testing/app.webui-smoke.md`
- `docs/function-maps/ui.protocol.md`
- `docs/testing/ui.protocol.md`

Implementation direction:

- Header/dashboard must be current selected-session scoped.
- It must show running agents, delegated tasks, review/block/rework state, and
  semantic task/timer/tool activity details.
- It must expose every child task and the canonical Worker transcript.
- It must render retry/failover as transient same-turn status that clears on recovery.
- It must avoid fake user prompts, repeated internal task prompts, and empty-session flashes.

Expected tests:

- browser-visible mobile and desktop WebUI tests
- ADP truth comparison for selected session, child tasks, Worker transcripts, and tool/timer cards
- negative tests for global historical task leakage
- negative tests for fake user prompt rendering

### 6. Close Android true-device proof

Target docs:

- `docs/function-maps/app.android-client.md`
- `docs/testing/app.android-client.md`

Implementation direction:

- Rebuild APK and install only when Android/native/app packaging changes.
- Verify foreground `com.freehand.android` activity and canonical WebUI DOM probe.
- Prove phone layout can open Agent sheet, inspect child task, enter Worker session, and return.
- Keep explicit error screen/status for load failures; no fallback UI.

Expected tests:

- Android unit tests if native code changes
- Gradle build when APK changes
- `adb install` / upgrade to online device
- true-device screenshot/probe evidence

## Risk Matrix

| risk | mitigation |
| --- | --- |
| Master appears complete after dispatch only | runtime gate rejects complete with open required children; online proof must include rework/next-round work |
| blocked task duplication | same semantic task recovery tests; dashboard selected-session filtering |
| stale foreground tool side effects after attention | paired failed tool-result invalidation tests |
| stale terminal truth after attention | terminal-persistence preflight tests |
| raw transcript leakage | context planner rejection and provider-request inspection |
| Worker pause/resume publishing stale success | safe-point acknowledgement and negative stale-success test |
| WebUI shows old/global tasks | selected-session scoped ADP/WebUI proof |
| Android drifts from WebUI | WebView-only invariant and true-device proof |

## Verification Matrix

Minimum local gates:

```bash
cargo fmt --check
cargo clippy -p freehand-runtime --all-targets -- -D warnings
cargo clippy -p freehand-blocks --all-targets -- -D warnings
cargo clippy -p freehand-contracts --all-targets -- -D warnings
cargo test -p freehand-runtime master_runner::tests:: -- --nocapture
cargo test -p freehand-runtime live_master_attention -- --nocapture
cargo test -p freehand-runtime runtime_live_submit -- --nocapture
cargo test -p freehand-runtime production_worker_runner -- --nocapture
cargo test -p freehand-blocks -- --nocapture
cargo test -p freehand-contracts -- --nocapture
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
git diff --check
```

Minimum online gates:

```bash
scripts/install-launchd.sh restartS
curl -fsS http://127.0.0.1:4042/health
freehand-cliS adp-smoke --url ws://127.0.0.1:4042/adp
scripts/verify-master-three-worker-e2e-online.sh
scripts/verify-worker-subtasks-online.py
scripts/verify-worker-subtasks-webui-online.mjs
```

If Android/native changes are included:

```bash
cd apps/freehand-android
JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew testDebugUnitTest assembleDebug
adb devices -l
adb install -r app/build/outputs/apk/debug/app-debug.apk
apps/freehand-android/scripts/verify-device-ui.sh <device>
```

Online proof must save artifacts under `artifacts/webui-online/` or
`artifacts/android-device/` and must compare visible UI state with ADP truth.

## Implementation Steps

1. Refresh context:
   - read `AGENTS.md`, `CACHE.md`, `MEMORY.md`, `note.md`
   - search MemoryPalace
   - inspect resource map, function map, mainline call map, lifecycle manifest,
     and test design before edits
2. Close Worker pause/resume lifecycle with safe-point tests.
3. Close same-task recovery and no-duplicate-blocked-task policy.
4. Run local focused tests and gates.
5. Restart S-profile and run online Master/Worker/WebUI proof.
6. Rebuild/install Android only if Android/native packaging changed; otherwise
   prove phone WebView against daemon-hosted WebUI when requested.
7. Update function maps, mainline JSON, generated wiki, test designs, note,
   MEMORY, and local skill when truth changes.
8. Commit only the scoped tracked files; leave unrelated dirty/untracked files
   untouched.

## Definition of Done

The task is complete only when:

- Worker lifecycle, Master lifecycle, and integration loop each have independent positive/negative tests.
- A real or controlled online run proves at least three configured Workers can
  execute child tasks, receive review/rework, recover or reassign when needed,
  and converge to final user completion only after total-objective evaluation.
- WebUI shows the current session's Agent/task state, supports per-child
  inspection, and matches ADP truth.
- Android proof is current if Android/app packaging changed or phone UI is part
  of the claim.
- No fallback UI/path/retry semantics were added.
- All required docs, maps, generated wiki, MEMORY/note/local skill updates are
  synchronized.
- Final report states what is verified online, what remains unverified, and the
  exact artifact paths.
