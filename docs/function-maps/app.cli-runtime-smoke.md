# Function Map: `app.cli-runtime-smoke`

- feature_id: `app.cli-runtime-smoke`
- owner crate: `apps/freehand-cli`
- owner module: `apps/freehand-cli/src/main.rs`
- owner entry symbols:
  - `run`
  - `run_reason_e2e_smoke`

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
- smoke runtime failures return explicit reason/runtime errors
- rewrite recovery block is reported as explicit blocked outcome, not disguised as success

## Shared Multi-Reference Functions

- `ReasonRuntimeHarness::run_provider_turn`
  - owner: `crates/freehand-testkit`
  - purpose: black-box route provider semantic outputs through turn truth into usage-driven rewrite policy
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI reason E2E smoke tests
  - why shared: app and project tests must reuse one runtime harness path
- `ReasonRuntimeHarness::apply_resume_rebuild`
  - owner: `crates/freehand-testkit`
  - purpose: black-box route restore status into resume-rebuild/block decision
  - allowed callers: CLI smoke command, project tests
  - related tests: CLI recovery-block smoke tests
  - why shared: recovery smoke must reuse one runtime harness path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-cli/src/main.rs` | parse CLI command and dispatch config startup or reason E2E smoke | CLI args | selected command path | shell/operator | CLI dispatcher | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load runtime config from `~/.freehand/config.toml` | runtime home config path | selected config truth | CLI dispatcher | config owner | bound |
| 03 | `run_reason_e2e_smoke` | `apps/freehand-cli/src/main.rs` | build scripted E2E runtime harness request from selected agent | selected agent + scenario | terminal-facing smoke summary | CLI dispatcher | app smoke runner | bound |
| 04 | `ReasonRuntimeHarness::run_provider_turn` | `crates/freehand-testkit/src/lib.rs` | route provider usage into turn truth and rewrite policy | scripted provider outputs + compaction scenario | turn truth + optional compaction outcome | app smoke runner | testkit harness | bound |
| 05 | `ReasonRuntimeHarness::apply_resume_rebuild` | `crates/freehand-testkit/src/lib.rs` | route restore state into recovery policy | restore status + optional rebuild payload | recovery outcome | app smoke runner | testkit harness | bound |

## Metadata / Request Isolation Notes

- CLI scenario selection, config selection, and harness options stay outside request text
- provider usage, recovery facts, and rewrite decisions remain metadata/runtime-side until they are projected as smoke output
- CLI smoke output reports terminal summary only; it does not expose hidden prompt mutations

## Sync Status Against Code

- CLI config startup path is implemented
- CLI reason E2E smoke path is implemented
- harness-backed app E2E smoke now exists before production CLI/server runtime loop
- remaining gap: production non-smoke command loop is still pending
