# Wiki: `app.runtime-daemon`

Generated from `docs/mainline-calls/app.runtime-daemon.json`. Do not edit by hand.

- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- function map: `docs/function-maps/app.runtime-daemon.md`
- generated wiki: `docs/wiki/app.runtime-daemon.md`
- test design: `docs/testing/app.runtime-daemon.md`

## Resource Operation Backlinks

- remote_relay_transport.register_host
- remote_relay_transport.query_account_directory
- remote_relay_transport.proxy_http
- remote_relay_transport.proxy_adp

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process accepts remote-relay [--bind HOST:PORT] to start a standalone account-scoped relay transport service
- daemon process may be started by macOS launchd through the installed freehand-daemon-launchd wrapper
- each configured Worker process has an agent-specific launchd label, env file, stdout log, and stderr log; a shared workerS service is not the Worker pool
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- daemon bootstrap routes Master mode to the runtime-backed UI host and Slave mode to the production Worker runner
- Slave runner construction records typed Worker process identity in agent.lifecycle; daemon and launchd do not infer AgentBoard health
- runtime bootstrap consumes configured local and paired node topology before daemon transport starts
- if persisted runtime turn truth exists, daemon bootstrap restores it through the injected runtime owner before serving query and SSE routes
- daemon injects the runtime dispatcher and its shared UI state into the protocol-only HTTP and SSE transport
- daemon injects the same runtime dispatcher as the protocol-owned runtime query port for ADP read-only owner queries, including task list/history and error-center metadata
- Master mode keeps the HTTP/WebUI/ADP host lifetime independent from the background Master lifecycle runner; runner stop/error is explicit stderr evidence and does not crash the host
- daemon exposes the same runtime dispatcher and shared UI state through protocol-owned ADP WebSocket frames at /adp
- mutation commands travel through protocol-owned ingress validation and dispatch envelope building before runtime dispatch
- session CRUD and rollback commands travel through the same ADP command path and remain runtime/reason-owned mutations
- explicit checkpoint rewind can travel through the same HTTP command ingress without adding app-owned business logic
- checkpoint summary query travels through the shared protocol-only HTTP query route from runtime-populated UI state
- daemon ADP WebSocket accepts task list and error-center subscriptions through the shared protocol transport and injected runtime query port
- remote relay accepts explicit host registrations at /relay/hosts, stores account/daemon/relay-host/upstream truth in RemoteRelayDirectory, and exposes account directory snapshots at /relay/directory/{account_id}
- remote relay accepts registered-host HTTP requests under /relay/daemon/{relay_host_id}/..., forwards them to the upstream daemon path, preserves query strings, rewrites static WebUI HTML/JS daemon-root paths to the relay namespace, and keeps ADP WebSocket pass-through at /relay/daemon/{relay_host_id}/adp

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon can run as a launchd user service with fixed WebUI bind, RunAtLoad, KeepAlive, explicit FREEHAND_DAEMON_BIN, and stdout/stderr logs under ~/.freehand/logs
- daemon serves query and continuous SSE projections from the runtime-owned shared UI state
- daemon serves ADP WebSocket command/query/subscribe frames from the same runtime-owned shared UI state and runtime query port, so WebUI, Android, and CLI automation can use one control/status path
- daemon Master host remains healthy when the background Master lifecycle runner stops or returns an owner-truth error; the error is printed to daemon stderr instead of being converted into a launchd process exit
- daemon serves task list/history ADP query results through runtime's task owner bridge without becoming task truth owner
- daemon serves error-center ADP query results through runtime's metadata query bridge without becoming error-center or metadata truth owner
- daemon restart can serve restored terminal projection before any new submit arrives
- daemon SSE subscriptions stay open across later runtime turn updates and observe the same protocol-owned projections as query consumers
- daemon can rewind a previously checkpointed writable-tool mutation through runtime owner dispatch while leaving turn/session/UI truth untouched
- daemon can serve checkpoint summary query results after writable mutation and after explicit rewind without reading checkpoint files in app code
- daemon remains a host process and does not own reason or node semantics itself
- daemon serves task list subscription events from runtime-published task projections after task tool mutations
- daemon serves error-center initial subscription snapshots from runtime metadata projection
- ADP AgentBoard and AgentLifecycle queries expose owner-projected Worker process health and restart identity without app-owned PID logic
- daemon ADP session management can create, rename, archive, list archived sessions, restore, submit turns, rollback latest effective turn, and query the resulting effective transcript
- each Slave daemon runs one configured Worker's production claim/execute/report loop without binding WebUI or ADP transport
- remote relay returns account-scoped relay directory projections without credential payloads
- remote relay proxies registered daemon WebUI root/assets/query/health HTTP responses to clients under the relay host namespace
- remote relay proxies registered daemon /adp WebSocket frames bidirectionally without parsing task/session semantics

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
- Master lifecycle runner stop/error is explicit daemon stderr evidence but is not a server lifetime error; HTTP/ADP host errors still return daemon errors and may stop the host process
- missing checkpoint rewind manifests surface protocol-mapped target-not-found failure over the same HTTP command ingress
- Slave mode rejects UI bind arguments instead of starting a mixed Worker and UI host
- async command ingress does not execute injected synchronous provider or runtime work inline; it returns explicit transport failure if the dispatch task itself fails
- task and error-center subscription initial query/projection failures surface explicit ADP failure frames
- session rollback failures surface explicit ADP failure frames instead of app-owned transcript mutation
- remote relay rejects invalid host registrations and unregistered relay host requests explicitly instead of synthesizing fallback endpoints
- remote relay upstream URL/proxy failures surface as relay transport errors and do not become task/session success truth

## Shared Multi-Reference Functions

- `serve_webui_listener`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: provide one protocol-only HTTP and SSE transport implementation for both smoke and runtime host apps
  - allowed callers: apps/freehand-server, apps/freehand-daemon
  - related tests: WebUI transport smoke, daemon submit and query smoke
  - why shared: avoids a duplicate second copy of UI transport behavior
- `serve_remote_relay_listener`
  - owner: `apps/freehand-server/src/remote_relay.rs`
  - purpose: provide one relay transport implementation for host registration, account directory query, namespaced WebUI HTTP proxy, and ADP WebSocket proxy
  - allowed callers: apps/freehand-server, apps/freehand-daemon
  - related tests: cargo test -p freehand-server --lib remote_relay -- --nocapture
  - why shared: avoids mixing relay pass-through IO into Master/Worker runtime host code
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

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `main` | `apps/freehand-daemon/src/main.rs` | launch daemon process entrypoint and forward to CLI runner | process entry | process exit result | operator or service manager | app host entrypoint |  |  |  | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime and bootstrap helpers |  |  |  | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host and port semantics | bind flag value | socket address | daemon CLI runner | bind parser |  |  |  | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup or tests | `freehand-runtime` |  |  |  | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener plus shared state plus dispatch port | live HTTP and SSE boundary | daemon host | shared transport owner |  |  |  | bound |
| 06 | `handle_query_checkpoints` | `apps/freehand-server/src/lib.rs` | serve checkpoint summaries from injected protocol state | HTTP checkpoint query | UI checkpoint snapshot JSON | daemon-hosted WebUI transport | protocol state |  |  |  | bound |
| 07 | `handle_adp_socket` | `apps/freehand-server/src/lib.rs` | upgrade daemon-hosted ADP WebSocket connections into protocol-owned command/query/subscribe frame handling | WebSocket ADP frames plus shared protocol state plus dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner |  |  |  | bound |
| 08 | `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP command/query/subscribe frames and matching subscription events on one connection | WebSocket ADP connection plus shared protocol state plus dispatch port | ADP response frames and subscription events | ADP socket route | protocol state and runtime dispatch port |  |  |  | bound |
| 08a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge |  |  |  | bound |
| 09 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | load daemon env and exec the configured installed daemon binary on the fixed service bind | ~/.freehand/daemon.env | daemon process exec | macOS launchd | FREEHAND_DAEMON_BIN serve |  |  |  | bound |
| 09a | `sanitize_launchd_component` | `scripts/install-launchd.sh` | derive deterministic agent-specific Worker label, env, and log components | configured Worker agent id | launchd-safe identity component | launchd worker install and restart profiles | Worker service path builder |  |  |  | bound |
| 09b | `enable_launchd_service` | `scripts/install-launchd.sh` | enable persistent production LaunchAgents unless an isolated verifier explicitly skips enable overrides | install or restart launchd profile | launchctl enable or no persistent override | launchd install and restart profiles | launchctl enable |  |  |  | bound |
| 09 | `handle_adp_socket / RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs / crates/freehand-runtime/src/lib.rs` | serve daemon ADP task list/error-center query and subscribe surfaces from runtime owner truth | ADP task or error-center query/subscribe frame | ADP task/error-center query result or subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime query/projection owner |  |  |  | bound |
| 10 | `run_master_mode / monitor_master_lifecycle_runner` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP as the Master host lifetime while monitoring the background Master lifecycle runner stop/error without treating it as a daemon host crash | Master bootstrap plus bind plus lifecycle runner task | healthy HTTP/ADP host plus explicit stderr lifecycle-runner stop/error evidence | daemon CLI | shared WebUI transport plus ProductionMasterRunner::run_until |  |  |  | bound |
| 11 | `handle_adp_socket / RuntimeCommandDispatcher::dispatch` | `apps/freehand-server/src/lib.rs / crates/freehand-runtime/src/lib.rs` | serve daemon ADP session CRUD and rollback commands through shared protocol transport and runtime owner dispatch | ADP session management command or session transcript query frame | ADP command receipt plus active/archived/effective transcript query projection | daemon-hosted ADP client | shared WebUI transport plus runtime.ui-command-dispatch |  |  |  | bound |
| 12 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | route configured Slave mode into the production Worker runner without UI transport or app-owned health inference | selected Slave bootstrap | long-running Worker service whose process truth is written by agent.lifecycle | daemon CLI | run_blocking_worker_service |  |  |  | bound |
| 13 | `run_blocking_worker_service` | `apps/freehand-daemon/src/main.rs` | isolate the synchronous Worker and provider loop from the daemon async runtime thread | Worker service closure | Worker service result or explicit blocking-task join failure | run_worker_mode | tokio::task::spawn_blocking |  |  |  | bound |
| 14 | `run_remote_relay_mode / serve_remote_relay_listener` | `apps/freehand-daemon/src/main.rs / apps/freehand-server/src/remote_relay.rs` | start standalone relay transport service with its own relay directory registry | relay bind address | live relay transport HTTP/WS boundary | daemon CLI | shared relay transport owner |  |  |  | bound |
| 15 | `RemoteRelayDirectory::publish_host` | `apps/freehand-server/src/remote_relay.rs` | register one account and daemon relay host and normalize endpoint candidates | host registration JSON | relay host record | relay /relay/hosts route | relay directory owner | remote_relay_transport | remote_relay_transport | remote_relay_transport.register_host | bound |
| 16 | `RemoteRelayDirectory::account_directory` | `apps/freehand-server/src/remote_relay.rs` | return account-scoped relay directory snapshot | account id | sorted relay daemon host records | relay /relay/directory/{account_id} route | relay directory owner | remote_relay_transport | remote_relay_transport | remote_relay_transport.query_account_directory | bound |
| 17 | `proxy_relay_daemon_http` | `apps/freehand-server/src/remote_relay.rs` | proxy registered daemon HTTP requests to upstream WebUI, health, and query routes while preserving query strings and rewriting static HTML/JS daemon-root paths to the relay namespace | relay host id plus namespaced HTTP path plus query | proxied HTTP response or explicit relay error | relay /relay/daemon/{relay_host_id}/... routes | reqwest upstream client | remote_relay_transport | remote_relay_transport | remote_relay_transport.proxy_http | bound |
| 18 | `handle_relay_daemon_adp` | `apps/freehand-server/src/remote_relay.rs` | proxy registered daemon ADP WebSocket frames bidirectionally to upstream /adp | relay host id plus client WebSocket | proxied ADP response frames or explicit relay error | relay /relay/daemon/{relay_host_id}/adp route | tokio-tungstenite upstream client | remote_relay_transport | remote_relay_transport | remote_relay_transport.proxy_adp | bound |

## Sync Status Against Mainline Call

- daemon bootstrap is bound in code
- Master mode host-survival monitoring is bound in code: background lifecycle runner stop/error no longer terminates the WebUI/ADP host process
- daemon now injects `RuntimeCommandDispatcher` into shared protocol-only HTTP and SSE transport
- provider-backed submit, query, continuous-SSE restore, provider-failure surfacing, restart resume of turn-id allocation, direct-message HTTP smoke, checkpoint rewind HTTP smoke, missing-checkpoint rewind HTTP failure smoke, and corrupt-checkpoint-bootstrap startup smoke are covered through the daemon app boundary
- ADP WebSocket command/query/subscribe control is covered through the daemon app boundary, including query-as-command rejection
- ADP task list/history query control is covered through the daemon app boundary
- ADP error-center metadata query control is covered through the daemon app boundary
- checkpoint query projection is covered through daemon HTTP after writable mutation and after rewind
- config-selected bootstrap is now bound in code and uses configured peer topology
- configured Slave startup binds runtime.master-worker-loop instead of failing the app host
- configured Slave runner construction is black-box checked for persisted process PID, instance, alive, and restart truth under agent.lifecycle
- Worker launchd defaults bind identity as com.freehand.worker[ S].<agent>, worker[ S].<agent>.env, and matching agent-specific logs
- agent-specific launchd naming has a non-mutating executable fixture, isolated runtime proof starts three distinct Slave daemon processes, and launchd-managed three-service recovery is proven through KeepAlive restart plus AgentBoard restart-count truth
- remote relay transport is bound in code: focused tests and scripts/verify-remote-relay-local-online.sh register a relay host, query the account directory, proxy upstream namespaced WebUI root/assets/query/health HTTP, proxy upstream /adp, and prove missing hosts return explicit relay_host_not_found
- relay endpoint authRequired is directory/route metadata only in this slice; relay HTTP/ADP access authentication is not implemented or claimed, so exposure must remain on trusted local/Tailscale routes until a dedicated auth owner lands with negative online proof
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
