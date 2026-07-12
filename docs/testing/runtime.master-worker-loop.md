# Test Design: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner: `crates/freehand-runtime`
- host: `apps/freehand-daemon`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `timer.fire_master_wakeup`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `timer.fire_master_wakeup` | bound | `cargo test -p freehand-runtime timer -- --nocapture` covers durable timer due-claim, one-shot, recurring, local-time cron/daily/weekly, wakeup prompt, failure release, and Master runner tests | `cargo test -p freehand-runtime production_master -- --nocapture` covers production Master runner smokes where due timers create internal Master wakeup turns without task-state mutation | `scripts/verify-timer-tool-online.sh` covers S-profile timer online proof and restart-due proof showing persisted timer wakeup fires after due time and completes timer truth |


## Lifecycle Under Test

1. Master Task Center has a registered Worker and an Assigned task.
2. Slave daemon selects the configured Worker and starts the production runner.
3. Worker LaunchAgent starts at login, stays alive, and restarts the same
   configured Worker process after an unexpected exit.
4. Runner opens the paired Master's Task Center namespace.
5. Runner claims one task for its Worker identity and persists lease heartbeat.
6. Runner expands a leading `~`, canonicalizes `task.target_cwd` through symlinks, and locks the Worker to that canonical workspace.
7. Runner executes one provider/reason turn under Worker identity and Worker tool policy.
8. Runner writes `review_ready` on successful completion, `interrupted` on
   provider/network system failure after provider-owned retry exhaustion, or
   `blocked` on task-content, path-preflight, model-terminal, or other
   non-provider execution failure.
9. Runner returns to polling without inventing task truth while idle.
10. Restart reads the same task/execution/agent/history truth.
11. Each Master lifecycle event attempt runs in a task-scoped internal
    lifecycle session with event-and-attempt-isolated turn and trace ids; the
    session name is reused per task to avoid user-facing session explosion.
12. Before Task Center EventInbox processing, the Master lifecycle runner claims
    due independent timer schedules from durable timer truth and starts an
    internal Master wakeup turn with the persisted prompt.
13. A successful target-task mutation is evaluated against an explicit
    event-specific decision boundary. Once reached, the framework closes the
    lifecycle turn immediately instead of asking the model to wait for future
    Worker events inside the same turn.
14. A lifecycle decision has a finite round budget. Exhaustion becomes an
    explicit blocked lifecycle turn and leaves the EventInbox cursor retryable;
    it must never remain as an unbounded active reason turn.
15. Retryable Master decision failures keep the same durable event cursor,
    back off, and retry without terminating the daemon. Task Center/state
    persistence failures remain fatal because owner truth is unavailable.

## Current-Configuration Closure Matrix

The production acceptance target is the configured one-Master/one-Worker
topology in `~/.freehand/config.toml`. More workers and multiple independent
BigTasks remain out of scope until every row below is closed.

| branch | required owner truth | required next action |
| --- | --- | --- |
| success | `TaskReviewSubmitted` with deliverables and evidence | Master approves and closes, or rejects with requirements |
| review rejected | `TaskReviewRejected` remains durable | same Worker receives a new execution with rejection requirements |
| provider/network system failure after internal retries | `TaskInterrupted` with paired reason/evidence | same task is requeued to the configured Worker with a new execution |
| task-content, path-preflight, or model-terminal failure | `TaskBlocked` with paired reason/evidence | Master explicitly retries/reassigns or leaves the task blocked |
| Worker process crash | boot writes `TaskInterrupted` for missing/expired lease | task is requeued to the configured Worker with a new execution |
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
- Master guidance names the configured paired Worker and forbids assigning
  production tasks to historical AgentBoard entries.
- Master task execution accepts `task(op="assign")` only when `agent_id`
  exactly matches the configured paired Worker; the accepted assignment writes
  one `TaskAssigned` event for that Worker.
- Master task creation accepts only `dispatch.mode="none"` or
  `dispatch.mode="agent"` targeting the configured paired Worker. Omitted,
  `auto`, and `self` dispatch cannot select from persisted historical agents.
- Master guidance forbids embedding `task(...)` lifecycle calls in Worker task
  content; the Worker runner owns claim, heartbeat, review submission, and
  blocked execution facts.
- Master guidance tells the model to schedule a timer instead of dead-waiting
  when the next useful wait exceeds 3 minutes, then continue other ready
  Master-side work.
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
- due one-shot timer runs an internal Master wakeup with the persisted prompt,
  completes timer truth, and leaves Task Center truth empty.
- due recurring timer fires once, increments `fired_count`, and reschedules
  while below `max_runs`; daily, weekly, and cron recurrence uses local timezone
  semantics.
- interrupted tasks assigned to the configured Worker are requeued once and
  claimed with a new execution id.
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
- missing/incomplete Master decisions leave the event cursor unchanged and are
  retried by the long-running runner with bounded exponential backoff.
- approved review truth remains retryable until the Master closes it.
- blocked truth remains retryable until the Master either reassigns it or
  persists a `blocked_decision` note through `task(op="append")`.
- interrupted truth remains retryable until it leaves `Interrupted`.

### Negative

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
- Master review failure does not approve or close the task.
- review prose without task mutation leaves the event retryable and returns an
  explicit `MissingReviewDecision`.
- approve without close leaves the review event retryable.
- blocked prose without a persisted Task Center decision leaves the event
  retryable.
- interrupted prose without reassignment leaves the event retryable.
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
- provider/network system failure and non-provider task execution failure
- deterministic Master executor drives:
  - review approve and close
  - review rejection with persisted requirements
  - blocked decision persisted through `task(op="append")`
  - missing review decision and same-event retry
  - due timer wakeup without task truth
  - recurring timer reschedule up to `max_runs`
  - timer wakeup failure release back to active retryable state
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
- restart test opens a new runner against the same runtime home and verifies same task/execution/agent/history ids.
- strict restart recovery proofs stop/unload daemonS/workerS as needed, seed
  review/rejected/blocked/running truth through
  `freehand-cliS task-restart-seed-*`, restart daemonS/workerS as needed, and
  verify TaskHistory reaches the expected next lifecycle event.

## Project Black-Box Coverage

- start S-profile Master daemon on fixed port 4042
- start a separate configured Slave daemon process
- install the S-profile Worker LaunchAgent and prove `RunAtLoad`, `KeepAlive`,
  no `--bind`, shared pair token, and stable running PID
- submit a real Master request that creates and assigns work outside `~/.freehand`
- verify Worker TaskHistory contains:
  - `TaskAssigned`
  - `TaskResumed`
  - `TaskHeartbeat`
- `TaskReviewSubmitted`, `TaskInterrupted`, or `TaskBlocked`
- verify the same task/execution/agent ids after Worker restart
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
- more than one concurrently executing task
- recursive Worker-created subagents
- task approval/close by the Worker
- UI projection changes
- remote node transport; first production slice uses the shared local Task Center runtime home

## Definition Of Done

- positive and negative tests above are green
- function map and mainline manifest bind real symbols
- `cargo test -p freehand-tools worker_implemented -- --nocapture`
- `cargo test -p freehand-runtime production_worker_runner -- --nocapture`
- `cargo test -p freehand-daemon worker_mode -- --nocapture`
- `cargo test -p xtask ci_cd -- --nocapture`
- `bash -n scripts/install-launchd.sh`
- `cargo run -p xtask -- mainlines generate`
- `cargo run -p xtask -- mainlines check`
- `cargo run -p xtask -- gates check`
- S-profile online TaskHistory proves claim + heartbeat + terminal execution fact
