# Test Design: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner: `crates/freehand-runtime`
- host: `apps/freehand-daemon`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `timer.fire_master_wakeup`
  - `agent.heartbeat`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `timer.fire_master_wakeup` | bound | `cargo test -p freehand-runtime timer -- --nocapture` covers durable timer due-claim, one-shot, recurring, local-time cron/daily/weekly, wakeup prompt, failure release, and Master runner tests | `cargo test -p freehand-runtime production_master -- --nocapture` covers production Master runner smokes where due timers create internal Master wakeup turns without task-state mutation | `scripts/verify-timer-tool-online.sh` covers S-profile timer online proof and restart-due proof showing persisted timer wakeup fires after due time and completes timer truth |
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

## Current-Configuration Closure Matrix

The production acceptance target is one configured Master plus at least three
explicit configured Worker identities in `~/.freehand/config.toml`. Every
Worker runs in its own daemon process and may execute at most one claimed task
at a time. Multiple independent BigTasks remain outside this slice.

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

Lifecycle closure must not rely on a user sending another chat message. The
Master loop is event/board driven, and every retry/review decision must write
Task Center truth before another execution starts.

## White-Box Coverage

- runner lifecycle tests live in `crates/freehand-runtime/src/worker_runner/tests.rs`
- Master lifecycle tests live in `crates/freehand-runtime/src/master_runner/tests.rs`
- lease renewal ownership lives in `crates/freehand-runtime/src/worker_runner/heartbeat.rs`

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
- A child `task_closed` event whose parent has all same-session children closed
  triggers exactly one parent-session evaluation turn in the original persisted
  parent session. The evaluation must compare the original user objective,
  each decomposed child task's goal/deliverables/acceptance, and accepted Worker
  review truth. It may create and assign correction, improvement, or newly
  discovered child tasks; it may claim final completion only when the overall
  user objective is verified complete.
- A child `task_closed` event whose parent still has any open child task is a
  no-op for parent evaluation.
- Replayed `task_closed` events or daemon restart after evaluation do not
  repeat the same parent decision or duplicate next-round tasks for the same
  parent/closed-child set, including evaluation turns that ended waiting after
  creating more work.
- Master "timer scheduled" claims must be backed by a successful `timer`
  tool result and timer ledger truth; verbal completion text alone is not proof
  of a scheduled wakeup.
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
- Master lifecycle model responses containing nullable unused status fields
  remain valid; a non-null status type mismatch is polished in another model
  round instead of killing the lifecycle runner.
- provider/executor failure during one Master lifecycle decision leaves the
  event cursor unchanged and is retried by the long-running runner.
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
- A closed child task with an open sibling must not trigger parent evaluation.
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
- Task Center and lifecycle-state read/write failures stop the Master runner
  explicitly instead of being treated as retryable model/provider failures.
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
  - review approve and close
  - review rejection with persisted requirements
  - blocked decision persisted through `task(op="append")`
  - missing review decision and same-event retry
  - all-children-closed parent evaluation in the original parent session
  - parent evaluation request contains original user objective history plus
    each child task's goal/deliverables/acceptance and accepted review result
  - parent evaluation creates a same-session improvement task when the combined
    child results do not satisfy the overall objective
  - open-sibling closed event no-op
  - parent evaluation idempotency across repeated runner ticks/restart,
    including a persisted waiting evaluation that already created next work
- runtime/query projection coverage proves `QuerySessionTurns` can see a
  background-persisted parent evaluation turn by restoring
  `ReasonPersistence` owner truth into UI projection.
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
- real live bridge test proves Worker schema excludes `task` and unrestricted shell tools, read-only tools may inspect readable external paths, and mutation tools keep workspace root as task cwd.
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
