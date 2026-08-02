# Wiki: `config.core`

Generated from `docs/mainline-calls/config.core.json`. Do not edit by hand.

- owner crate: `crates/freehand-config`
- owner module: `crates/freehand-config/src/lib.rs`
- function map: `docs/function-maps/config.core.md`
- generated wiki: `docs/wiki/config.core.md`
- test design: `docs/testing/config.core.md`

## Resource Operation Backlinks

- config.compile_agent_relay_connection
- config.mutate_provider_config
- config.mutate_model_group_config
- config.compile_remote_daemon_registry

## Request Mainline

- config load begins from `~/.freehand/config.toml`
- one requested agent name selects one `[agents.<name>]` entry
- selected agent compiles its ordered `paired_agents` into typed peer-topology metadata from the same agent registry
- Master selection returns one or more configured Slave Worker peers; Slave selection returns exactly one configured Master peer
- selected agent references one primary `[providers.<id>]` entry and may reference one distinct fallback provider through `fallback_provider`
- validation resolves startup mode, ordered unique reciprocal multi-peer bindings, primary/fallback provider bindings, explicit protocol declarations, auth-source invariants, and unknown-field rejection
- optional per-Agent Relay configuration is admitted only as one atomic URL plus token-env pair and resolves the token only in the selected runtime projection
- optional `[remote_daemon_accounts.<id>]` and `[remote_daemons.<id>]` tables compile into one account-scoped remote daemon registry with explicit endpoint candidates
- remote daemon route selection is config-owned: direct Tailscale/IPv6/IPv4 candidates score below relay candidates, health failures make a candidate non-selectable, and relay is selected only through explicit candidate truth
- QR/deep-link bootstrap requests enter through versioned remote daemon bootstrap bundles with expiry, nonce, selected route endpoint, and one-time credential metadata
- provider definition upsert requests enter only through ProviderConfigUpdate and upsert_provider_config_in_path; the config owner validates provider id, provider type, protocol, base URL, model, and env-var auth before persistence without changing the selected agent binding
- provider selection requests enter only through AgentProviderSelectionConfigUpdate and switch_agent_provider_in_path; the config owner validates existing enabled primary/fallback provider ids before atomically rewriting only the selected agent binding and clears active model_group to avoid hidden route conflicts
- model group definition upsert requests enter only through ModelGroupConfigUpdate and upsert_model_group_config_in_path; the config owner validates route providers/models, enabled-provider constraints, and load-balance weights before persistence without changing provider definitions
- model group selection requests enter only through AgentModelGroupSelectionConfigUpdate and switch_agent_model_group_in_path; the config owner validates existing enabled model group ids before atomically rewriting only the selected agent model_group binding
- provider capability-test requests may resolve any enabled configured provider by id through LoadedConfig::select_provider_for_test without changing the selected agent primary/fallback binding
- legacy provider/model update requests enter only through ProviderConfigUpdate and update_provider_config_in_path; the config owner validates provider id, provider type, protocol, base URL, model, and env-var auth before persistence and preserves existing fallback binding
- Agent resource-count update requests enter only through AgentResourceConfigUpdate and update_agent_resource_config_in_path; the config owner validates Master-only intent and `1..=5` Worker resources before persistence

## Response Mainline

- validated config returns one selected agent runtime configuration plus one primary provider runtime configuration and an optional fallback provider runtime configuration
- selected agent runtime configuration includes either one complete runtime-only Relay URL/token projection or no Relay connection; safe config projections never expose the resolved token
- selected agent runtime configuration includes explicit local node id plus an ordered typed peer list containing peer name, mode, node id, allowed IP, and pair-token env metadata for runtime bootstrap
- selected provider runtime configuration includes explicit protocol, auth source, fallback provider id, and safe projection metadata only
- explicit provider capability-test selection returns one enabled provider runtime configuration with resolved auth source for a live test and does not persist or switch active config
- safe provider registry projection returns every configured provider id, enabled flag, type, protocol, sanitized endpoint, model, auth type, and auth source without exposing credential values
- safe model group registry projection returns every configured group id, enabled flag, label, primary/sub/search/title/fallback routes, and load-balance routes without exposing credential values
- remote daemon registry projection carries accounts, daemon endpoint candidates, selected active endpoint, route diagnostics, and restart-required semantics without leaking credential values
- bootstrap link builders emit `freehand://daemon/import?payload=...` or `https://freehand.local/daemon/import?payload=...` payloads with canonical daemon `activeEndpoint` field; safe summaries redact one-time credential values
- provider definition upserts persist one provider table to the canonical config path with env-var auth only, preserve active primary/fallback selection, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- provider selection updates persist only the selected agent primary/fallback provider binding, preserve all provider definitions, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- model group definition upserts persist one model group table to the canonical config path, preserve provider definitions and active selection, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- model group selection updates persist only the selected agent model_group binding, preserve provider definitions and group definitions, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- legacy provider/model updates persist to the canonical config path with env-var auth only, switch the selected primary provider, preserve fallback selection when valid, return a selected-agent safe projection, and mark restart-required semantics for runtime/UI consumers
- Agent resource-count updates persist reciprocal Master/Worker topology to the canonical config path, preserve surviving peer order, clone the first Worker as the shared-provider template when growing, remove trailing Worker tables when shrinking, and return a restart-required selected-agent projection
- restart is required before config changes take effect

## Error Mainline

- missing config, invalid agent selection, empty/duplicate/self peer, missing peer, same-mode peer, non-reciprocal relation, Master without a Worker, Slave with zero or multiple Masters, invalid primary/fallback provider binding, invalid auth source, incomplete or invalid Agent Relay connection, missing or empty Relay token env, unknown provider fields, disabled provider selection, or permission mismatch return explicit errors
- legacy singular `paired_agent` is rejected by the typed parser; only `paired_agents` is valid
- invalid remote daemon account ids, missing daemon account bindings, invalid direct endpoint host/port, relay endpoints without account relay URL, undeclared active endpoint, unknown route health endpoint, duplicate health records, no selectable route, expired bootstrap, malformed bootstrap, or empty bootstrap credential return explicit errors
- invalid provider definition updates, missing env-var auth, missing selected agent, invalid provider selections, disabled providers, and same-primary fallback selection fail before overwrite; failed updates must leave previous config bytes intact
- invalid model group definitions, missing route providers/models, unknown or disabled route providers, zero load-balance weights, missing selected agent, unknown selected group, and disabled selected group fail before overwrite; failed updates must leave previous config bytes intact
- invalid Agent resource-count updates, non-Master targets, and missing targets fail before overwrite; failed updates must leave previous config bytes intact
- fallback provider selection fails explicitly when the referenced provider is missing, disabled, or equal to the primary provider
- provider capability-test selection fails explicitly when the requested provider id is missing or disabled
- safe config projection must not expose resolved provider secrets

## Shared Multi-Reference Functions


## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `default_config_path` | `crates/freehand-config/src/lib.rs` | resolve the only supported config path | HOME env | config path | startup orchestration | path resolver |  |  |  | bound |
| 02 | `load_default_config` | `crates/freehand-config/src/lib.rs` | load default config file from runtime home | default path | loaded config | CLI/server startup | file loader |  |  |  | bound |
| 03 | `load_config_from_path` | `crates/freehand-config/src/lib.rs` | read config file from one explicit path | explicit path | loaded config | CLI/tests | parser entry |  |  |  | bound |
| 04 | `parse_config` | `crates/freehand-config/src/lib.rs` | parse raw TOML into typed config tables | raw config text | raw parsed config | file loader | TOML parser |  |  |  | bound |
| 05 | `validate_config` | `crates/freehand-config/src/lib.rs` | validate agent registry, ordered multi-peer topology, provider registry, protocol declaration, auth invariants, and unknown-field rejection | raw parsed config | validated loaded config | parser | validator |  |  |  | bound |
| 06 | `LoadedConfig::providers` | `crates/freehand-config/src/lib.rs` | expose validated provider registry truth | loaded config | provider registry view | tests/runtime wiring | registry accessor |  |  |  | bound |
| 06a | `LoadedConfig::safe_provider_registry / ProviderConfig::safe_projection` | `crates/freehand-config/src/lib.rs` | project every configured provider without credential values | loaded provider registry | safe provider registry projection | runtime.ui-command-dispatch / tests | provider registry projector |  |  |  | bound |
| 06b | `LoadedConfig::model_groups` | `crates/freehand-config/src/lib.rs` | expose validated model group registry truth | loaded config | model group registry view | tests/runtime wiring | registry accessor |  |  |  | bound |
| 06c | `LoadedConfig::safe_model_group_registry / ModelGroupConfig::safe_projection` | `crates/freehand-config/src/lib.rs` | project every configured model group route without credential values | loaded model group registry | safe model group registry projection | runtime.ui-command-dispatch / tests | model group registry projector |  |  |  | bound |
| 07 | `LoadedConfig::select_agent` | `crates/freehand-config/src/lib.rs` | select one agent and resolve its provider binding, ordered typed peer topology, env-backed provider auth, and optional atomic Relay URL plus token-env connection | agent name plus env | selected agent runtime config with runtime-only resolved Relay token when configured | CLI/server startup | env resolver | config | config | config.compile_agent_relay_connection | bound |
| 08 | `ProviderAuthConfig::source_kind` | `crates/freehand-config/src/lib.rs` | expose safe provider auth source classification without returning key material | provider auth config | inline or env source kind | config selector/runtime config projection | auth source classifier |  |  |  | bound |
| 09 | `ProviderConfigUpdate` | `crates/freehand-config/src/lib.rs` | carry owner-backed provider/model update intent | agent/provider/model/base-url/env-var fields | validated config-owner update input | runtime.ui-command-dispatch | config owner DTO |  |  |  | bound |
| 10 | `update_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate, apply, reparse, select, and atomically persist legacy provider/model config changes while preserving fallback selection | config path plus provider update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | config | config | config.mutate_provider_config | bound |
| 10a | `upsert_provider_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist one provider definition without changing active primary/fallback selection | config path plus provider definition update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | config | config | config.mutate_provider_config | bound |
| 10b | `switch_agent_provider_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist one agent primary/fallback provider selection without rewriting provider definitions | config path plus agent provider selection update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | config | config | config.mutate_provider_config | bound |
| 10c | `upsert_model_group_config_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist one model group definition without rewriting provider definitions or active provider selection | config path plus model group definition update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | config | config | config.mutate_model_group_config | bound |
| 10d | `switch_agent_model_group_in_path` | `crates/freehand-config/src/lib.rs` | validate and atomically persist one agent model-group selection without rewriting provider definitions or group definitions | config path plus agent model group selection update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner | config | config | config.mutate_model_group_config | bound |
| 11 | `persist_config_atomically` | `crates/freehand-config/src/lib.rs` | write new config through temp file plus rename after validation succeeds | validated TOML text | replaced canonical config file | update_provider_config_in_path | filesystem persistence |  |  |  | bound |
| 12 | `select_provider_for_agent` | `crates/freehand-config/src/lib.rs` | resolve one typed primary or fallback provider binding and its independent auth source | provider registry plus agent name plus provider id plus route role | selected provider runtime config or route-specific explicit config error | LoadedConfig::select_agent | provider registry/auth resolver |  |  |  | bound |
| 12a | `LoadedConfig::select_provider_for_test` | `crates/freehand-config/src/lib.rs` | resolve any enabled configured provider for an explicit capability test without changing selected agent provider bindings | loaded provider registry plus provider id plus env | selected provider runtime config or explicit config error | runtime.ui-command-dispatch provider test | provider registry/auth resolver |  |  |  | bound |
| 13 | `AgentResourceConfigUpdate` | `crates/freehand-config/src/lib.rs` | carry owner-backed Master Worker resource-count intent | agent name plus resource count | validated config-owner update input | runtime.ui-command-dispatch | config owner DTO |  |  |  | bound |
| 14 | `update_agent_resource_config_in_path` | `crates/freehand-config/src/lib.rs` | validate, apply, reparse, select, and atomically persist reciprocal Master/Worker resource topology changes | config path plus resource-count update | selected agent projection from saved config | runtime.ui-command-dispatch / tests | config persistence owner |  |  |  | bound |
| 15 | `validate_remote_daemon_registry` | `crates/freehand-config/src/lib.rs` | validate account-scoped remote daemon registry, direct/relay endpoint candidates, and active endpoint invariants | raw remote daemon TOML tables or bootstrap bundle parts | RemoteDaemonRegistryConfig or explicit config error | validate_config / bootstrap validator | registry validator | config | remote_daemon_registry | config.compile_remote_daemon_registry | bound |
| 16 | `LoadedConfig::remote_daemon_registry` | `crates/freehand-config/src/lib.rs` | expose compiled remote daemon registry truth without provider, pair-token, or credential values | loaded config | remote daemon registry view | CLI/tests/runtime bootstrap helpers | registry accessor |  |  |  | bound |
| 17 | `RemoteDaemonRegistryConfig::build_route_plan / RemoteDaemonRegistryConfig::select_route` | `crates/freehand-config/src/lib.rs` | build endpoint candidates and select one route using direct-first cost, declared health, and explicit diagnostics | daemon id plus optional endpoint health records | selected remote daemon route or explicit no-selectable route error | CLI/bootstrap/tests | route selector |  |  |  | bound |
| 18 | `RemoteDaemonRegistryConfig::build_bootstrap_bundle / RemoteDaemonRegistryConfig::build_bootstrap_bundle_for_selected_route` | `crates/freehand-config/src/lib.rs` | create versioned remote daemon bootstrap bundle with expiry, nonce, selected endpoint, and one-time credential metadata | daemon id plus credential plus expiry plus nonce | secret-bearing bootstrap bundle for QR/deep-link encoding | CLI/tests | registry plus route selector |  |  |  | bound |
| 19 | `build_remote_daemon_bootstrap_link / build_remote_daemon_bootstrap_web_link` | `crates/freehand-config/src/lib.rs` | encode a validated bootstrap bundle as URL-safe app or web deep link | bootstrap bundle | freehand://daemon/import?payload=... or web import URL | CLI/tests | base64url JSON encoder |  |  |  | bound |
| 20 | `parse_remote_daemon_bootstrap_link` | `crates/freehand-config/src/lib.rs` | parse, validate, expiry-check, and safe-summary a bootstrap deep link | app/web link or raw payload plus current unix time | validated bootstrap bundle | Android parity tests / future import surfaces | base64url JSON decoder plus registry validator |  |  |  | bound |

## Sync Status Against Mainline Call

- code binding landed for config loader, parser, validator, provider registry accessor, selected agent/provider selector, and explicit enabled-provider test selector
- selected-agent projection now includes ordered reciprocal multi-peer topology metadata, one bound primary provider runtime configuration, and one optional distinct fallback provider runtime configuration
- singular `paired_agent` schema and selected-agent fields are physically removed
- provider protocol must be explicit and unknown provider fields are rejected
- safe provider projection must not expose resolved API keys or tokens
- provider/model update is bound through ProviderConfigUpdate and update_provider_config_in_path; invalid updates do not overwrite config and saved env-var auth never writes resolved secret values
- model group definition and selection updates are bound through ModelGroupConfigUpdate, AgentModelGroupSelectionConfigUpdate, upsert_model_group_config_in_path, and switch_agent_model_group_in_path; invalid route/group selection updates do not overwrite config
- remote daemon registry validation, direct-first route selection, route-selected bootstrap bundle generation, app/web bootstrap link encoding, parsing, expiry rejection, and secret-redacted safe summaries are code-bound
- generated wiki must be regenerated from `docs/mainline-calls/config.core.json` when this function-map truth changes
