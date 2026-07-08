# Test Design: `agent.lifecycle`

- feature_id: `agent.lifecycle`
- owner: `crates/freehand-task` initially
- lifecycle path under test:
  - typed runtime/provider/tool/error/task event enters lifecycle reducer
  - reducer updates one agent's lifecycle snapshot
  - AgentBoard projection exposes lifecycle truth
  - runtime/ADP/CLI query returns lifecycle truth without UI-local inference
  - restart proof re-queries the same agent id once persistence is implemented

## White-Box Coverage

- `model_thinking` state from typed provider/model request event
- `tool_running` state from typed tool execution event
- `recovering` state from failed-tool/schema-polishing/provider-retry typed facts
- `blocked` state from typed blocker fact
- current activity, last activity, elapsed time, task/execution/turn binding, and model/tool/error counters
- raw assistant prose is not accepted as lifecycle input
- unknown agent id returns explicit not-found

## Module Black-Box Coverage

- runtime can query AgentBoard truth without UI-local state
- runtime can query one AgentLifecycleSnapshot by agent id
- CLI/ADP headless sample can query lifecycle truth for a known agent id
- malformed lifecycle event returns explicit validation error and does not mutate truth

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

- implementation is pending after D1 owner/map closeout
- persistence and restart same-id proof are pending until AgentBoard query exists
- no standalone model-facing `agent` tool is planned for Phase 1
