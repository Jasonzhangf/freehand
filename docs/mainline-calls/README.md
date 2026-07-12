# Mainline Calls

This directory is the machine-readable mainline call source of truth for migrated features.

- each file under `docs/mainline-calls/*.json` is a machine-readable mainline call source
- each source file binds one `feature_id` to owner, request mainline, response mainline, error mainline, shared functions, and call table rows
- source files that implement resource operations must include `resource_operations` entries matching `docs/resource-maps/core.json`
- bound resource operations must also be referenced from at least one `call_table` row through `source_resource`, `target_resource`, and `resource_operation`
- row resource endpoints are checked against the resource operation binding by `xtask gates check`
- bound row-level resource edges must also appear in `docs/resource-maps/core.json` `source_edge_registry`; mainline rows are backlinks, not the top-level resource-edge registry
- human-readable function maps remain under `docs/function-maps/`
- generated wiki artifacts live under `docs/wiki/`
- generate wiki with `cargo run -p xtask -- mainlines generate`
- validate wiki freshness with `cargo run -p xtask -- mainlines check`
- `docs/wiki/**` is generated wiki output and must not be edited by hand

Current migrated features:

- `app.runtime-daemon`
- `app.webui-smoke`
- `app.cli-live-turn`
- `app.cli-runtime-smoke`
- `contracts.core`
- `config.core`
- `foundation.workspace`
- `node.master-slave`
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
- `metadata.core`
- `runtime.ui-command-dispatch`
