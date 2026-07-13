# Function Map: `config.core`

- feature_id: `config.core`
- owner crate: `crates/freehand-config`
- owner module: `crates/freehand-config/src/lib.rs`
- owner entry symbols:
  - `default_config_path`
  - `load_default_config`
  - `load_config_from_path`
  - `LoadedConfig::select_agent`
  - `select_provider_for_agent`
  - `ProviderAuthConfig::source_kind`
  - `LoadedConfig::providers`
  - `update_provider_config_in_path`
  - `ProviderConfigUpdate`
  - `parse_config`
  - `validate_config`

## Request Mainline

- config load begins from `~/.freehand/config.toml`
- requested agent name selects one `[agents.<name>]` entry
- selected agent also resolves explicit peer-topology metadata from the same agent registry
- selected agent references one primary `[providers.<id>]` entry and may reference one distinct fallback provider through `fallback_provider`
- validation resolves startup mode, reciprocal peer binding, primary/fallback provider bindings, explicit protocol declarations, auth-source invariants, and unknown-field rejection
- provider/model update requests enter only through `ProviderConfigUpdate` and `update_provider_config_in_path`; the config owner validates provider id, provider type, protocol, base URL, model, and env-var auth before persistence

## Response Mainline

- validated config returns one selected agent runtime configuration plus one primary provider runtime configuration and an optional fallback provider runtime configuration
- selected agent runtime configuration includes explicit local node id, paired agent name, paired mode, paired node id, paired allowed IP, and paired pair-token env metadata for runtime bootstrap
- selected provider runtime configuration carries safe auth source kind (`inline` or `env`) separately from the resolved API key so UI projections do not infer or expose secret values
- provider/model updates persist to the canonical config path with env-var auth only, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- restart is required before config changes take effect

## Error Mainline

- missing config, invalid agent selection, self-pairing, missing paired agent, same-mode paired agents, non-reciprocal pairing, invalid primary/fallback provider binding, invalid auth source, unknown provider fields, disabled provider selection, or permission mismatch return explicit errors
- invalid provider update inputs or missing env-var auth fail before overwrite; failed updates must leave the previous config bytes intact
- fallback provider selection fails explicitly when the referenced provider is missing, disabled, or equal to the primary provider
- API keys and pair token values are runtime-only fields and must not enter UI-safe config status projection

## Shared Multi-Reference Functions

- none

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `default_config_path` | `crates/freehand-config/src/lib.rs` | resolve default config path | HOME env | config path | startup orchestration | path resolver | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load default config file | default path | loaded config | CLI/server startup | file loader | bound |
| 03 | `load_config_from_path` | `crates/freehand-config/src/lib.rs` | read config file | explicit path | loaded config | CLI/tests | parser entry | bound |
| 04 | `parse_config` | `crates/freehand-config/src/lib.rs` | parse raw TOML into typed config | raw config text | raw parsed config | file loader | TOML parser | bound |
| 05 | `validate_config` | `crates/freehand-config/src/lib.rs` | validate agent registry and provider registry invariants | raw parsed config | validated loaded config | parser | validator | bound |
| 06 | `LoadedConfig::providers` | `crates/freehand-config/src/lib.rs` | expose validated provider registry truth | loaded config | provider registry view | tests/runtime wiring | registry accessor | bound |
| 07 | `LoadedConfig::select_agent` | `crates/freehand-config/src/lib.rs` | select and resolve one agent plus its bound provider and paired topology metadata | agent name + env | selected agent runtime config with safe provider auth source kind plus runtime-only resolved key | CLI/server startup | env resolver | bound |
| 08 | `ProviderAuthConfig::source_kind` | `crates/freehand-config/src/lib.rs` | expose safe provider auth source classification without returning key material | provider auth config | `inline` or `env` source kind | config selector/runtime config projection | auth source classifier | bound |
| 09 | `ProviderConfigUpdate` | `crates/freehand-config/src/lib.rs` | carry owner-backed provider/model update intent | agent/provider/model/base-url/env-var fields | validated config-owner update input | runtime.ui-command-dispatch | config owner DTO | bound |
| 10 | `update_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate, apply, reparse, select, and atomically persist provider/model config changes | config path + provider update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | bound |
| 11 | `persist_config_atomically` | `crates/freehand-config/src/lib.rs` | write new config through temp file plus rename after validation succeeds | validated TOML text | replaced canonical config file | `update_provider_config_in_path` | filesystem persistence | bound |
| 12 | `select_provider_for_agent` | `crates/freehand-config/src/lib.rs` | resolve one typed primary or fallback provider binding and its independent auth source | provider registry + agent name + provider id + route role | selected provider runtime config or route-specific explicit config error | `LoadedConfig::select_agent` | provider registry/auth resolver | bound |

## Sync Status Against Code

- code binding landed for config loader, parser, validator, provider registry accessor, and agent/provider selector
- selected-agent projection now includes reciprocal peer-topology metadata, one bound primary provider runtime configuration, and one optional distinct fallback provider runtime configuration
- selected-provider projection now includes `auth_source` so downstream UI-safe projections can show auth source type without exposing API keys
- provider/model update is bound through `ProviderConfigUpdate` and `update_provider_config_in_path`; invalid updates do not overwrite config and saved env-var auth never writes resolved secret values
- generated wiki must be regenerated from `docs/mainline-calls/config.core.json` when this function-map truth changes
