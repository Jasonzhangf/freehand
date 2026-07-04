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
  - agent registry exposes self agent

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
- cancel releases the assignee and prevents later resume
- close_agent rejects busy agents

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
- tool registry exposes `task` as one implemented built-in tool schema

## Project Black-Box Impact

- first slice is runtime/tool/persistence level only
- WebUI/ADP task projection and online restart proof are required before claiming UI task management

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
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- no real worker execution
- no queue runner
- no UI task timeline
