# Config Core Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### Config source

- config lives only at `~/.freehand/config.toml`
- project-local config override is not part of v1

### Multi-agent organization

- one `config.toml` may define multiple local agents
- multi-agent layout uses named tables:
  - `[agents.<name>]`
- one `config.toml` may also define multiple provider entries
- provider layout uses named tables:
  - `[providers.<id>]`

### Agent config ownership

- all agents, including `master`, are managed through config
- master and slave do not use separate config systems
- each agent selects exactly one configured provider by provider id

### Minimal required fields for v1

- `name`
- `mode`
- `node_id`
- `paired_agents`
- `pair_token`
- `provider`

### Provider config ownership

- provider/model selection belongs to `freehand-config`
- provider registry is configured in the same `~/.freehand/config.toml`
- `providers.<id>.id` is validated against the table key
- one selected agent may only start with one enabled provider
- unused transport/runtime implementation knobs must not enter config truth before they have a real owner path

### Provider required fields for v1

- `id`
- `enabled`
- `type`
- `protocol`
- `base_url`
- `default_model`
- `auth`

### Provider protocol rules

- `type = "openai"` supports:
  - `protocol = "responses"`
  - `protocol = "chat_completions"`
- `type = "anthropic"` supports:
  - `protocol = "messages"`
- `protocol` is required for every provider entry
- config selection must not guess protocol from `type`, model name, or transport backend
- provider config rejects unknown fields; unimplemented knobs must fail explicitly instead of being silently ignored

### Provider auth rules

- first-version auth type is `apikey`
- provider auth accepts exactly one of:
  - `api_key`
  - `api_key_env`
- selected runtime config resolves `api_key_env` at startup selection time
- CLI/debug projection must not print the resolved API key

### Canonical shape example

```toml
[providers.mini27]
id = "mini27"
enabled = true
type = "openai"
protocol = "responses"
base_url = "http://guizhouyun.site:2080"
default_model = "MiniMax-M2.7"

[providers.mini27.auth]
type = "apikey"
api_key_env = "MINI27_API_KEY"

[providers.claude]
id = "claude"
enabled = true
type = "anthropic"
protocol = "messages"
base_url = "https://api.anthropic.com"
default_model = "claude-sonnet-4-20250514"

[providers.claude.auth]
type = "apikey"
api_key = "sk-inline"

[agents.master]
name = "master"
mode = "master"
node_id = "master-node"
paired_agents = ["worker-alpha", "worker-beta", "worker-gamma"]
pair_token = "FREEHAND_MASTER_TOKEN"
provider = "mini27"

[agents.worker-alpha]
name = "worker-alpha"
mode = "slave"
node_id = "worker-alpha-node"
paired_agents = ["master"]
allowed_pair_ip = "127.0.0.1"
pair_token = "FREEHAND_WORKER_ALPHA_TOKEN"
provider = "claude"

[agents.worker-beta]
name = "worker-beta"
mode = "slave"
node_id = "worker-beta-node"
paired_agents = ["master"]
pair_token = "FREEHAND_WORKER_BETA_TOKEN"
provider = "claude"

[agents.worker-gamma]
name = "worker-gamma"
mode = "slave"
node_id = "worker-gamma-node"
paired_agents = ["master"]
pair_token = "FREEHAND_WORKER_GAMMA_TOKEN"
provider = "claude"
```

### Field alias compatibility

- canonical config field names stay snake_case
- config parser also accepts these user-facing aliases:
  - `baseURL` -> `base_url`
  - `defaultModel` -> `default_model`
  - `apiKey` -> `api_key`
  - `apiKeyEnv` -> `api_key_env`

### Mode-specific rules

- `mode` is required for all agents
- allowed modes confirmed so far:
  - `master`
  - `slave`
- `node_id` is required for all agents
- `paired_agents` is required for all agents; legacy singular `paired_agent` is rejected
- a Master must reference at least one configured Slave Worker
- a Slave Worker must reference exactly one configured Master
- peer names must be non-empty and unique
- every peer must reference another configured agent
- paired agents must point back to each other
- paired agents must use opposite modes in first-version local topology
- `allowed_pair_ip` is optional
- when `allowed_pair_ip` is omitted, pairing source IP is not filtered
- `pair_token` is not inline secret text
- `pair_token` is an environment variable reference
- runtime host bootstrap must validate paired token equality before local pairing starts

### Local startup model

- one process starts one agent
- CLI selects the target `agent name`
- selected agent resolution also resolves the bound provider and provider auth source
- selected agent resolution carries an ordered typed peer topology for runtime bootstrap

### Config activation

- config changes take effect only after restart
- hot reload is not part of v1

### Agent resource count

- Master Worker resource count is config-owned topology, not UI-local state.
- owner update API: `AgentResourceConfigUpdate` through `update_agent_resource_config_in_path`.
- allowed count: `1..=5` Worker resources for the selected Master.
- current resource mode is shared-provider: generated or retained Worker peers use the first existing Worker as the provider/fallback template.
- growing preserves existing peer order and appends deterministic Worker ids from the first Worker name.
- shrinking removes trailing reciprocal Worker agent tables because the v1 schema has no disabled Worker state.
- persisted changes are restart-only; live AgentBoard/process truth must not be fabricated before daemon/Worker restart.

## Remaining TBD

- whether provider-specific auth types beyond `apikey` are needed
- config validation error projection format beyond current typed error strings

## Update trigger

Update this doc when:

- config file location changes
- agent table layout changes
- provider table layout changes
- required field set changes
- provider selection rules change
- startup/restart semantics change
