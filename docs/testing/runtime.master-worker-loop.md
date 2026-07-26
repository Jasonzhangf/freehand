# Test Design: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner: `crates/freehand-runtime`
- host: `apps/freehand-daemon`
- resource map: `docs/resource-maps/core.json`
- lifecycle manifest: `docs/lifecycles/master-worker-lifecycle.json`
- lifecycle review: `docs/wiki/master-worker-lifecycle.md`
- resource operation coverage:
  - `timer.fire_master_wakeup`
  - `timer.schedule`
  - `timer.cancel`
  - `timer.list`
  - `master_work.resolve_attention`
  - `master_work.admit_resolution_context`
  - `agent.heartbeat`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `timer.fire_master_wakeup` | bound | `cargo test -p freehand-runtime timer -- --nocapture` covers durable timer due-claim, one-shot, recurring, local-time cron/daily/weekly, wakeup prompt, failure release, and Master runner tests | `cargo test -p freehand-runtime production_master -- --nocapture` covers production Master runner smokes where due timers create internal Master wakeup turns without task-state mutation | `scripts/verify-timer-tool-online.sh` covers S-profile timer online proof and restart-due proof showing persisted timer wakeup fires after due time and completes timer truth |
| `timer.schedule` | bound | `cargo test -p freehand-runtime timer -- --nocapture` covers relative, absolute, interval, daily, weekly, cron schedule construction, max-runs validation, and persisted wakeup prompt truth | `cargo test -p freehand-runtime runtime_timer_ui_commands -- --nocapture` covers runtime UI command dispatch persisting a timer schedule through TimerStore owner truth | `node scripts/verify-webui-timer-dashboard-online.mjs` covers S-profile WebUI scheduling through ADP and owner-backed timer truth |
| `timer.cancel` | bound | `cargo test -p freehand-runtime timer -- --nocapture` covers TimerStore cancellation and TimerCancelled ledger persistence | `cargo test -p freehand-runtime runtime_timer_ui_commands -- --nocapture` covers runtime UI cancel dispatch returning owner receipt and cancelled projection | `node scripts/verify-webui-timer-dashboard-online.mjs` covers S-profile WebUI cancellation through ADP and verifies TimerCancelled ledger truth |
| `timer.list` | bound | `cargo test -p freehand-runtime runtime_timer_ui_commands -- --nocapture` covers UI-safe timer schedule/event projection from TimerStore truth | `cargo test -p freehand-ui-protocol timer_ -- --nocapture` covers protocol TimerList DTO roundtrip and command/query route shape | `node scripts/verify-webui-timer-dashboard-online.mjs` covers S-profile Timer dashboard list rendering and ADP QueryTimerList projection without new top-level sessions |
| `master_work.resolve_attention` | bound | `cargo test -p freehand-runtime production_master_busy -- --nocapture` and `cargo test -p freehand-runtime production_master_attention -- --nocapture` cover lower-priority deferral, safe-point high-priority interruption, isolated control-turn identity, raw-control-transcript exclusion, mid-provider/tool no-interrupt, and exact identity restoration | `cargo test -p freehand-runtime runtime_live_submit -- --nocapture` covers live dispatcher active-work register/clear plus concurrent-work rejection without ordinal gaps | `cargo test -p freehand-runtime production_master_attention -- --nocapture` covers suspended-foreground linkage, isolated lifecycle request identity, raw-control-transcript exclusion, checkpoint-missing failure, and exact-identity restoration at the runtime owner boundary |
| `master_work.admit_resolution_context` | bound | `cargo test -p freehand-runtime production_master_resume -- --nocapture` and `cargo test -p freehand-blocks attention_resolution_segment -- --nocapture` cover one-shot typed resolution consumption, return-identity rejection, raw transcript rejection, segment ordering, and rewrite-base rejection | `cargo test -p freehand-runtime live_master_attention -- --nocapture` covers refreshed TaskSpaceSnapshot plus typed AttentionResolution admission, stale tool paired failure/no side effect, and stale terminal non-persistence | `cargo test -p freehand-runtime live_master_attention -- --nocapture` is the focused project proof; daemon/WebUI/Android online preemption remains explicitly unclaimed |
| `agent.heartbeat` | bound | `cargo test -p freehand-task agent_process -- --nocapture` locks typed start/heartbeat validation, restart identity, TTL health, and no task-activity fallback | `cargo test -p freehand-runtime production_worker_runner -- --nocapture` proves constructor start, idle tick heartbeat, same-agent restart, and active-loop wiring | `scripts/verify-master-three-worker-e2e-online.sh` proves isolated three-process fresh/offline/restart AgentBoard truth |


## Lifecycle Under Test

1. Master Task Center has three registered configured Workers and one Assigned
   task per Worker.
2. Each Slave daemon selects one configured Worker and starts its own production
   runner.
3. Agent-specific Worker LaunchAgents start at login, stay alive, and restart
   the same configured Worker process after an unexpected exit.
4. Each runner opens its only configured Master's Task Center namespace.
5. Each runner writes one typed process-start event and refreshes the same
   process instance on idle ticks and while executing work.
6. Each runner claims only the task assigned to its own Worker identity and
   persists lease heartbeat.
7. Runner expands a leading `~`, canonicalizes `task.target_cwd` through symlinks, and locks the Worker to that canonical workspace.
8. Runner executes one provider/reason turn under Worker identity and Worker tool policy.
9. Runner writes `review_ready` on successful completion, `interrupted` on
   provider/network system failure after provider-owned retry exhaustion, or
   `blocked` on task-content, path-preflight, model-terminal, or other
   non-provider execution failure.
10. Runner returns to polling without inventing task truth while idle.
11. Restart reads the same task/execution/agent/history truth and increments
    process restart count only for a new process-instance identity.
12. Each Master lifecycle event attempt runs in a task-scoped internal
    lifecycle session with event-and-attempt-isolated turn and trace ids; the
    session name is reused per task to avoid user-facing session explosion.
13. Task Center EventInbox processing has priority over due independent timer
    schedules. The Master lifecycle runner claims due timer schedules only when
    no current task event produced a lifecycle outcome.
14. A successful target-task mutation is evaluated against an explicit
    event-specific decision boundary. Once reached, the framework closes the
    lifecycle turn immediately instead of asking the model to wait for future
    Worker events inside the same turn.
15. A lifecycle decision has a finite round budget. Exhaustion becomes an
    explicit blocked lifecycle turn and leaves the EventInbox cursor retryable;
    it must never remain as an unbounded active reason turn.
16. Retryable Master decision failures keep the same durable event cursor,
    back off, and retry without terminating the daemon. Task Center/state
    persistence failures remain fatal because owner truth is unavailable.
17. Background lifecycle, parent-evaluation, and timer wakeup turns publish
    reason/debug/task-list hooks into the same shared UI protocol state as
    foreground submits, so retry/backoff/failover/schema repair has an owning
    session id, turn id, and queryable model-waiting state before the next tick.
18. Master attention processing is two-phase. Admission consumes EventInbox in
    source order and advances the cursor only after durable admission or
    explicit non-attention classification. Dequeue then uses deterministic
    weighted aging: blocked showstoppers and critical/high-priority work carry
    large weight, while admission sequence aging prevents low-priority
    starvation without wall-clock timing.
19. Busy-Master continuation is cooperative and identity-bound. A foreground
    `SuspendRequested` becomes `SuspendedByAttention` only when the exact
    foreground session/turn reaches an interruptible safe point. Provider,
    tool-effect, and terminal-persistence in-flight phases remain
    `SuspendRequested`; an isolated lifecycle/control turn cannot mutate the
    foreground safe point. A returned typed resolution is consumed once,
    refreshes TaskSpaceSnapshot, invalidates stale tools/terminal candidates,
    and enters the next provider request without raw transcript/provider
    payload admission.

## Current-Configuration Closure Matrix

The production acceptance target is one configured Master plus at least three
explicit configured Worker identities in `~/.freehand/config.toml`. Every
Worker runs in its own daemon process and may execute at most one claimed task
at a time. Multiple independent BigTasks remain outside this slice.

## Closed-Loop Lifecycle Contract

Every lifecycle below must close through owner truth, not through UI state,
model prose, or a user sending another message. A lifecycle branch is closed
only when the table's owner truth, next action, and observation projection are
all present.

| lifecycle | owner truth | abnormal states | required closure action | observable projection | verification entrance |
| --- | --- | --- | --- | --- | --- |
| Task / execution | TaskRuntime task ledger, task snapshot, execution id, review state | `TaskInterrupted`, `TaskBlocked`, `TaskReviewRejected`, stale `TaskReviewSubmitted`, stale `Approved` without `Closed` | Master must reject/retry, reassign same task, append blocked decision, approve+close, or leave a deliberate blocked decision; Worker must not silently retry blocked/interrupted work | TaskBoard, TaskHistory, EventInbox, selected parent session | `cargo test -p freehand-runtime production_master_runner -- --nocapture --test-threads=1`; `scripts/verify-master-three-worker-e2e-online.sh` |
| Worker process / agent resource | AgentLifecycle and AgentBoard process instance, pid, heartbeat, TTL-derived alive, current task/execution binding | heartbeat stale, `alive=false`, process restart, current binding after terminal task truth | launchd may restart the process; Master uses TaskHistory plus AgentBoard truth to choose same-task retry or cross-Worker takeover; terminal task truth must clear current binding | AgentBoard process fields, `restart_count`, `alive`, `current_task_id`, `current_execution_id`, `last_activity` | `cargo test -p freehand-task agent_process -- --nocapture`; `scripts/verify-master-three-worker-e2e-online.sh` offline/restart phase |
| Master supervisor attention | Master loop state, EventInbox cursor, pending attention, lifecycle turn, target-task decision boundary | missing review decision, incomplete approval, blocked without append, interrupted without reassignment, provider retry/failover/schema repair | keep the same attention item retryable with bounded backoff until owner mutation closes the boundary; after retry cap, record explicit blocked-decision truth where applicable; never mark success without Task Center mutation | internal lifecycle session, ErrorCenter, SessionList active turn, TaskHistory cursor advance | `cargo test -p freehand-runtime production_master_runner -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime runtime_query_session_turns_ -- --nocapture --test-threads=1` |
| Master background crash/restart | Master loop state `cursor`, durable `pending_attention`, `retry_event_id`, `retry_attempt`, TaskRuntime EventInbox | process exits after EventInbox admission but before provider decision; provider/system executor failure after a decision attempt | restart must reuse the same durable attention item, preserve or reset the attempt according to what was persisted, and close only after Task Center owner mutation; cursor must not advance to success on prose/error alone | master-loop state JSON, internal lifecycle turn id `attempt-N`, TaskHistory terminal/mutation truth | `cargo test -p freehand-runtime production_master_runner_recovers_after_crash_with_admitted_attention_before_decision -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime production_master_runner_restarts_after_executor_failure_and_closes_same_event -- --nocapture --test-threads=1` |
| Master foreground active work crash/restart | `master_work` active-work checkpoint plus reason active-turn snapshot | foreground Master provider/tool turn owner process disappears; active-work checkpoint has no live in-memory owner | runtime bootstrap must either interrupt and close the matching active reason turn or clear invalid checkpoint-only truth before accepting new work; it must not require a new user message to unlock the UI | selected parent session terminal projection and absent active-work checkpoint | `cargo test -p freehand-runtime live_dispatch_recovers_dead_owner_master_active_work_before_new_turn -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime live_bootstrap_clears_dead_owner_master_active_work_without_active_snapshot -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime live_bootstrap_clears_dead_owner_master_active_work_without_session_truth -- --nocapture --test-threads=1` |
| Worker running-task crash/restart | TaskRuntime lease snapshot, task ledger, AgentLifecycle current binding | Worker exits while task is `Running`; lease missing/expired before next boot | next TaskRuntime boot writes `TaskInterrupted`, releases the Worker binding, and Master restart must explicitly reassign the same task before any new Worker execution starts | TaskHistory `TaskInterrupted -> TaskAssigned`, same task id, AgentBoard idle/reusable Worker | `cargo test -p freehand-runtime production_master_runner_reassigns_expired_running_task_after_restart -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime production_worker_runner_expired_lease_waits_for_master_reassignment -- --nocapture --test-threads=1` |
| Parent user session / workset | reason persistence parent turn truth, Task parent links, parent evaluation marker keyed by parent session plus logical turn workset | child tasks still actionable, first closed exact-round child while siblings remain open, decided `TaskBlocked` without parent follow-up, failed/interrupted/cancelled parent-evaluation turn | parent stays `waiting` while any same-logical-turn child is actionable; once all close, one idempotent parent evaluation approves final completion, creates next-round work, or records blocker; once no active/reviewable sibling remains and a blocked child has Master `blocked_decision`, one idempotent parent blocked follow-up closes the parent session observably as blocked; failed evaluation remains retryable | selected parent session turns, task tree, parent `runtime-turn-N` / `runtime-turn-N-rM` cards | `cargo test -p freehand-runtime production_master_runner_groups_parent_workset_by_logical_turn_rounds -- --nocapture --test-threads=1`; `cargo test -p freehand-runtime production_master_runner_projects_decided_worker_block_to_parent_session -- --nocapture`; `scripts/verify-master-three-worker-e2e-online.sh` |

| branch | required owner truth | required next action |
| --- | --- | --- |
| success | `TaskReviewSubmitted` with deliverables and evidence | Master approves and closes, or rejects with requirements |
| review rejected | `TaskReviewRejected` remains durable | same Worker receives a new execution with rejection requirements |
| provider/network system failure after internal retries | `TaskInterrupted` with paired reason/evidence | Worker leaves the task interrupted; Master chooses same-Worker retry or cross-Worker takeover, then explicitly reassigns the same task |
| task-content, path-preflight, or model-terminal failure | `TaskBlocked` with paired reason/evidence | Master explicitly retries/reassigns or leaves the task blocked |
| Worker process crash | boot writes `TaskInterrupted` for missing/expired lease | Worker leaves the task interrupted; Master inspects TaskHistory plus AgentBoard and explicitly reassigns the same task |
| Worker-specific route no longer productive | interruption evidence plus AgentBoard availability remain owner truth | Master may replace the same task's assignment with another configured available Worker; task id and parent session id stay unchanged |
| Worker process heartbeat stale/missing | AgentBoard retains task/execution history and projects `alive=false` after the owner TTL | launchd may supervise the process, while Master/UI rely only on AgentBoard owner truth |
| daemon restart while idle | task/agent/cursor truth reloads unchanged | loops resume without duplicate task mutation |
| daemon restart while review is pending | review truth reloads unchanged | Master review loop continues from durable task truth |
| daemon stopped before review is pending | `TaskReviewSubmitted` is seeded through TaskRuntime owner API while Master daemon is offline | after restart, Master consumes persisted review truth and closes or rejects |
| daemon stopped before rejected retry | `TaskReviewRejected` is seeded through TaskRuntime owner API while Master and Worker daemons are offline | after restart, Worker uses a new execution and Master closes or rejects the new review |
| daemon stopped before blocked decision | `TaskBlocked` is seeded through TaskRuntime owner API while Master daemon is offline | after restart, Master writes a persisted `blocked_decision` or reassignment |
| Worker stopped while task is running | lease-backed `Running` is seeded through TaskRuntime owner API, then the Worker service is stopped long enough for lease expiry | after Worker/Master recovery, TaskHistory contains `TaskInterrupted`, a new execution, and terminal review/close or explicit blocked truth |
| Master exits after durable EventInbox admission but before provider decision | `pending_attention` contains the event and cursor has advanced only to durable admission; `retry_event_id` is absent | after restart, the same event is processed at attempt 0 and must close through Task Center mutation, not be skipped by the cursor |
| Master provider/system failure during lifecycle decision | `pending_attention` still contains the event, `retry_event_id` names it, and `retry_attempt` is incremented | after restart, the next lifecycle turn uses attempt N, keeps task truth unchanged until the decision mutation succeeds, and then clears retry state |
| Master foreground process exits with active user work | `master_work` checkpoint points at a dead owner process and matching reason active-turn truth | bootstrap interrupts/closes the active turn or clears invalid checkpoint-only truth before new UI/ADP work is accepted |
| provider fixture verifier disables background lifecycle | `FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER=1` is present only in the temporary verifier daemon env | WebUI/ADP live submit remains available, but background Master lifecycle runner is not started, so historical Task Center events cannot consume the temporary provider fixture; restore removes the env and restarts normal lifecycle service |

Lifecycle closure must not rely on a user sending another chat message. The
Master loop is event/board driven, and every retry/review decision must write
Task Center truth before another execution starts.

## White-Box Coverage

- runner lifecycle tests live in `crates/freehand-runtime/src/worker_runner/tests.rs`
- Master lifecycle tests live in `crates/freehand-runtime/src/master_runner/tests.rs`
- lease renewal ownership lives in `crates/freehand-runtime/src/worker_runner/heartbeat.rs`
- daemon host fixture isolation is covered by `cargo test -p freehand-daemon daemon_master_lifecycle_disable_env_is_explicit_test_only -- --nocapture`

### Positive

- Worker tool definitions include governed read/write/search tools and local planning tools.
- Worker tool definitions exclude `task` and unrestricted shell tools.
- Master guidance names the complete ordered configured Worker id set and forbids assigning
  production tasks to historical AgentBoard entries.
- Master task execution accepts `task(op="assign")` only when `agent_id`
  belongs to the configured Worker set; the accepted assignment writes
  one `TaskAssigned` event for that Worker.
- interrupted Master decision receives AgentBoard resource truth and explicitly
  treats Agent as a reusable pool resource independent of Session ownership
- interrupted prose without assignment fails with
  `MissingInterruptedDecision`; a positive takeover test reassigns the same
  task from `worker-gamma` to `worker-alpha`, preserves parent session id, and
  writes no duplicate Task:
  `production_master_runner_requires_interrupted_assignment_decision`,
  `production_master_runner_can_take_over_interrupted_task_to_another_worker`
- a multi-Worker positive test accepts the second configured Worker while a
  historical/non-configured Worker is rejected first; the failed attempt writes
  no task mutation.
- Master task creation accepts only `dispatch.mode="none"` or
  `dispatch.mode="agent"` targeting one configured Worker. Omitted,
  `auto`, and `self` dispatch cannot select from persisted historical agents.
- Master guidance forbids embedding `task(...)` lifecycle calls in Worker task
  content; the Worker runner owns claim, heartbeat, review submission, and
  blocked execution facts.
- Master guidance tells the model to schedule a timer instead of dead-waiting
  when the next useful wait exceeds 3 minutes, then continue other ready
  Master-side work.
- Master completion guidance tells the model that Worker dispatch, heartbeat,
  review pending, or timer scheduling are lifecycle progress, not final
  user-task completion; when no immediate Master work remains but Task
  Center/timer truth will continue, the turn must use `claim="waiting"`.
- Runtime rejects a Master user-session `claim="complete"` while any child task
  with the same `parent_session_id` remains open; the provider gets repair
  feedback and may continue or return `claim="waiting"`, but the parent session
  must not project `TerminalStatus::Success`.
- Runtime rejects a Master user-session `claim="waiting"` when all same-session
  child tasks are terminal and no active/running source timer exists; a state
  that needs another user choice must close as blocked/user-needed, not as
  lifecycle `ToolPending`.
- A child `task_closed` event whose parent has all same-logical-parent-turn
  children closed triggers exactly one parent-session evaluation turn in the
  original persisted parent session. Child tasks created by
  `runtime-turn-N`, `runtime-turn-N-r2`, and later rounds of the same logical
  Master request are one workset; parent evaluation must not start after the
  first exact-round child closes while same-logical-turn siblings remain open.
  The evaluation must compare the original user objective, each decomposed
  child task's goal/deliverables/acceptance, and accepted Worker review truth.
  It may create and assign correction, improvement, or newly discovered child
  tasks; it may claim final completion only when the overall user objective is
  verified complete.
- A child `task_closed` event whose parent still has any open child task is a
  no-op for parent evaluation.
- A decided `TaskBlocked` child whose same-logical parent workset has no
  active/reviewable siblings triggers exactly one parent-session blocked
  follow-up turn in the original persisted parent session. The follow-up must
  include original user objective truth, Worker blocker evidence, and the
  Master `blocked_decision` note, and must not claim final success.
- Replayed `task_closed` events or daemon restart after evaluation do not
  repeat the same parent decision or duplicate next-round tasks for the same
  parent/closed-child set, including evaluation turns that ended waiting after
  creating more work.
- Master "timer scheduled" claims must be backed by a successful `timer`
  tool result and timer ledger truth; verbal completion text alone is not proof
  of a scheduled wakeup.
- Consecutive timer schedules in the same source turn receive distinct
  framework-generated timer ids and remain as separate persisted schedules;
  second-level clock collisions must not overwrite earlier timer truth.
- Master guidance requires timer prompts to say what current truth to inspect,
  what waited condition to revisit, and what decision to make.
- Assigned task is claimed once with one execution id.
- Claim writes `TaskResumed` and `TaskHeartbeat`.
- Successful model completion writes `TaskReviewSubmitted` with matching task/execution/agent ids.
- Worker session id and turn id are deterministic from task/execution identity.
- Worker prompt includes both requested `target_cwd` and canonical locked workspace, and requires path preflight for absolute paths, `~`, and symlinks.
- Symlinked `~/...` target cwd can execute: the runner expands `~`, follows the symlink to the canonical workspace, preserves the requested path in task truth, and asks the Worker to report both paths.
- no-task tick returns `Idle` and leaves task history unchanged.
- due one-shot timer with persisted source-session truth injects its persisted
  wakeup prompt as a new turn in that original session; it never reopens or
  resumes the old turn. The prior turn remains terminal, the injected turn gets
  the next runtime-turn ordinal, timer truth completes, and Task Center truth
  is unchanged. A timer without a source session remains an internal wakeup.
- chained timers whose stored source points at an older `master-timer-*`
  execution resolve through timer truth to the original user session instead
  of allowing source-session drift.
- due recurring timer fires once, increments `fired_count`, and reschedules
  while below `max_runs`; daily, weekly, and cron recurrence uses local timezone
  semantics.
- due timer failure cannot starve pending Task Center lifecycle events; a
  review-ready task is still approved/closed before the failed timer wakeup is
  retried.
- interrupted tasks are not requeued by a Worker. A Worker tick stays idle and
  writes no new `TaskAssigned` event until Master explicitly reassigns the
  same task to the selected configured Worker; only then may a new execution
  begin.
- rejected submissions are requeued to the same Worker and the next prompt
  contains the persisted rejection requirements.
- Master review processing converts `ReviewSubmitted` into either
  `Rejected` or `Approved -> Closed`.
- Master lifecycle cursor advances only after the matching task decision is
  accepted.
- Master attention admission preserves EventInbox source order even when the
  next processed item is selected by higher severity/task priority.
- Master attention dequeue gives large weight to critical major-change
  attention, `TaskBlocked` showstoppers, and bounded task priority.
- Master attention dequeue uses admission-sequence aging so an old
  low-priority item eventually beats fresh high-weight arrivals without using
  time or sleeps.
- Retryable Master lifecycle failure preserves the same pending attention item,
  admitted sequence, and cursor; stale no-op events are removed and selection
  continues in the same runner tick.
- A Master restart after durable EventInbox admission but before provider
  decision reuses the persisted `pending_attention` item at attempt 0 and
  closes only after the target Task Center mutation:
  `production_master_runner_recovers_after_crash_with_admitted_attention_before_decision`.
- A Master restart after provider/system executor failure preserves
  `retry_event_id` and `retry_attempt`, uses the next event-scoped lifecycle
  turn id, leaves task truth unchanged until mutation, then clears retry state:
  `production_master_runner_restarts_after_executor_failure_and_closes_same_event`.
- A Master restart after a Worker exits mid-`Running` boots TaskRuntime,
  observes the expired lease as `TaskInterrupted`, and explicitly reassigns
  the same task before another Worker execution:
  `production_master_runner_reassigns_expired_running_task_after_restart`.
- A stale persisted EventInbox cursor is repaired by clearing the cursor,
  rewriting loop state, and replaying current Task Center ledger truth; a
  missing-task EventInbox row or pending attention is skipped explicitly and
  cannot block later actionable events in the same runner tick.
- Parent overall-goal evaluation reads the original persisted first user turn
  from reason `TurnStarted` ledger truth, not the UI coalesced transcript
  projection or latest repaired snapshot; repair rounds and control text cannot
  replace the user objective.
- Parent workset reconciliation reads waiting/idempotency/next-turn state from
  authoritative closed/active turn snapshots only. It must not use selected
  transcript restore or parse historical reason ledgers while polling in the
  background Master loop.
- If a historical all-children-closed parent set has no original persisted user
  objective truth, the runner returns `ParentEvaluationSkipped`, records that
  child-set key as skipped, does not call the Master executor, and does not
  finalize the parent session.
- Master user-session completion schema rejects `claim="complete"` while any
  child task in the same parent session remains actionable or unresolved
  (`Created`, `WaitingAgent`, `Assigned`, `Running`, `Interrupted`, `Paused`,
  `Blocked`, `ReviewSubmitted`, `Approved`, or `Rejected`), preserving the
  no-premature-completion boundary covered by
  `cargo test -p freehand-runtime live_master_rejects_complete_while_parent_child_task_open -- --nocapture`.
- Master user-session completion schema allows `claim="complete"` when the
  same parent session only has terminal historical children such as
  `Cancelled`, `Failed`, or `Closed`; cancelled wrong-path child tasks must not
  keep the parent UI in stale waiting state, covered by
  `cargo test -p freehand-runtime live_master_allows_complete_with_terminal_cancelled_child_tasks -- --nocapture`.
- review-ready decision boundary closes only after the target task reaches
  `Rejected` or `Closed`; `Approved` alone remains incomplete and retryable.
- blocked decision boundary closes after the target task records a new
  Master-owned task event, including `task(op="append")` with a concise
  `blocked_decision` note or reassignment.
- interrupted decision boundary closes after the target task leaves
  `Interrupted`.
- reaching a lifecycle decision boundary closes the reason turn in the same
  round as the successful task tool result; no extra provider request is made
  merely to emit completion prose.
- each event uses the same deterministic task-scoped lifecycle session id but a
  distinct event-scoped turn and trace id, so session names stay bounded while
  event audit remains separable.
- retrying the same event increments a persisted attempt id and uses a fresh
  deterministic lifecycle turn and trace id, so retry attempts remain auditable
  without creating a new user-visible session name.
- restart after pre-decision crash with durable `pending_attention` and no
  retry state:
  `production_master_runner_recovers_after_crash_with_admitted_attention_before_decision`
- restart after provider/system executor failure with persisted retry state:
  `production_master_runner_restarts_after_executor_failure_and_closes_same_event`
- restart after Worker `Running` lease expiry and Master reassignment of the
  same task:
  `production_master_runner_reassigns_expired_running_task_after_restart`
- Master lifecycle model responses containing nullable unused status fields
  remain valid; a non-null status type mismatch is polished in another model
  round instead of killing the lifecycle runner.
- provider/executor failure during one Master lifecycle decision leaves the
  event cursor unchanged and is retried by the long-running runner.
- provider retry/failover during a background Master lifecycle or parent
  evaluation turn is visible through shared `UiProtocolState` and through
  selected `QuerySessionTurns` recovery from ErrorCenter metadata, so UI/Android
  can show which session/turn is retrying instead of only showing a stale
  `waiting lifecycle` or `submitted` row.
- provider/executor failure during a timer wakeup records timer failure truth,
  releases the timer to active retryable state, and surfaces a retryable Master
  execution error instead of leaving the schedule stuck in `running`.
- provider/executor failure during a timer wakeup must not prevent already
  pending review-ready, blocked, or interrupted Task Center events from being
  processed first.
- missing/incomplete Master decisions leave the event cursor unchanged and are
  retried by the long-running runner with bounded exponential backoff.
- approved review truth remains retryable until the Master closes it.
- blocked truth remains retryable until the Master either reassigns it or
  persists a `blocked_decision` note through `task(op="append")`.
- repeated blocked-decision provider failures eventually append an explicit
  Master-owned `blocked_decision` and advance to other pending lifecycle events
  without marking the blocked task successful.
- interrupted truth remains retryable until it leaves `Interrupted`.

### Negative

- A source-backed timer must not execute only in a hidden `master-timer-*`
  session, must not reuse/reopen its source turn id, and a chained timer must
  not retain an internal session when persisted timer ancestry identifies a
  user session.

- Master config cannot construct `ProductionWorkerRunner`.
- Worker cannot execute `task` even if a malformed/provider-injected call bypasses schema exposure.
- Master task content must not instruct the Worker to call `task`; doing so is
  a prompt-contract failure because the Worker schema physically excludes it.
- Master assignment to a historical or otherwise non-configured Worker returns
  a paired failed tool result, writes no `TaskAssigned` event, and remains
  available to the model for a corrected assignment in the next round.
- Master task creation with omitted/auto/self dispatch, or explicit dispatch to
  a non-configured Worker, returns a paired failed tool result and writes no
  `TaskCreated` or `TaskAssigned` event.
- missing `target_cwd` records `Blocked`; model execution does not start.
- non-canonicalizable `target_cwd` records `Blocked`; model execution does not start.
- missing `~/...` target cwd records `Blocked` with both original and expanded path; model execution does not start.
- missing target cwd under an existing parent records `Blocked` as workspace-preflight failure that explicitly says this is not a repository permission denial and likely means `target_cwd` was misused for a not-yet-created output directory.
- missing target cwd under a symlink parent records `Blocked` before model
  execution and includes target_cwd path diagnostic evidence for
  nearest_existing, nearest_existing_canonical, symlink_ancestors, and
  missing_suffix.
- provider/network system failure after internal provider retry exhaustion records
  `Interrupted`, reuses the same task on the next Worker tick, and never writes
  `ReviewReady` for the failed execution.
- non-provider task execution failure records `Blocked`; it is not silently
  retried by the Worker and never writes `ReviewReady`.
- reporting `Blocked` releases the Worker resource to `Available`; task blocked
  truth must not globally pause the configured Worker
- a deliberately paused task keeps the Worker resource `Paused`; Worker startup
  and Task Center boot must not erase an active pause
- startup recovery repairs legacy `Paused` agent snapshots only when no
  authoritative assigned task is actually `Paused`
- claim or heartbeat failure stops before provider execution.
- heartbeat from an older Worker-owned `TaskRuntime` after an external cancel
  is rejected by Task Center truth and must not append `TaskHeartbeat`, recreate
  a lease, or overwrite `TaskCancelled`.
- Worker result reporting after an external cancel is rejected by Task Center
  truth and must not write `TaskReviewSubmitted` or `TaskBlocked` over terminal
  task truth.
- failure to persist the blocked fact remains an explicit runner error.
- blocked tasks are not silently retried by the Worker.
- an interrupted/rejected retry never reuses the previous execution id.
- a Worker runner cannot claim a task assigned to another configured Worker.
- Master review failure does not approve or close the task.
- A Master turn that only creates/assigns a Worker task must not project
  `TerminalStatus::Success` or a completed final answer for the user objective;
  it must remain lifecycle-observable as pending/running until worker review,
  Master acceptance, and final synthesis close the user-facing lifecycle.
- A Master user-session completion with open child tasks is rejected before
  terminal success, even if the model emits a syntactically valid
  `claim="complete"` schema.
- A Master user-session `claim="waiting"` with only terminal child tasks and no
  source timer is rejected before `ToolPending` can persist; covered by
  `cargo test -p freehand-runtime live_master_rejects_waiting_when_child_tasks_are_terminal_and_no_owner_will_wake -- --nocapture --test-threads=1`.
- A closed child task with an open sibling must not trigger parent evaluation.
- A Worker `TaskBlocked` without a Master-owned `blocked_decision` must not
  project a user-visible parent blocked follow-up.
- A decided `TaskBlocked` child must not project a parent blocked follow-up
  while a same-logical-turn sibling child remains active or reviewable.
- A closed child task whose siblings were created in later rounds of the same
  logical `runtime-turn-N` must not trigger parent evaluation until all
  same-logical-turn children are terminal closed; covered by
  `cargo test -p freehand-runtime production_master_runner_groups_parent_workset_by_logical_turn_rounds -- --nocapture --test-threads=1`.
- A next-round parent evaluation must include prior accepted child review truth
  from the same external objective scope, not only the final integration task;
  covered by
  `cargo test -p freehand-runtime production_master_runner_closed_loop_requires_next_round_before_final_evaluation -- --nocapture --test-threads=1`.
- A later external user objective must not inherit closed child review truth
  from older same-session objectives; covered by
  `cargo test -p freehand-runtime production_master_runner_parent_context_excludes_prior_user_turn_children -- --nocapture --test-threads=1`.
- A closed child workset in a later parent turn must still trigger parent
  evaluation when older same-session child tasks from earlier turns are blocked
  and the durable EventInbox cursor has already advanced past the close event.
- A replayed closed-child event for an already evaluated parent/child set must
  not trigger another parent follow-up turn.
- A parent evaluation must not treat accepted child results as sufficient merely
  because they can be summarized. If the original goal remains incomplete, the
  evaluation must create/assign next-round task truth or return an explicit
  blocked decision; a user-visible final success is reserved for verified goal
  completion.
- review prose without task mutation leaves the event retryable and returns an
  explicit `MissingReviewDecision`.
- approve without close leaves the review event retryable.
- blocked prose without a persisted Task Center decision leaves the event
  retryable.
- blocked-decision provider failure cannot starve later lifecycle events
  forever; after the retry cap, the runner records the provider-unavailable
  blocked decision as `TaskProgressed`.
- interrupted prose without reassignment leaves the event retryable.
- Worker-owned interrupted auto-requeue is a red path: it must not race the
  Master lifecycle decision or silently preserve the previous assignee.
- a successful mutation of another task does not satisfy the current event's
  decision boundary.
- a `continue` response without a target-task decision cannot run forever;
  round-budget exhaustion closes the reason turn as blocked and leaves the
  lifecycle event cursor unchanged.
- a background lifecycle ErrorCenter row for a terminal turn must not reactivate
  the session as `waiting_model`; terminal reason truth remains authoritative.
- Task Center and lifecycle-state read/write failures stop the Master runner
  explicitly instead of being treated as retryable model/provider failures.
- stale historical EventInbox cursor or missing-task attention is not a current
  owner-truth failure: it is repaired or dropped with explicit log evidence and
  must not terminate the daemon host or hide later current-session events.
- missing original parent objective truth for a historical closed-child set must
  not become a success, aggregation, or final answer; it is a skipped
  non-final evaluation and must be idempotent on the next tick.
- timer state read/write failures stop the Master runner explicitly because
  independent timer truth is unavailable.
- historical lifecycle turns from another task are absent from the current
  provider request; same-task lifecycle turns remain internal framework truth
  and must not appear in user-facing session lists.
- existing Master shell denial and runtime-home boundary tests remain green.

## Module Black-Box Coverage

- deterministic fake executor drives `run_once` through:
  - idle
  - successful completion
- provider/network system failure and non-provider task execution failure,
  including Anthropic and OpenAI-compatible request/stream/status provider
  errors mapping to retryable `TaskInterrupted`
- deterministic Master executor drives:
  - attention admission/dequeue contract:
    `master_attention_admission_preserves_event_inbox_order`,
    `master_attention_dequeue_gives_blocked_and_task_priority_large_weight`,
    `master_attention_dequeue_ages_old_low_priority_item_without_starvation`,
    and `master_attention_retry_keeps_same_pending_item`
  - stale EventInbox cursor repair and stale pending-attention drop:
    `production_master_runner_repairs_stale_event_inbox_cursor_from_loop_state`
    and `production_master_runner_drops_stale_pending_attention_and_continues`
  - review approve and close
  - review rejection with persisted requirements
  - blocked decision persisted through `task(op="append")`
  - decided Worker block projected to the original parent session:
    `production_master_runner_projects_decided_worker_block_to_parent_session`
  - undecided Worker block and active sibling block guards:
    `production_master_runner_does_not_project_undecided_worker_block_to_parent_session`
    and `production_master_runner_does_not_block_parent_while_sibling_child_is_active`
  - missing review decision and same-event retry
  - all-children-closed parent evaluation in the original parent session
  - stale waiting parent workset recovery after cursor advancement while an
    older same-session turn remains blocked:
    `production_master_runner_recovers_closed_parent_workset_after_cursor_advanced`
  - parent evaluation request contains original user objective history plus
    each current-workset child task's goal/deliverables/acceptance and accepted
    review result, and final next-round evaluation carries same-objective prior
    accepted child truth:
    `production_master_runner_closed_loop_requires_next_round_before_final_evaluation`
  - parent evaluation context excludes prior user-turn closed child truth:
    `production_master_runner_parent_context_excludes_prior_user_turn_children`
  - missing original parent objective truth is non-final and idempotently skipped:
    `production_master_runner_rejects_parent_evaluation_without_persisted_goal_truth`
  - parent evaluation creates a same-session improvement task when the combined
    child results do not satisfy the overall objective
  - open-sibling closed event no-op
  - parent evaluation idempotency across repeated runner ticks/restart,
    including a persisted waiting evaluation that already created next work
  - background reconciliation with a poisoned reason ledger and valid
    authoritative parent/evaluation snapshots:
    `production_master_runner_parent_reconciliation_uses_authoritative_snapshots_not_ui_ledger`
- active-work handshake tests drive:
  - `production_master_foreground_acknowledges_suspend_at_safe_point`
    through `SuspendRequested -> SuspendedByAttention` at the exact foreground
    safe point while an isolated control turn remains unable to mutate it
  - `production_master_foreground_never_suspends_mid_effect` through provider,
    tool-effect, and terminal-persistence in-flight phases that must remain
    `SuspendRequested`
  - `production_master_attention_uses_isolated_control_turn` proves the
    lifecycle decision request is event/attempt scoped, has a session, turn,
    and trace distinct from the suspended foreground work, and runs while the
    exact foreground checkpoint remains `SuspendedByAttention`
  - `production_master_attention_raw_transcript_never_enters_user_session`
    proves even an executor summary containing a raw-control sentinel cannot
    enter the foreground reason session or the typed `AttentionResolution`
  - `production_master_resume_consumes_resolution_once` and
    `production_master_resume_rejects_mismatched_return_identity`
- live bridge attention continuation tests drive:
  - `live_master_attention_invalidates_stale_tool_without_side_effect`, proving
    the stale tool is paired with one failed tool result, not executed, and the
    next request contains typed AttentionResolution plus refreshed task truth
  - `live_master_attention_rejects_stale_terminal_persistence`, proving a
    terminal candidate produced before attention never enters durable
    closed-turn truth and the next request re-reasons with typed resolution
- runtime/query projection coverage proves `QuerySessionTurns` can see a
  background-persisted parent evaluation turn by restoring
  `ReasonPersistence` owner truth into UI projection, and hides internal
  timer/parent-evaluation prompts from both raw request text and original-task
  context-derived display candidates while preserving terminal truth.
- effective provider-history coverage proves rebuilt
  `SessionHistory.base_context_segments` hides internal parent-evaluation
  prompts while preserving the terminal summary:
  `cargo test -p freehand-runtime effective_context_hides_internal_parent_evaluation_prompt -- --nocapture --test-threads=1`.
- due timer wakeup without task truth
- recurring timer reschedule up to `max_runs`
- timer wakeup failure release back to active retryable state
- task-event priority over due timer failure
- live provider fixture drives:
  - an invalid historical-Worker assignment followed by a corrected configured
    Worker assignment; the second provider request must contain the paired
    failed tool result, and Task Center must contain only the corrected
    `TaskAssigned`
  - an omitted-dispatch create followed by corrected `dispatch.mode="none"`
    create and configured Worker assignment; the same task id must be reusable,
    proving the rejected create wrote no Task Center truth
  - a target-task mutation followed by `continue` semantics and proves the
    lifecycle turn closes after the mutation without issuing another request
  - an unrelated task mutation and proves it does not satisfy the boundary
  - repeated `continue` without a decision and proves finite blocked closeout
  - two different lifecycle events and proves their lifecycle session name is
    task-scoped while turn and trace ids remain event-scoped
- real live bridge test proves Worker schema excludes `task` and unrestricted shell tools; Worker read/search/list/mutation path tools are all locked to the canonical task cwd after symlink/canonical resolution, and external path attempts return model-visible boundary guidance instead of inviting external probing.
- runner tests prove `~/...` plus symlink target cwd resolves to a canonical Worker workspace and missing `~/...` blocks before model execution.
- daemon test proves:
  - Master mode creates UI host
  - Slave mode creates Worker runner
  - Slave mode does not bind WebUI/ADP transport
- config/runtime boundary tests prove the Master consumes an ordered
  three-Worker set while every selected Worker consumes exactly one Master peer
- restart test opens a new runner against the same runtime home and verifies same task/execution/agent/history ids.
- strict restart recovery proofs stop/unload the exact Master/agent-specific
  Worker services as needed, seed
  review/rejected/blocked/running truth through
  `freehand-cliS task-restart-seed-*`, restart daemonS/workerS as needed, and
  verify TaskHistory reaches the expected next lifecycle event.

## Project Black-Box Coverage

- start S-profile Master daemon on fixed port 4042
- start three separate configured Slave daemon processes:
  `worker-alpha`, `worker-beta`, and `worker-gamma`
- install agent-specific S-profile Worker LaunchAgents and prove unique labels,
  env files, log files, `RunAtLoad`, `KeepAlive`, no `--bind`, shared pair
  token, and three stable distinct PIDs
- submit a real Master request that creates and assigns work outside `~/.freehand`
- verify three distinct Worker TaskHistory streams contain:
  - `TaskAssigned`
  - `TaskResumed`
  - `TaskHeartbeat`
  - `TaskReviewSubmitted`, `TaskInterrupted`, or `TaskBlocked`
- prove the alpha/beta/gamma tasks use three distinct `agent_id` values and no
  Worker claims another Worker's assigned task
- verify the same task/execution/agent ids after Worker restart
- three-worker online proof must require Master quality evaluation against the
  original goal, decomposed goal/deliverables/acceptance, and accepted review
  truth; the first evaluation must create next-round work and only the later
  evaluation may complete the original parent session
- controlled online proof is landed in
  `scripts/verify-master-three-worker-e2e-online.sh`: it starts three explicit
  Worker processes in an isolated runtime home, writes JSON evidence, forces
  gamma provider retry exhaustion into `TaskInterrupted`, requires Master to
  replace the same gamma task assignment with alpha, forces beta reject/rework
  and an integration next round, rejects premature parent success, and checks
  final-evaluation restart idempotency
- the controlled three-Worker verifier must treat initial dispatch as a
  foreground waiting turn, not as a busy-polling aggregation turn. After the
  three initial child tasks are created and assigned, the fixture returns
  `claim="waiting"`; the script then observes Worker execution, lifecycle
  review, parent evaluation, and final completion through ADP
  TaskBoard/TaskHistory/SessionTurns truth. The script fails if the foreground
  SubmitUserInput receipt is an error or lacks `reason_live_turn_completed`.
  Current launchd proof session
  `online-launchd-three-worker-evaluation-1784190586-60111` locked this path
  with beta reject/rework, gamma same-task takeover, next-round integration,
  final `runtime-turn-3` Success, and `final_evaluation_count=1`.
- launchd-managed online proof is landed in
  `scripts/verify-launchd-three-worker-services-online.sh`: it reuses the same
  three-Worker parent-evaluation verifier with Worker start mode set to
  launchd, then kills the now-idle gamma resource and requires KeepAlive to
  restart the same reusable Agent with a new PID/process instance and
  AgentBoard `restart_count=1`; task identity and execution ownership remain
  proven separately by the gamma-to-alpha TaskHistory
- only claim production closure when the Worker produced a real deliverable or an explicit real-provider blocked result

## Runtime Evidence

- `~/.freehand/state/tasks`
- `~/.freehand/state/agents`
- `~/.freehand/state/turns`
- `~/.freehand/state/timers`
- `~/.freehand/ledgers/tasks`
- `~/.freehand/ledgers/timers`
- `~/.freehand/ledgers/reason`
- `~/.freehand/logs`

## Known Non-Goals

- multi-task context switching inside one Worker process
- more than one concurrently executing task per Worker process
- recursive Worker-created subagents
- task approval/close by the Worker
- UI projection changes
- remote node transport; first production slice uses the shared local Task Center runtime home
- real-provider recovery remains outside the controlled fixture proof

## Standalone Lifecycle Gaps

- Master idle attention admission/dequeue is now code-bound for source-ordered
  admission, weighted severity/task-priority dequeue, deterministic aging,
  retry preservation, and stale no-op removal.
- Busy-Master active-work identity, priority comparison, safe-point state, typed
  attention resolution, isolated control-turn identity, exact return identity,
  and raw transcript/provider payload rejection are now unit-bound by
  `master_work.resolve_attention`.
- Worker pause/resume is focused-test bound through the runner pause monitor,
  live cancel token, stale-output suppression, and resumed controlled task
  selection. Do not claim live Master busy-preemption closure until the real
  daemon shows foreground Master work pausing at a declared safe point,
  isolated attention decision completing, and the original foreground work
  continuing with typed resolution only.
- Existing three-Worker E2E evidence does not close busy-Master preemption.

## Definition Of Done

- positive and negative tests above are green
- function map and mainline manifest bind real symbols
- `cargo test -p freehand-tools worker_implemented -- --nocapture`
- `cargo test -p freehand-runtime production_worker_runner -- --nocapture`
- `cargo test -p freehand-runtime master_assignment_gate -- --nocapture`
- `cargo test -p freehand-daemon worker_mode -- --nocapture`
- `cargo test -p xtask ci_cd -- --nocapture`
- `bash -n scripts/install-launchd.sh`
- `scripts/verify-launchd-worker-naming.sh`
- `bash -n scripts/verify-master-three-worker-e2e-online.sh`
- `scripts/verify-master-three-worker-e2e-online.sh`
- `bash -n scripts/verify-launchd-three-worker-services-online.sh`
- `scripts/verify-launchd-three-worker-services-online.sh`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- S-profile online TaskHistory proves three distinct Worker agent/process ids,
  claim + heartbeat + terminal execution fact, Master review/rework, next-round
  task creation, and final parent-goal completion
