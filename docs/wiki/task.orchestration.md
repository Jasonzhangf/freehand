# Wiki: `task.orchestration`

Generated from `docs/mainline-calls/task.orchestration.json`. Do not edit by hand.

- owner crate: `crates/freehand-task`
- owner module: `crates/freehand-task/src/lib.rs`
- function map: `docs/function-maps/task.orchestration.md`
- generated wiki: `docs/wiki/task.orchestration.md`
- test design: `docs/testing/task.orchestration.md`

## Request Mainline

- runtime receives a provider tool call named `task`
- runtime routes `task` tool calls to `execute_task_tool` instead of generic file/tool execution
- `TaskRuntime::boot` loads task snapshots, task leases, and self-agent snapshot into memory
- `TaskRuntime::boot` interrupts running tasks whose lease is missing, mismatched, inactive, or expired
- `TaskRuntime::create_task` validates required task content, goal, deliverables, and acceptance
- create action writes append-only ledger events and atomic snapshots
- dispatch mode can assign the self/available agent or leave the task in `WaitingAgent`
- `TaskRuntime::assign_task` binds waiting, created, or interrupted tasks to an available agent
- `TaskRuntime::claim_next_task` lets an agent claim its highest-priority assigned task into lease-backed Running state
- `TaskRuntime::record_execution` writes worker progress for running tasks into task ledger truth
- `TaskRuntime::cancel_task` moves non-terminal tasks to Cancelled and releases assignee state
- `TaskRuntime::create_agent` and `TaskRuntime::close_agent` manage persisted worker agent snapshots
- lifecycle actions use explicit task mutation requests and validate state transitions before writing truth
- `TaskRuntime::resume_task` enters `Running` and creates a lease-backed heartbeat record
- `TaskRuntime::heartbeat_task` refreshes the lease for the assigned running agent
- Phase 1 TaskBoard query reads task snapshots, agent registry state, blocked items, review queue, and current skeleton stale projection
- Phase 1 ExecutionFact sync admits typed running/recovering/blocked/review_ready facts into Task Center truth without parsing raw prose
- Phase 1 SchedulerTick computes elapsed/stale/soft-timeout/hard-timeout facts without making business decisions

## Response Mainline

- `TaskRuntime::query_task` returns persisted task snapshot truth
- `TaskRuntime::list_tasks` returns task snapshots filtered by status and assignee
- `TaskRuntime::task_history` returns ordered persisted task ledger events
- `TaskRuntime::list_agents` returns current in-memory agent registry projection
- `TaskRuntime::query_agent` returns one agent snapshot
- append, pause, resume, heartbeat, assign, cancel, submit_review, approve, reject, and close return event-backed mutation results
- claim_next returns either the claimed running task or an explicit no-task result
- record_execution returns an event-backed worker progress mutation summary
- create_agent and close_agent return persisted agent snapshot summaries
- task tool result returns semantic task ids, status, event names, sequence numbers, or JSON snapshots
- Phase 1 TaskBoard query returns board-level task, blocker, review, stale, and agent binding summaries
- Phase 1 ExecutionFact sync returns event-backed Task Center updates while preserving recovering as non-terminal
- Phase 1 SchedulerTick returns durable/replayable fact events and recommendations only

## Error Mainline

- missing task fields return explicit task errors
- unknown task id returns explicit task-not-found
- unknown agent id returns explicit agent-not-found
- invalid lifecycle transitions return explicit invalid-transition errors
- heartbeat for non-running or unassigned tasks returns invalid-transition and writes no lease
- assigning to unavailable agents and closing busy agents return explicit errors without mutating task or agent truth
- claiming with an empty agent queue returns no-task without mutating truth
- recording execution for a non-running task returns invalid-transition and writes no event
- history for unknown task returns explicit task-not-found
- persistence failures return explicit task persistence errors
- task failures become failed tool results and can be sent back to the model
- malformed ExecutionFact returns explicit validation error and writes no Task Center truth
- SchedulerTick persistence failure returns explicit task runtime error and does not pretend stale/timeout facts were admitted
- pending Phase 1: recovering facts never become task failure

## Shared Multi-Reference Functions

- `TaskRuntime::boot`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: rebuild memory state from persisted task and agent snapshots
  - allowed callers: runtime task tool bridge, future daemon bootstrap
  - related tests: create_task_writes_ledger_snapshot_and_recovers_on_boot, boot_interrupts_running_task_with_expired_lease, task_tool_create_persists_and_queries_task
  - why shared: keeps startup recovery in task owner, not UI/runtime glue
- `TaskRuntime::mutate_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: validate lifecycle transitions, write ledger and snapshot, and update memory state
  - allowed callers: TaskRuntime lifecycle methods
  - related tests: review_reject_resume_submit_approve_close_lifecycle_persists, close_before_review_approval_is_rejected
  - why shared: keeps lifecycle mutation sequencing single-sourced
- `TaskRuntime::heartbeat_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: refresh active running task lease and persist heartbeat event
  - allowed callers: runtime task tool bridge
  - related tests: resume_creates_lease_and_heartbeat_extends_it, heartbeat_for_assigned_task_is_rejected_without_lease_write, task_tool_resume_and_heartbeat_persist_running_lease
  - why shared: keeps task execution liveness truth in task owner
- `TaskRuntime::assign_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: bind waiting, created, or interrupted tasks to an available agent and persist assignment truth
  - allowed callers: runtime task tool bridge
  - related tests: waiting_task_assigns_to_available_agent_and_recovers, task_tool_agent_assign_cancel_close_lifecycle
  - why shared: keeps agent selection mutation in task owner
- `TaskRuntime::claim_next_task`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: claim the highest-priority assigned task for an agent and enter lease-backed Running state
  - allowed callers: runtime task tool bridge
  - related tests: claim_next_runs_highest_priority_assigned_task_with_lease, claim_next_empty_queue_returns_none_without_mutation, task_tool_claim_next_runs_highest_priority_task
  - why shared: keeps queued task selection and running lease transition in task owner
- `TaskRuntime::record_execution`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: append semantic worker execution progress for running tasks
  - allowed callers: runtime task tool bridge
  - related tests: record_execution_writes_progress_for_running_task, record_execution_rejects_non_running_task_without_sequence_advance, task_tool_record_execution_requires_running_task
  - why shared: keeps worker progress event truth in task owner
- `TaskRuntime::create_agent`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: create persisted idle worker agent snapshots with declared capabilities
  - allowed callers: runtime task tool bridge
  - related tests: create_agent_persists_recovers_and_closes_when_idle, task_tool_agent_assign_cancel_close_lifecycle
  - why shared: keeps agent registry mutation in task owner
- `TaskRuntime::query_task_board`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: project owner-backed TaskBoard truth from task snapshots, execution bindings, blockers, review queue, stale facts, and agent registry
  - allowed callers: runtime query dispatch, CLI/ADP headless samples, tests
  - related tests: task_board_projects_owner_truth_with_filtered_views
  - why shared: keeps TaskBoard truth in Task Center instead of UI-local state
- `TaskRuntime::apply_execution_fact`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: admit typed execution facts into Task Center transition/event truth
  - allowed callers: Agent Lifecycle sync, runtime task bridge, tests
  - related tests: execution_fact_recovering_keeps_running_and_writes_event, execution_fact_blocked_and_review_ready_update_board_truth, execution_fact_validation_failure_writes_no_truth
  - why shared: keeps worker execution state changes in Task Center rather than scattered runtime/UI logic
- `TaskRuntime::run_scheduler_tick`
  - owner: `crates/freehand-task/src/lib.rs`
  - purpose: compute elapsed/stale/soft-timeout/hard-timeout facts and wake recommendations without making business decisions
  - allowed callers: runtime scheduler, CLI/ADP headless samples, tests
  - related tests: scheduler_tick_emits_stale_and_timeout_facts_without_decisions, scheduler_tick_soft_timeout_does_not_fail_task, scheduler_tick_recent_progress_is_not_stale, scheduler_tick_facts_recover_after_boot
  - why shared: keeps framework time sensing in one owner-backed task runtime

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `reasonix_aligned_builtin_specs` | `crates/freehand-tools/src/lib.rs` | expose one `task` tool schema with op-dispatched arguments | static registry truth | provider tool definition | runtime live bridge | tool registry | bound |
| 02 | `execute_task_tool` | `crates/freehand-runtime/src/lib.rs` | route task tool calls into task owner with runtime home, session, turn, and trace context | task tool call | tool result text | runtime live bridge | task runtime | bound |
| 03 | `TaskRuntime::boot` | `crates/freehand-task/src/lib.rs` | load task and agent snapshots into memory | runtime home and owner agent | ready task runtime | runtime task bridge | task owner | bound |
| 04 | `TaskRuntime::create_task` | `crates/freehand-task/src/lib.rs` | validate, persist, assign/wait, and update memory state | task create request | task snapshot plus ledger events | runtime task bridge | task owner | bound |
| 05 | `TaskRuntime::query_task` | `crates/freehand-task/src/lib.rs` | return one task snapshot truth | task id | task snapshot | runtime task bridge | task owner | bound |
| 06 | `TaskRuntime::list_tasks` | `crates/freehand-task/src/lib.rs` | return task snapshots filtered by status and assignee for queue and UI projection | task list query | task snapshots | runtime task bridge | task owner | bound |
| 07 | `TaskRuntime::task_history` | `crates/freehand-task/src/lib.rs` | return ordered persisted task ledger events for timeline and debug projection | task id | task ledger events | runtime task bridge | task owner | bound |
| 08 | `TaskRuntime::submit_review` | `crates/freehand-task/src/lib.rs` | record review submission with deliverables and evidence | task review submission | review-submitted task snapshot and event | runtime task bridge | task owner | bound |
| 09 | `TaskRuntime::approve_review` | `crates/freehand-task/src/lib.rs` | approve submitted review before close | task mutation request | approved task snapshot and event | runtime task bridge | task owner | bound |
| 10 | `TaskRuntime::close_task` | `crates/freehand-task/src/lib.rs` | close only approved or otherwise closeable tasks and release assignee state | task mutation request | closed task snapshot and event | runtime task bridge | task owner | bound |
| 11 | `TaskRuntime::heartbeat_task` | `crates/freehand-task/src/lib.rs` | refresh the lease for an assigned running task | task heartbeat request | running task snapshot plus active lease | runtime task bridge | task owner | bound |
| 12 | `reconcile_running_leases` | `crates/freehand-task/src/lib.rs` | interrupt running tasks with missing, mismatched, inactive, or expired leases during boot | persisted task snapshots plus lease snapshot | recovered runtime state | task boot | task owner | bound |
| 13 | `TaskRuntime::assign_task` | `crates/freehand-task/src/lib.rs` | assign waiting, created, or interrupted tasks to an available agent | task assignment request | assigned task snapshot plus queued agent state | runtime task bridge | task owner | bound |
| 14 | `TaskRuntime::claim_next_task` | `crates/freehand-task/src/lib.rs` | claim the highest-priority assigned task for an agent and enter lease-backed running state | agent task claim request | claimed running task snapshot or no-task outcome | runtime task bridge | task owner | bound |
| 15 | `TaskRuntime::record_execution` | `crates/freehand-task/src/lib.rs` | append semantic worker execution progress for a running task | worker execution record request | running task snapshot plus progress event | runtime task bridge | task owner | bound |
| 16 | `TaskRuntime::cancel_task` | `crates/freehand-task/src/lib.rs` | cancel non-terminal tasks and release assignee state | task mutation request | cancelled task snapshot plus released agent state | runtime task bridge | task owner | bound |
| 17 | `TaskRuntime::create_agent` | `crates/freehand-task/src/lib.rs` | create persisted idle worker agents | agent create request | available agent snapshot | runtime task bridge | task owner | bound |
| 14 | `TaskRuntime::close_agent` | `crates/freehand-task/src/lib.rs` | close only idle agents | agent mutation request | closed agent snapshot | runtime task bridge | task owner | bound |
| 18 | `TaskRuntime::query_task_board` | `crates/freehand-task/src/lib.rs` | project TaskBoard truth for master, scheduler, UI, and headless query | task snapshots plus execution facts plus agent registry | TaskBoard projection | runtime query dispatch | task owner | bound |
| 19 | `TaskRuntime::apply_execution_fact` | `crates/freehand-task/src/lib.rs` | admit typed ExecutionFact state into Task Center without raw prose parsing | ExecutionFact | task snapshot plus event | Agent Lifecycle sync / runtime | task owner | bound |
| 20 | `TaskRuntime::run_scheduler_tick` | `crates/freehand-task/src/lib.rs` | compute elapsed/stale/timeout facts without business decisions | scheduler tick request plus task snapshots | durable scheduler facts | runtime scheduler / CLI sample | task owner | bound |

## Sync Status Against Mainline Call

- first implementation supports `create`, `query`, `list_agents`, and `query_agent`
- current implementation supports `append`, `pause`, `resume`, `heartbeat`, `assign`, `claim_next`, `record_execution`, `history`, `list_tasks`, `cancel`, `submit_review`, `approve`, `reject`, `close`, `create_agent`, and `close_agent`
- review-before-close is locked by positive and negative tests
- lease-backed Running recovery is locked by positive and negative tests
- agent registry lifecycle is locked by positive and negative tests
- worker progress event recording is locked by positive and negative tests
- Phase 1 TaskBoard owner-internal skeleton is implemented
- Phase 1 ExecutionFact owner-internal sync is implemented
- Phase 1 SchedulerTick owner-internal facts are implemented
- real worker execution, UI task projection, and multi-agent dispatch are pending
