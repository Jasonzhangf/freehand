# Wiki: `runtime.master-worker-loop`

Generated from `docs/mainline-calls/runtime.master-worker-loop.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/runtime.master-worker-loop.md`
- generated wiki: `docs/wiki/runtime.master-worker-loop.md`
- test design: `docs/testing/runtime.master-worker-loop.md`

## Resource Operation Backlinks

- timer.fire_master_wakeup
- master_work.resolve_attention
- master_work.admit_resolution_context
- agent.heartbeat

## Request Mainline

- one daemon process selects one configured agent
- Master mode starts the WebUI/ADP host plus a background lifecycle runner over Task Center EventInbox truth
- the Master lifecycle runner admits events after its durable cursor into pending_attention in EventInbox source order, then invokes the Master model for the dequeued current review-ready, blocked, interrupted, or all-current-children-closed parent-evaluation truth
- Task Center lifecycle attention has priority over due internal timers; due timers are claimed only when pending_attention is empty
- pending_attention dequeue uses deterministic weighted aging: blocked showstoppers and critical/high-priority work carry large score weight, while admission-sequence aging prevents starvation without wall-clock timing
- foreground Master user work is persisted as master_work with active identity, priority, safe point, suspension state, and exact return identity; lower-priority attention stays queued while higher-priority attention can only suspend at a declared safe point
- while foreground work is SuspendedByAttention, the selected attention decision runs as a task-scoped master-lifecycle control request with event/attempt-isolated session, turn, and trace ids distinct from the suspended foreground identity; executor prose or raw control transcript text is not copied into the foreground session or typed AttentionResolution
- after a restored high-priority attention resolution, the original foreground live turn consumes the typed resolution once, refreshes TaskSpaceSnapshot, admits volatile/no-cache AttentionResolution request context, invalidates stale tool or terminal candidates, and continues the same logical turn
- each actionable event attempt uses a task-scoped internal master-lifecycle reason session, deterministic event-and-attempt-isolated turn and trace ids, and an explicit target-task decision boundary
- when a child task_closed event arrives, the runner checks all terminal-included child tasks with the same parent_session_id and starts an overall-goal evaluation turn in the original parent session only after every current child is Closed; the evaluation receives original user objective history, decomposed child task requirements, and accepted review truth
- Slave mode constructs a production Worker runner instead of a Master UI dispatcher
- each Worker opens its only configured Master's Task Center namespace and uses its own distinct configured agent id as execution identity
- Worker construction records one typed process-start event; every idle or active poll tick and the active task-heartbeat loop refresh the same process instance through agent.lifecycle
- each Worker tick claims the highest-priority Assigned task for that Worker
- claim persists one execution id and lease heartbeat
- task target cwd expands a leading ~, canonicalizes through symlinks, and becomes the locked Worker execution root
- Worker live reasoning receives the requested target_cwd, canonical locked workspace, and path-preflight instructions for absolute paths, ~, and symlinks
- Worker provider requests expose governed workspace tools but exclude recursive `task` and unrestricted shell tools
- Master provider guidance binds dispatch to the ordered configured Worker id set, excludes historical AgentBoard entries as production targets, forbids task lifecycle calls in Worker task content, and requires path/symlink/canonical evidence for external cwd delegation
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
- stale or already-satisfied pending attention items are removed and dequeue continues in the same runner tick
- a resolved busy-Master attention stores typed decision kind and changed task ids in master_work and restores the original work_id, session_id, logical turn id, and trace id without raw Worker/control transcripts, control-turn summaries, or provider payloads
- a resumed foreground Master provider request contains refreshed TaskSpaceSnapshot plus typed AttentionResolution while preserving the original session, logical turn, and trace identity
- approved review-ready truth remains retryable until Master closes the task
- parent evaluation truth is durable-idempotent by parent session plus closed child task set, so EventInbox replay or daemon restart cannot repeat the same decision or duplicate next-round task creation; successful, waiting, and blocked evaluations are durable while failed/interrupted/cancelled evaluations remain retryable
- parent evaluation reads only the first-round persisted user objective from authoritative reason truth; UI-coalesced repair/control rounds cannot replace the goal
- blocked truth remains retryable until Master reassigns it or persists a blocked-decision note through task(op=append)
- after repeated provider/executor failures while deciding an already blocked task, the runner appends an explicit Master-owned blocked_decision and continues other pending lifecycle events without marking the task successful
- interrupted truth remains retryable until it leaves Interrupted
- interrupted tasks remain unchanged until Master explicitly reassigns the same task to the selected configured Worker; rejected tasks are requeued to their previously assigned Worker because Master already made the rework decision
- blocked tasks remain explicit Master decisions and are never silently retried by the Worker
- successful Worker completion writes one review-ready execution fact
- provider/network system failure after internal provider retries writes one interrupted execution fact for same-task retry; non-provider task execution failure writes one blocked execution fact
- Worker reason/session truth persists under Worker agent identity
- task/execution/lease/agent truth persists under the paired Master's Task Center namespace
- AgentBoard and AgentLifecycle return Worker PID, process instance, heartbeat timestamp, restart count, and TTL-derived alive from the lifecycle owner
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
- interrupted Master decisions receive current AgentBoard resource truth and may replace the same task assignment from one configured Worker to another without changing task_id or parent_session_id
- Agent is a reusable execution resource independent of Session; current session attachment is derived only through task parent plus assignment, lease, and execution truth
- unrelated task mutation cannot satisfy the current event decision boundary
- assignment to a historical or non-configured Worker returns a paired failed tool result and writes no TaskAssigned event
- task creation with omitted, auto, self, or non-configured-agent dispatch returns a paired failed tool result and writes no task truth
- lifecycle decision round-budget exhaustion is explicit blocked reason truth rather than an indefinitely active turn
- timer wakeup executor failure records timer failure truth, releases the schedule back to active retryable state, and surfaces a retryable Master execution error
- timer wakeup executor failure must not prevent already-pending Task Center lifecycle events from being processed first
- missing or mismatched active-work checkpoint blocks busy-Master restoration explicitly rather than continuing a stale foreground turn
- repeated blocked-decision provider failure is converted into an explicit TaskProgressed blocked decision after the retry cap so one blocked task cannot permanently starve later lifecycle events
- Task Center and lifecycle-state failures stop the Master runner while lifecycle executor and missing-decision failures remain retryable
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
  - allowed callers: run_live_reason_turn, run_worker_live_reason_turn, runtime tests
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

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `ProductionWorkerRunner::from_default_config` | `crates/freehand-runtime/src/worker_runner.rs` | load selected Slave config and bind paired Master Task Center namespace | configured agent name | Worker runner | daemon Slave startup | config and runtime owner |  |  |  | bound |
| 02 | `ProductionWorkerRunner::run` | `crates/freehand-runtime/src/worker_runner.rs` | run periodic Worker ticks with explicit cadence | runner and interval | long-running Worker service | daemon Slave mode | ProductionWorkerRunner::run_once |  |  |  | bound |
| 03 | `ProductionWorkerRunner::run_once` | `crates/freehand-runtime/src/worker_runner.rs` | claim one Assigned task, canonicalize target cwd with ~ expansion and symlink resolution, heartbeat, execute, and report | Task Center and Worker identity | idle, review-ready, or blocked outcome | Worker service loop and tests | task owner and live bridge |  |  |  | bound |
| 04 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | choose and claim the highest-priority Assigned task for Worker | Worker id, execution id, and lease TTL | claimed task plus TaskResumed and heartbeat truth | ProductionWorkerRunner::run_once | task owner |  |  |  | bound |
| 05 | `WorkerHeartbeat::start` | `crates/freehand-runtime/src/worker_runner/heartbeat.rs` | renew the claimed task lease and same process-instance heartbeat while provider execution remains active | claimed task, execution, Worker, and process identity | periodic TaskHeartbeat plus agent heartbeat truth or explicit heartbeat error without overwriting external terminal truth | ProductionWorkerRunner::run_once | task owner and agent.lifecycle owner |  |  |  | bound |
| 06 | `run_worker_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one Worker task in canonical task cwd with Worker tool policy and path-preflight prompt contract | selected Worker config and live request | closed live reason outcome | ProductionWorkerRunner::run_once | provider/reason live bridge |  |  |  | bound |
| 07 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | persist review-ready or blocked result for the same execution unless Task Center truth is externally terminal | typed execution fact | terminal task mutation | ProductionWorkerRunner::run_once | task owner |  |  |  | bound |
| 08 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | select Slave host path without constructing Master UI dispatcher | daemon agent selection | Worker service process | daemon CLI | ProductionWorkerRunner::run |  |  |  | bound |
| 09 | `ProductionMasterRunner::from_default_config` | `crates/freehand-runtime/src/master_runner.rs` | load selected Master config and bind the Master Task Center namespace | configured agent name | Master lifecycle runner | daemon Master startup | config and runtime owner |  |  |  | bound |
| 10 | `ProductionMasterRunner::run_until` | `crates/freehand-runtime/src/master_runner.rs` | poll Task Center lifecycle events, retry model/provider decision failures with bounded backoff, and stop on owner-truth failure or daemon shutdown | runner and cancellation signal | long-running Master lifecycle service | daemon Master mode | ProductionMasterRunner::run_once |  |  |  | bound |
| 11 | `ProductionMasterRunner::run_once` | `crates/freehand-runtime/src/master_runner.rs` | admit Task Center events in source order, dequeue pending attention by deterministic weighted aging, preserve retry identity, drain stale no-ops, then run due timer wakeups only when attention is empty | Task Center cursor plus pending attention plus timer store | task decision, timer-fired, or idle outcome | Master lifecycle service and tests | TaskRuntime::query_event_inbox, ProductionMasterRunner::admit_attention_events, highest_priority_attention_index, ProductionMasterRunner::handle_event, and ProductionMasterRunner::handle_due_timer |  |  |  | bound |
| 11a | `ProductionMasterRunner::admit_attention_events` | `crates/freehand-runtime/src/master_runner.rs` | persist attention in EventInbox order and advance the cursor without priority reordering | ordered EventInbox rows and Master loop state | durable pending attention plus monotonic admission sequence | ProductionMasterRunner::run_once | TaskRuntime::query_task and MasterLoopState |  |  |  | bound |
| 11b | `highest_priority_attention_index` | `crates/freehand-runtime/src/master_runner.rs` | select by severity, bounded task priority, deterministic admission aging, and stable tie-breaks | durable pending attention plus next admission sequence | selected pending attention index | ProductionMasterRunner::run_once | pure score comparator |  |  |  | bound |
| 11c | `register_master_active_work / clear_master_active_work_if_current` | `crates/freehand-runtime/src/master_runner.rs` | persist and clear foreground Master work identity under the master-work lock | live Master submit identity | active-work checkpoint or explicit concurrent-work rejection | runtime live submit dispatcher | active-work JSON and lock file |  |  |  | bound |
| 11d | `ProductionMasterRunner::apply_busy_attention_policy` | `crates/freehand-runtime/src/master_runner.rs` | compare pending attention score with foreground work priority, defer lower-priority attention, and request or enter suspension only at declared safe points | pending attention plus master_work checkpoint | deferred attention, suspend request, or suspended active work | ProductionMasterRunner::run_once | active-work store and weighted attention score |  |  |  | bound |
| 11e | `ProductionMasterRunner::restore_active_work_after_attention` | `crates/freehand-runtime/src/master_runner.rs` | persist typed attention resolution from the event-scoped isolated control decision and restore the exact foreground work identity without copying control transcript text | suspended master_work plus Task Center decision outcome | running active work with typed resolution and original work, session, turn, and trace identity | ProductionMasterRunner::run_once | active-work store | master_work | task | master_work.resolve_attention | bound |
| 11f | `admit_master_attention_resolution_for_next_round` | `crates/freehand-runtime/src/lib.rs` | consume one validated resolution, refresh TaskSpaceSnapshot, and admit volatile/no-cache AttentionResolution before the original foreground work continues | running master_work typed resolution plus current task truth | next-round request-context candidates without stale task or terminal semantics | Master live safe-point continuation paths | context planner candidate admission | master_work | request_context | master_work.admit_resolution_context | bound |
| 12 | `ProductionMasterRunner::handle_due_timer` | `crates/freehand-runtime/src/master_runner.rs` | resolve persisted timer source ancestry, inject a new wakeup-prompt turn into the original user session or execute a source-less internal turn, and complete, reschedule, or release timer truth | due timer schedule | timer-fired outcome or retryable execution error | ProductionMasterRunner::run_once | timer store and live reason turn | timer | turn | timer.fire_master_wakeup | bound |
| 13 | `TimerStore::claim_due / TimerStore::complete_due / TimerStore::fail_due` | `crates/freehand-runtime/src/lib.rs` | persist independent timer schedule state and timer ledger events outside Task Center truth | timer state json and timer ledger | running, completed, or active timer truth | Master timer tool and Master runner | timer store owner |  |  |  | bound |
| 14 | `ProductionMasterRunner::handle_event` | `crates/freehand-runtime/src/master_runner.rs` | invoke the Master model for current review-ready, blocked, interrupted, or all-children-closed parent evaluation truth; interrupted decisions receive AgentBoard truth and may replace the existing assignment | task snapshot, trigger event, and AgentBoard resource truth | same task advanced, blocked observed, parent evaluated, no-op, or explicit error | ProductionMasterRunner::run_once | run_master_lifecycle_reason_turn, parent evaluation live turn, ReasonPersistence, and task owner |  |  |  | bound |
| 14a | `ProductionMasterRunner::handle_parent_task_closed / parent_user_objectives` | `crates/freehand-runtime/src/master_runner.rs` | decide whether a child task_closed event completes the current parent child set, recover only the original first-round user objective from authoritative reason truth, and build an idempotent overall-goal evaluation request | original parent user objective history plus closed child task definitions and accepted review truth | next-round task creation, explicit blocker, verified final completion, or no-op | ProductionMasterRunner::handle_event | TaskRuntime, ReasonPersistence, and Master live reason turn |  |  |  | bound |
| 15 | `run_master_lifecycle_reason_turn` | `crates/freehand-runtime/src/lib.rs` | execute one event-isolated lifecycle decision with a target-task boundary and finite round budget | selected Master config, typed lifecycle prompt, and decision boundary | closed Master turn and target Task Center mutation | ProductionMasterRunner::handle_event | provider and reason live bridge |  |  |  | bound |
| 16 | `configured_worker_task_boundary_failure` | `crates/freehand-runtime/src/lib.rs` | validate Master task create and assign routing against the configured Worker topology | task tool call and ordered configured Worker id set | explicit topology failure or allowed mutation path | execute_registry_tool_call_with_workspace | pure boundary validator |  |  |  | bound |
| 17 | `execute_registry_tool_call_with_workspace` | `crates/freehand-runtime/src/lib.rs` | enforce configured Worker-set routing before task-tool mutation and route Master timer calls to independent timer truth | Master task or timer call and ordered configured Worker id set | paired failed result or owner-routed task/timer mutation | provider and reason live bridge | configured_worker_task_boundary_failure, task tool owner, and timer store owner |  |  |  | bound |
| 18 | `run_master_mode` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP host and Master lifecycle runner under one daemon lifetime | Master bootstrap and bind | supervised Master daemon | daemon CLI | server host and ProductionMasterRunner::run_until |  |  |  | bound |
| 19 | `ProductionWorkerRunner::record_process_started / ProductionWorkerRunner::record_process_heartbeat_in` | `crates/freehand-runtime/src/worker_runner.rs` | emit one unique process instance at runner construction and refresh it on every poll tick | configured Worker identity, PID, and process-instance id | persisted agent.lifecycle process health | Worker construction and ProductionWorkerRunner::run_once | TaskRuntime::apply_agent_lifecycle_event |  |  |  | bound |

## Sync Status Against Mainline Call

- Task Center claim, heartbeat, execution fact, persistence, and recovery APIs are already bound
- Master workspace boundary, external-cwd delegation, and path/symlink dispatch guidance are already bound
- independent timer schedule truth, Master due-timer wakeup routing, and failure release are code-bound
- production Master lifecycle runner, durable cursor, source-ordered attention admission, weighted-aging idle dequeue, retry-preserved attention identity, and supervised Master daemon startup are code-bound
- busy-Master active-work identity, lower-priority deferral, safe-point high-priority suspension state, typed attention resolution, exact return identity, one-shot live continuation injection, stale tool no-side-effect invalidation, and stale terminal non-persistence are focused-test bound
- production Worker runner, rejected-task requeue, Master-owned interrupted recovery with AgentBoard-guided same-task cross-Worker takeover, Worker-specific live tool policy, periodic task and process heartbeat, and Slave daemon startup are code-bound
- config-selected Master guidance, TaskSpaceSnapshot, and task mutation boundary consume the full ordered Worker peer set; singular configured-Worker fields are physically removed
- each Slave Worker runner consumes its one configured Master peer and keeps task claim/execution identity distinct per configured Worker process
- the controlled online verifier starts worker-alpha, worker-beta, and worker-gamma as three distinct daemon processes, proves one initial task per Worker without cross-claim, forces beta reject/rework plus a next-round integration task, and persists JSON proof before explicit PID cleanup
- deterministic positive and negative tests cover Master review close/reject/missing-decision plus Worker idle/review-ready/blocked/retry/missing-workspace/role boundaries
- online daemon/WebUI/Android proof for busy-Master live preemption remains a separate verification gap
- generated wiki must be regenerated whenever this mainline changes
- Worker startup and Task Center boot now recover historical blocked-task paused snapshots without erasing explicit pause truth
