# Function Map: `runtime.master-worker-loop`

- feature_id: `runtime.master-worker-loop`
- owner crate: `crates/freehand-runtime`
- owner modules:
  - `crates/freehand-runtime/src/master_runner.rs`
  - `crates/freehand-runtime/src/master_runner/tests.rs`
  - `crates/freehand-runtime/src/worker_runner.rs`
  - `crates/freehand-runtime/src/worker_runner/heartbeat.rs`
  - `crates/freehand-runtime/src/worker_runner/tests.rs`
  - `crates/freehand-runtime/src/lib.rs`
  - `crates/freehand-runtime/src/path_diagnostics.rs`
  - `crates/freehand-runtime/src/tests.rs`
- host wiring: `apps/freehand-daemon/src/main.rs`
- task truth dependency: `crates/freehand-task`
- config truth dependency: `crates/freehand-config`
- tool policy dependency: `crates/freehand-tools`
- mainline call source: `docs/mainline-calls/runtime.master-worker-loop.json`
- generated wiki: `docs/wiki/runtime.master-worker-loop.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `timer.fire_master_wakeup`
  - `timer.schedule`
  - `timer.cancel`
  - `timer.list`
  - `master_work.resolve_attention`
  - `master_work.admit_resolution_context`
  - `agent.heartbeat`
- owner entry symbols:
  - `ProductionWorkerRunner::from_default_config`
  - `ProductionWorkerRunner::run_once`
  - `ProductionWorkerRunner::run`
  - `WorkerPauseMonitor::start`
  - `worker_pause_requested`
  - `run_worker_live_reason_turn`
  - `master_session_completion_rejection`
  - `take_master_attention_resolution_if_current`
  - `admit_master_attention_resolution_for_next_round`
  - `pair_master_attention_invalidated_tool_calls`
  - `enter_master_terminal_persistence`
  - `recover_stale_lifecycle_waits_on_bootstrap`
  - `session_has_lifecycle_owner_for_turn`
  - `task_can_wake_parent_lifecycle`
  - `owner_turn_matches_target`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `timer`
  - `master_work`
- touched resources:
  - `turn`
  - `request_context`
  - `task`
  - `agent`
  - `ui_projection`
- resource operations:
  - `timer.fire_master_wakeup`
  - `timer.schedule`
  - `timer.cancel`
  - `timer.list`
  - `master_work.resolve_attention`
  - `master_work.admit_resolution_context`
  - `agent.heartbeat`
- forbidden shortcuts:
  - Timer schedules must not be encoded as task lifecycle state.
  - Runtime command workspace mutation must go through checkpoint owner admission.

## Request Mainline

- one daemon process selects one configured agent
- Master mode starts the runtime/UI command path and a background lifecycle
  runner over Task Center EventInbox truth
- the Master lifecycle runner keeps a durable event cursor and invokes the
  Master model for current review-ready, blocked, interrupted, or all-current-children-closed parent-evaluation truth
- a stale durable EventInbox cursor from historical/global Task Center truth is
  repaired by clearing the cursor and replaying the current Task Center ledger;
  stale EventInbox rows or pending attention for missing tasks are logged and
  skipped instead of permanently stopping current lifecycle progress
- interrupted decisions include current AgentBoard resource truth. Agent
  identity is independent of Session; Master may retry the same Worker or
  replace the temporary assignment with another available configured Worker
  while preserving task id and parent session id
- Task Center lifecycle events are admitted into durable
  `pending_attention` in EventInbox source order before the cursor advances.
  Dequeue is separate: severity, bounded task priority, and deterministic
  admission-sequence aging select the next item. Critical major changes,
  blocked showstoppers, and high-priority work carry large weight; aging keeps
  old low-priority work from permanent starvation without wall-clock timing
- foreground Master user work is persisted separately as `master_work`.
  Lower-priority attention stays queued; higher-priority attention may request
  suspension while provider or tool effects are in flight and may enter
  `SuspendedByAttention` only at a declared safe point. The persisted
  resolution contains typed changed-task identity and exact return identity,
  never raw Worker/control transcripts or provider payloads
- while foreground work is `SuspendedByAttention`, the selected attention
  decision still runs as a task-scoped `master-lifecycle-*` control request with
  event/attempt-isolated turn and trace ids distinct from the suspended
  foreground session, turn, and trace. Executor prose or raw control transcript
  text is not copied into the foreground user session or typed
  `AttentionResolution`
- after a high-priority attention resolution is restored, the original
  foreground live turn consumes that typed resolution exactly once, refreshes
  TaskSpaceSnapshot, admits `AttentionResolution` as volatile/no-cache
  developer context, and continues the same logical turn instead of starting a
  synthetic user prompt
- stale provider tool calls produced before the attention resolution are paired
  with failed tool results and are not executed; stale terminal candidates are
  discarded before terminal persistence, so neither stale tool side effects nor
  stale closed-turn truth can survive the changed Task Center state
- Task Center lifecycle attention has priority over due internal timers; due
  timers are claimed only when the durable attention queue is empty, so a
  failed timer wakeup cannot starve pending review/blocked/interrupted
  decisions
- each actionable event attempt uses the task-scoped internal
  `master-lifecycle-<task>` reason session, plus deterministic
  event-and-attempt-isolated turn and trace ids, and an explicit target-task
  decision boundary
- each Master lifecycle, parent-evaluation, and timer wakeup live turn publishes
  the same reason/debug/task-list hooks into the shared `UiProtocolState` as
  foreground user submits, so provider retry, failover, schema repair, tool
  execution, terminal status, and task mutations are immediately observable by
  owning session and turn instead of existing only in stderr or ErrorCenter
  query side channels
- every lifecycle is closed by its owning truth source: TaskRuntime owns
  task/execution success, blocked, interrupted, rejected, approved, and closed
  transitions; AgentLifecycle owns Worker process liveness, restart count, and
  current binding; Master loop state owns attention retry/backoff and cursor
  advance; reason persistence owns parent-session waiting/final turn truth
- if the Master process exits after durable EventInbox admission but before a
  provider decision, the next Master start reuses `pending_attention` and the
  unchanged cursor instead of skipping the event; if provider/system failure
  returned before exit, `retry_event_id` and `retry_attempt` select the next
  event-scoped attempt turn
- if a foreground Master process exits while `master_work` is open, runtime
  bootstrap recovers only from the `master_work` checkpoint plus matching
  reason active-turn truth: it interrupts/closes the stale active turn or
  clears invalid checkpoint-only state before accepting new UI/ADP work
- live runtime bootstrap also reconciles persisted user-session
  `ToolPending` turns before restoring UI projection. It reads TaskBoard through
  `TaskRuntime::boot_read_only`, TimerStore schedules, and non-recoverable live
  `master_work` truth; if the latest effective turn has no active turn and no
  open same-logical-turn task, active/running source timer, or live Master work
  owner, the same turn is durably re-closed as `Blocked`. Owner-backed waits
  remain `ToolPending`.
- Master supervision is a Task Center plus AgentBoard decision loop. A Worker
  process restart or stale heartbeat is never treated as task success; it is
  only resource truth that the Master combines with TaskHistory to reassign the
  same interrupted task, keep a blocked task explicit, or close an accepted
  review
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- each Worker opens its only configured Master's Task Center namespace and uses
  its own distinct configured agent id as execution identity
- Worker construction records one typed process-start event; every idle or
  active poll tick and the active task-heartbeat loop refresh the same process
  instance heartbeat through `agent.lifecycle`
- Worker process exit during `Running` is recovered only by TaskRuntime lease
  truth. The next Worker or Master TaskRuntime boot writes `TaskInterrupted`
  for a missing/expired lease, releases the current Agent binding, and leaves
  the same task idle until Master explicitly reassigns it.
- each worker tick queries the highest-priority Assigned task for that worker
- a selected task is claimed with one execution id and lease heartbeat
- the task target cwd expands a leading `~`, canonicalizes through symlinks, and becomes the worker's locked execution root
- worker live reasoning receives task goal, content, deliverables, acceptance criteria, the requested `target_cwd`, the canonical locked workspace, and path-preflight instructions
- Worker target_cwd preflight failures include the shared target_cwd path
  diagnostic so blocked truth names symlink ancestors, nearest existing
  canonical parent, and missing suffix before the Master asks the user for
  clarification
- while a Worker execution is active, the runner monitors persisted `WorkerControlOp::Pause` truth for the same task/execution and wires it into `LiveReasonCancelToken`; the live bridge may only stop at its existing provider/tool/terminal safe points
- worker provider requests expose governed workspace tools but exclude recursive `task` and unrestricted shell tools
- Master provider guidance binds dispatch to the ordered configured Worker id set,
  rejects historical AgentBoard entries as production dispatch targets, and
  forbids putting `task(...)` lifecycle instructions into Worker task content
- Master provider guidance tells the model that waits exceeding 3 minutes must
be converted into `timer(op="schedule")`; after scheduling, the Master should
continue other ready work rather than dead-waiting in the current turn
- Master completion evidence must not say a timer was scheduled unless the same
  turn contains a successful `timer` tool result; a verbal "scheduled" summary
  without timer ledger truth is not accepted as durable wakeup truth
- Timer schedule, cancel, and list operations remain independent timer owner
  truth. UI command/query wiring may call the timer owner through runtime, but
  neither WebUI nor Task Center becomes timer truth.
- Master provider guidance tells the model to preserve user-supplied paths,
  avoid repeated Master-side probes outside runtime home, require Worker
  symlink/canonical-path evidence, and never invent `/workspace`, `/tmp`, or
  sibling output dirs when the user supplied a repository path
- Master provider guidance distinguishes lifecycle progress from user-task
  completion: creating/assigning a Worker task, receiving heartbeat, or
  scheduling a timer must finish the current turn as `claim="waiting"` when the
  user objective still depends on future Task Center/timer truth; `claim="complete"`
  is reserved for verified final user-facing completion
- Runtime independently rejects a Master user-session `claim="complete"` while
  any Task Center child task with the same `parent_session_id` remains
  actionable or unresolved: `Created`, `WaitingAgent`, `Assigned`,
  `Running`, `Interrupted`, `Paused`, `Blocked`, `ReviewSubmitted`,
  `Approved`, or `Rejected`. Terminal historical children such as
  `Cancelled`, `Failed`, and `Closed` are not running agents and must not
  keep the parent UI in stale waiting state.
- Runtime independently rejects a Master user-session `claim="waiting"` unless
  current owner truth has an open same-session child task or an active/running
  source timer. A closed/cancelled/failed child workset plus no source timer is
  not a lifecycle that can wake itself; if the next step is a user choice, the
  model must close the turn as `claim="blocked"` with the required choice, or
  use `claim="complete"` only when the final user objective is actually
  satisfied.
- When a child `task_closed` event arrives, the runner checks the same
  logical parent-turn child workset. `runtime-turn-N`, `runtime-turn-N-r2`,
  and later repair/tool-result rounds are one parent logical turn for workset
  membership; exact round ids must not let the first closed child trigger
  parent evaluation while sibling children from the same logical request are
  still open. If all children in that workset are `Closed` and the same
  parent/child-set evaluation was not already recorded, it starts a follow-up
  Master turn in the original parent session. The turn evaluates original user
  objective history against each decomposed task's
  goal/deliverables/acceptance and accepted Worker review truth, then either
  creates/assigns correction or next-round child tasks, records an explicit
  blocker, or claims final completion only when the overall objective is
  verified complete.
- When a child remains `TaskBlocked` after the Master has persisted an explicit
  `blocked_decision`, and no same-logical-parent-turn child is still active or
  reviewable, the runner validates any completed-parent marker against the
  effective parent transcript. A marker whose follow-up exists only as raw
  rolled-back audit truth is cleared and re-run with a non-colliding fresh turn
  id. The runner starts a user-visible blocked follow-up turn in the original
  parent session. The follow-up carries original user objective truth, Worker
  blocker evidence, and the Master blocked decision so the parent session
  closes observably as blocked instead of staying on stale waiting lifecycle
  text.
- Master task-tool execution independently enforces membership in the configured
  Worker set; a non-configured assignment becomes a paired failed tool result
  and cannot mutate Task Center truth
- Master task creation independently rejects omitted, auto, self, or
  non-configured-agent dispatch so persisted historical agents cannot be
  selected before the explicit assignment gate

## Response Mainline

- no Assigned task returns an explicit idle outcome without task mutation
- a due one-shot timer injects its persisted wakeup prompt as a new turn in
  its persisted original user session when source-session truth exists; it
  does not reopen or resume the source turn. Source-less timers use an internal
  Master wakeup. Both complete without creating or mutating task truth
- timer ancestry is resolved through persisted schedules so an older
  `master-timer-*` source cannot drift subsequent wakeups away from the
  original user session
- a due recurring timer fires, increments its fire count, and reschedules until
  its configured run limit is reached; daily, weekly, and cron recurrence uses
  local timezone semantics
- Master review-ready handling must move the task to rejected or
  approved/closed before its event cursor advances
- once the event-specific target-task decision boundary is reached, the reason
  turn closes in the same tool-result round and control returns to EventInbox
  polling; the model never waits for a future Worker event inside that turn
- persisted timer wakeup prompts tell the future Master turn what current truth
  to inspect, what waited condition to revisit, and what decision to make
- Timer list projections expose only schedule and ledger fields as UI-safe
  dashboard rows; they do not expose runtime debug metadata and do not turn
  timer state into task lifecycle state.
- lifecycle decision rounds are finite; exhaustion closes blocked and leaves
  the event cursor retryable
- retryable lifecycle executor and missing-decision failures keep the durable
  attention item, admitted sequence, and EventInbox cursor unchanged, persist
  the next attempt id, apply bounded exponential backoff, and do not stop the
  daemon
- restart recovery is keyed by what owner truth was durably written before the
  exit: admitted attention with no retry state reruns attempt 0, while a
  persisted retry state reruns the same event at attempt N; both paths require
  Task Center mutation before cursor advancement
- provider retry/failover inside a background lifecycle or parent-evaluation
  turn is a live model-waiting state on the owning session/turn. It must be
  published before retry backoff or fallback, and the durable ErrorCenter row is
  sufficient for query-time recovery after restart or a missed hook.
- success, failure, and retry branches each write a terminal or retryable owner
  state before the loop advances: success becomes review/approve/close truth,
  retry becomes rejected or interrupted truth with a later new execution id,
  non-retryable failure becomes blocked truth plus an explicit Master decision,
  and no branch may remain as an unbounded active reason turn
- stale or already-satisfied attention items are removed and dequeue continues
  in the same runner tick; a stale high-weight event cannot hide a later
  actionable parent/task event
- a resumed foreground Master provider request contains the refreshed
  TaskSpaceSnapshot plus the typed `AttentionResolution`, preserves original
  session/logical-turn/trace identity, and requires the model to re-evaluate
  the original objective before any new tool call or terminal claim
- approved review-ready truth remains retryable until the Master closes it
- parent evaluation truth is durable-idempotent by parent session plus closed child task workset, so EventInbox replay or daemon restart cannot repeat the same evaluation decision or duplicate next-round task creation; successful, waiting, and blocked terminal evaluation turns are durable decisions, while failed/interrupted/cancelled turns remain retryable
- child workset admission is scoped to the parent logical turn ordinal, not the whole parent session and not the exact `runtime-turn-N-rM` repair/tool-result round; historical blocked children from earlier user turns must not keep a later corrected user turn stuck in `waiting lifecycle`, while sibling child tasks created across multiple rounds of the same logical request must close before parent evaluation starts
- parent evaluation prompt context is wider than the current idempotency
  workset when the Master itself created next-round child tasks: it includes
  all closed child review truth from the same parent session between the latest
  external user objective turn ordinal and the current parent/evaluation turn,
  so final synthesis sees prior accepted alpha/beta/gamma truth plus the
  integration task; it still excludes child truth from older user objectives
- when the durable EventInbox cursor has already advanced past a `task_closed` event without a completed/skipped parent evaluation marker, the Master loop reconciles TaskBoard truth while idle and re-enters parent evaluation for a still-waiting parent logical turn
- when the durable EventInbox cursor has advanced past a `TaskBlocked` plus
  Master-owned `blocked_decision`, the Master loop reconciles TaskBoard truth
  while idle and injects one idempotent parent-session blocked follow-up only
  after the same logical workset has no active/reviewable child tasks
- parent workset reconciliation reads parent waiting state, idempotency markers,
  and next evaluation turn ordinals from authoritative closed/active turn
  snapshots only; it must not call selected-transcript UI restore or parse
  historical reason ledgers while the background Master loop is polling
- bootstrap stale-wait reconciliation uses the same reason-owner closed-turn
  truth and writes a `Blocked` terminal event only for no-owner `ToolPending`
  turns. A waiting turn with a wakeable child task, active/running source
  timer, or live non-recoverable `master_work` checkpoint remains waiting.
- parent evaluation reads the original first-round user objective from
  authoritative reason `TurnStarted` ledger truth via
  `ReasonPersistence::restore_turn_start_snapshots`; UI-coalesced
  repair/control rounds and latest effective snapshots cannot replace or
  become the overall goal
- if a historical all-children-closed parent set has no persisted original user
  objective truth, the runner records that exact child set as
  `ParentEvaluationSkipped` without finalizing, then continues current
  lifecycle processing
- `QuerySessionTurns` hides internal timer/parent-evaluation follow-up prompts
  from both raw `request.user_text` and original-task-context-derived display
  text while preserving terminal status, tool truth, and final summaries
- Provider-visible rebuilt `SessionHistory` also hides internal
  timer/parent-evaluation prompt text while preserving the terminal summary as
  historical assistant context. UI hiding alone is insufficient because future
  Master turns consume `SessionHistory.base_context_segments`.
- a blocked task may remain blocked only when the Master records that outside
  action is still required through `task(op="append")`, or explicitly
  reassigns it
- after repeated provider/executor failures while trying to decide an already
  blocked task, the runner appends an explicit Master-owned `blocked_decision`
  noting lifecycle provider unavailability and continues other pending events;
  this never marks the blocked task successful
- interrupted truth remains retryable until the task leaves `Interrupted`
- multiple configured Worker runner processes may independently claim only
  tasks assigned to their own agent id; one runner cannot consume another
  configured Worker's task
- Worker ticks never requeue `Interrupted` tasks. Interrupted truth remains
  unchanged until Master explicitly chooses same-Worker retry or cross-Worker
  takeover and reassigns the same task
- Worker ticks requeue only `Rejected` tasks previously bound to that Worker,
  because Master already made the explicit review/rework decision
- paused Worker executions stop as `Idle` after the live bridge observes the pause cancel token at a safe point; stale success, stale block, and heartbeat errors after pause do not overwrite `TaskPaused` truth
- persisted resume control lets the runner select the existing `Running` task/execution and re-enter Worker reasoning without allocating a replacement task or losing the execution identity
- claim persists `TaskResumed` and `TaskHeartbeat` before provider execution
- successful worker completion writes one `ExecutionFactKind::ReviewReady`
- provider/network system failure after internal provider retry exhaustion writes one
  `ExecutionFactKind::Interrupted`, keeping the same task retryable with a new
  execution id instead of creating a replacement task
- task-content, path-preflight, model-terminal, or non-provider execution
  failure writes one `ExecutionFactKind::Blocked`
- blocked execution releases the Worker resource to `Available`; task blockage
  and Worker resource pause are distinct owner truths
- Worker startup relies on Task Center boot reconciliation to repair historical
  paused snapshots only when no assigned task remains explicitly paused
- blocked tasks remain Master decisions and are never silently retried by the
  Worker runner
- worker reason/session truth remains persisted under worker agent identity
- task/execution/lease/agent truth remains persisted under the paired Master's Task Center namespace
- AgentBoard/AgentLifecycle return Worker PID, process instance, heartbeat
  timestamp, restart count, and TTL-derived `alive` from the lifecycle owner
- expired `Running` leases become `TaskInterrupted` during TaskRuntime boot and
  must be followed by a Master-owned reassignment of the same task before any
  new Worker execution starts
- periodic runner ticks continue after idle, success, or blocked outcomes
- Worker heartbeat and result reporting are rejected if Task Center truth has
  externally terminalized the task; stale Worker runtime state cannot overwrite
  cancel/close/fail truth

## Error Mainline

- Master-selected config is rejected by the Worker runner
- Slave-selected config is rejected by the Master lifecycle runner
- Master review prose without a Task Center mutation is an explicit
  `MissingReviewDecision` error and the event cursor does not advance
- Master approval without close is an explicit `IncompleteReviewDecision`
  error and the event cursor does not advance
- nullable unused response-status fields do not stop the Master lifecycle turn;
  non-null response-status type mismatches are polished in another model round
- Master blocked prose without a persisted `task(op="append")`
  `TaskProgressed` decision is an
  explicit `MissingBlockedDecision` error
- Master interrupted prose without reassignment is an explicit
  `MissingInterruptedDecision` error
- Master provider/system failure or process exit during lifecycle decision
  cannot advance the EventInbox cursor past an unmutated task; the durable
  pending item is retried after restart until Task Center truth changes
- interrupted takeover cannot create a duplicate task for the same objective;
  it must mutate assignment on the existing Task
- unrelated task mutation cannot satisfy the current event decision boundary
- assignment to a historical or non-configured Worker returns a paired failed
  tool result and writes no `TaskAssigned` event
- task creation with omitted/auto/self dispatch, or explicit dispatch to a
  historical/non-configured Worker, returns a paired failed tool result and
  writes no task truth
- lifecycle decision round-budget exhaustion is explicit blocked reason truth,
  never an indefinitely active turn
- timer wakeup executor failure records timer failure truth, releases the
  schedule back to active retryable state, and surfaces a retryable Master
  execution error instead of leaving the timer stuck running
- timer wakeup executor failure must not prevent already-pending Task Center
  lifecycle events from being processed first
- repeated blocked-decision provider failure is converted into an explicit
  `TaskProgressed` blocked decision after the retry cap so one blocked task
  cannot permanently starve later lifecycle events
- lifecycle cursor parse/write failures stop the Master loop explicitly
- stale historical EventInbox cursor or missing-task attention is repaired or
  dropped explicitly; other Task Center owner-truth failures still stop the
  Master loop because current owner truth is unavailable
- bootstrap stale-wait reconciliation failures from TaskBoard, TimerStore,
  active-work, or reason persistence are explicit bootstrap errors. Missing
  recovery truth for a deleted/incomplete session is skipped, but a restorable
  no-owner `ToolPending` latest turn is not hidden in UI; it is durably closed
  as `Blocked`.
- Task Center and lifecycle-state failures are fatal owner-truth failures;
  lifecycle executor and missing/incomplete decision failures are retryable
- missing paired Master identity or invalid provider config blocks runner bootstrap
- missing or non-canonicalizable target cwd records blocked task truth before model execution, with classified wording for missing parent, likely output-directory misuse, permission denial, and generic canonicalization failure
- claim/heartbeat persistence failure returns an explicit runner error and does not start the model
- process-start or process-heartbeat persistence failure returns an explicit
  runner error and does not claim or execute work
- heartbeat or result reporting after external cancel returns explicit Task
  Center failure and does not append Worker lifecycle truth
- provider/network system failure is paired to the claimed task/execution as
  interrupted truth after internal provider retries are exhausted; task-content
  and path/preflight failures remain blocked truth
- failure to persist the blocked fact is returned as a combined explicit runner error
- worker cannot call `task`; schema excludes it and execution policy rejects it if received
- Worker failure never becomes `review_ready`, approved, closed, or successful UI truth
- retryable task recovery must allocate a new execution id; old execution
  history remains immutable

## Shared Multi-Reference Functions

- `run_live_reason_turn_with_policy`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: share provider/reason loop mechanics while applying an explicit Master or Worker tool/workspace policy
  - allowed callers: `run_live_reason_turn`, `run_worker_live_reason_turn`, runtime tests
  - related tests: Master boundary tests, Worker tool-policy tests
  - why shared: provider/reason lifecycle must not be copied for Worker execution
- `BuiltinToolRegistry::worker_implemented_definitions`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose implemented Worker tools while physically excluding recursive `task` and unrestricted shell tools
  - allowed callers: runtime live bridge and tool-registry tests
  - related tests: Worker schema inclusion/exclusion tests and shell rejection test
  - why shared: Worker capability and write-boundary policy must have one registry owner
- `admit_master_attention_resolution_for_next_round`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert one validated master-work attention resolution plus a refreshed TaskSpaceSnapshot into request-context candidates for the original foreground turn
  - allowed callers: live Master safe-point continuation paths and runtime tests
  - related tests: `live_master_attention_invalidates_stale_tool_without_side_effect`, `live_master_attention_rejects_stale_terminal_persistence`, and `production_master_resume_consumes_resolution_once`
  - why shared: stale tool invalidation, stale terminal invalidation, and before-provider continuation must use the same typed context admission semantics
- `recover_stale_lifecycle_waits_on_bootstrap` / `session_has_lifecycle_owner_for_turn`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: close no-owner persisted `ToolPending` session turns during live runtime bootstrap while preserving owner-backed lifecycle waits
  - allowed callers: `RuntimeCommandDispatcher::new` live bootstrap only
  - related tests: `live_bootstrap_closes_stale_toolpending_without_lifecycle_owner`, `live_bootstrap_keeps_toolpending_when_child_task_can_wake_parent`
  - why shared: UI restore, TaskBoard, timers, and active Master work must use one owner-truth classification for whether a wait can wake itself after restart
- `path_resolution_diagnostic_text` / `expand_leading_tilde_path`
  - owner: `crates/freehand-runtime/src/path_diagnostics.rs`
  - purpose: render one shared target-cwd diagnostic with requested, expanded, nearest-existing, canonical parent, missing suffix, and symlink ancestor evidence
  - allowed callers: Worker target-cwd preflight, Master task-tool target-cwd diagnostics
  - related tests: Worker target-cwd symlink and missing-path preflight tests; `task_tool_create_returns_symlink_parent_path_diagnostic`
  - why shared: path/symlink diagnosis must not diverge between Worker preflight and Master task dispatch failure text

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config + runtime owner | bound |
| 02 | `ProductionWorkerRunner::run` | `crates/freehand-runtime/src/worker_runner.rs` | run periodic Worker ticks with explicit cadence | runner + interval | long-running Worker service | daemon Slave mode | `run_once` | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task or resumed controlled task, canonicalize target cwd with `~` expansion and symlink resolution, heartbeat, monitor pause truth, execute, and report | Task Center + Worker identity + WorkerControl truth | idle/review-ready/interrupted/blocked outcome without stale paused overwrite | Worker service loop/tests | task owner + live bridge | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim highest-priority Assigned task for Worker | worker id + execution id + lease TTL | claimed task + TaskResumed/heartbeat truth | Worker runner | task owner | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease and same process-instance heartbeat while provider execution remains active | claimed task/execution/worker/process identity | periodic TaskHeartbeat plus agent heartbeat truth or explicit heartbeat error | `ProductionWorkerRunner::run_once` | task owner + agent.lifecycle owner | bound |
| 05a | `WorkerPauseMonitor::start` / `worker_pause_requested` | `crates/freehand-runtime/src/worker_runner.rs` | poll same-task/same-execution WorkerControl ledger truth and set the live cancel token when latest task-state control is applied pause; after execution, suppress stale review/block/heartbeat overwrite while `TaskPaused` remains truth | task id + execution id + persisted WorkerControl events | cooperative live pause at safe point plus idle runner outcome | `ProductionWorkerRunner::run_once` | task owner + live bridge cancel token | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one worker task in canonical task cwd with Worker tool policy and path-preflight prompt contract | selected Worker config + live request | closed live reason outcome | Worker runner | provider/reason live bridge | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready, interrupted, or blocked result for same execution | typed execution fact | terminal task mutation | Worker runner | task owner | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | runtime Worker runner | bound |
| 09 | `ProductionMasterRunner::from_default_config` | `crates/freehand-runtime/src/master_runner.rs` | load selected Master config and bind the Master Task Center namespace | configured agent name | Master lifecycle runner | daemon Master startup | config + runtime owner | bound |
| 10 | `ProductionMasterRunner::run_until` | `crates/freehand-runtime/src/master_runner.rs` | poll Task Center lifecycle events, retry model/provider decision failures with bounded backoff across process restart, and stop on owner-truth failure or daemon shutdown | runner + cancellation signal | long-running Master lifecycle service | daemon Master mode | `ProductionMasterRunner::run_once` | bound |
| 11 | `ProductionMasterRunner::run_once` | `crates/freehand-runtime/src/master_runner.rs` | repair stale EventInbox cursor if needed, admit EventInbox source order into durable attention before decision execution, dequeue by deterministic weighted aging, preserve retry identity across restart, drain stale no-ops, reconcile decided-blocked and closed parent worksets whose parent turn still waits for lifecycle, then consider due timers | Task Center cursor + pending attention + timer store | task decision, blocked parent follow-up, parent-evaluation skip/evaluation, timer-fired, or idle outcome | Master lifecycle service/tests | `query_event_inbox_repairing_stale_cursor` + `admit_attention_events` + `highest_priority_attention_index` + `handle_event` + `reconcile_blocked_parent_worksets` + `reconcile_closed_parent_worksets` + `handle_due_timer` | bound |
| 11a | `ProductionMasterRunner::admit_attention_events` | `crates/freehand-runtime/src/master_runner.rs` | persist attention in EventInbox order and advance the cursor without priority reordering, while skipping stale rows whose task no longer exists in current Task Center truth | ordered EventInbox rows + Master loop state | durable pending attention + monotonic admission sequence or explicit stale-row skip | `ProductionMasterRunner::run_once` | TaskRuntime query + Master loop state | bound |
| 11b | `highest_priority_attention_index` | `crates/freehand-runtime/src/master_runner.rs` | select by severity, bounded task priority, deterministic admission aging, and stable tie-breaks | durable pending attention + next admission sequence | selected pending index | `ProductionMasterRunner::run_once` | pure score comparator | bound |
| 11c | `register_master_active_work` / `clear_master_active_work_if_current` | `crates/freehand-runtime/src/master_runner.rs` | persist and clear foreground Master work identity under the master-work lock | live Master submit identity | active-work checkpoint or explicit concurrent-work rejection | runtime live submit dispatcher | active-work JSON + lock file | bound |
| 11d | `ProductionMasterRunner::apply_busy_attention_policy` | `crates/freehand-runtime/src/master_runner.rs` | compare pending attention score with foreground work priority, defer lower-priority attention, and request/suspend higher-priority attention only at declared safe points | pending attention + master_work checkpoint | deferred attention, suspend request, or suspended active work | `ProductionMasterRunner::run_once` | active-work store + weighted attention score | bound |
| 11e | `ProductionMasterRunner::restore_active_work_after_attention` | `crates/freehand-runtime/src/master_runner.rs` | persist typed attention resolution from the event-scoped isolated control decision and restore the exact foreground work identity without copying control transcript text | suspended master_work + Task Center decision outcome | running active work with typed resolution and original work/session/turn/trace identity | `ProductionMasterRunner::run_once` | active-work store | bound |
| 11f | `admit_master_attention_resolution_for_next_round` | `crates/freehand-runtime/src/lib.rs` | consume one validated resolution, refresh TaskSpaceSnapshot, and admit volatile/no-cache AttentionResolution before the original foreground work continues | running master_work typed resolution + current task truth | next-round request-context candidates without stale task/terminal semantics | Master live safe-point continuation paths | context planner candidate admission | bound |
| 12 | `ProductionMasterRunner::handle_due_timer` | `crates/freehand-runtime/src/master_runner.rs` | execute a due independent timer wakeup and complete/reschedule/release timer truth | due timer schedule | timer-fired outcome or retryable execution error | `run_once` | timer store + live reason turn | bound |
| 13 | `TimerStore::claim_due` / `TimerStore::complete_due` / `TimerStore::fail_due` | `crates/freehand-runtime/src/timer_store.rs` | persist independent timer schedule state and timer ledger events outside Task Center truth | timer state json + timer ledger | running/completed/active timer truth | Master timer tool + Master runner | timer store owner | bound |
| 14 | `ProductionMasterRunner::handle_event` | `crates/freehand-runtime/src/master_runner.rs` | invoke the Master model for current review-ready, blocked, interrupted, or all-children-closed parent evaluation truth; interrupted decisions receive AgentBoard resource truth, may replace the existing task assignment, and remain retryable after provider/system exit until Task Center truth changes | task snapshot + trigger event + AgentBoard | same task advanced, blocked observed, parent evaluated, no-op, or explicit error | `run_once` | Master live reason turn + task owner | bound |
| 14a | `ProductionMasterRunner::handle_parent_task_closed` / `ProductionMasterRunner::reconcile_closed_parent_worksets` / `ProductionMasterRunner::reconcile_blocked_parent_worksets` / `parent_completed_context_children` / `parent_latest_external_objective_ordinal_at_or_before` / `parent_blocked_subtask_truth` / `parent_blocked_follow_up_live_request` / `parent_turn_group_key` / `parent_turns_share_logical_group` / `parent_logical_turn_waits_for_lifecycle` / `persisted_parent_evaluation_summary` / `persisted_parent_blocked_follow_up_summary` / `raw_parent_blocked_follow_up_summary` / `next_parent_evaluation_turn_id` / `parent_user_objectives` | `crates/freehand-runtime/src/master_runner.rs` | decide whether a child `task_closed` event, an all-closed idle reconciliation, or a decided `TaskBlocked` workset should resume the current logical parent session; group `runtime-turn-N` and `runtime-turn-N-rM` children together while keeping different logical user turns separate; read waiting/idempotency/next-turn state from authoritative effective turn snapshots plus reason-owned raw reserved turn ids without selected-transcript ledger backfill; recover first-round external user objectives from authoritative reason `TurnStarted` ledger truth across global runtime turn ordinals; widen completed-child prompt context to same-objective prior closed child truth for Master-created next-round evaluation while excluding older user objectives; build either an idempotent overall-goal evaluation request or an idempotent parent-session blocked follow-up request; if a completed blocked marker points only to a raw rolled-back follow-up, clear that stale marker and re-run with a non-colliding next turn id; if historical parent objective truth is missing, record a non-final skipped evaluation for that exact child set | original parent user objective history + authoritative parent logical-turn/evaluation snapshot truth + current same-logical-parent-turn evaluation workset + same-objective prior closed child accepted review truth + decided blocked child truth + raw rolled-back follow-up reservation truth | next-round task creation, explicit blocker, verified final completion, skipped non-final historical evaluation, user-visible parent blocked follow-up, a reissued blocked follow-up after rollback invalidation, or no-op without background reason-ledger replay | `handle_event` / `run_once` idle reconciliation | TaskRuntime + ReasonPersistence + Master live reason turn | bound |
| 15 | `run_master_lifecycle_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one event-isolated lifecycle decision with a target-task boundary, finite round budget, and restart-stable event/attempt turn identity | selected Master config + typed lifecycle prompt + decision boundary | closed Master turn and Task Center mutation | `ProductionMasterRunner::handle_event` | provider/reason live bridge | bound |
| 15a | `master_session_completion_rejection` / `master_session_lifecycle_owner_truth` | `crates/freehand-runtime/src/lib.rs` | validate Master completion schema against current same-session Task Center child truth and source timer truth before terminal persistence; reject stale `claim="waiting"` when no owner resource can wake the lifecycle without another user message | parsed `CompletionSubmission` + parent `session_id` + TaskBoard child statuses + timer schedules | accepted terminal/waiting decision or schema rejection forcing blocked/user-choice or complete/evidence semantics | `run_live_reason_turn_with_policy` | TaskRuntime + TimerStore + completion schema retry loop | bound |
| 16 | `configured_worker_task_boundary_failure` | `crates/freehand-runtime/src/lib.rs` | validate Master task create/assign routing against the configured Worker topology | task tool call + ordered configured Worker id set | explicit topology failure or allowed mutation path | `execute_registry_tool_call_with_workspace` | pure boundary validator | bound |
| 17 | `execute_registry_tool_call_with_workspace` | `crates/freehand-runtime/src/lib.rs` | enforce configured Worker-set routing before task-tool mutation and route Master timer tool calls to independent timer truth | Master task/timer tool call + ordered configured Worker id set | paired failed result or owner-routed task/timer mutation | provider/reason live bridge | topology validator + task tool/timer owners | bound |
| 18 | `run_master_mode` / `master_lifecycle_runner_disabled_for_test` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP host and Master lifecycle runner under one daemon lifetime, except explicit fixture verifier runs may set `FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER=1` to keep ADP/WebUI live submit active while preventing background Task Center lifecycle decisions from sharing a temporary provider fixture | Master bootstrap + bind + explicit test-only lifecycle-disable env | supervised Master daemon with lifecycle runner, or verifier-isolated WebUI/ADP host without background lifecycle runner | daemon CLI | server host + optional `ProductionMasterRunner::run_until` | bound |
| 19 | `ProductionWorkerRunner::record_process_started` / `ProductionWorkerRunner::record_process_heartbeat_in` | `crates/freehand-runtime/src/worker_runner.rs` | emit one unique process instance at runner construction and refresh it on every poll tick | configured Worker identity + PID + process-instance id | persisted agent.lifecycle process health | Worker construction / `run_once` | `TaskRuntime::apply_agent_lifecycle_event` | bound |
| 20 | `RuntimeCommandDispatcher::new` / `recover_stale_lifecycle_waits_on_bootstrap` / `session_has_lifecycle_owner_for_turn` / `task_can_wake_parent_lifecycle` / `owner_turn_matches_target` | `crates/freehand-runtime/src/lib.rs` | reconcile live-bootstrap persisted `ToolPending` user-session turns before UI projection restore; distinguish wakeable task/timer/live-master owners from stale no-owner waits | runtime home + Master agent id + reason closed-turn truth + TaskBoard + TimerStore + master_work checkpoint | owner-backed waits preserved as `ToolPending`, stale no-owner waits durably re-closed as `Blocked`, or explicit bootstrap error | `RuntimeCommandDispatcher::new` live bootstrap | `ReasonPersistence` + `TaskRuntime::boot_read_only` + `TimerStore` + `master_runner` active-work owner truth | bound |

## Sync Status Against Code

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master local-workspace tool boundary, external-cwd delegation, and path/symlink dispatch guidance are already bound; Master uses locked workspace tools directly for the current selected session cwd and delegates different-cwd, isolated, concurrent, long-running, or resumable work to Worker tasks
- production Worker runner, Worker-specific live tool policy, pause monitor cancel-token safe-point handling, resumed controlled task selection, periodic task
  lease heartbeat, process heartbeat, and Slave daemon startup are code-bound
- independent timer persistence and recurrence logic is now code-bound in
  `crates/freehand-runtime/src/timer_store.rs`; `src/lib.rs` only routes the
  Master timer tool call into that owner
- config-selected Master guidance, TaskSpaceSnapshot, and task mutation boundary
  consume the full ordered Worker peer set; singular configured-Worker fields
  are physically removed
- each Slave Worker runner consumes its one configured Master peer and keeps
  task claim/execution identity distinct per configured Worker process
- the controlled online verifier starts `worker-alpha`, `worker-beta`, and
  `worker-gamma` as three distinct daemon processes, enforces one initial task
  per Worker without cross-claim, forces reject/rework plus a next-round
  integration task, and persists JSON proof before explicit PID cleanup
- deterministic positive/negative tests cover idle, review-ready, blocked, missing workspace, role mismatch, and Worker tool capability boundaries
- busy-Master live continuation is focused-test bound for one-shot resolution
  consumption, stale tool no-side-effect invalidation, stale terminal
  non-persistence, raw transcript rejection, mismatched return-identity
  rejection, and cooperative no-mid-effect suspension
- online daemon/WebUI/Android proof for busy-Master live preemption remains a
  separate verification gap; do not treat focused runtime tests as full product
  closure
- live bootstrap stale `ToolPending` recovery is code-bound before UI projection restore and focused-test bound for both no-owner close and wakeable-child preservation
- generated wiki must be regenerated whenever this mainline changes
