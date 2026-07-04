# Wiki: `app.runtime-daemon`

Generated from `docs/mainline-calls/app.runtime-daemon.json`. Do not edit by hand.

- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- function map: `docs/function-maps/app.runtime-daemon.md`
- generated wiki: `docs/wiki/app.runtime-daemon.md`
- test design: `docs/testing/app.runtime-daemon.md`

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process may be started by macOS launchd through the installed freehand-daemon-launchd wrapper
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- runtime bootstrap consumes configured local and paired node topology before daemon transport starts
- if persisted runtime turn truth exists, daemon bootstrap restores it through the injected runtime owner before serving query and SSE routes
- daemon injects the runtime dispatcher and its shared UI state into the protocol-only HTTP and SSE transport
- daemon injects the same runtime dispatcher as the protocol-owned runtime query port for ADP read-only owner queries, including task list/history and error-center metadata
- daemon exposes the same runtime dispatcher and shared UI state through protocol-owned ADP WebSocket frames at /adp
- mutation commands travel through protocol-owned ingress validation and dispatch envelope building before runtime dispatch
- explicit checkpoint rewind can travel through the same HTTP command ingress without adding app-owned business logic
- checkpoint summary query travels through the shared protocol-only HTTP query route from runtime-populated UI state
- daemon ADP WebSocket accepts task list and error-center subscriptions through the shared protocol transport and injected runtime query port

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon can run as a launchd user service with fixed WebUI bind, RunAtLoad, KeepAlive, explicit FREEHAND_DAEMON_BIN, and stdout/stderr logs under ~/.freehand/logs
- daemon serves query and continuous SSE projections from the runtime-owned shared UI state
- daemon serves ADP WebSocket command/query/subscribe frames from the same runtime-owned shared UI state and runtime query port, so WebUI, Android, and CLI automation can use one control/status path
- daemon serves task list/history ADP query results through runtime's task owner bridge without becoming task truth owner
- daemon serves error-center ADP query results through runtime's metadata query bridge without becoming error-center or metadata truth owner
- daemon restart can serve restored terminal projection before any new submit arrives
- daemon SSE subscriptions stay open across later runtime turn updates and observe the same protocol-owned projections as query consumers
- daemon can rewind a previously checkpointed writable-tool mutation through runtime owner dispatch while leaving turn/session/UI truth untouched
- daemon can serve checkpoint summary query results after writable mutation and after explicit rewind without reading checkpoint files in app code
- daemon remains a host process and does not own reason or node semantics itself
- daemon serves task list subscription events from runtime-published task projections after task tool mutations
- daemon serves error-center initial subscription snapshots from runtime metadata projection

## Error Mainline

- invalid daemon CLI input returns explicit startup error
- missing daemon env file, missing launchd wrapper env values, or missing executable daemon binary returns explicit wrapper startup error
- runtime dispatcher bootstrap failure returns explicit daemon startup error
- runtime checkpoint projection bootstrap failure returns explicit daemon startup error
- corrupt checkpoint projection bootstrap truth returns explicit daemon startup error before transport serve
- runtime dispatch failures return protocol-mapped HTTP failures through the shared transport layer
- ADP command/query/subscribe misuse returns explicit protocol failure frames on the WebSocket connection
- task query misses return explicit ADP target-not-found failure frames from the runtime query bridge
- error-center query/projection failures return explicit ADP failure frames from the runtime query bridge
- missing checkpoint rewind manifests surface protocol-mapped target-not-found failure over the same HTTP command ingress
- slave-mode agent selection returns explicit daemon startup error
- async command ingress does not execute injected synchronous provider or runtime work inline; it returns explicit transport failure if the dispatch task itself fails
- task and error-center subscription initial query/projection failures surface explicit ADP failure frames

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
- `handle_adp_socket`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: serve protocol-owned ADP WebSocket command/query/subscribe frames for daemon-hosted UI and headless automation clients
  - allowed callers: apps/freehand-server, apps/freehand-daemon
  - related tests: daemon ADP command/query/subscribe smoke, daemon ADP query-as-command rejection smoke
  - why shared: keeps WebUI, Android, CLI, and daemon automation on one protocol transport instead of duplicating state/control access

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `main` | `apps/freehand-daemon/src/main.rs` | launch daemon process entrypoint and forward to CLI runner | process entry | process exit result | operator or service manager | app host entrypoint | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime and bootstrap helpers | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host and port semantics | bind flag value | socket address | daemon CLI runner | bind parser | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup or tests | `freehand-runtime` | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener plus shared state plus dispatch port | live HTTP and SSE boundary | daemon host | shared transport owner | bound |
| 06 | `handle_query_checkpoints` | `apps/freehand-server/src/lib.rs` | serve checkpoint summaries from injected protocol state | HTTP checkpoint query | UI checkpoint snapshot JSON | daemon-hosted WebUI transport | protocol state | bound |
| 07 | `handle_adp_socket` | `apps/freehand-server/src/lib.rs` | upgrade daemon-hosted ADP WebSocket connections into protocol-owned command/query/subscribe frame handling | WebSocket ADP frames plus shared protocol state plus dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner | bound |
| 08 | `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP command/query/subscribe frames and matching subscription events on one connection | WebSocket ADP connection plus shared protocol state plus dispatch port | ADP response frames and subscription events | ADP socket route | protocol state and runtime dispatch port | bound |
| 08a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge | bound |
| 09 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | load daemon env and exec the configured installed daemon binary on the fixed service bind | ~/.freehand/daemon.env | daemon process exec | macOS launchd | FREEHAND_DAEMON_BIN serve | bound |
| 09 | `handle_adp_socket / RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs / crates/freehand-runtime/src/lib.rs` | serve daemon ADP task list/error-center query and subscribe surfaces from runtime owner truth | ADP task or error-center query/subscribe frame | ADP task/error-center query result or subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime query/projection owner | bound |

## Sync Status Against Mainline Call

- daemon bootstrap is bound in code
- daemon now injects `RuntimeCommandDispatcher` into shared protocol-only HTTP and SSE transport
- provider-backed submit, query, continuous-SSE restore, provider-failure surfacing, restart resume of turn-id allocation, direct-message HTTP smoke, checkpoint rewind HTTP smoke, missing-checkpoint rewind HTTP failure smoke, and corrupt-checkpoint-bootstrap startup smoke are covered through the daemon app boundary
- ADP WebSocket command/query/subscribe control is covered through the daemon app boundary, including query-as-command rejection
- ADP task list/history query control is covered through the daemon app boundary
- ADP error-center metadata query control is covered through the daemon app boundary
- checkpoint query projection is covered through daemon HTTP after writable mutation and after rewind
- config-selected bootstrap is now bound in code and uses configured peer topology
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
