# Wiki: `app.cli-live-turn`

Generated from `docs/mainline-calls/app.cli-live-turn.json`. Do not edit by hand.

- owner crate: `apps/freehand-cli`
- owner module: `apps/freehand-cli/src/main.rs`
- function map: `docs/function-maps/app.cli-live-turn.md`
- generated wiki: `docs/wiki/app.cli-live-turn.md`
- test design: `docs/testing/app.cli-live-turn.md`

## Request Mainline

- operator invokes `freehand-cli reason-live`
- CLI loads default config and selects one agent plus bound provider
- CLI derives the runtime home from the default config path and passes it to the shared live bridge
- CLI uses a stable default session id per selected agent unless an explicit session flag is provided
- CLI delegates the live turn to the runtime-owned live bridge through `freehand-runtime`

## Response Mainline

- CLI prints one safe summary of visible text, reasoning-event count, usage, broadcast count, completion rounds, schema rejection count, tool execution count, restore status, and terminal truth
- provider wire payloads and secrets never appear in CLI output

## Error Mainline

- invalid command shape returns explicit usage
- unsupported provider selection returns explicit bridge error
- provider execution failures return explicit live-turn errors
- persistence restore or write failures return explicit live-turn errors

## Shared Multi-Reference Functions

- `run_live_reason_turn`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: bridge config-selected provider execution into one live request with persistence and tool-loop support without leaking provider DTOs into app code
  - allowed callers: runtime dispatch, CLI, daemon, project tests
  - related tests: CLI live-turn smoke tests, live bridge mock tests, daemon live-command smoke tests
  - why shared: app boundary and runtime dispatch must reuse one live bridge path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run` | `apps/freehand-cli/src/main.rs` | parse CLI command and dispatch config summary, smoke, or live turn | CLI args | selected command path | shell/operator | CLI dispatcher | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load runtime config from `~/.freehand/config.toml` | runtime home config path | selected config truth | CLI dispatcher | config owner | bound |
| 03 | `default_config_path` | `crates/freehand-config/src/lib.rs` | derive runtime home from config truth | default config path | runtime home parent | CLI live runner | config owner | bound |
| 04 | `run_reason_live` | `apps/freehand-cli/src/main.rs` | run one config-selected provider live request through app boundary | selected agent plus prompt plus stream/session flags | terminal-facing live summary | CLI dispatcher | app live runner | bound |
| 05 | `run_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | bridge selected provider execution into one persisted tool-capable live request | selected config plus live turn request | turn truth plus broadcasts plus persistence/tool summary | app live runner | runtime bridge | bound |

## Sync Status Against Mainline Call

- CLI live-turn command is implemented
- CLI live-turn path reuses the runtime-owned bridge
- CLI output strips completion tagged JSON from visible text projection and reports completion loop counts
- CLI live-turn now reports runtime-home persistence restore status and tool execution summary
- generated wiki must be regenerated from `docs/mainline-calls/app.cli-live-turn.json` when this function-map truth changes
