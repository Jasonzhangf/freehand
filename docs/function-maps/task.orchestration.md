# Function Map: `task.orchestration`

- feature_id: `task.orchestration`
- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- owner entry symbols:
  - `TaskRuntime::boot`
  - `TaskRuntime::boot_master`
  - `TaskRuntime::boot_read_only`
  - `TaskRuntime::create_task`
  - `TaskRuntime::append_task`
  - `TaskRuntime::pause_task`
  - `TaskRuntime::resume_task`
  - `TaskRuntime::heartbeat_task`
  - `TaskRuntime::assign_task`
  - `TaskRuntime::claim_next_task`
  - `TaskRuntime::record_execution`
  - `TaskRuntime::cancel_task`
  - `TaskRuntime::create_agent`
  - `TaskRuntime::close_agent`
  - `TaskRuntime::submit_review`
  - `TaskRuntime::approve_review`
  - `TaskRuntime::reject_review`
  - `TaskRuntime::close_task`
  - `TaskRuntime::query_task`
  - `TaskRuntime::attach_task_to_session`
  - `TaskRuntime::list_tasks`
  - `TaskRuntime::task_history`
  - `TaskRuntime::list_agents`
  - `TaskRuntime::query_agent`
  - `TaskRuntime::query_task_board`
  - `TaskRuntime::query_task_space_snapshot`
  - `TaskRuntime::query_event_inbox`
  - `TaskRuntime::preview_master_poll`
  - `TaskRuntime::run_master_poll`
  - `TaskRuntime::apply_execution_fact`
  - `TaskRuntime::run_scheduler_tick`
- mainline call source: `docs/mainline-calls/task.orchestration.json`
- generated wiki: `docs/wiki/task.orchestration.md`

## Resource Map Binding

- owned resources:
  - `task`
- touched resources:
  - `session`
  - `agent`
- resource operations:
  - `task.create` (`task -> session`): persist the optional parent session id
    on task truth; this does not attach or assign the Agent to the Session
  - `task.attach_session` (`task -> session`): persist an observing session for
    an explicitly queried existing task while preserving its creation parent
    and without granting task mutation authority
  - `task.assign` (`task -> agent`): create or replace the temporary assignment
    while preserving the same task id and parent session id
  - `task.cancel` (`task -> task`): move a non-terminal task to Cancelled and
    release any temporary Agent assignment
- forbidden shortcuts:
  - Session must not own or directly bind Agent lifecycle truth
  - current-session Agent attachment must be derived through Task parent truth
    plus current task assignment/lease/execution
  - interruption/retry/takeover must not create a duplicate Task for the same
    task objective
- resource map: `docs/resource-maps/core.json`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `execute_task_tool` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots, task leases, self-agent snapshot, and persisted agent lifecycle snapshots into memory
- `TaskRuntime::boot_master` runs the master-only recovery path: full lifecycle reconcile plus the stale-blocked sweep that cancels long-blocked worker tasks and releases their lifecycle projections on the same boot; only the master may cancel abandoned worker-blocked tasks because workers boot with the master as task owner
- `TaskRuntime::boot_read_only` loads the same persisted projection without creating a self-agent snapshot and without running lease/lifecycle reconcile writes; ADP query/projection paths use it so read routes cannot mutate Task Center truth
- `TaskRuntime::boot` preserves freshly resumed running tasks during the bounded lease-acquisition window, then interrupts running tasks whose lease remains missing, mismatched, inactive, or expired
- lease-expiry recovery writes `TaskInterrupted` with the old `execution_id` as a fencing token and clears active execution truth so late facts from the stale worker generation are rejected
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots; shared atomic JSON persistence uses per-process unique temp paths so concurrent TaskRuntime boots or index writes do not steal each other's temp file
- task ledger append, task snapshot write, and task index rewrite run inside one `TaskStore::with_task_ledger_lock` critical section; event seq is reallocated from disk ledger truth while locked to prevent duplicate event ids across processes
- lease create, refresh, and removal serialize the complete leases.json read-modify-write transaction through one TaskStore advisory lock; boot recovery removes only invalid task ids instead of replacing concurrent lease truth
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`
- `TaskRuntime::assign_task` binds waiting, created, or interrupted tasks to an available agent
- `TaskRuntime::claim_next_task` lets an agent claim its highest-priority assigned task into lease-backed Running state with a durable execution_id
- `TaskRuntime::record_execution` writes worker progress for running tasks into task ledger truth
- `TaskRuntime::cancel_task` moves non-terminal tasks to Cancelled and releases assignee state
- `TaskRuntime::create_agent` and `TaskRuntime::close_agent` manage persisted worker agent snapshots
- lifecycle actions use explicit task mutation requests and validate state transitions before writing truth
- `TaskRuntime::resume_task` enters `Running` and creates a lease-backed heartbeat record
- `TaskRuntime::heartbeat_task` refreshes the lease for the assigned running agent
- `TaskRuntime` mutation, heartbeat, and execution-fact writes re-read persisted task truth before appending ledger or snapshot state
- Phase 1 TaskBoard query reads task snapshots, agent registry state, blocked items, review queue, and current skeleton stale projection
- `TaskRuntime::query_task_space_snapshot` builds a bounded read-only prompt snapshot from task snapshots, AgentLifecycle projection, and recent master-visible ledger events without running boot lease recovery, scheduler fact replay, or EventInbox cursor pagination
- Phase 1 ExecutionFact sync admits typed running/recovering/blocked/interrupted/review_ready facts into Task Center truth without parsing raw prose
- Phase 1 SchedulerTick computes elapsed/stale/soft-timeout/hard-timeout facts without making business decisions
- Phase 2A worker loop keeps execution_id attached to claim/start, progress, blocked, recovering, review, reject, retry, approve, and close evidence
- Phase 2A close requires approved review; blocked or rejected tasks cannot be closed as a shortcut around review acceptance
- Phase 2B EventInbox projects master-visible task, execution, review, and scheduler events from Task Center ledger truth with a globally unique cursor shaped as timestamp, task id, sequence, and event id
- EventInbox v2 cursor reads only task ledger rows whose per-task seq is above the decoded watermark; legacy cursors still materialize history once for compatibility validation
- Phase 2B EventInbox accepts legacy three-part cursors by skipping all events with the matching legacy prefix, so duplicated historical cursor rows do not replay as new events
- Phase 2B replay_from_start=true makes MasterPoll ignore a stale persisted cursor for closeout and recovery proof; omitted EventInbox/MasterPoll limit drains all pending rows, while explicit finite limits remain pagination only
- Phase 2B master poll loads TaskBoard, AgentBoard, EventInbox cursor, and persisted processed cursor, then classifies states without applying business mutations
- Blocked task truth releases the assigned Worker resource to Available while preserving the blocked task for Master decision
- TaskRuntime boot repairs legacy paused Worker snapshots only when no authoritative assigned task is explicitly Paused

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- explicit query/history from another visible user session persists one
  idempotent `TaskSessionAttached` observation relation; framework-internal
  lifecycle/timer/Worker sessions do not attach
- task snapshot load rehydrates observing-session membership from matching
  `TaskSessionAttached` ledger rows so later heartbeat/execution snapshots
  cannot erase current-session observability; hydration does not change task
  status, `last_event_seq`, or `last_event_id`
- `TaskRuntime::list_tasks` returns task snapshot lists filtered by status and assignee
- `TaskRuntime::task_history` returns ordered persisted task ledger events
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- task tool create result returns semantic task ids, status, event counts, and
  target_cwd path diagnostics when present, including requested/expanded path,
  nearest existing parent, canonical parent, symlink ancestors, and missing
  suffix
- review lifecycle actions return event-backed mutation summaries
- heartbeat returns event-backed running-state mutation summary
- heartbeat and execution-fact mutation re-read persisted task truth before
  writing so older `TaskRuntime` instances cannot overwrite terminal owner truth
- claim_next returns either the claimed running task plus `execution_id` or an explicit no-task result
- record_execution returns an event-backed worker progress mutation summary
- agent create/close returns persisted agent snapshot summaries
- TaskBoard query returns board-level task, blocker, review, stale, and agent
  binding summaries
- TaskSpaceSnapshot query returns actionable tasks first, bounded blocked and
  review-ready ids, AgentLifecycle health, and newest recent events for prompt
  context while leaving full TaskBoard/EventInbox semantics unchanged
- ExecutionFact sync returns event-backed Task Center updates while preserving
  recovering as non-terminal; interrupted execution facts preserve same-task
  retryability instead of creating replacement task truth
- review rejection remains non-terminal task lifecycle truth; a later execution
  fact may resume the rejected task into running retry and submit review again
- SchedulerTick returns durable/replayable fact events and recommendations only
- EventInbox query returns ordered master-visible events after the requested or
  persisted cursor
- MasterPoll returns EventInbox events, TaskBoard, AgentBoard, compact
  classifications, recommended semantic action labels, and the persisted next
  cursor
- MasterPoll closeout advances the cursor only after the requested window is
  read; same-cursor recovery proof must use `replay_from_start=true` plus an
  unlimited drain so stale persisted cursors and paginated backlog cannot hide
  unprocessed events

## Error Mainline

- missing task fields return explicit task errors
- unknown task id returns explicit task-not-found
- unknown agent id returns explicit agent-not-found
- persistence failures return explicit task persistence errors
- lease lock/open/unlock failures return explicit task persistence errors and
  do not pretend the lease mutation succeeded
- invalid lifecycle transitions return explicit `InvalidTransition` errors and do not write ledger/snapshot truth
- heartbeat for non-running or unassigned tasks returns explicit invalid transition and writes no lease
- heartbeat or execution facts from a stale runtime after a terminal mutation
  return explicit invalid transition and write no ledger, lease, or snapshot truth
- assigning to unavailable agents and closing busy agents return explicit errors without mutating task/agent truth
- claiming with an empty agent queue returns no-task without mutating truth
- claiming with an empty execution id returns explicit missing-field and writes no truth
- closing before approved review returns explicit invalid transition and writes no close event
- recording execution for a non-running task returns explicit invalid transition and writes no event
- task failures become failed tool results and can be sent back to the model
- history for unknown task returns explicit task-not-found
- malformed ExecutionFact returns explicit validation error and writes no Task
  Center truth
- SchedulerTick persistence failure returns explicit task runtime error and
  does not pretend stale/timeout facts were admitted
- recovering facts never become task failure
- Phase 2A: schema/tool/execution mismatch is not task failure; only invalid
  owner transition or provider/system failure should fail command dispatch
- Phase 2B: unknown event cursor returns explicit replay/cursor error and does
  not advance the persisted master cursor
- Phase 2B: legacy three-part event cursors match only existing event prefixes;
  unknown legacy prefixes still return explicit cursor-not-found
- Phase 2B: `replay_from_start=true` combined with `after_cursor` returns an
  explicit cursor-mode error and does not advance the persisted master cursor
- Phase 2B: master poll cursor persistence failure returns explicit task
  runtime error and must not pretend events were processed

## Shared Multi-Reference Functions

- `TaskRuntime::boot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: rebuild memory state from persisted task, agent, and lifecycle snapshots
  - allowed callers: runtime task tool bridge, future daemon bootstrap
  - why shared: keeps startup recovery in task owner, not UI/runtime glue
- `TaskRuntime::boot_master`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: master recovery boot that additionally cancels abandoned stale-blocked worker tasks and releases their lifecycle projections
  - allowed callers: master runner `open_task_center`
  - related tests: `boot_promotes_stale_blocked_task_to_cancelled_after_ttl`, `boot_worker_reconcile_preserves_stale_blocked_task_from_other_worker`
  - why shared: keeps the master-only stale-blocked sweep in the task owner so a worker boot (which owns the master as task owner) cannot cancel other workers' blocked tasks
- `write_json_atomic`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: atomically replace task/agent/index/lease JSON truth with unique
    temp paths safe for concurrent local Worker process boot
  - allowed callers: TaskStore persistence helpers
  - related tests: `atomic_json_write_survives_parallel_same_path_writers`
  - why shared: one atomic write owner prevents per-store ad hoc persistence
    and multi-process temp-file collisions
- `TaskStore::with_lease_state_lock`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: serialize the complete shared lease load/mutate/atomic-write
    transaction across independent Worker processes
  - allowed callers: TaskStore lease create/refresh/remove helpers
  - related tests: `lease_state_rmw_preserves_parallel_distinct_writers`,
    `lease_state_rmw_removes_only_target_during_parallel_refresh`
  - why shared: one lock owner prevents per-caller locking gaps and lost lease
    updates
- `TaskRuntime::boot_read_only`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: rebuild persisted Task Center projections without self-agent
    creation, lease recovery, paused-agent repair, released-lifecycle repair, or
    cursor mutation side effects
  - allowed callers: runtime ADP query dispatch, TaskSpaceSnapshot/read-only
    projections, tests
  - related tests: `boot_read_only_does_not_reconcile_running_lease_truth`
  - why shared: keeps query/projection boot side-effect free while preserving
    boot recovery for master/worker runners
- `TaskStore::append_event_and_snapshot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: serialize ledger append, snapshot atomic write, and task index
    rewrite through one task-ledger advisory lock and allocate event seq from
    disk ledger truth while locked
  - allowed callers: TaskRuntime mutation methods, lease recovery, scheduler
    fact writer
  - related tests: `task_ledger_writes_are_serialized_across_processes`,
    `concurrent_runtimes_never_produce_duplicate_event_ids`
  - why shared: one writer boundary prevents cross-process seq collisions and
    partial ledger/snapshot/index updates
- `TaskStore::append_event_and_snapshot_if_status`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: same ledger-locked append as `append_event_and_snapshot`, but
    re-reads the persisted task snapshot under the lock and only appends when
    the disk status still equals the expected status; makes one-time boot
    recovery transitions idempotent across concurrent owner boots
  - allowed callers: boot recovery writers such as `reconcile_stale_blocked`
  - related tests: `boot_master_cancels_stale_blocked_only_once_across_recovery_boots`
  - why shared: prevents duplicate cancellation events and repeated agent
    release when two master processes boot against the same stale task
- `TaskStore::load_master_visible_events_after_watermark`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: load only master-visible task ledger rows whose per-task sequence is
    newer than the v2 EventInbox watermark
  - allowed callers: `TaskRuntime::query_event_inbox`,
    `TaskRuntime::preview_master_poll`, `TaskRuntime::run_master_poll`
  - related tests:
    `phase2b_event_cursor_uses_task_sequence_watermark_and_legacy_skips_duplicates`,
    `phase2b_v2_cursor_delivers_new_lower_task_id_event_with_same_timestamp`
  - why shared: keeps EventInbox delivery truth incremental by per-task ledger
    sequence instead of reparsing all old events for v2 cursors
- `TaskRuntime::query_task_board`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: project owner-backed TaskBoard truth from task snapshots,
    execution bindings, blockers, review queue, stale facts, and agent registry
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests: `task_board_projects_owner_truth_with_filtered_views`
  - why shared: keeps TaskBoard truth in Task Center instead of UI-local state
- `TaskRuntime::query_task_space_snapshot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: build the bounded read-only TaskSpaceSnapshot projection used by
    provider prompt context without invoking boot recovery, scheduler fact
    replay, or EventInbox cursor pagination
  - allowed callers: provider.reason-live-bridge live context builder, tests
  - related tests:
    `task_space_snapshot_is_bounded_and_does_not_replay_scheduler_facts`
  - why shared: keeps prompt task-space projection in Task Center owner truth
    instead of rebuilding full task runtime state from the live bridge
- `TaskRuntime::apply_execution_fact`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: admit typed execution facts into Task Center transition/event truth
  - allowed callers: Agent Lifecycle sync, runtime task bridge, tests
  - related tests:
    `execution_fact_recovering_keeps_running_and_writes_event`,
    `execution_fact_blocked_and_review_ready_update_board_truth`,
    `execution_fact_validation_failure_writes_no_truth`,
    `phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id`
  - why shared: keeps worker execution state changes in Task Center rather than
    scattered runtime/UI logic
- `TaskRuntime::run_scheduler_tick`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: compute elapsed/stale/soft-timeout/hard-timeout facts and wake
    recommendations without making business decisions
  - allowed callers: runtime scheduler, CLI/ADP headless samples, tests
  - related tests:
    `scheduler_tick_emits_stale_and_timeout_facts_without_decisions`,
    `scheduler_tick_soft_timeout_does_not_fail_task`,
    `scheduler_tick_recent_progress_is_not_stale`,
    `scheduler_tick_facts_recover_after_boot`
  - why shared: keeps framework time sensing in one owner-backed task runtime
- `TaskRuntime::query_event_inbox`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: project master-visible event inbox rows from task ledger truth
    after a globally unique cursor, with compatibility for legacy three-part
    cursor prefixes
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests:
    `phase2b_event_inbox_projects_events_and_recovers_master_cursor`
  - why shared: keeps "what happened since last master check" in Task Center
    truth instead of UI/runtime-local polling state
- `TaskRuntime::run_master_poll`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: read TaskBoard, AgentBoard, EventInbox, and persisted cursor to
    classify master-visible state without applying business decisions
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests:
    `phase2b_master_poll_classifies_board_and_does_not_mutate_tasks`
  - why shared: keeps framework sensing and cursor advancement centralized
    while leaving approve/reject/assign/close decisions to explicit task actions

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | expose one `task` tool schema with op-dispatched arguments | static registry truth | provider tool definition | runtime live bridge | tool registry | bound |
| 02 | `execute_task_tool` | `crates/freehand-runtime/src/lib.rs` | route task tool calls into task owner with runtime home/session/turn context and append target_cwd path diagnostics to create results | task tool call | tool result text with task truth and optional path diagnostic | runtime live bridge | task runtime + path diagnostic helper | bound |
| 03 | `TaskRuntime::boot` / `TaskRuntime::boot_master` / `TaskRuntime::boot_read_only` | `crates/freehand-task/src/lib.rs` | load task and agent snapshots; owner runners reconcile lease/lifecycle truth, master recovery additionally sweeps stale-blocked worker tasks, read-only query callers do not write recovery truth | runtime home + owner agent | ready reconciled runtime or side-effect-free projection runtime | runtime task bridge / runtime query dispatch | task owner | bound |
| 04 | `TaskRuntime::create_task` | `crates/freehand-task/src/lib.rs` | validate, persist, attach optional parent session, assign/wait, and update memory state | task create request | task snapshot + ledger events | runtime task bridge | task owner | bound |
| 05 | `TaskRuntime::query_task` | `crates/freehand-task/src/lib.rs` | return one task snapshot truth | task id | task snapshot | runtime task bridge | task owner | bound |
| 06 | `TaskRuntime::list_tasks` | `crates/freehand-task/src/lib.rs` | return task snapshots filtered by status and assignee for queue/UI projection | task list query | task snapshots | runtime task bridge | task owner | bound |
| 07 | `TaskRuntime::task_history` | `crates/freehand-task/src/lib.rs` | return ordered task ledger events for timeline/debug projection | task id | task ledger events | runtime task bridge | task owner | bound |
| 08 | `TaskRuntime::list_agents` / `TaskRuntime::query_agent` | `crates/freehand-task/src/lib.rs` | return agent registry truth | agent query | agent snapshots | runtime task bridge | task owner | bound |
| 09 | `TaskRuntime::append_task` / `pause_task` / `resume_task` | `crates/freehand-task/src/lib.rs` | mutate non-review lifecycle states through one transition validator | task mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 10 | `TaskRuntime::submit_review` / `approve_review` / `reject_review` / `close_task` | `crates/freehand-task/src/lib.rs` | enforce review-before-close lifecycle and persist each transition | review mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 11 | `TaskRuntime::heartbeat_task` | `crates/freehand-task/src/lib.rs` | refresh the lease for an assigned running task and persist a heartbeat event | task heartbeat request | running task snapshot + lease | runtime task bridge | task owner | bound |
| 12 | `reconcile_running_leases` | `crates/freehand-task/src/lib.rs` | preserve fresh lease-acquisition windows, then interrupt running tasks with missing, mismatched, inactive, or expired leases during owner boot and include old `execution_id` as fencing token | persisted task snapshots + lease snapshot | recovered runtime state + fenced `TaskInterrupted` event | task boot | task owner | bound |
| 13 | `TaskRuntime::assign_task` | `crates/freehand-task/src/lib.rs` | assign waiting/created/interrupted task to an available agent, including replacing an interrupted task's previous assignee | task assignment request | same task snapshot + new temporary assignment + agent queued state | runtime task bridge | task owner | bound |
| 14 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | claim the highest-priority assigned task for an agent and enter lease-backed running state | agent task-claim request | claimed running task snapshot or no-task outcome | runtime task bridge | task owner | bound |
| 15 | `TaskRuntime::record_execution` | `crates/freehand-task/src/lib.rs` | append semantic worker execution progress for a running task | worker execution record request | running task snapshot + progress event | runtime task bridge | task owner | bound |
| 16 | `TaskRuntime::cancel_task` | `crates/freehand-task/src/lib.rs` | cancel non-terminal task and release assignee state for `task.cancel` | task mutation request | cancelled task snapshot + released agent | runtime task bridge | task owner | bound |
| 17 | `TaskRuntime::create_agent` / `close_agent` | `crates/freehand-task/src/lib.rs` | create persisted idle worker agents and close only idle agents | agent mutation request | agent snapshot | runtime task bridge | task owner | bound |
| 18 | `TaskRuntime::query_task_board` | `crates/freehand-task/src/lib.rs` | project TaskBoard truth for master, scheduler, UI, and headless query | task snapshots + execution facts + agent registry | TaskBoard projection | runtime query dispatch | task owner | bound |
| 19 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | admit typed ExecutionFact state into Task Center without raw prose parsing, including failed terminal truth and active-execution fencing rejection | ExecutionFact | task snapshot + event or explicit stale-generation rejection | Agent Lifecycle sync / runtime | task owner | bound |
| 20 | `TaskRuntime::run_scheduler_tick` | `crates/freehand-task/src/lib.rs` | compute elapsed/stale/timeout facts without business decisions | scheduler tick request + task snapshots | durable scheduler facts | runtime scheduler / CLI sample | task owner | bound |
| 21 | `TaskRuntime::claim_next_task` / `TaskRuntime::apply_execution_fact` / `TaskRuntime::reject_review` / `TaskRuntime::approve_review` / `TaskRuntime::close_task` | `crates/freehand-task/src/lib.rs` | execute Phase 2A worker lifecycle from assigned queue through review rejection, retry, approval, and close | worker claim/execution/review commands | ordered task snapshot and ledger truth with stable execution id | runtime ADP command dispatch / CLI sample | task owner | bound |
| 22 | `TaskStore::write_agent_lifecycle_snapshot` / `TaskStore::load_agent_lifecycle_snapshots` | `crates/freehand-task/src/lib.rs` | persist and restore typed agent lifecycle projection separately from releasable agent resource state | agent lifecycle snapshot | restart-queryable lifecycle truth | task event projection / boot | lifecycle owner storage | bound |
| 23 | `TaskRuntime::query_event_inbox` | `crates/freehand-task/src/lib.rs` | project master-visible event inbox entries from task ledger events after a v2 per-task sequence watermark, with legacy cursor compatibility validation | task ledgers + optional v2/legacy cursor | EventInbox projection and next cursor | runtime query dispatch / CLI sample | task owner | bound |
| 24 | `TaskRuntime::run_master_poll` / `TaskRuntime::preview_master_poll` | `crates/freehand-task/src/lib.rs` | load TaskBoard, AgentBoard, EventInbox, classify master-visible states, and either persist processed cursor for command mode or leave cursor truth untouched for query preview | master poll request + persisted cursor | master poll outcome with classifications and next cursor | runtime ADP command/query dispatch / CLI sample | task owner | bound |
| 25 | `write_json_atomic` | `crates/freehand-task/src/lib.rs` | atomically replace JSON persistence files with process/nanos/counter-qualified temp paths | serializable task owner truth | replaced JSON truth without cross-writer temp collisions | TaskStore persistence helpers | filesystem atomic rename | bound |
| 26 | `TaskStore::with_lease_state_lock` | `crates/freehand-task/src/lib.rs` | serialize shared lease read-modify-write mutation across independent Worker processes | lease create/refresh/remove mutation | complete `leases.json` truth without lost updates or stale reintroduction | TaskStore lease helpers | filesystem advisory lock + atomic rename | bound |
| 27 | `TaskStore::append_event_and_snapshot` / `TaskStore::append_event_and_snapshot_if_status` | `crates/freehand-task/src/lib.rs` | lock ledger mutation, assign disk-based seq, append ledger row, atomically write task snapshot, and rewrite task index as one cross-process critical section; the `if_status` variant re-reads disk status under the lock and skips when it no longer matches (idempotent one-time recovery) | task snapshot + pending ledger event + optional expected status | durable unique-seq ledger/snapshot/index truth, or skip when status advanced | TaskRuntime mutation and recovery writers | filesystem advisory lock + task store atomic writers | bound |
| 28 | `TaskRuntime::query_task_space_snapshot` | `crates/freehand-task/src/lib.rs` | project a bounded read-only task-space snapshot for provider prompt context without boot recovery, scheduler fact replay, or EventInbox cursor pagination | runtime home + owner agent + task/event limits | bounded TaskSpaceSnapshotProjection with tasks, blocked/review-ready queues, AgentLifecycle health, and newest master-visible events | provider.reason-live-bridge live context builder | task owner | bound |
| 29 | `reconcile_stale_blocked` | `crates/freehand-task/src/lib.rs` | promote a Blocked task that stays Blocked beyond `STALE_BLOCKED_TTL_SECONDS` (7 days) with no fresh progress to Cancelled during master-only recovery boot, clearing `active_execution_id` and releasing the assigned agent; uses `append_event_and_snapshot_if_status` so concurrent master boots append exactly one `TaskCancelled` event | persisted task snapshots + agent snapshots | recovered runtime state + one `TaskCancelled` event with `reason=stale_blocked_ttl` + released agent | task boot ReconcileMaster block | task owner | bound |

## Sync Status Against Code

- first implementation supports `create`, `query`, `list_agents`, and `query_agent`
- current implementation also supports `append`, `pause`, `resume`, `heartbeat`, `assign`, `claim_next`, `record_execution`, `history`, `list_tasks`, `cancel`, `submit_review`, `approve`, `reject`, `close`, `create_agent`, and `close_agent`
- Phase 1 TaskBoard owner-internal skeleton is implemented
- Phase 1 ExecutionFact owner-internal sync is implemented
- Phase 1 SchedulerTick owner-internal facts are implemented
- Phase 2A real worker execution loop is implemented through headless ADP/CLI
- Phase 2B EventInbox and master poll owner surfaces are implemented and
  S-profile verified with same-cursor restart proof
- Phase 2C worker-control truth is implemented under `worker.control` and
  S-profile verified with same-id restart proof
- Phase 2D WebUI task/agent/control projection is implemented through
  `app.webui-smoke`; Android true-device proof is separate and not implied here
- remaining gap is production non-smoke master/worker orchestration: daemon-owned
  scheduler/worker runner activation, configured worker resource recycling, and
  non-fixture real-provider behavioral evaluation

- read-only boot is implemented for query/projection callers:
  `boot_read_only_does_not_reconcile_running_lease_truth`

- cross-process task ledger serialization is implemented and proven by
  `task_ledger_writes_are_serialized_across_processes`

- stale execution generation fencing is implemented and proven by
  `stale_execution_fact_after_interrupted_fencing_is_rejected`

- ExecutionFact failed terminalization is implemented and proven by
  `execution_fact_failed_marks_task_terminal_and_releases_agent`
