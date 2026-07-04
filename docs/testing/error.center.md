# Test Design: `error.center`

- feature_id: `error.center`
- owner: `crates/freehand-control`
- lifecycle path under test:
  - runtime observes schema/tool/provider failure
  - error center classifies domain, class, recovery action, retry fields, and visibility
- runtime writes error-center metadata with writer owner and pipeline node provenance
- runtime continues repair/re-entry/failure path only after metadata admission succeeds
- runtime-backed ADP query reads accepted error-center metadata rows without exposing raw text

## White-Box Coverage

- schema rejection classifies as `schema` / `validation` / `repair_schema` before retry cap
- schema rejection at retry cap classifies as `stop_turn`
- provider executor failure classifies as `provider` / `recoverable` / `fail_turn`
- tool failure classifies as `tool` / `validation` / `repair_schema`
- unknown source classifies as runtime/fatal/escalate_to_user
- runtime error-center query projection filters rows by session, trace, turn, and domain
- runtime error-center query projection returns watermarked fields and raw hash only, not raw provider/tool/user/assistant text

## Module Black-Box Coverage

- runtime writes `error.center` metadata for completion schema rejection
- runtime writes `error.center` metadata for failed tool results before model re-entry
- runtime writes `error.center` metadata for provider executor failure before terminal failure materialization
- runtime metadata ledger failure blocks the error-center decision path with explicit `MetadataFailed`
- error metadata carries raw hash, not raw provider/tool/user/assistant text
- daemon ADP query returns runtime-backed `ErrorCenterEvents` projection from metadata truth
- CLI `adp-error-query` summarizes error-center rows without exposing raw metadata payloads

## Project Black-Box Impact

- error-center metadata is now observable through ADP query and initial subscribe projection
- online S-profile ADP proof is required before claiming daemon-visible error-center query works
- browser-visible WebUI error-center cards remain future scope and require separate browser proof

## Required Checks

```bash
cargo test -p freehand-control
cargo test -p freehand-runtime live_bridge_records_error_center_metadata_for_schema_repair -- --nocapture
cargo test -p freehand-runtime live_bridge_returns_unknown_tool_as_failed_tool_result_without_terminalizing -- --nocapture
cargo test -p freehand-runtime live_bridge_writes_provider_error_metadata_on_executor_failure -- --nocapture
cargo test -p freehand-runtime runtime_query_reads_error_center_metadata_without_raw_text -- --nocapture
cargo test -p freehand-daemon daemon_adp_queries_runtime_error_center_truth -- --nocapture
cargo test -p freehand-cli -- --nocapture
cargo fmt --check
cargo run -p xtask -- mainlines generate
cargo run -p xtask -- mainlines check
cargo run -p xtask -- gates check
```

## Known Gaps

- task/node/UI error policy is not routed through error center yet
- status schema repair loop is still partial
- error-center ADP query and initial subscription projection are implemented; live push after new metadata writes is not implemented yet
- no WebUI error-center card rendering yet
- worker execution and task recovery still need later integration
