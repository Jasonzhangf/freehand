# Test Design: `config.core`

- feature_id: `config.core`
- owner: `crates/freehand-config`
- resource map: `docs/resource-maps/core.json`
- lifecycle path under test:
  - load `~/.freehand/config.toml`
  - resolve `[agents.<name>]`
  - compile ordered `paired_agents` peer topology from the agent registry
  - require every Master to have at least one configured Slave Worker
  - require every Slave Worker to have exactly one configured Master
  - validate reciprocal, opposite-mode, unique peer-topology fields
  - resolve `[providers.<id>]`
  - select one agent per process
  - select one enabled primary provider per agent
  - optionally select one distinct enabled fallback provider per agent
  - resolve provider auth source without leaking secret projection
  - preserve safe auth source kind on selected provider config for downstream UI-safe status projection
  - project the complete configured provider registry without resolved secrets so users can select any enabled provider by id
  - resolve any enabled configured provider by id for live capability tests without changing active agent selection
  - validate and persist owner-backed provider definition upserts without changing the selected agent provider
  - validate and persist owner-backed primary/fallback provider selection changes without rewriting provider definitions
  - retain the legacy combined provider/model update path for existing CLI callers while new UI flows use separate upsert and selection operations
  - validate and persist owner-backed Master Worker resource-count updates through the canonical config path
  - keep active Worker resources in `1..=5`, preserve declared peer order, clone the first Worker as the shared-provider template when growing, and remove trailing reciprocal Worker tables when shrinking
  - compile remote daemon accounts, daemon endpoint candidates, route selection diagnostics, and QR/deep-link bootstrap bundles from config owner truth
  - validate restart-only config activation

## Resource Operation Test Coverage

| resource operation | status | white-box coverage | module black-box coverage | project black-box coverage |
| --- | --- | --- | --- | --- |
| `config.mutate_provider_config` | bound | `cargo test -p freehand-config provider -- --nocapture --test-threads=1` covers safe provider registry projection, definition-only upsert, selection-only switch, same-primary fallback rejection, disabled-provider rejection, and no-overwrite failures | `cargo test -p freehand-config -- --nocapture` exercises the public config mutation boundary through owner APIs and reparsed saved TOML | `cargo test -p freehand-runtime runtime_dispatch_upserts_provider_registry_without_switching_active_selection -- --nocapture --test-threads=1` and `cargo test -p freehand-runtime runtime_dispatch_switches_agent_provider_selection_without_hot_reload -- --nocapture --test-threads=1` prove runtime/UI mutations route through `config.core` and remain restart-required |
| `config.compile_remote_daemon_registry` | bound | `cargo test -p freehand-config remote_daemon -- --nocapture` covers registry validation, direct-first route selection, relay selection after direct health failure, no-selectable route error, route-selected bootstrap bundles, app/web bootstrap link parsing, expiry rejection, and secret-redacted summaries | `cargo test -p freehand-config -- --nocapture` exercises the public config loader/accessor boundary for remote daemon registry and bootstrap helpers | `cargo run -p xtask -- gates check` enforces resource-map/mainline/function/test binding; full relay tunnel and live Tailscale probing remain outside this config-owned source edge |
- white-box plan:
  - parse and validate agent/provider schema, ordered reciprocal multi-peer topology invariants, explicit protocol declaration, unknown-field rejection, auth-source invariants, and env resolution rules
  - assert inline and env auth providers project `ProviderAuthSourceKind` while resolved API keys remain runtime-only
  - assert `fallback_provider` resolves a second provider with its own type, protocol, model, endpoint, and auth source
  - reject fallback references that are missing, disabled, or equal to the primary provider
  - assert safe provider registry projection preserves every configured provider id, enabled flag, type, protocol, sanitized base URL/host, model, auth type, and auth source without API-key values
  - assert provider definition upsert can add a third provider while preserving the current primary/fallback binding and every existing provider table
  - assert provider selection can switch among existing enabled providers without rewriting provider definitions and can explicitly clear or replace fallback selection
  - reject missing/disabled primary selection and same-primary fallback selection before overwrite so the original config bytes remain unchanged
  - assert provider/model update accepts valid env-var auth, writes only `api_key_env`, returns a restart-required selected-agent projection, and does not write resolved API-key values
  - assert invalid provider update input fails before overwrite so the original config bytes remain unchanged
  - assert a resource-count update grows one Master from one to five reciprocal Workers, gives every active Worker the Master provider/fallback bindings, and returns a restart-required selected-agent projection
  - assert shrinking removes only trailing Workers owned by that Master and preserves surviving peer order
  - assert zero, six, unknown-agent, and non-Master resource-count updates fail before overwrite so original config bytes remain unchanged
  - assert valid remote daemon registry loads one account with multiple daemons and Tailscale/relay endpoint candidates
  - assert route selection prefers Tailscale/IPv6/IPv4 direct candidates over relay when health is unknown or successful
  - assert route selection skips direct health failures and selects declared relay endpoint only when the account declares `relay_url`
  - assert route selection fails explicitly when every endpoint has failure/auth-failure health or health references an unknown endpoint
  - assert QR/deep-link bootstrap bundles round-trip through base64url JSON, emit Android-compatible daemon `activeEndpoint`, reject expired payloads, and keep credential values out of safe summaries
  - assert route-selected bootstrap changes only the selected `active_endpoint` in the exported daemon bundle instead of letting Android score routes locally
  - positive peer-topology coverage locks one Master plus three ordered Workers and selected-agent projection of every peer name, mode, node id, allowed IP, and pair-token env
  - negative peer-topology coverage locks empty peer sets, empty peer names, duplicate peers, self-pairing, missing peers, same-mode peers, non-reciprocal peers, and a Slave bound to multiple Masters
  - legacy singular `paired_agent` is rejected as an unknown field; no compatibility parser or runtime fallback exists
- module black-box plan:
  - load config file and select named Master with full primary/fallback provider runtime selection plus stable ordered three-Worker topology projection through the public config boundary
  - select each named Worker and prove its compiled peer set contains exactly its configured Master
  - load config file and select named remote daemon route through `LoadedConfig::remote_daemon_registry`
  - generate a route-selected bootstrap link from a one-time credential and prove parser rejects expired links
- project black-box impact:
  - CLI startup path consumes one named agent configuration and projects selected provider metadata without exposing API key
  - CLI `remote-daemon-bootstrap-link` consumes config-owned remote daemon registry truth and emits a scan/import deep link using a one-time credential env var
  - runtime/UI config status queries can list every configured provider and current primary/fallback selection without reading raw provider auth fields or resolved API keys
  - runtime/UI provider definition and provider selection commands persist only through `config.core` and must surface restart-required semantics instead of hot-reload success
  - runtime/UI Agent resource controls can persist only config-owned reciprocal topology; frontend state cannot invent capacity
  - machine-readable mainline truth remains the only source for generated wiki artifacts
- fixtures / replay inputs / runtime evidence paths:
  - config fixtures under crate test fixtures
  - runtime evidence under `~/.freehand/state/config`
- known gaps:
  - provider failover execution remains owned by `provider.reason-live-bridge`; `config.core` owns only validated route selection truth
  - remote daemon live health probing, relay signaling/tunnel IO, and Tailscale OS auto-connect are not implemented in `config.core`; this feature owns registry, route plan, and bootstrap contract only
- sync status between design and implementation:
  - white-box, module black-box, and project black-box baseline cover multi-provider registry, reciprocal multi-peer topology, selected-provider projection, and explicit provider test selection
  - `cargo test -p freehand-config select_provider_for_test_resolves_any_configured_enabled_provider -- --nocapture` covers enabled-provider capability-test selection by id
  - `cargo test -p freehand-config -- --nocapture` covers the three-Worker positive path plus legacy singular, duplicate, multi-Master Worker negative paths, remote daemon registry, route selection, and bootstrap link paths
  - `cargo test -p freehand-config remote_daemon -- --nocapture` covers the focused remote daemon route/bootstrap slice
  - selected-provider auth source is regression-locked for inline and env configurations
  - provider registry safe projection, definition-only upsert, and selection-only mutation positive/negative paths are regression-locked
  - provider/model update positive and invalid-input no-overwrite paths are regression-locked
  - Agent resource-count grow/shrink plus out-of-range/no-overwrite paths are regression-locked
  - migrated mainline-call source and generated wiki are kept in sync with this test design
