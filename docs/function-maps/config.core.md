# Function Map: `config.core`

- feature_id: `config.core`
- owner crate: `crates/freehand-config`
- owner module: `crates/freehand-config/src/lib.rs`
- owner entry symbols:
  - `default_config_path`
  - `load_default_config`
  - `load_config_from_path`
  - `LoadedConfig::select_agent`
  - `parse_config`
  - `validate_config`

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

- none

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `default_config_path` | `crates/freehand-config/src/lib.rs` | resolve default config path | HOME env | config path | startup orchestration | path resolver | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load default config file | default path | loaded config | CLI/server startup | file loader | bound |
| 03 | `load_config_from_path` | `crates/freehand-config/src/lib.rs` | read config file | explicit path | loaded config | CLI/tests | parser entry | bound |
| 04 | `parse_config` | `crates/freehand-config/src/lib.rs` | parse raw TOML into typed config | raw config text | raw parsed config | file loader | TOML parser | bound |
| 05 | `validate_config` | `crates/freehand-config/src/lib.rs` | validate config invariants | raw parsed config | validated loaded config | parser | validator | bound |
| 06 | `LoadedConfig::select_agent` | `crates/freehand-config/src/lib.rs` | select and resolve one agent | agent name + env | selected agent config | CLI/server startup | env resolver | bound |

## Sync Status Against Code

- code binding landed for config loader, parser, validator, and agent selector
