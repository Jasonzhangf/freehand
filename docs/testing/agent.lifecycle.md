# Test Design: `agent.lifecycle`

- feature_id: `agent.lifecycle`
- owner: `crates/freehand-task` initially
- lifecycle path under test:
  - typed runtime/provider/tool/error/task event enters lifecycle reducer
  - reducer updates one agent's lifecycle snapshot
  - AgentBoard projection exposes lifecycle truth
  - runtime/ADP/CLI query returns lifecycle truth without UI-local inference
  - restart proof re-queries the same agent id once persistence is implemented
  - lifecycle snapshot persists independently from agent resource state so close
    can release a worker while restart query still sees the typed closed state
  - Phase 2A worker execution and review events project lifecycle state from
    typed Task Center truth, not from raw model text

## White-Box Coverage

- `model_thinking` state from typed provider/model request event
- `tool_running` state from typed tool execution event
- `recovering` state from failed-tool/schema-polishing/provider-retry typed facts
- `blocked` state from typed blocker fact
- current activity, last activity, elapsed time, task/execution/turn binding, and model/tool/error counters
- worker progress, review_ready, retrying, approved, and closed task lifecycle projections keep the current execution id visible until terminal close
- persisted lifecycle snapshot survives reboot and keeps the same execution id
  visible for Phase 2A verification
- raw assistant prose is not accepted as lifecycle input
- unknown agent id returns explicit not-found
- implemented owner test: `agent_lifecycle_reducer_projects_model_tool_recovering_and_blocked`
- Phase 2A owner test:
  `phase2a_worker_claim_reject_retry_approve_close_recovers_same_execution_id`

## Module Black-Box Coverage

- runtime can query AgentBoard truth without UI-local state
- runtime can query one AgentLifecycleSnapshot by agent id
- CLI/ADP headless sample can query lifecycle truth for a known agent id
- malformed lifecycle event returns explicit validation error and does not mutate truth
- CLI/ADP Phase 2A sample can query lifecycle truth for the worker across
  claim, blocked, recovering, review/retry, approved, and closed phases

## Project Black-Box Impact

- Master will later consume AgentBoard summaries instead of raw logs.
- UI/Android will later render lifecycle projections instead of inferring state.
- Worker Control may read lifecycle truth for status answers, but it must not mutate task truth.

## Required Checks

```bash
cargo test -p freehand-task
cargo test -p freehand-runtime
cargo test -p freehand-ui-protocol
cargo test -p freehand-cli
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- D3 owner-internal skeleton is implemented in `crates/freehand-task`
- ADP/CLI lifecycle query surface is implemented for headless Phase 2A proof
- restart same-id proof is implemented headlessly and live-validated on
  S-profile `127.0.0.1:4042`
- Phase 2A restart same-id lifecycle proof is implemented by
  `master-worker-foundation-sample --verify ...`
- no standalone model-facing `agent` tool is planned for Phase 1
