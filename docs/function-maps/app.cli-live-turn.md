# Function Map: `app.cli-live-turn`

- feature_id: `app.cli-live-turn`
- owner crate: `apps/freehand-cli`
- owner module: `apps/freehand-cli/src/main.rs`
- owner entry symbols:
  - `run`
  - `run_reason_live`

## Request Mainline

- operator invokes `freehand-cli reason-live`
- CLI loads default config and selects one agent plus bound provider
- CLI delegates the live turn to `freehand-testkit::run_live_reason_turn`

## Response Mainline

- CLI prints one safe summary of visible text, reasoning-event count, usage, broadcast count, completion rounds, schema rejection count, and terminal truth
- provider wire payloads and secrets never appear in CLI output

## Error Mainline

- invalid command shape returns explicit usage
- unsupported provider selection returns explicit bridge error
- provider execution failures return explicit live-turn errors

## Shared Multi-Reference Functions

- `run_live_reason_turn`
  - owner: `crates/freehand-testkit/src/lib.rs`
  - purpose: bridge config-selected provider execution into one reason turn without leaking provider DTOs into app code
  - allowed callers: CLI, project tests
  - related tests: CLI live-turn smoke tests, live bridge mock tests
  - why shared: app and tests must reuse one live bridge path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-cli/src/main.rs` | parse CLI command and dispatch config summary, smoke, or live turn | CLI args | selected command path | shell/operator | CLI dispatcher | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load runtime config from `~/.freehand/config.toml` | runtime home config path | selected config truth | CLI dispatcher | config owner | bound |
| 03 | `run_reason_live` | `apps/freehand-cli/src/main.rs` | run one config-selected provider live turn through app boundary | selected agent + prompt + stream flag | terminal-facing live summary | CLI dispatcher | app live runner | bound |
| 04 | `run_live_reason_turn` | `crates/freehand-testkit/src/lib.rs` | bridge selected provider execution into one reason turn | selected config + live turn request | turn truth + broadcasts | app live runner | testkit bridge | bound |

## Sync Status Against Code

- CLI live-turn command is implemented
- CLI live-turn path reuses `freehand-testkit` bridge
- CLI output strips completion tagged JSON from visible text projection and reports completion loop counts
