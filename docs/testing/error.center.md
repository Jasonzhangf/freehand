# Test Design: `error.center`

- feature_id: `error.center`
- owner: `crates/freehand-control`
- lifecycle path under test:
  - runtime observes schema/tool/provider failure
  - error center classifies domain, class, recovery action, retry fields, and visibility
  - runtime writes error-center metadata with writer owner and pipeline node provenance
  - runtime continues repair/re-entry/failure path only after metadata admission succeeds

## White-Box Coverage

- schema rejection classifies as `schema` / `validation` / `repair_schema` before retry cap
- schema rejection at retry cap classifies as `stop_turn`
- provider executor failure classifies as `provider` / `recoverable` / `fail_turn`
- tool failure classifies as `tool` / `validation` / `repair_schema`
- unknown source classifies as runtime/fatal/escalate_to_user

## Module Black-Box Coverage

- runtime writes `error.center` metadata for completion schema rejection
- runtime writes `error.center` metadata for failed tool results before model re-entry
- runtime writes `error.center` metadata for provider executor failure before terminal failure materialization
- runtime metadata ledger failure blocks the error-center decision path with explicit `MetadataFailed`
- error metadata carries raw hash, not raw provider/tool/user/assistant text

## Project Black-Box Impact

- first slice has no WebUI/ADP-visible error-center projection claim
- online ADP/UI proof is required before claiming user-visible error-center status cards

## Required Checks

```bash
cargo test -p freehand-control
cargo test -p freehand-runtime live_bridge_records_error_center_metadata_for_schema_repair -- --nocapture
cargo test -p freehand-runtime live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing -- --nocapture
cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture
cargo fmt --check
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- task/node/UI error policy is not routed through error center yet
- status schema repair loop is still partial
- no ADP query/subscribe projection for error-center metadata yet
- no WebUI error-center card rendering yet
- worker execution and task recovery still need later integration
