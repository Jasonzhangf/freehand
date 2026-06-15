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

### Agent config ownership

- all agents, including `master`, are managed through config
- master and slave do not use separate config systems

### Minimal required fields for v1

- `name`
- `mode`
- `pair_token`

### Mode-specific rules

- `mode` is required for all agents
- allowed modes confirmed so far:
  - `master`
  - `slave`
- `allowed_pair_ip` is optional
- when `allowed_pair_ip` is omitted, pairing source IP is not filtered
- `pair_token` is not inline secret text
- `pair_token` is an environment variable reference

### Local startup model

- one process starts one agent
- CLI selects the target `agent name`

### Config activation

- config changes take effect only after restart
- hot reload is not part of v1

## Open Questions / TBD

- exact top-level `config.toml` schema beyond agent tables
- exact field names for env-var reference
- exact value shape for `mode`
- whether `name` is duplicated inside `[agents.<name>]` or derived from table key
- whether `pair_token` is required for `master`, `slave`, or both
- config validation error projection format
- `name` is validated against the `[agents.<name>]` table key
- `pair_token` is required for both `master` and `slave`
- `pair_token` is the environment variable name, resolved at startup selection time

## Update trigger

Update this doc when:

- config file location changes
- agent table layout changes
- required field set changes
- startup/restart semantics change
