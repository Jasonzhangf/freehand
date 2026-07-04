# Test Design: `task.orchestration`

- feature_id: `task.orchestration`
- owner: `crates/freehand-task`
- lifecycle path under test:
  - task tool action is admitted by runtime tool bridge
  - task owner validates task fields
  - task owner writes ledger and snapshot
  - task runtime memory state is rebuilt on boot
  - query returns persisted task truth
  - agent registry exposes self agent

## White-Box Coverage

- create task writes ledger, snapshot, index, and recovers after boot
- create with no dispatch becomes `WaitingAgent`
- boot registers self agent as `Available`
- review reject/resume/submit/approve/close lifecycle persists and recovers
- close before review approval is rejected

## Module Black-Box Coverage

- runtime task tool create persists a task and returns task id/status
- runtime task tool query returns snapshot JSON
- runtime task tool list_agents returns self agent
- runtime task tool close before approval returns failure
- runtime task tool review submission, approval, and close return event-backed success
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
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- no real worker execution
- no queue runner
- no lease/heartbeat recovery yet
- no UI task timeline
