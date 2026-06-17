# Wiki: `app.runtime-daemon`

Generated from `docs/mainline-calls/app.runtime-daemon.json`. Do not edit by hand.

- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- function map: `docs/function-maps/app.runtime-daemon.md`
- generated wiki: `docs/wiki/app.runtime-daemon.md`
- test design: `docs/testing/app.runtime-daemon.md`

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- runtime bootstrap consumes configured local and paired node topology before daemon transport starts
- if persisted runtime turn truth exists, daemon bootstrap restores it through the injected runtime owner before serving query and SSE routes
- daemon injects the runtime dispatcher and its shared UI state into the protocol-only HTTP and SSE transport
- mutation commands travel through protocol-owned ingress validation and dispatch envelope building before runtime dispatch
- explicit checkpoint rewind can travel through the same HTTP command ingress without adding app-owned business logic
- checkpoint summary query travels through the shared protocol-only HTTP query route from runtime-populated UI state

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon serves query and continuous SSE projections from the runtime-owned shared UI state
- daemon restart can serve restored terminal projection before any new submit arrives
- daemon SSE subscriptions stay open across later runtime turn updates and observe the same protocol-owned projections as query consumers
- daemon can rewind a previously checkpointed writable-tool mutation through runtime owner dispatch while leaving turn/session/UI truth untouched
- daemon can serve checkpoint summary query results after writable mutation and after explicit rewind without reading checkpoint files in app code
- daemon remains a host process and does not own reason or node semantics itself

## Error Mainline

- invalid daemon CLI input returns explicit startup error
- runtime dispatcher bootstrap failure returns explicit daemon startup error
- runtime checkpoint projection bootstrap failure returns explicit daemon startup error
- runtime dispatch failures return protocol-mapped HTTP failures through the shared transport layer
- missing checkpoint rewind manifests surface protocol-mapped target-not-found failure over the same HTTP command ingress
- slave-mode agent selection returns explicit daemon startup error
- async command ingress does not execute injected synchronous provider or runtime work inline; it returns explicit transport failure if the dispatch task itself fails

## Shared Multi-Reference Functions

- `serve_webui_listener`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: provide one protocol-only HTTP and SSE transport implementation for both smoke and runtime host apps
  - allowed callers: apps/freehand-server, apps/freehand-daemon
  - related tests: WebUI transport smoke, daemon submit and query smoke
  - why shared: avoids a duplicate second copy of UI transport behavior
- `RuntimeCommandDispatcher::dispatch`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: execute protocol-owned dispatch envelope against runtime owner modules
  - allowed callers: runtime host apps, runtime tests
  - related tests: runtime dispatch receipt smoke
  - why shared: keeps reason and node command execution outside app boundary
- `RuntimeCommandDispatcher::from_default_config`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: load default config and bootstrap runtime dispatcher for one selected agent
  - allowed callers: runtime host app, bootstrap tests
  - related tests: config-selected bootstrap smoke
  - why shared: keeps startup config selection out of app host glue while preserving one-process-one-agent flow
- `RuntimeCommandDispatcher::refresh_checkpoint_projection_from_config`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: populate protocol state with runtime-owned checkpoint summaries for daemon HTTP query consumers
  - allowed callers: runtime dispatcher bootstrap, runtime submit dispatch, runtime rewind dispatch
  - related tests: daemon checkpoint rewind HTTP smoke
  - why shared: keeps checkpoint projection refresh in runtime owner instead of app host code

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `main` | `apps/freehand-daemon/src/main.rs` | launch daemon process entrypoint and forward to CLI runner | process entry | process exit result | operator or service manager | app host entrypoint | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime and bootstrap helpers | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host and port semantics | bind flag value | socket address | daemon CLI runner | bind parser | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup or tests | `freehand-runtime` | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener plus shared state plus dispatch port | live HTTP and SSE boundary | daemon host | shared transport owner | bound |
| 06 | `handle_query_checkpoints` | `apps/freehand-server/src/lib.rs` | serve checkpoint summaries from injected protocol state | HTTP checkpoint query | UI checkpoint snapshot JSON | daemon-hosted WebUI transport | protocol state | bound |

## Sync Status Against Mainline Call

- daemon bootstrap is bound in code
- daemon now injects `RuntimeCommandDispatcher` into shared protocol-only HTTP and SSE transport
- provider-backed submit, query, continuous-SSE restore, provider-failure surfacing, restart resume of turn-id allocation, direct-message HTTP smoke, checkpoint rewind HTTP smoke, and missing-checkpoint rewind HTTP failure smoke are covered through the daemon app boundary
- checkpoint query projection is covered through daemon HTTP after writable mutation and after rewind
- config-selected bootstrap is now bound in code and uses configured peer topology
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
