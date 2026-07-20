# Test Design: `task.orchestration`

- feature_id: `task.orchestration`
- owner: `crates/freehand-task`
- resource map: `docs/resource-maps/core.json`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `task.create` | bound | `cargo test -p freehand-task create_task_writes_ledger_snapshot_and_recovers_on_boot -- --nocapture` verifies durable task identity and optional parent truth | `cargo test -p freehand-runtime parent_evaluation -- --nocapture` verifies parent-session task discovery remains task-owned | `scripts/verify-master-three-worker-e2e-online.sh` verifies one parent session owns stable child task ids across multiple Worker executions |
| `task.attach_session` | bound | `cargo test -p freehand-task attach_task_to_session_is_idempotent_and_preserves_parent attachment_survives_later_snapshot_without_attachment -- --nocapture` verifies idempotent durable attachment, ledger hydration after a later stale snapshot, and no event-sequence rollback | `cargo test -p freehand-runtime task_tool_query_attaches_existing_task_to_current_visible_session -- --nocapture` verifies explicit current-session query creates owner truth | `make verify-webui-online` verifies selected-session dashboards consume owner-projected attachments without leaking unrelated historical tasks |
| `task.assign` | bound | `cargo test -p freehand-task execution_fact_interrupted_marks_task_retryable_without_blocked_truth -- --nocapture` verifies same task takeover from the interrupted Worker to another available Agent | `cargo test -p freehand-runtime production_master_runner_can_take_over_interrupted_task_to_another_worker -- --nocapture` verifies Master uses AgentBoard resource truth and preserves task/session identity | `scripts/verify-master-three-worker-e2e-online.sh` forces gamma provider retry exhaustion, verifies `TaskInterrupted`, then proves Master replaces the same task assignment with alpha while preserving task and parent session identity |

- lifecycle path under test:
  - task tool action is admitted by runtime tool bridge
  - task owner validates task fields
  - task owner writes ledger and snapshot
  - task owner atomic JSON persistence uses unique temp paths so concurrent
    local Worker process boot/index writes cannot collide on the same temporary
    file
  - shared `leases.json` mutations serialize the complete read-modify-write
    transaction so independent Worker processes cannot lose another task's
    lease or reintroduce a concurrently removed lease
  - task runtime memory state is rebuilt on boot
  - running state is lease-backed and heartbeat-refreshable
  - boot recovery preserves a freshly resumed running task during the bounded
    lease-acquisition window so a concurrent runtime cannot interrupt the task
    between `TaskResumed` persistence and lease persistence
  - boot recovery interrupts running tasks whose lease is still missing after
    the acquisition window or whose lease is expired
  - agent registry persists and recovers worker snapshots
  - blocked task truth releases the Worker resource back to `Available`;
    `AgentStatus::Paused` is reserved for an explicitly paused task
  - boot reconciles legacy paused Worker snapshots against authoritative task
    truth: no assigned `Paused` task means the idle Worker becomes `Available`
  - waiting tasks can be assigned to an available agent
  - interrupted tasks return to schedulable truth and may replace the previous
    assignee with another available Agent while keeping task id and parent
    session id unchanged
  - assigned worker queue can claim the highest-priority task into running state
  - worker claim binds a durable execution id that survives restart and is visible on task snapshot, claim outcome, ledger payload, and UI projection
  - running workers can record execution progress into task ledger truth
  - worker execution facts carry the same execution id through progress, blocked, recovering, review_ready, reject, retry, approve, and close evidence
  - AgentLifecycle snapshots persist separately from releasable AgentSnapshot
    resource state so restart verify can query the last typed lifecycle state
  - task ledger history can be queried as ordered lifecycle events
  - `TaskSessionAttached` is rehydrated from matching ledger events when a later
    heartbeat or execution writer persisted a snapshot without the attachment;
    hydration changes only attachment membership and never rewinds status,
    `last_event_seq`, or `last_event_id`
  - cancellation releases assigned agent state
  - cancellation and other terminal task truth are authoritative across
    multiple `TaskRuntime` instances; an older in-memory runtime must re-read
    persisted task truth before heartbeat or execution-fact mutation
  - stale heartbeat or execution-fact writes after `Cancelled` return explicit
    invalid transition and do not append ledger events, recreate leases, or
    overwrite terminal snapshots
  - query returns persisted task truth
  - list_tasks returns task snapshots for queue and UI projection queries
  - agent registry exposes self agent
  - Phase 1 TaskBoard query projects owner-backed board truth
  - TaskSpaceSnapshot query projects a bounded read-only prompt snapshot without
    boot lease recovery, scheduler fact replay, or EventInbox cursor pagination
- Phase 1 ExecutionFact sync admits typed worker execution facts into Task Center,
  including interrupted system-retry truth that keeps the same task retryable
  - Phase 1 SchedulerTick emits elapsed/stale/timeout facts without business decisions
  - Phase 2B EventInbox projects master-visible ledger events after a globally
    unique cursor
  - EventInbox v2 cursor is a deterministic per-task sequence watermark. Event
    delivery eligibility is `event.seq > watermark[task_id]`; timestamp and
    task-id ordering may arrange one returned batch but never decide whether a
    newly appended event was already consumed
  - legacy timestamp/task/seq cursors migrate conservatively: strictly older
    timestamps and the named task sequence are marked consumed, while other
    same-timestamp tasks replay rather than risk being skipped
  - Phase 2B EventInbox cursor uses event id as a tie-break and legacy
    three-part cursors skip all matching duplicate-prefix events
  - a new lower-sorting task id created in the same second after a cursor must
    still be returned by the v2 per-task watermark; this is the regression lock
    for multi-round parent-evaluation closure
  - Phase 2B master poll reads TaskBoard, AgentBoard, EventInbox, and persisted
    cursor, then classifies state without applying business mutations
  - Phase 2B closeout samples must use `replay_from_start=true` to ignore a
    stale persisted master cursor, then drain all pending EventInbox rows when
    proving same-cursor recovery; finite limits are pagination only and cannot
    prove that no events remain after the persisted cursor

## White-Box Coverage

- create task writes ledger, snapshot, index, and recovers after boot
- atomic JSON persistence survives parallel same-path writers:
  `atomic_json_write_survives_parallel_same_path_writers`
- parallel distinct lease writers preserve every task lease:
  `lease_state_rmw_preserves_parallel_distinct_writers`
- parallel lease refresh/removal keeps refreshed tasks and removes only the
  intended tasks:
  `lease_state_rmw_removes_only_target_during_parallel_refresh`
- create with no dispatch becomes `WaitingAgent`
- interrupted execution stays on the same task and can be assigned to another
  available Agent:
  `execution_fact_interrupted_marks_task_retryable_without_blocked_truth`
- boot registers self agent as `Available`
- review reject/resume/submit/approve/close lifecycle persists and recovers
- close before review approval is rejected
- Phase 2A close rejects blocked and rejected tasks before approved review
- resume creates a task lease and records a heartbeat event
- heartbeat refreshes an active running lease
- boot preserves a freshly resumed running task whose lease write is still
  inside the acquisition grace:
  `boot_preserves_fresh_running_task_during_lease_acquisition_grace`
- boot interrupts a running task whose lease is still missing after the
  acquisition grace:
  `boot_interrupts_running_task_with_missing_lease_after_acquisition_grace`
- boot changes running tasks with expired leases to `Interrupted`
- heartbeat for a non-running task is rejected and writes no lease
- create_agent persists, recovers, and closes an idle agent
- assign moves `WaitingAgent` to `Assigned` and marks the assignee busy with queued work
- claim_next picks the highest-priority assigned task for an agent and creates a running lease
- claim_next requires non-empty execution id, stores it, and recovers it after boot
- claim_next returns no task without mutation when the agent queue is empty
- record_execution writes progress only for `Running` tasks
- record_execution rejects non-running tasks without advancing event sequence
- task_history returns ordered ledger events including execution progress
- task_history for unknown task returns explicit task-not-found
- list_tasks filters by status and assignee
- cancel releases the assignee and prevents later resume
- stale runtime heartbeat after cancel is rejected without terminal overwrite:
  `stale_runtime_heartbeat_after_cancel_is_rejected_without_terminal_overwrite`
- stale runtime execution fact after cancel is rejected without terminal overwrite:
  `stale_runtime_execution_fact_after_cancel_is_rejected_without_terminal_overwrite`
- close_agent rejects busy agents
- blocked execution releases the Worker resource and permits a different task
  assignment
- explicit task pause keeps the Worker `Paused` and rejects unrelated task
  assignment
- boot repairs a legacy paused Worker snapshot left by a blocked task while
  preserving pause when an assigned task is actually `Paused`
- TaskBoard query returns blocked/review/stale filtered views and agent/task binding summaries
- implemented owner test: `task_board_projects_owner_truth_with_filtered_views`
- TaskSpaceSnapshot prompt projection remains bounded and does not replay
  scheduler facts:
  `task_space_snapshot_is_bounded_and_does_not_replay_scheduler_facts`
- ExecutionFact recovering keeps task non-terminal:
  `execution_fact_recovering_keeps_running_and_writes_event`
- ExecutionFact blocked creates a master-visible event:
  `execution_fact_blocked_and_review_ready_update_board_truth`
- ExecutionFact interrupted creates retryable task truth:
  `execution_fact_interrupted_marks_task_retryable_without_blocked_truth`
- ExecutionFact review_ready enters review queue:
  `execution_fact_blocked_and_review_ready_update_board_truth`
- malformed ExecutionFact writes no Task Center truth:
  `execution_fact_validation_failure_writes_no_truth`
- SchedulerTick soft timeout does not fail a task:
  `scheduler_tick_soft_timeout_does_not_fail_task`
- SchedulerTick stale requires no heartbeat/progress past threshold:
  `scheduler_tick_recent_progress_is_not_stale`
- SchedulerTick hard timeout requires master decision and does not automatically fail the task:
  `scheduler_tick_emits_stale_and_timeout_facts_without_decisions`
- SchedulerTick facts are durable/replayable:
  `scheduler_tick_facts_recover_after_boot`
- Phase 2A worker lifecycle:
  `phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id`
- Phase 2A close guard:
  `phase2a_close_requires_approved_review_for_blocked_and_rejected`
- Phase 2B EventInbox projection and cursor recovery:
  `phase2b_event_inbox_projects_events_and_recovers_master_cursor`
- Phase 2B master poll classification without mutation:
  `phase2b_master_poll_classifies_board_and_does_not_mutate_tasks`
- Phase 2B unknown cursor rejection:
  `phase2b_event_inbox_rejects_unknown_cursor_without_advancing_master_cursor`
- Phase 2B cursor uniqueness and legacy compatibility:
  `phase2b_event_cursor_uses_task_sequence_watermark_and_legacy_skips_duplicates`
- Phase 2B backlog pagination regression:
  runtime black-box coverage must create more than 100 master-visible events
  and use omitted limit to prove closeout drains the full backlog before
  persisting the master cursor
- Phase 2B replay cursor regression:
  `phase2b_master_poll_replay_from_start_advances_stale_master_cursor` proves
  replay mode ignores a stale cursor, advances to the latest cursor, and rejects
  replay combined with an explicit cursor without mutating cursor truth

## Module Black-Box Coverage

- runtime task tool create persists a task and returns task id/status plus
  target_cwd path diagnostics for symlink-parent/missing-leaf paths
- runtime task tool query returns snapshot JSON
- runtime task tool list_agents returns self agent
- runtime task tool close before approval returns failure
- runtime task tool review submission, approval, and close return event-backed success
- runtime task tool resume plus heartbeat persists a running lease
- runtime task tool create_agent/assign/cancel/close_agent covers agent lifecycle and busy-close rejection
- runtime/task-owner stale instance protection rejects heartbeat and
  execution-fact writes after terminal owner truth from another runtime
- runtime task tool claim_next returns the highest-priority assigned task and running lease
- runtime task tool record_execution writes semantic worker execution progress
- runtime task tool history returns task ledger timeline JSON
- runtime task tool list_tasks returns filtered task snapshots
- tool registry exposes `task` as one implemented built-in tool schema
- runtime/ADP TaskBoard query returns Task Center board truth without UI-local state
- runtime/ADP ExecutionFact sync returns event-backed Task Center updates
- runtime/ADP SchedulerTick query/sample emits durable facts only
- runtime/ADP Phase 2A command path can create worker agent, assign task, claim with execution id, reject review, retry via execution fact, approve, close, and verify the same ids after restart
- runtime/ADP Phase 2B query path can read EventInbox after a cursor from Task Center truth
- runtime/ADP Phase 2B command path can run master poll, return compact
  classifications, advance the persisted cursor, and verify the same cursor
  after restart without changing task statuses
- runtime/ADP Phase 2B closeout path must use `replay_from_start=true` plus
  omitted limit for full EventInbox/MasterPoll drains; explicit finite limit
  stays a pagination request and is not valid same-cursor closeout evidence

## Project Black-Box Impact

- first slice is runtime/tool/persistence level only
- WebUI/ADP task projection is now owned by `app.webui-smoke`; task.orchestration
  remains the owner truth behind that projection
- Phase 1 headless ADP/CLI proof is required before claiming multi-task foundation closeout
- Phase 2A headless ADP/CLI proof is required before claiming worker execution loop closeout; UI remains out of scope
- Phase 2B headless ADP/CLI proof is required before claiming master poll/EventInbox closeout; UI remains out of scope and framework must not apply business decisions
- Phase 2B headless proof must cover both stale persisted cursor recovery and a
  backlog larger than the old page-size default so closeout cannot pass while
  only processing the first page

## Required Checks

```bash
cargo test -p freehand-task
cargo test -p freehand-task atomic_json_write_survives_parallel_same_path_writers -- --nocapture
cargo test -p freehand-task lease_state_rmw_preserves_parallel_distinct_writers -- --nocapture
cargo test -p freehand-task lease_state_rmw_removes_only_target_during_parallel_refresh -- --nocapture
cargo test -p freehand-task boot_preserves_fresh_running_task_during_lease_acquisition_grace -- --nocapture
cargo test -p freehand-task boot_interrupts_running_task_with_missing_lease_after_acquisition_grace -- --nocapture
cargo test -p freehand-task task_space_snapshot_is_bounded_and_does_not_replay_scheduler_facts -- --nocapture
cargo test -p freehand-tools
cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture
cargo test -p freehand-runtime task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval -- --nocapture
cargo test -p freehand-runtime task_tool_resume_and_heartbeat_persist_running_lease -- --nocapture
cargo test -p freehand-runtime task_tool_agent_assign_cancel_close_lifecycle -- --nocapture
cargo test -p freehand-task stale_runtime_heartbeat_after_cancel_is_rejected_without_terminal_overwrite -- --nocapture
cargo test -p freehand-task stale_runtime_execution_fact_after_cancel_is_rejected_without_terminal_overwrite -- --nocapture
cargo test -p freehand-runtime task_tool_claim_next_runs_highest_priority_task -- --nocapture
cargo test -p freehand-runtime task_tool_record_execution_requires_running_task -- --nocapture
cargo test -p freehand-runtime task_tool_history_returns_ordered_execution_timeline -- --nocapture
cargo test -p freehand-runtime task_tool_list_tasks_filters_queue_projection -- --nocapture
cargo test -p freehand-runtime task_board_query_projects_owner_truth -- --nocapture
cargo test -p freehand-runtime execution_fact_sync_updates_task_center -- --nocapture
cargo test -p freehand-runtime scheduler_tick_emits_facts_without_decisions -- --nocapture
cargo test -p freehand-task phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id -- --nocapture
cargo test -p freehand-task phase2a_close_requires_approved_review_for_blocked_and_rejected -- --nocapture
cargo test -p freehand-task phase2b_event_inbox_projects_events_and_recovers_master_cursor -- --nocapture
cargo test -p freehand-task phase2b_master_poll_classifies_board_and_does_not_mutate_tasks -- --nocapture
cargo test -p freehand-task phase2b_event_inbox_rejects_unknown_cursor_without_advancing_master_cursor -- --nocapture
cargo test -p freehand-task phase2b_master_poll_replay_from_start_advances_stale_master_cursor -- --nocapture
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- Phase 2A real worker execution loop is implemented headlessly and
  live-validated on S-profile `127.0.0.1:4042` with restart same-id proof
- Phase 2B EventInbox/master poll is implemented and live-validated on S-profile
  `127.0.0.1:4042` with same-cursor restart proof
- Phase 2C worker-control is implemented under `worker.control` and
  live-validated with same-id restart proof
- Phase 2D WebUI projection is implemented under `app.webui-smoke`; Android
  true-device proof is separate from this task owner test design
- no production daemon-owned worker queue runner
- no production daemon-owned master scheduler loop
- no non-fixture real-provider behavioral eval for autonomous task dispatch
- TaskBoard owner-internal skeleton is implemented in `crates/freehand-task`
- runtime/ADP TaskBoard surface is implemented for Phase 1/2A headless proof
- ExecutionFact owner-internal sync is implemented in `crates/freehand-task`
- runtime/ADP ExecutionFact surface is implemented for Phase 1/2A headless proof
- SchedulerTick owner-internal facts are implemented in `crates/freehand-task`
- runtime/ADP SchedulerTick sample is implemented for Phase 1 headless proof
