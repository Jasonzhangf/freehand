# Config Directories

## Repo Config Truth

- `Cargo.toml`
- `rust-toolchain.toml`
- `docs/config/`
- future crate config schemas under owner crates

## Runtime Config Truth

- `~/.freehand/state/config`
- per-agent startup config files live under runtime config truth

## Confirmed Startup Config Role

- local multi-agent management is handled by `config.toml`
- one `config.toml` may define multiple local agents
- one `config.toml` may define multiple providers
- config source path is only `~/.freehand/config.toml`
- multi-agent layout uses `[agents.<name>]`
- provider layout uses `[providers.<id>]`
- each agent has a startup configuration file
- startup configuration decides how the agent starts
- all agents, including `master`, are configured there
- each agent binds to exactly one provider id from the provider registry
- if configured as `slave`, startup configuration must include at least:
  - `name`
  - `mode`
  - pairing token
- `allowed_pair_ip` is optional
- when `allowed_pair_ip` is omitted, pairing source IP is not filtered
- `pair_token` is an environment variable reference
- provider auth supports `api_key` or `api_key_env`, but the selected runtime projection must never print the resolved secret
- one process starts one agent, selected by CLI agent name
- config changes take effect only after restart

## Rule

- config schema changes must update function map and config docs
- secret values stay out of repo config files
- runtime-resolved config snapshots belong under `~/.freehand`
- multi-agent config truth should stay centralized in `config.toml`, not split ad hoc across unrelated files
