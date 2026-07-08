# Test Design: `task.orchestration`

- feature_id: `task.orchestration`
- owner: `crates/freehand-task`
- lifecycle path under test:
  - task tool action is admitted by runtime tool bridge
  - task owner validates task fields
  - task owner writes ledger and snapshot
  - task runtime memory state is rebuilt on boot
  - running state is lease-backed and heartbeat-refreshable
  - boot recovery interrupts running tasks whose lease is missing or expired
  - agent registry persists and recovers worker snapshots
  - waiting tasks can be assigned to an available agent
  - assigned worker queue can claim the highest-priority task into running state
  - running workers can record execution progress into task ledger truth
  - task ledger history can be queried as ordered lifecycle events
  - cancellation releases assigned agent state
  - query returns persisted task truth
  - list_tasks returns task snapshots for queue and UI projection queries
  - agent registry exposes self agent
  - pending Phase 1: TaskBoard query projects owner-backed board truth
  - pending Phase 1: ExecutionFact sync admits typed worker execution facts into Task Center
  - pending Phase 1: SchedulerTick emits elapsed/stale/timeout facts without business decisions

## White-Box Coverage

- create task writes ledger, snapshot, index, and recovers after boot
- create with no dispatch becomes `WaitingAgent`
- boot registers self agent as `Available`
- review reject/resume/submit/approve/close lifecycle persists and recovers
- close before review approval is rejected
- resume creates a task lease and records a heartbeat event
- heartbeat refreshes an active running lease
- boot changes running tasks with expired leases to `Interrupted`
- heartbeat for a non-running task is rejected and writes no lease
- create_agent persists, recovers, and closes an idle agent
- assign moves `WaitingAgent` to `Assigned` and marks the assignee busy with queued work
- claim_next picks the highest-priority assigned task for an agent and creates a running lease
- claim_next returns no task without mutation when the agent queue is empty
- record_execution writes progress only for `Running` tasks
- record_execution rejects non-running tasks without advancing event sequence
- task_history returns ordered ledger events including execution progress
- task_history for unknown task returns explicit task-not-found
- list_tasks filters by status and assignee
- cancel releases the assignee and prevents later resume
- close_agent rejects busy agents
- pending: TaskBoard query returns blocked/review/stale filtered views and agent/task binding summaries
- pending: ExecutionFact recovering keeps task non-terminal
- pending: ExecutionFact blocked creates a master-visible event
- pending: ExecutionFact review_ready enters review queue
- pending: SchedulerTick soft timeout does not fail a task
- pending: SchedulerTick stale requires no heartbeat/progress past threshold
- pending: SchedulerTick hard timeout requires master decision and does not automatically fail the task

## Module Black-Box Coverage

- runtime task tool create persists a task and returns task id/status
- runtime task tool query returns snapshot JSON
- runtime task tool list_agents returns self agent
- runtime task tool close before approval returns failure
- runtime task tool review submission, approval, and close return event-backed success
- runtime task tool resume plus heartbeat persists a running lease
- runtime task tool create_agent/assign/cancel/close_agent covers agent lifecycle and busy-close rejection
- runtime task tool claim_next returns the highest-priority assigned task and running lease
- runtime task tool record_execution writes semantic worker execution progress
- runtime task tool history returns task ledger timeline JSON
- runtime task tool list_tasks returns filtered task snapshots
- tool registry exposes `task` as one implemented built-in tool schema
- pending: runtime TaskBoard query returns Task Center board truth without UI-local state
- pending: runtime ExecutionFact sync returns event-backed Task Center updates
- pending: runtime SchedulerTick query/sample emits durable facts only

## Project Black-Box Impact

- first slice is runtime/tool/persistence level only
- WebUI/ADP task projection and online restart proof are required before claiming UI task management
- Phase 1 headless ADP/CLI proof is required before claiming multi-task foundation closeout

## Required Checks

```bash
cargo test -p freehand-task
cargo test -p freehand-tools
cargo test -p freehand-runtime task_tool_create_persists_and_queries_task -- --nocapture
cargo test -p freehand-runtime task_tool_review_lifecycle_rejects_early_close_and_closes_after_approval -- --nocapture
cargo test -p freehand-runtime task_tool_resume_and_heartbeat_persist_running_lease -- --nocapture
cargo test -p freehand-runtime task_tool_agent_assign_cancel_close_lifecycle -- --nocapture
cargo test -p freehand-runtime task_tool_claim_next_runs_highest_priority_task -- --nocapture
cargo test -p freehand-runtime task_tool_record_execution_requires_running_task -- --nocapture
cargo test -p freehand-runtime task_tool_history_returns_ordered_execution_timeline -- --nocapture
cargo test -p freehand-runtime task_tool_list_tasks_filters_queue_projection -- --nocapture
cargo test -p freehand-runtime task_board_query_projects_owner_truth -- --nocapture
cargo test -p freehand-runtime execution_fact_sync_updates_task_center -- --nocapture
cargo test -p freehand-runtime scheduler_tick_emits_facts_without_decisions -- --nocapture
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- no real worker execution
- no queue runner
- no UI task timeline
- TaskBoard/ExecutionFact/SchedulerTick implementation is pending after D1 owner-map closeout
