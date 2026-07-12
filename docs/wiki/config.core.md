# Wiki: `config.core`

Generated from `docs/mainline-calls/config.core.json`. Do not edit by hand.

- owner crate: `crates/freehand-config`
- owner module: `crates/freehand-config/src/lib.rs`
- function map: `docs/function-maps/config.core.md`
- generated wiki: `docs/wiki/config.core.md`
- test design: `docs/testing/config.core.md`

## Request Mainline

- config load begins from `~/.freehand/config.toml`
- one requested agent name selects one `[agents.<name>]` entry
- selected agent also resolves explicit peer-topology metadata from the same agent registry
- selected agent references one `[providers.<id>]` entry
- validation resolves startup mode, reciprocal peer binding, provider binding, explicit protocol declaration, auth-source invariants, and unknown-field rejection
- provider/model update requests enter only through ProviderConfigUpdate and update_provider_config_in_path; the config owner validates provider id, provider type, protocol, base URL, model, and env-var auth before persistence

## Response Mainline

- validated config returns one selected agent runtime configuration plus one selected provider runtime configuration
- selected agent runtime configuration includes explicit local node id, paired agent name, paired mode, paired node id, paired allowed IP, and paired pair-token env metadata for runtime bootstrap
- selected provider runtime configuration includes explicit protocol, auth source, and safe projection metadata only
- provider/model updates persist to the canonical config path with env-var auth only, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- restart is required before config changes take effect

## Error Mainline

- missing config, invalid agent selection, self-pairing, missing paired agent, same-mode paired agents, non-reciprocal pairing, invalid provider binding, invalid auth source, unknown provider fields, disabled provider selection, or permission mismatch return explicit errors
- invalid provider update inputs or missing env-var auth fail before overwrite; failed updates must leave previous config bytes intact
- no fallback config source exists
- safe config projection must not expose resolved provider secrets

## Shared Multi-Reference Functions


## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `default_config_path` | `crates/freehand-config/src/lib.rs` | resolve the only supported config path | HOME env | config path | startup orchestration | path resolver |  |  |  | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load default config file from runtime home | default path | loaded config | CLI/server startup | file loader |  |  |  | bound |
| 03 | `load_config_from_path` | `crates/freehand-config/src/lib.rs` | read config file from one explicit path | explicit path | loaded config | CLI/tests | parser entry |  |  |  | bound |
| 04 | `parse_config` | `crates/freehand-config/src/lib.rs` | parse raw TOML into typed config tables | raw config text | raw parsed config | file loader | TOML parser |  |  |  | bound |
| 05 | `validate_config` | `crates/freehand-config/src/lib.rs` | validate agent registry, provider registry, reciprocal peer topology, protocol declaration, auth invariants, and unknown-field rejection | raw parsed config | validated loaded config | parser | validator |  |  |  | bound |
| 06 | `LoadedConfig::providers` | `crates/freehand-config/src/lib.rs` | expose validated provider registry truth | loaded config | provider registry view | tests/runtime wiring | registry accessor |  |  |  | bound |
| 07 | `LoadedConfig::select_agent` | `crates/freehand-config/src/lib.rs` | select one agent and resolve its provider binding, paired topology metadata, and env-backed auth source | agent name plus env | selected agent runtime config | CLI/server startup | env resolver |  |  |  | bound |
| 08 | `ProviderAuthConfig::source_kind` | `crates/freehand-config/src/lib.rs` | expose safe provider auth source classification without returning key material | provider auth config | inline or env source kind | config selector/runtime config projection | auth source classifier |  |  |  | bound |
| 09 | `ProviderConfigUpdate` | `crates/freehand-config/src/lib.rs` | carry owner-backed provider/model update intent | agent/provider/model/base-url/env-var fields | validated config-owner update input | runtime.ui-command-dispatch | config owner DTO |  |  |  | bound |
| 10 | `update_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate, apply, reparse, select, and atomically persist provider/model config changes | config path plus provider update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner |  |  |  | bound |
| 11 | `persist_config_atomically` | `crates/freehand-config/src/lib.rs` | write new config through temp file plus rename after validation succeeds | validated TOML text | replaced canonical config file | update_provider_config_in_path | filesystem persistence |  |  |  | bound |

## Sync Status Against Mainline Call

- code binding landed for config loader, parser, validator, provider registry accessor, and agent/provider selector
- selected-agent projection now includes reciprocal peer-topology metadata and one bound provider runtime configuration
- provider protocol must be explicit and unknown provider fields are rejected
- safe provider projection must not expose resolved API keys or tokens
- provider/model update is bound through ProviderConfigUpdate and update_provider_config_in_path; invalid updates do not overwrite config and saved env-var auth never writes resolved secret values
- generated wiki must be regenerated from `docs/mainline-calls/config.core.json` when this function-map truth changes
