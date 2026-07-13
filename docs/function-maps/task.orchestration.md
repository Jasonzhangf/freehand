# Function Map: `task.orchestration`

- feature_id: `task.orchestration`
- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- owner entry symbols:
  - `TaskRuntime::boot`
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
  - `TaskRuntime::list_tasks`
  - `TaskRuntime::task_history`
  - `TaskRuntime::list_agents`
  - `TaskRuntime::query_agent`
  - `TaskRuntime::query_task_board`
  - `TaskRuntime::query_event_inbox`
  - `TaskRuntime::run_master_poll`
  - `TaskRuntime::apply_execution_fact`
  - `TaskRuntime::run_scheduler_tick`
- mainline call source: `docs/mainline-calls/task.orchestration.json`
- generated wiki: `docs/wiki/task.orchestration.md`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `task.orchestration` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots, self-agent snapshot, and persisted
  agent lifecycle snapshots into memory
- `TaskRuntime::boot` loads task leases, preserves a freshly resumed running
  task during the bounded lease-acquisition window, and interrupts running
  tasks whose lease is still missing after that window or is invalid/expired
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`
- `assign_task` binds waiting/created/interrupted tasks to an available agent
- `claim_next_task` lets an agent claim its highest-priority assigned task into `Running` with a lease and durable `execution_id`
- `record_execution` writes worker execution progress only for running tasks
- `create_agent` and `close_agent` manage persisted worker agent snapshots
- `cancel_task` moves a non-terminal task to `Cancelled` and releases assignee state
- `Blocked` releases the assignee resource to `Available`; `Paused` is reserved
  for explicit task pause truth
- boot reconciles legacy paused agent snapshots against loaded task truth and
  preserves `Paused` only when an assigned task is actually paused
- lifecycle actions use explicit mutation request types and validate allowed transitions before writing ledger/snapshot truth
- `resume_task` enters `Running` and creates a lease-backed heartbeat record
- `heartbeat_task` refreshes the lease for the assigned running agent
- TaskBoard query reads task snapshots, agent registry state, blocked items,
  review queue, and current skeleton stale projection
- ExecutionFact sync admits typed running/recovering/blocked/interrupted/review_ready
  facts into Task Center truth without parsing raw prose
- Phase 2A worker loop keeps `execution_id` attached to claim/start,
  progress, blocked, recovering, review, reject, retry, approve, and close
  evidence so restart verification can query the same execution
- Phase 2A close requires approved review; blocked/rejected tasks cannot be
  closed as a shortcut around review acceptance
- SchedulerTick computes elapsed/stale/soft-timeout/hard-timeout facts without
  making business decisions
- Phase 2B EventInbox projects master-visible task, execution, review, and
  scheduler events from Task Center ledger truth with a globally unique
  cursor shaped as timestamp, task id, sequence, and event id
- Phase 2B EventInbox accepts legacy three-part cursors by skipping all events
  with the matching legacy prefix, so duplicated historical cursor rows do not
  replay as new events
- `replay_from_start=true` makes MasterPoll ignore the persisted cursor and
  rescan EventInbox from the beginning; omitted EventInbox/MasterPoll limit
  means drain all matching rows, while explicit finite limits remain pagination
  only
- Phase 2B master poll loads TaskBoard, AgentBoard, EventInbox cursor, and
  persisted processed cursor, then classifies states without applying business
  mutations

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- `TaskRuntime::list_tasks` returns task snapshot lists filtered by status and assignee
- `TaskRuntime::task_history` returns ordered persisted task ledger events
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- task tool result returns semantic task ids, status, event counts, or JSON snapshots
- review lifecycle actions return event-backed mutation summaries
- heartbeat returns event-backed running-state mutation summary
- heartbeat and execution-fact mutation re-read persisted task truth before
  writing so older `TaskRuntime` instances cannot overwrite terminal owner truth
- claim_next returns either the claimed running task plus `execution_id` or an explicit no-task result
- record_execution returns an event-backed worker progress mutation summary
- agent create/close returns persisted agent snapshot summaries
- TaskBoard query returns board-level task, blocker, review, stale, and agent
  binding summaries
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
- `TaskRuntime::query_task_board`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: project owner-backed TaskBoard truth from task snapshots,
    execution bindings, blockers, review queue, stale facts, and agent registry
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests: `task_board_projects_owner_truth_with_filtered_views`
  - why shared: keeps TaskBoard truth in Task Center instead of UI-local state
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
| 02 | `execute_task_tool` | `crates/freehand-runtime/src/lib.rs` | route task tool calls into task owner with runtime home/session/turn context | task tool call | tool result text | runtime live bridge | task runtime | bound |
| 03 | `TaskRuntime::boot` | `crates/freehand-task/src/lib.rs` | load task and agent snapshots into memory | runtime home + owner agent | ready task runtime | runtime task bridge | task owner | bound |
| 04 | `TaskRuntime::create_task` | `crates/freehand-task/src/lib.rs` | validate, persist, assign/wait, and update memory state | task create request | task snapshot + ledger events | runtime task bridge | task owner | bound |
| 05 | `TaskRuntime::query_task` | `crates/freehand-task/src/lib.rs` | return one task snapshot truth | task id | task snapshot | runtime task bridge | task owner | bound |
| 06 | `TaskRuntime::list_tasks` | `crates/freehand-task/src/lib.rs` | return task snapshots filtered by status and assignee for queue/UI projection | task list query | task snapshots | runtime task bridge | task owner | bound |
| 07 | `TaskRuntime::task_history` | `crates/freehand-task/src/lib.rs` | return ordered task ledger events for timeline/debug projection | task id | task ledger events | runtime task bridge | task owner | bound |
| 08 | `TaskRuntime::list_agents` / `TaskRuntime::query_agent` | `crates/freehand-task/src/lib.rs` | return agent registry truth | agent query | agent snapshots | runtime task bridge | task owner | bound |
| 09 | `TaskRuntime::append_task` / `pause_task` / `resume_task` | `crates/freehand-task/src/lib.rs` | mutate non-review lifecycle states through one transition validator | task mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 10 | `TaskRuntime::submit_review` / `approve_review` / `reject_review` / `close_task` | `crates/freehand-task/src/lib.rs` | enforce review-before-close lifecycle and persist each transition | review mutation request | task snapshot + ledger event | runtime task bridge | task owner | bound |
| 11 | `TaskRuntime::heartbeat_task` | `crates/freehand-task/src/lib.rs` | refresh the lease for an assigned running task and persist a heartbeat event | task heartbeat request | running task snapshot + lease | runtime task bridge | task owner | bound |
| 12 | `reconcile_running_leases` | `crates/freehand-task/src/lib.rs` | preserve fresh lease-acquisition windows, then interrupt running tasks with missing, mismatched, inactive, or expired leases during boot | persisted task snapshots + lease snapshot | recovered runtime state | task boot | task owner | bound |
| 13 | `TaskRuntime::assign_task` | `crates/freehand-task/src/lib.rs` | assign waiting/created/interrupted task to an available agent | task assignment request | assigned task snapshot + agent queued state | runtime task bridge | task owner | bound |
| 14 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | claim the highest-priority assigned task for an agent and enter lease-backed running state | agent task-claim request | claimed running task snapshot or no-task outcome | runtime task bridge | task owner | bound |
| 15 | `TaskRuntime::record_execution` | `crates/freehand-task/src/lib.rs` | append semantic worker execution progress for a running task | worker execution record request | running task snapshot + progress event | runtime task bridge | task owner | bound |
| 16 | `TaskRuntime::cancel_task` | `crates/freehand-task/src/lib.rs` | cancel non-terminal task and release assignee state | task mutation request | cancelled task snapshot + released agent | runtime task bridge | task owner | bound |
| 17 | `TaskRuntime::create_agent` / `close_agent` | `crates/freehand-task/src/lib.rs` | create persisted idle worker agents and close only idle agents | agent mutation request | agent snapshot | runtime task bridge | task owner | bound |
| 18 | `TaskRuntime::query_task_board` | `crates/freehand-task/src/lib.rs` | project TaskBoard truth for master, scheduler, UI, and headless query | task snapshots + execution facts + agent registry | TaskBoard projection | runtime query dispatch | task owner | bound |
| 19 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | admit typed ExecutionFact state into Task Center without raw prose parsing, including interrupted system-retry truth | ExecutionFact | task snapshot + event | Agent Lifecycle sync / runtime | task owner | bound |
| 20 | `TaskRuntime::run_scheduler_tick` | `crates/freehand-task/src/lib.rs` | compute elapsed/stale/timeout facts without business decisions | scheduler tick request + task snapshots | durable scheduler facts | runtime scheduler / CLI sample | task owner | bound |
| 21 | `TaskRuntime::claim_next_task` / `TaskRuntime::apply_execution_fact` / `TaskRuntime::reject_review` / `TaskRuntime::approve_review` / `TaskRuntime::close_task` | `crates/freehand-task/src/lib.rs` | execute Phase 2A worker lifecycle from assigned queue through review rejection, retry, approval, and close | worker claim/execution/review commands | ordered task snapshot and ledger truth with stable execution id | runtime ADP command dispatch / CLI sample | task owner | bound |
| 22 | `TaskStore::write_agent_lifecycle_snapshot` / `TaskStore::load_agent_lifecycle_snapshots` | `crates/freehand-task/src/lib.rs` | persist and restore typed agent lifecycle projection separately from releasable agent resource state | agent lifecycle snapshot | restart-queryable lifecycle truth | task event projection / boot | lifecycle owner storage | bound |
| 23 | `TaskRuntime::query_event_inbox` | `crates/freehand-task/src/lib.rs` | project master-visible event inbox entries from ordered task ledger events after a globally unique cursor, with legacy three-part cursor prefix compatibility | task ledgers + optional cursor | EventInbox projection and next cursor | runtime query dispatch / CLI sample | task owner | bound |
| 24 | `TaskRuntime::run_master_poll` | `crates/freehand-task/src/lib.rs` | load TaskBoard, AgentBoard, EventInbox, classify master-visible states, and persist processed cursor without task business mutations | master poll request + persisted cursor | master poll outcome with classifications and next cursor | runtime ADP command dispatch / CLI sample | task owner | bound |

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
