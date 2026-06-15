# Function Map: `config.core`

- feature_id: `config.core`
- owner crate: `crates/freehand-config`
- owner module: `TBD until implementation lands`
- owner entry symbols:
  - `TBD until implementation lands`

## Request Mainline

- config load begins from `~/.freehand/config.toml`
- requested agent name selects one `[agents.<name>]` entry
- validation resolves startup mode and startup invariants

## Response Mainline

- validated config returns one selected agent runtime configuration
- restart is required before config changes take effect

## Error Mainline

- missing config, invalid agent selection, invalid startup mode, or permission mismatch return explicit errors
- no fallback config source exists

## Shared Multi-Reference Functions

- pending until implementation lands

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TBD` | `TBD` | load config file | config path | raw config text | CLI/server startup | config loader | binding pending |
| 02 | `TBD` | `TBD` | parse config | raw config text | parsed config tree | config loader | parser | binding pending |
| 03 | `TBD` | `TBD` | select named agent | parsed config tree + agent name | one agent config | startup orchestration | selector | binding pending |
| 04 | `TBD` | `TBD` | validate startup config | one agent config | validated runtime config | selector | validator | binding pending |

## Sync Status Against Code

- design stub only
- implementation binding pending
