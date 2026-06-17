# Wiki: `app.cli-runtime-smoke`

Generated from `docs/mainline-calls/app.cli-runtime-smoke.json`. Do not edit by hand.

- owner crate: `apps/freehand-cli`
- owner module: `apps/freehand-cli/src/main.rs`
- function map: `docs/function-maps/app.cli-runtime-smoke.md`
- generated wiki: `docs/wiki/app.cli-runtime-smoke.md`
- test design: `docs/testing/app.cli-runtime-smoke.md`

## Request Mainline

- operator invokes `freehand-cli`
- CLI parses the command shape and selects one agent plus its bound provider from `~/.freehand/config.toml`
- for reason E2E smoke, CLI builds one scripted runtime harness request
- provider semantic outputs enter the harness, then reason turn truth, then rewrite runtime, then terminal reporting

## Response Mainline

- config startup path prints selected-agent summary plus selected-provider metadata without exposing provider secret values
- reason E2E smoke prints scenario name, selected agent, rewrite outcome, rewrite version, and latest usage summary
- CLI output remains a terminal-facing projection, not debug ledger raw payload

## Error Mainline

- invalid command shape returns explicit usage
- missing config or missing agent selection returns explicit config errors
- smoke runtime failures return explicit reason or runtime errors
- rewrite recovery block is reported as explicit blocked outcome, not disguised as success

## Shared Multi-Reference Functions

- `ReasonRuntimeHarness::run_provider_turn`
  - owner: `crates/freehand-testkit/src/lib.rs`
  - purpose: route provider semantic outputs through turn truth into usage-driven rewrite policy
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI reason E2E smoke tests
  - why shared: app and project tests must reuse one runtime harness path
- `ReasonRuntimeHarness::apply_resume_rebuild`
  - owner: `crates/freehand-testkit/src/lib.rs`
  - purpose: route restore status into resume-rebuild or block decision
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI recovery-block smoke tests
  - why shared: recovery smoke must reuse one runtime harness path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-cli/src/main.rs` | parse CLI command and dispatch config startup or reason E2E smoke | CLI args | selected command path | shell/operator | CLI dispatcher | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load runtime config from `~/.freehand/config.toml` | runtime home config path | selected config truth | CLI dispatcher | config owner | bound |
| 03 | `run_reason_e2e_smoke` | `apps/freehand-cli/src/main.rs` | build scripted E2E runtime harness request from selected agent | selected agent plus scenario | terminal-facing smoke summary | CLI dispatcher | app smoke runner | bound |
| 04 | `ReasonRuntimeHarness::run_provider_turn` | `crates/freehand-testkit/src/lib.rs` | route provider usage into turn truth and rewrite policy | scripted provider outputs plus compaction scenario | turn truth plus optional compaction outcome | app smoke runner | testkit harness | bound |
| 05 | `ReasonRuntimeHarness::apply_resume_rebuild` | `crates/freehand-testkit/src/lib.rs` | route restore state into recovery policy | restore status plus optional rebuild payload | recovery outcome | app smoke runner | testkit harness | bound |

## Sync Status Against Mainline Call

- CLI config startup path is implemented
- CLI reason E2E smoke path is implemented
- harness-backed app E2E smoke now exists before production CLI or server runtime loop
- remaining gap: production non-smoke command loop is still pending
- generated wiki must be regenerated from `docs/mainline-calls/app.cli-runtime-smoke.json` when this function-map truth changes
