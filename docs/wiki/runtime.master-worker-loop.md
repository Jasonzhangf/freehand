# Wiki: `runtime.master-worker-loop`

Generated from `docs/mainline-calls/runtime.master-worker-loop.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.master-worker-loop.md`
- generated wiki: `docs/wiki/runtime.master-worker-loop.md`
- test design: `docs/testing/runtime.master-worker-loop.md`

## Resource Operation Backlinks

- timer.fire_master_wakeup
- timer.schedule
- timer.cancel
- timer.list
- master_work.resolve_attention
- master_work.admit_resolution_context
- agent.heartbeat
- agent.project_runtime_agent_activity

## Request Mainline

- one daemon process selects one configured agent
- Master mode starts the WebUI/ADP host plus a background lifecycle runner over Task Center EventInbox truth
- the Master lifecycle runner admits events after its durable cursor into pending_attention in EventInbox source order, then invokes the Master model for the dequeued current review-ready, blocked, interrupted, or all-current-children-closed parent-evaluation truth
- a stale durable EventInbox cursor from historical/global Task Center truth is repaired by clearing the cursor and replaying current Task Center ledger truth; stale rows or pending attention for missing tasks are logged and skipped instead of permanently stopping current lifecycle progress
- Task Center lifecycle attention has priority over due internal timers; due timers are claimed only when pending_attention is empty
- pending_attention dequeue uses deterministic weighted aging: blocked showstoppers and critical/high-priority work carry large score weight, while admission-sequence aging prevents starvation without wall-clock timing
- foreground Master user work is persisted as master_work with active identity, priority, safe point, suspension state, and exact return identity; lower-priority attention stays queued while higher-priority attention can only suspend at a declared safe point
- while foreground work is SuspendedByAttention, the selected attention decision runs as a task-scoped master-lifecycle control request with event/attempt-isolated session, turn, and trace ids distinct from the suspended foreground identity; executor prose or raw control transcript text is not copied into the foreground session or typed AttentionResolution
- after a restored high-priority attention resolution, the original foreground live turn consumes the typed resolution once, refreshes TaskSpaceSnapshot, admits volatile/no-cache AttentionResolution request context, invalidates stale tool or terminal candidates, and continues the same logical turn
- each actionable event attempt uses a task-scoped internal master-lifecycle reason session, deterministic event-and-attempt-isolated turn and trace ids, and an explicit target-task decision boundary
- each Master lifecycle, parent-evaluation, and timer wakeup live turn publishes reason/debug/task-list hooks into the same shared UiProtocolState as foreground user submits so retry, failover, schema repair, tool execution, terminal status, and task mutations are observable by owning session and turn
- every lifecycle is closed by its owner truth source: TaskRuntime owns task/execution success, blocked, interrupted, rejected, approved, and closed transitions; AgentLifecycle owns Worker liveness, process restart count, and current binding; Master loop state owns attention retry/backoff and cursor advance; reason persistence owns parent-session waiting/final turn truth
- if the Master process exits after durable EventInbox admission but before a provider decision, the next Master start reuses pending_attention and the unchanged cursor instead of skipping the event; if provider/system failure returned before exit, retry_event_id and retry_attempt select the next event-scoped attempt turn
- if a foreground Master process exits while master_work is open, runtime bootstrap recovers only from the master_work checkpoint plus matching reason active-turn truth: it interrupts/closes the stale active turn or clears invalid checkpoint-only state before accepting new UI/ADP work
- live runtime bootstrap reconciles persisted user-session ToolPending turns before UI projection restore by reading reason closed-turn truth plus read-only TaskBoard, TimerStore schedules, and live non-recoverable master_work checkpoints; no-owner waits are closed as Blocked and owner-backed waits stay ToolPending
- Master supervision combines TaskHistory with AgentBoard resource truth; Worker process restart or stale heartbeat is never task success and may only drive same-task retry, cross-Worker takeover, or explicit blocked decision through Task Center mutation
- when a child task_closed event arrives, the runner checks all terminal-included child tasks with the same parent_session_id and starts an overall-goal evaluation turn in the original parent session only after every current child is Closed; the evaluation receives original user objective history, decomposed child task requirements, and accepted review truth
- when a child remains TaskBlocked after the Master has persisted an explicit blocked_decision, and no same-logical-parent-turn child is still active or reviewable, the runner starts a user-visible blocked follow-up turn in the original parent session using original user objective truth, Worker blocker evidence, and the Master blocked decision; on restart, a completed-parent marker is trusted only if the effective parent transcript still contains the matching blocked follow-up, otherwise raw rolled-back follow-up evidence is treated as stale and the parent check is re-run with a fresh reserved turn id
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- each Worker opens its only configured Master's Task Center namespace and uses its own distinct configured agent id as execution identity
- Worker construction records one typed process-start event; every idle or active poll tick and the active task-heartbeat loop refresh the same process instance through agent.lifecycle
- Worker process exit during Running is recovered only by TaskRuntime lease truth: the next Worker or Master TaskRuntime boot writes TaskInterrupted for a missing or expired lease, releases the current Agent binding, and leaves the same task idle until Master explicitly reassigns it
- each Worker tick claims the highest-priority Assigned task for that Worker
- claim persists one execution id and lease heartbeat
- task target cwd expands a leading ~, canonicalizes through symlinks, and becomes the locked Worker execution root
- Worker live reasoning receives the requested target_cwd, canonical locked workspace, and path-preflight instructions for absolute paths, ~, and symlinks; target_cwd preflight failures include shared path diagnostics with symlink ancestors, canonical nearest parent, and missing suffix
- while Worker execution is active, the runner monitors same-task/same-execution WorkerControl pause truth and wires applied pause into LiveReasonCancelToken so the live bridge stops only at existing safe points
- Worker provider requests expose governed workspace tools but exclude recursive `task` and unrestricted shell tools
- Master provider guidance binds dispatch to the ordered configured Worker id set, excludes historical AgentBoard entries as production targets, forbids task lifecycle calls in Worker task content, and requires path/symlink/canonical evidence for external cwd delegation
- Runtime rejects a Master user-session claim=waiting unless current owner truth has an open same-session child task or an active/running source timer; terminal child history plus no source timer is not a self-waking lifecycle and user-choice waits must close as blocked/user-needed instead of ToolPending
- Master task-tool execution independently rejects assignment to any non-configured Worker before Task Center mutation
- Master task creation rejects omitted, auto, self, or non-configured-agent dispatch before persisted historical agents can be selected

## Response Mainline

- no Assigned task returns an explicit idle outcome without task mutation
- a due one-shot timer with persisted source truth injects its wakeup prompt as a new next-ordinal turn in its original user session without reopening the source turn; a source-less timer fires as an internal Master wakeup; neither creates or mutates task truth
- stored master-timer source ancestry resolves through timer schedules so chained timers cannot drift away from the original user session
- a due recurring timer fires, increments its fire count, and reschedules until its configured run limit is reached; daily, weekly, and cron recurrence uses local timezone semantics
- Master review-ready handling must reject or approve and close before the lifecycle cursor advances
- reaching the event-specific target-task decision boundary closes the reason turn in the same tool-result round and returns control to EventInbox polling
- lifecycle decision rounds are finite and exhaustion closes blocked while leaving the event cursor retryable
- retryable lifecycle executor and missing-decision failures keep the durable attention item, admitted sequence, and cursor unchanged, persist the next attempt id, apply bounded exponential backoff, and do not stop the daemon
- restart recovery is keyed by what owner truth was durably written before exit: admitted attention with no retry state reruns attempt 0, while persisted retry state reruns the same event at attempt N; both paths require Task Center mutation before cursor advancement
- provider retry or failover inside a background lifecycle or parent-evaluation turn is a live model-waiting state on the owning session and turn; it is published before retry backoff or fallback, and durable ErrorCenter metadata is sufficient for query-time recovery after restart or a missed hook
- success, failure, and retry branches each write terminal or retryable owner truth before loop advance: success becomes review/approve/close truth, retry becomes rejected or interrupted truth with a later new execution id, non-retryable failure becomes blocked truth plus explicit Master decision, and no branch may remain an unbounded active reason turn
- stale or already-satisfied pending attention items are removed and dequeue continues in the same runner tick
- a resolved busy-Master attention stores typed decision kind and changed task ids in master_work and restores the original work_id, session_id, logical turn id, and trace id without raw Worker/control transcripts, control-turn summaries, or provider payloads
- a resumed foreground Master provider request contains refreshed TaskSpaceSnapshot plus typed AttentionResolution while preserving the original session, logical turn, and trace identity
- approved review-ready truth remains retryable until Master closes the task
- parent evaluation truth is durable-idempotent by parent session plus closed child task set in the same logical parent turn, so EventInbox replay or daemon restart cannot repeat the same decision or duplicate next-round task creation; successful, waiting, and blocked evaluations are durable while failed/interrupted/cancelled evaluations remain retryable
- decided blocked-child follow-up truth is durable-idempotent by parent session, logical parent turn, blocked task id, and Master blocked_decision event sequence; EventInbox replay or daemon restart cannot duplicate the same parent blocked follow-up turn; rollback invalidation removes the completed marker from loop truth and reserves raw rolled-back turn ids so the new follow-up cannot collide with rollback markers
- parent workset membership groups runtime-turn-N and runtime-turn-N-rM children into one logical parent turn; exact repair/tool-result round ids must not let the first closed child trigger parent evaluation while same-logical-turn siblings remain open
- parent evaluation prompt context is wider than the current idempotency workset when the Master itself created next-round child tasks: it includes all closed child review truth from the same parent session between the latest external user objective turn ordinal and the current parent/evaluation turn, so final synthesis sees prior accepted alpha/beta/gamma truth plus the integration task; it still excludes child truth from older user objectives
- parent workset reconciliation reads waiting state, idempotency markers, and next evaluation turn ordinals from authoritative closed/active turn snapshots only; background Master polling must not call selected-transcript UI restore or parse historical reason ledgers
- bootstrap stale-wait reconciliation writes a Blocked terminal event for a no-owner latest ToolPending turn before SessionList/UI projection restore, but preserves ToolPending when an open child task, active/running source timer, or live Master work checkpoint can still wake the lifecycle
- parent evaluation reads only the first-round persisted user objective from authoritative reason TurnStarted ledger truth; UI-coalesced repair/control rounds and latest repaired snapshots cannot replace the goal
- if a historical all-children-closed parent set has no persisted original user objective truth, the runner records that exact child set as ParentEvaluationSkipped without finalizing and continues current lifecycle processing
- QuerySessionTurns hides internal timer or parent-evaluation follow-up prompts from both raw request text and original-task-context-derived display text while preserving terminal status, tool truth, and final summaries
- provider-visible rebuilt SessionHistory hides internal timer and parent-evaluation prompt text while preserving terminal summaries as historical assistant context, so future Master turns do not consume framework prompts as user memory
- blocked truth remains retryable until Master reassigns it or persists a blocked-decision note through task(op=append)
- after repeated provider/executor failures while deciding an already blocked task, the runner appends an explicit Master-owned blocked_decision and continues other pending lifecycle events without marking the task successful
- interrupted truth remains retryable until it leaves Interrupted
- interrupted tasks remain unchanged until Master explicitly reassigns the same task to the selected configured Worker; rejected tasks are requeued to their previously assigned Worker because Master already made the rework decision
- paused Worker execution returns idle after safe-point cancellation and does not publish stale review, stale block, or heartbeat failure over TaskPaused truth
- persisted resume control lets the runner re-enter the existing Running task/execution without allocating a replacement task or losing execution identity
- blocked tasks remain explicit Master decisions and are never silently retried by the Worker
- successful Worker completion writes one review-ready execution fact
- provider/network system failure after internal provider retries writes one interrupted execution fact for same-task retry; non-provider task execution failure writes one blocked execution fact
- Worker reason/session truth persists under Worker agent identity
- task/execution/lease/agent truth persists under the paired Master's Task Center namespace
- AgentBoard and AgentLifecycle return Worker PID, process instance, heartbeat timestamp, restart count, and TTL-derived alive from the lifecycle owner
- expired Running leases become TaskInterrupted during TaskRuntime boot and must be followed by a Master-owned reassignment of the same task before any new Worker execution starts
- periodic Worker ticks continue after idle, success, or blocked outcomes
- blocked Worker execution releases the configured Worker resource to Available while preserving TaskBlocked for Master decision
- Worker heartbeat and result reporting are rejected when paired Task Center truth has externally terminalized the task
- Worker startup repairs legacy paused snapshots only when Task Center truth has no explicitly paused assigned task

## Error Mainline

- Master-selected config is rejected by the Worker runner
- Slave-selected config is rejected by the Master lifecycle runner
- Master review prose without a Task Center mutation returns MissingReviewDecision and leaves the event retryable
- Master approval without close returns IncompleteReviewDecision and leaves the event retryable
- nullable unused response-status fields remain valid and non-null response-status type mismatches are polished without killing the lifecycle runner
- Master blocked prose without a persisted task(op=append) TaskProgressed decision returns MissingBlockedDecision and leaves the event retryable
- Master interrupted prose without reassignment returns MissingInterruptedDecision and leaves the event retryable
- Master provider/system failure or process exit during lifecycle decision cannot advance the EventInbox cursor past an unmutated task; the durable pending item is retried after restart until Task Center truth changes
- a Master claim=waiting without open same-session child-task truth or active/running source-timer truth is schema-rejected; closed child worksets and user-choice waits must not persist as lifecycle ToolPending
- bootstrap stale-wait reconciliation failures from TaskBoard, TimerStore, active-work, or reason persistence stop bootstrap explicitly; restorable no-owner ToolPending is never hidden as UI-only cleanup and is durably closed as Blocked
- interrupted Master decisions receive current AgentBoard resource truth and may replace the same task assignment from one configured Worker to another without changing task_id or parent_session_id
- Agent is a reusable execution resource independent of Session; current session attachment is derived only through task parent plus assignment, lease, and execution truth
- unrelated task mutation cannot satisfy the current event decision boundary
- assignment to a historical or non-configured Worker returns a paired failed tool result and writes no TaskAssigned event
- task creation with omitted, auto, self, or non-configured-agent dispatch returns a paired failed tool result and writes no task truth
- lifecycle decision round-budget exhaustion is explicit blocked reason truth rather than an indefinitely active turn
- timer wakeup executor failure records timer failure truth, releases the schedule back to active retryable state, and surfaces a retryable Master execution error
- timer wakeup executor failure must not prevent already-pending Task Center lifecycle events from being processed first
- a background lifecycle ErrorCenter row for a terminal turn must not reactivate the session as waiting_model because terminal reason truth remains authoritative
- missing or mismatched active-work checkpoint blocks busy-Master restoration explicitly rather than continuing a stale foreground turn
- repeated blocked-decision provider failure is converted into an explicit TaskProgressed blocked decision after the retry cap so one blocked task cannot permanently starve later lifecycle events
- Task Center and lifecycle-state failures stop the Master runner while lifecycle executor and missing-decision failures remain retryable
- stale historical EventInbox cursor or missing-task attention is repaired or dropped explicitly; other Task Center owner-truth failures still stop the Master loop because current owner truth is unavailable
- timer state read/write failures stop the Master runner explicitly because independent timer truth is unavailable
- Master lifecycle cursor parse or write failure stops the loop explicitly
- missing or invalid target cwd records blocked truth before model execution, with classified wording for missing parent, likely output-directory misuse, permission denial, and generic canonicalization failure
- claim or heartbeat persistence failure stops before provider execution
- process-start or process-heartbeat persistence failure stops before task claim or provider execution
- heartbeat or result reporting after external cancel returns explicit Task Center failure and does not append Worker lifecycle truth
- failure to persist a blocked execution fact remains an explicit runner error
- Worker cannot call recursive `task` through schema or execution policy
- Worker failure never becomes review-ready, approved, closed, or successful UI truth
- explicitly paused Worker task remains unavailable for unrelated assignment and must not be auto-repaired on startup

## Shared Multi-Reference Functions

- `run_live_reason_turn_with_policy`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: share provider/reason loop mechanics while applying explicit Master or Worker tool/workspace policy
  - allowed callers: run_live_reason_turn, run_master_lifecycle_reason_turn_with_hooks, run_worker_live_reason_turn, runtime tests
  - related tests: Master boundary tests, Worker tool-policy tests
  - why shared: provider/reason lifecycle must not be copied for Worker execution
- `BuiltinToolRegistry::worker_implemented_definitions`
  - owner: `crates/freehand-tools/src/lib.rs`
  - purpose: expose implemented Worker tools while excluding recursive `task` and unrestricted shell tools
  - allowed callers: runtime live bridge, tool-registry tests
  - related tests: Worker schema inclusion/exclusion tests, Worker shell rejection test
  - why shared: Worker capability and write-boundary policy must have one registry owner
- `admit_master_attention_resolution_for_next_round`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert one validated master-work attention resolution plus a refreshed TaskSpaceSnapshot into request-context candidates for the original foreground turn
  - allowed callers: live Master safe-point continuation paths, runtime tests
  - related tests: live_master_attention_invalidates_stale_tool_without_side_effect, live_master_attention_rejects_stale_terminal_persistence, production_master_resume_consumes_resolution_once
  - why shared: stale tool invalidation, stale terminal invalidation, and before-provider continuation must use the same typed context admission semantics
- `path_resolution_diagnostic_text / expand_leading_tilde_path`
  - owner: `crates/freehand-runtime/src/path_diagnostics.rs`
  - purpose: render shared target-cwd diagnostics with requested, expanded, nearest-existing, canonical parent, missing suffix, and symlink ancestor evidence
  - allowed callers: Worker target-cwd preflight, Master task-tool target-cwd diagnostics
  - related tests: Worker target-cwd symlink and missing-path preflight tests, task_tool_create_returns_symlink_parent_path_diagnostic
  - why shared: path and symlink diagnosis must not diverge between Worker preflight and Master task dispatch failure text
- `sleep_with_cancel`
  - owner: `crates/freehand-runtime/src/lifecycle_wait.rs`
  - purpose: wait in bounded increments while observing one owner-supplied host cancellation token
  - allowed callers: ProductionWorkerRunner::run_until, ProductionMasterRunner cancellation waits
  - related tests: production_worker_runner_honors_preexisting_host_cancellation, production_worker_runner_host_cancellation_interrupts_active_task_before_exit
  - why shared: Master and Worker lifecycle loops must use one cancellation-aware wait primitive instead of duplicating polling semantics
- `recover_stale_lifecycle_waits_on_bootstrap / session_has_lifecycle_owner_for_turn`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: close no-owner persisted ToolPending session turns during live runtime bootstrap while preserving owner-backed lifecycle waits
  - allowed callers: RuntimeCommandDispatcher::new live bootstrap
  - related tests: live_bootstrap_closes_stale_toolpending_without_lifecycle_owner, live_bootstrap_tolerates_incomplete_authoritative_history_with_empty_ledger, live_bootstrap_keeps_toolpending_when_child_task_can_wake_parent
  - why shared: UI restore, TaskBoard, timers, and active Master work use one owner-truth classification for whether a wait can wake itself after restart

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config and runtime owner |  |  |  | bound |
| 02 | `ProductionWorkerRunner::run / ProductionWorkerRunner::run_until / sleep_with_cancel / ProductionWorkerRunner::close_host_cancelled_execution` | `crates/freehand-runtime/src/worker_runner.rs / crates/freehand-runtime/src/lifecycle_wait.rs` | run periodic Worker ticks until host cancellation and durably interrupt any still-active owned execution before service exit | runner, interval, and owner-supplied host cancellation token | long-running Worker service or explicit host-cancelled interruption closeout | daemon Slave mode | ProductionWorkerRunner::run_once / ProductionWorkerRunner::close_host_cancelled_execution |  |  |  | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task or resumed controlled task, canonicalize target cwd with ~ expansion and symlink resolution, heartbeat, monitor pause truth, execute, and report | Task Center, Worker identity, and WorkerControl truth | idle, review-ready, interrupted, or blocked outcome without stale paused overwrite | Worker service loop and tests | task owner and live bridge |  |  |  | bound |
| 02p | `ProductionWorkerRunner::current_agent_activity` | `crates/freehand-runtime/src/worker_runner.rs` | read the owning AgentLifecycle snapshot and project Worker activity into a typed control-side activity value without entering UI or ADP payloads | Worker identity plus read-only TaskRuntime lifecycle snapshot | typed Worker activity projection or explicit lifecycle read error | daemon Relay presence source | TaskRuntime::query_agent_lifecycle | agent | runtime_agent_activity | agent.project_runtime_agent_activity | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim the highest-priority Assigned task for Worker | Worker id, execution id, and lease TTL | claimed task plus TaskResumed and heartbeat truth | ProductionWorkerRunner::run_once | task owner |  |  |  | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease and same process-instance heartbeat while provider execution remains active | claimed task, execution, Worker, and process identity | periodic TaskHeartbeat plus agent heartbeat truth or explicit heartbeat error without overwriting external terminal truth | ProductionWorkerRunner::run_once | task owner and agent.lifecycle owner |  |  |  | bound |
| 05a | `WorkerPauseMonitor::start / worker_pause_requested` | `crates/freehand-runtime/src/worker_runner.rs` | poll same-task/same-execution WorkerControl ledger truth and set the live cancel token when latest task-state control is applied pause; after execution, suppress stale review/block/heartbeat overwrite while TaskPaused remains truth | task id, execution id, and persisted WorkerControl events | cooperative live pause at safe point plus idle runner outcome | ProductionWorkerRunner::run_once | task owner and live bridge cancel token |  |  |  | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one Worker task in canonical task cwd with Worker tool policy and path-preflight prompt contract | selected Worker config and live request | closed live reason outcome | ProductionWorkerRunner::run_once | provider/reason live bridge |  |  |  | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready or blocked result for the same execution unless Task Center truth is externally terminal | typed execution fact | terminal task mutation | ProductionWorkerRunner::run_once | task owner |  |  |  | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | ProductionWorkerRunner::run |  |  |  | bound |
| 09 | `ProductionMasterRunner::from_default_config` | `crates/freehand-runtime/src/master_runner.rs` | load selected Master config and bind the Master Task Center namespace | configured agent name | Master lifecycle runner | daemon Master startup | config and runtime owner |  |  |  | bound |
| 10 | `ProductionMasterRunner::run_until` | `crates/freehand-runtime/src/master_runner.rs` | poll Task Center lifecycle events, retry model/provider decision failures with bounded backoff across process restart, and stop on owner-truth failure or daemon shutdown | runner and cancellation signal | long-running Master lifecycle service | daemon Master mode | ProductionMasterRunner::run_once |  |  |  | bound |
| 11 | `ProductionMasterRunner::run_once` | `crates/freehand-runtime/src/master_runner.rs` | repair stale EventInbox cursor if needed, admit EventInbox source order into durable attention before decision execution, dequeue by deterministic weighted aging, preserve retry identity across restart, drain stale no-ops, reconcile decided-blocked and closed parent worksets whose parent turn still waits for lifecycle, then consider due timers | Task Center cursor plus pending attention plus timer store | task decision, blocked parent follow-up, parent-evaluation skip/evaluation, timer-fired, or idle outcome | Master lifecycle service and tests | ProductionMasterRunner::query_event_inbox_repairing_stale_cursor, ProductionMasterRunner::admit_attention_events, highest_priority_attention_index, ProductionMasterRunner::handle_event, ProductionMasterRunner::reconcile_blocked_parent_worksets, ProductionMasterRunner::reconcile_closed_parent_worksets, and ProductionMasterRunner::handle_due_timer |  |  |  | bound |
| 11a | `ProductionMasterRunner::admit_attention_events` | `crates/freehand-runtime/src/master_runner.rs` | persist attention in EventInbox order and advance the cursor without priority reordering, while skipping stale rows whose task no longer exists in current Task Center truth | ordered EventInbox rows and Master loop state | durable pending attention plus monotonic admission sequence or explicit stale-row skip | ProductionMasterRunner::run_once | TaskRuntime::query_task and MasterLoopState |  |  |  | bound |
| 11b | `highest_priority_attention_index` | `crates/freehand-runtime/src/master_runner.rs` | select by severity, bounded task priority, deterministic admission aging, and stable tie-breaks | durable pending attention plus next admission sequence | selected pending attention index | ProductionMasterRunner::run_once | pure score comparator |  |  |  | bound |
| 11c | `register_master_active_work / clear_master_active_work_if_current` | `crates/freehand-runtime/src/master_runner.rs` | persist and clear foreground Master work identity under the master-work lock | live Master submit identity | active-work checkpoint or explicit concurrent-work rejection | runtime live submit dispatcher | active-work JSON and lock file |  |  |  | bound |
| 11d | `ProductionMasterRunner::apply_busy_attention_policy` | `crates/freehand-runtime/src/master_runner.rs` | compare pending attention score with foreground work priority, defer lower-priority attention, and request or enter suspension only at declared safe points | pending attention plus master_work checkpoint | deferred attention, suspend request, or suspended active work | ProductionMasterRunner::run_once | active-work store and weighted attention score |  |  |  | bound |
| 11e | `ProductionMasterRunner::restore_active_work_after_attention` | `crates/freehand-runtime/src/master_runner.rs` | persist typed attention resolution from the event-scoped isolated control decision and restore the exact foreground work identity without copying control transcript text | suspended master_work plus Task Center decision outcome | running active work with typed resolution and original work, session, turn, and trace identity | ProductionMasterRunner::run_once | active-work store | master_work | task | master_work.resolve_attention | bound |
| 11f | `admit_master_attention_resolution_for_next_round` | `crates/freehand-runtime/src/lib.rs` | consume one validated resolution, refresh TaskSpaceSnapshot, and admit volatile/no-cache AttentionResolution before the original foreground work continues | running master_work typed resolution plus current task truth | next-round request-context candidates without stale task or terminal semantics | Master live safe-point continuation paths | context planner candidate admission | master_work | request_context | master_work.admit_resolution_context | bound |
| 11g | `ProductionMasterRunner::current_agent_activity / ProductionMasterRunner::run_until_with_policy / ProductionMasterRunner::record_terminal_failure` | `crates/freehand-runtime/src/master_runner.rs` | own the typed background Master activity lifecycle so actionable task, timer, and parent execution is Running, retry backoff is Waiting, terminal runner failure is Error, and idle, cancelled, or completed work is Idle | Master runner action, retry, cancellation, and terminal result | typed Master runtime activity projection with exact active-session count | daemon Relay presence source and Master runner monitor | runtime activity owner state | agent | runtime_agent_activity | agent.project_runtime_agent_activity | bound |
| 12 | `ProductionMasterRunner::handle_due_timer` | `crates/freehand-runtime/src/master_runner.rs` | resolve persisted timer source ancestry, inject a new wakeup-prompt turn into the original user session or execute a source-less internal turn, and complete, reschedule, or release timer truth | due timer schedule | timer-fired outcome or retryable execution error | ProductionMasterRunner::run_once | timer store and live reason turn | timer | turn | timer.fire_master_wakeup | bound |
| 13 | `TimerStore::claim_due / TimerStore::complete_due / TimerStore::fail_due` | `crates/freehand-runtime/src/timer_store.rs` | persist independent timer schedule state and timer ledger events outside Task Center truth | timer state json and timer ledger | running, completed, or active timer truth | Master timer tool and Master runner | timer store owner |  |  |  | bound |
| 13a | `TimerStore::schedule_from_request` | `crates/freehand-runtime/src/timer_store.rs` | build an independent timer schedule from a validated relative, absolute, interval, daily, weekly, or cron request while preserving the wakeup prompt and optional source session | timer schedule request | active timer schedule with persisted timing/repeat fields | timer tool execution and runtime UI command bridge | timer store owner | timer | timer | timer.schedule | bound |
| 13b | `TimerStore::cancel` | `crates/freehand-runtime/src/timer_store.rs` | mark one independent timer schedule cancelled and append TimerCancelled ledger truth without mutating Task Center state | timer id | cancelled timer schedule plus ledger event | timer tool execution and runtime UI command bridge | timer store owner | timer | timer | timer.cancel | bound |
| 13c | `project_timer_list_for_ui` | `crates/freehand-runtime/src/lib.rs` | project timer schedules and ledger events into UI-safe timer dashboard rows while filtering terminal truth only when the query asks for nonterminal rows | timer schedules and timer ledger events | UiTimerListProjection | RuntimeCommandDispatcher::query_runtime | ui.protocol DTOs | timer | ui_projection | timer.list | bound |
| 14 | `ProductionMasterRunner::handle_event` | `crates/freehand-runtime/src/master_runner.rs` | invoke the Master model for current review-ready, blocked, interrupted, or all-children-closed parent evaluation truth; interrupted decisions receive AgentBoard resource truth, may replace the existing task assignment, and remain retryable after provider/system exit until Task Center truth changes | task snapshot, trigger event, and AgentBoard resource truth | same task advanced, blocked observed, parent evaluated, no-op, or explicit error | ProductionMasterRunner::run_once | run_master_lifecycle_reason_turn, parent evaluation live turn, ReasonPersistence, and task owner |  |  |  | bound |
| 14a | `ProductionMasterRunner::handle_parent_task_closed / ProductionMasterRunner::reconcile_closed_parent_worksets / ProductionMasterRunner::reconcile_blocked_parent_worksets / parent_completed_context_children / parent_latest_external_objective_ordinal_at_or_before / parent_blocked_subtask_truth / parent_blocked_follow_up_live_request / parent_turn_group_key / parent_turns_share_logical_group / parent_logical_turn_waits_for_lifecycle / persisted_parent_evaluation_summary / persisted_parent_blocked_follow_up_summary / raw_parent_blocked_follow_up_summary / next_parent_evaluation_turn_id / parent_user_objectives` | `crates/freehand-runtime/src/master_runner.rs` | decide whether a child task_closed event, an all-closed idle reconciliation, or a decided TaskBlocked workset should resume the current logical parent session; group runtime-turn-N and runtime-turn-N-rM children together while keeping different logical user turns separate; read waiting/idempotency/next-turn state from authoritative effective turn snapshots plus reason-owned raw reserved turn ids without selected-transcript ledger backfill; recover first-round external user objectives from authoritative reason TurnStarted ledger truth across global runtime turn ordinals; widen completed-child prompt context to same-objective prior closed child truth for Master-created next-round evaluation while excluding older user objectives; build either an idempotent overall-goal evaluation request or an idempotent parent-session blocked follow-up request; if a completed blocked marker points only to a raw rolled-back follow-up, clear that stale marker and re-run with a non-colliding next turn id; if historical parent objective truth is missing, record a non-final skipped evaluation for that exact child set | original parent user objective history plus authoritative parent logical-turn/evaluation snapshot truth plus current same-logical-parent-turn evaluation workset plus same-objective prior closed child accepted review truth plus decided blocked child truth plus raw rolled-back follow-up reservation truth | next-round task creation, explicit blocker, verified final completion, skipped non-final historical evaluation, user-visible parent blocked follow-up, a reissued blocked follow-up after rollback invalidation, or no-op without background reason-ledger replay | ProductionMasterRunner::handle_event / ProductionMasterRunner::run_once idle reconciliation | TaskRuntime, ReasonPersistence, and Master live reason turn |  |  |  | bound |
| 15 | `run_master_lifecycle_reason_turn / run_master_lifecycle_reason_turn_with_hooks` | `crates/freehand-runtime/src/lib.rs` | execute one event-isolated lifecycle decision with a target-task boundary, finite round budget, and restart-stable event/attempt turn identity | selected Master config, typed lifecycle prompt, and decision boundary | closed Master turn, target Task Center mutation, and live retry/tool/terminal projection when hooks are supplied | ProductionMasterRunner::handle_event / LiveMasterTurnExecutor | provider and reason live bridge plus runtime UI hook callbacks |  |  |  | bound |
| 15a | `master_session_completion_rejection / master_session_lifecycle_owner_truth` | `crates/freehand-runtime/src/lib.rs` | validate Master completion schema against current same-session Task Center child truth and source timer truth before terminal persistence; reject stale claim=waiting when no owner resource can wake the lifecycle without another user message | parsed CompletionSubmission, parent session id, TaskBoard child statuses, and timer schedules | accepted terminal/waiting decision or schema rejection forcing blocked/user-choice or complete/evidence semantics | run_live_reason_turn_with_policy | TaskRuntime, TimerStore, and completion schema retry loop |  |  |  | bound |
| 16 | `configured_worker_task_boundary_failure` | `crates/freehand-runtime/src/lib.rs` | validate Master task create and assign routing against the configured Worker topology | task tool call and ordered configured Worker id set | explicit topology failure or allowed mutation path | execute_registry_tool_call_with_workspace | pure boundary validator |  |  |  | bound |
| 17 | `execute_registry_tool_call_with_workspace` | `crates/freehand-runtime/src/lib.rs` | enforce configured Worker-set routing before task-tool mutation and route Master timer calls to independent timer truth | Master task or timer call and ordered configured Worker id set | paired failed result or owner-routed task/timer mutation | provider and reason live bridge | configured_worker_task_boundary_failure, task tool owner, and timer store owner |  |  |  | bound |
| 18 | `run_master_mode / master_lifecycle_runner_disabled_for_test` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP host and Master lifecycle runner under one daemon lifetime while sharing the same UiProtocolState between foreground dispatch and background lifecycle execution, except explicit fixture verifier runs may set FREEHAND_TEST_DISABLE_MASTER_LIFECYCLE_RUNNER=1 to keep ADP/WebUI live submit active while preventing background Task Center lifecycle decisions from consuming a temporary provider fixture | Master bootstrap, bind, and explicit test-only lifecycle-disable env | supervised Master daemon with lifecycle runner, or verifier-isolated WebUI/ADP host without background lifecycle runner | daemon CLI | RuntimeCommandDispatcher::from_selected_agent_with_live, RuntimeCommandDispatcher::ui_state, optional ProductionMasterRunner::from_selected_agent_with_ui_state, server host, and optional ProductionMasterRunner::run_until |  |  |  | bound |
| 19 | `ProductionWorkerRunner::record_process_started / ProductionWorkerRunner::record_process_heartbeat_in` | `crates/freehand-runtime/src/worker_runner.rs` | emit one unique process instance at runner construction and refresh it on every poll tick | configured Worker identity, PID, and process-instance id | persisted agent.lifecycle process health | Worker construction and ProductionWorkerRunner::run_once | TaskRuntime::apply_agent_lifecycle_event |  |  |  | bound |
| 20 | `RuntimeCommandDispatcher::new / recover_stale_lifecycle_waits_on_bootstrap / session_has_lifecycle_owner_for_turn / task_can_wake_parent_lifecycle / owner_turn_matches_target` | `crates/freehand-runtime/src/lib.rs` | reconcile live-bootstrap persisted ToolPending user-session turns before UI projection restore; distinguish wakeable task, timer, and live-master owners from stale no-owner waits | runtime home, Master agent id, reason closed-turn truth, TaskBoard, TimerStore, and master_work checkpoint | owner-backed waits preserved as ToolPending, stale no-owner waits durably re-closed as Blocked, or explicit bootstrap error | RuntimeCommandDispatcher::new live bootstrap | ReasonPersistence, TaskRuntime::boot_read_only, TimerStore, and master_runner active-work owner truth |  |  |  | bound |

## Sync Status Against Mainline Call

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master workspace boundary, external-cwd delegation, and path/symlink dispatch guidance are already bound
- independent timer schedule truth, timer_store.rs persistence/recurrence, Master due-timer wakeup routing, and failure release are code-bound
- production Master lifecycle runner, durable cursor, source-ordered attention admission, weighted-aging idle dequeue, retry-preserved attention identity, and supervised Master daemon startup are code-bound
- stale EventInbox cursor repair, missing-task attention skip, and missing-parent-objective non-final ParentEvaluationSkipped are focused-test bound
- busy-Master active-work identity, lower-priority deferral, safe-point high-priority suspension state, typed attention resolution, exact return identity, one-shot live continuation injection, stale tool no-side-effect invalidation, and stale terminal non-persistence are focused-test bound
- background Master lifecycle UI hook publication and query-time ErrorCenter retry recovery are required for lifecycle observability; focused tests prove nonterminal retry projection and terminal non-reactivation before online closeout
- closed-loop lifecycle acceptance is per lifecycle, not per screenshot: TaskRuntime, AgentLifecycle, Master loop state, and reason persistence each need owner truth, next action, observation projection, and a verification entrance
- production Worker runner, pause monitor cancel-token safe-point handling, resumed controlled task selection, rejected-task requeue, Master-owned interrupted recovery with AgentBoard-guided same-task cross-Worker takeover, Worker-specific live tool policy, periodic task and process heartbeat, and Slave daemon startup are code-bound
- config-selected Master guidance, TaskSpaceSnapshot, and task mutation boundary consume the full ordered Worker peer set; singular configured-Worker fields are physically removed
- each Slave Worker runner consumes its one configured Master peer and keeps task claim/execution identity distinct per configured Worker process
- the controlled online verifier starts worker-alpha, worker-beta, and worker-gamma as three distinct daemon processes, proves one initial task per Worker without cross-claim, forces beta reject/rework plus a next-round integration task, and persists JSON proof before explicit PID cleanup
- deterministic positive and negative tests cover Master review close/reject/missing-decision plus Worker idle/review-ready/blocked/retry/missing-workspace/role boundaries
- online daemon/WebUI/Android proof for busy-Master live preemption remains a separate verification gap
- generated wiki must be regenerated whenever this mainline changes
- Worker startup and Task Center boot now recover historical blocked-task paused snapshots without erasing explicit pause truth
- crash/restart lifecycle closure is focused-test bound for admitted-attention pre-decision crash, provider/system executor failure retry across restart, foreground active-work bootstrap recovery, and Worker Running lease expiry followed by Master reassignment
- parent next-round evaluation prompt context is focused-test bound: final synthesis includes same-objective prior accepted child truth while excluding older user-turn child truth
- stale blocked parent completed markers after rollback are covered by production_master_runner_rechecks_stale_blocked_parent_marker_after_rollback
- live bootstrap stale ToolPending recovery is code-bound before UI projection restore and focused-test bound for no-owner close plus wakeable-owner preservation
