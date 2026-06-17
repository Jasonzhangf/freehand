# Mainline Calls

This directory is the machine-readable mainline call source of truth for migrated features.

- each file under `docs/mainline-calls/*.json` is a machine-readable mainline call source
- each source file binds one `feature_id` to owner, request mainline, response mainline, error mainline, shared functions, and call table rows
- human-readable function maps remain under `docs/function-maps/`
- generated wiki artifacts live under `docs/wiki/`
- generate wiki with `cargo run -p xtask -- mainlines generate`
- validate wiki freshness with `cargo run -p xtask -- mainlines check`
- `docs/wiki/**` is generated wiki output and must not be edited by hand

Current migrated features:

- `foundation.workspace`
- `provider.anthropic-adapter`
- `provider.openai-adapter`
- `provider.reason-live-bridge`
- `provider.semantic`
- `tool.registry`
- `ui.protocol`
- `reason.turn`
- `reason.persistence`
- `reason.session-history`
- `reason.rewrite-policy`
- `reason.context-planner`
- `debug.core`
- `runtime.ui-command-dispatch`
