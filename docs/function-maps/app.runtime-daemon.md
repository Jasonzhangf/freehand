# Function Map: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `remote_relay_transport.register_host`
  - `remote_relay_transport.query_account_directory`
  - `remote_relay_transport.proxy_http`
  - `remote_relay_transport.proxy_adp`
- owner entry symbols:
  - `main`
  - `run`
  - `run_remote_relay_mode`
  - `run_master_mode`
  - `monitor_master_lifecycle_runner`
  - `run_worker_mode`
  - `run_blocking_worker_service`
  - `build_runtime_dispatcher_from_default_config`
  - `parse_bind_arg`
  - `RemoteRelayDirectory::publish_host`
  - `RemoteRelayDirectory::account_directory`
  - `handle_relay_daemon_health`
  - `handle_relay_daemon_http_root`
  - `handle_relay_daemon_http`
  - `proxy_relay_daemon_http`
  - `handle_relay_daemon_adp`
  - `relay_adp_socket`
  - `restart_s_profile_relay_if_enabled`
  - `register_relay_host`
  - `wait_for_relay_health`
  - `wait_for_registered_host`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `remote_relay_transport`
- touched resources:
  - `runtime_command`
  - `ui_projection`
  - `agent`
- resource operations:
  - `remote_relay_transport.register_host`
  - `remote_relay_transport.query_account_directory`
  - `remote_relay_transport.proxy_http`
  - `remote_relay_transport.proxy_adp`
- forbidden shortcuts:
  - relay transport must not compile remote daemon config or own QR/bootstrap truth; route through `remote_daemon_registry`.
  - relay transport must not own node directory route scoring; route through `remote_daemon_directory`.
  - relay transport must not create task/session/agent lifecycle truth from pass-through frames.
  - Android must import/load configured daemon endpoints only; it must not own relay directory, route scoring, or pass-through IO.

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process accepts `remote-relay [--bind HOST:PORT]` to start a standalone account-scoped relay transport service
- daemon process may be started by macOS launchd through the installed `freehand-daemon-launchd` wrapper with explicit Android update manifest/APK env paths staged under runtime home
- S-profile launchd restart also restarts `com.freehand.relayS`, which binds the local Tailscale relay port and registers `studio-host` to the S-profile upstream before Android/WebView clients are treated as current
- each configured Worker process has an agent-specific launchd label, env file,
  stdout log, and stderr log; a shared `workerS` service is not the Worker pool
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- daemon bootstrap routes Master mode to the runtime-backed UI host and Slave mode to `runtime.master-worker-loop`
- Master mode starts the WebUI/ADP host as the daemon lifetime and supervises
  the background Master lifecycle runner separately, so a lifecycle runner
  owner-truth stop is observable without taking down HTTP/ADP status surfaces
- Slave runner construction records typed Worker process identity in
  `agent.lifecycle`; daemon/launchd do not infer AgentBoard health
- runtime bootstrap consumes configured local/paired node topology before daemon transport starts
- if persisted runtime turn truth exists, daemon bootstrap restores it through the injected runtime owner before serving query/SSE routes
- daemon injects the runtime dispatcher and its shared UI state into the protocol-only HTTP/SSE transport
- daemon injects the same runtime dispatcher as the protocol-owned runtime query port for ADP read-only owner queries, including task list/history and error-center metadata query/initial subscription snapshots
- daemon exposes the same runtime dispatcher and shared UI state through protocol-owned ADP WebSocket frames at `/adp`
- mutation commands travel through protocol-owned ingress validation and dispatch envelope building before runtime dispatch
- explicit checkpoint rewind can travel through the same HTTP command ingress without adding app-owned business logic
- remote relay accepts explicit host registrations at `/relay/hosts`, stores account/daemon/relay-host/upstream truth in `RemoteRelayDirectory`, and exposes account directory snapshots at `/relay/directory/{account_id}`
- remote relay accepts registered-host HTTP requests under `/relay/daemon/{relay_host_id}/...`, forwards them to the upstream daemon path, preserves query strings, rewrites static WebUI HTML/JS daemon-root paths to the relay namespace, and keeps ADP WebSocket pass-through at `/relay/daemon/{relay_host_id}/adp`

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon can run as a launchd user service with fixed WebUI port, Tailscale-IP bind when available, `RunAtLoad`, `KeepAlive`, stdout/stderr logs under `~/.freehand/logs`, and Android update distribution paths under `~/.freehand/dist/android`
- daemon launchd wrapper execs the explicit `FREEHAND_DAEMON_BIN` from `~/.freehand/daemon.env` instead of resolving a possibly stale daemon from `PATH`
- S-profile relay runs as `com.freehand.relayS` with fixed Tailscale `:44042`, explicit upstream `http://127.0.0.1:4042`, host registration for `studio-host`, and logs/env under `~/.freehand`
- daemon serves query and continuous SSE projections from the runtime-owned shared UI state
- daemon serves ADP WebSocket command/query/subscribe frames from the same runtime-owned shared UI state and runtime query port, so WebUI/Android/CLI automation can use one control/status path
- daemon Master host remains healthy when the background Master lifecycle
  runner stops or returns an owner-truth error; the error is printed to daemon
  stderr instead of being converted into a launchd process exit
- daemon serves task list/history ADP query results and task list subscription events through runtime's task owner bridge without becoming task truth owner
- daemon serves error-center ADP query results and initial subscription projections through runtime's metadata query bridge without becoming error-center or metadata truth owner
- daemon restart can serve restored terminal projection before any new submit arrives
- daemon SSE subscriptions stay open across later runtime turn updates and observe the same protocol-owned projections as query consumers
- daemon can rewind a previously checkpointed writable-tool mutation through runtime owner dispatch while leaving turn/session/UI truth untouched
- daemon remains a host process and does not own reason or node semantics itself
- each Slave daemon runs one configured Worker's production
  claim/execute/report loop without binding WebUI/ADP transport
- ADP AgentBoard/AgentLifecycle queries expose owner-projected Worker process
  health and restart identity without app-owned PID logic
- remote relay returns account-scoped relay directory projections without credential payloads
- remote relay proxies registered daemon WebUI root/assets/query/health HTTP responses to clients under the relay host namespace
- remote relay proxies registered daemon `/adp` WebSocket frames bidirectionally without parsing task/session semantics

## Error Mainline

- invalid daemon CLI input returns explicit startup error
- missing daemon env file, missing launchd wrapper env values, missing executable daemon binary, or incomplete Android update distribution staging returns explicit startup/update-route error instead of silently serving stale APK version truth
- runtime dispatcher bootstrap failure returns explicit daemon startup error
- corrupt checkpoint projection bootstrap truth returns explicit daemon startup error before transport serve
- runtime dispatch failures return protocol-mapped HTTP failures through the shared transport layer
- ADP command/query/subscribe misuse returns explicit protocol failure frames on the WebSocket connection
- task query misses, task subscription initial query failures, and error-center query/projection failures return explicit ADP failure frames from the runtime query bridge
- Master lifecycle runner stop/error is explicit daemon stderr evidence but is
  not a server lifetime error; HTTP/ADP host errors still return daemon errors
  and may stop the host process
- missing checkpoint rewind manifests surface protocol-mapped target-not-found failure over the same HTTP command ingress
- Slave mode rejects UI bind arguments and starts only the configured Worker runner
- Slave Worker service executes behind a blocking boundary; blocking-task panic/join failure returns an explicit daemon error instead of unwinding the async runtime
- async command ingress does not execute injected synchronous provider/runtime work inline; it returns explicit transport failure if the dispatch task itself fails
- remote relay rejects invalid host registrations and unregistered relay host requests explicitly instead of synthesizing fallback endpoints
- remote relay upstream URL/proxy failures surface as relay transport errors and do not become task/session success truth
- S-profile relay launchd/startup failures fail the restart/proof path explicitly instead of leaving Android WebView on a stale relay asset or old live projection

## Shared Multi-Reference Functions

- `serve_webui_listener`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: provide one protocol-only HTTP/SSE transport implementation for both smoke and runtime host apps
  - allowed callers: `apps/freehand-server`, `apps/freehand-daemon`
  - related tests: WebUI transport smoke, daemon submit/query smoke
  - why shared: avoids a duplicate second copy of UI transport behavior
- `serve_remote_relay_listener`
  - owner: `apps/freehand-server/src/remote_relay.rs`
  - purpose: provide one relay transport implementation for host registration, account directory query, namespaced WebUI HTTP proxy, and ADP WebSocket proxy
  - allowed callers: `apps/freehand-server`, `apps/freehand-daemon`
  - related tests: `cargo test -p freehand-server --lib remote_relay -- --nocapture`
  - why shared: avoids mixing relay pass-through IO into Master/Worker runtime host code
- `RuntimeCommandDispatcher::dispatch`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: execute protocol-owned dispatch envelope against runtime owner modules
  - allowed callers: runtime host apps and runtime tests
  - related tests: runtime dispatch receipt smoke
  - why shared: keeps reason/node command execution outside app boundary
  - `RuntimeCommandDispatcher::from_default_config`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: load default config and bootstrap runtime dispatcher for one selected agent
  - allowed callers: runtime host app and bootstrap tests
  - related tests: config-selected bootstrap smoke
  - why shared: keeps startup config selection out of app host glue while preserving one-process-one-agent flow
- `ProductionWorkerRunner::from_default_config`
  - owner: `crates/freehand-runtime/src/worker_runner.rs`
  - purpose: bootstrap one configured Slave process against the paired Master's Task Center namespace
  - allowed callers: daemon Slave host and runtime tests
  - related tests: daemon Worker mode bootstrap, production Worker runner tests
  - why shared: keeps claim/execute/report semantics out of daemon app glue
- `sanitize_launchd_component`
  - owner: `scripts/install-launchd.sh`
  - purpose: derive deterministic agent-specific Worker service/env/log names
  - allowed callers: launchd install/restart worker profiles
  - related tests: shell syntax plus
    `scripts/verify-launchd-worker-naming.sh`
  - why shared: one naming rule prevents different Worker processes from
    overwriting the same service/env/log truth
- `enable_launchd_service`
  - owner: `scripts/install-launchd.sh`
  - purpose: enable persistent production LaunchAgents by default while letting
    isolated online verifiers bootstrap/bootout unique labels without leaving
    launchctl enable overrides
  - allowed callers: launchd install/restart profiles
  - related tests: shell syntax plus
    `scripts/verify-launchd-three-worker-services-online.sh`
  - why shared: keeps production launchd behavior and temporary verifier cleanup
    behind one installer-owned switch
- `scripts/install-relay-launchd.sh`
  - owner: `scripts/install-relay-launchd.sh`
  - purpose: install/restart the relay transport as a first-class launchd service and register its upstream host before mobile clients consume it
  - allowed callers: operator, `scripts/install-launchd.sh` S-profile restart path
  - related tests: shell syntax, plan-only output, S-profile relay restart, relay health, relay ADP smoke, Android WebView DOM proof
  - why shared: prevents a manually started relay process from serving stale WebUI assets after the S-profile daemon is rebuilt

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `main` | `apps/freehand-daemon/src/main.rs` | launch daemon process entrypoint and forward to CLI runner | process entry | process exit result | operator/service manager | app host entrypoint | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime/bootstrap helpers | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host/port semantics | bind flag value | socket address | daemon CLI runner | bind parser | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup/tests | `freehand-runtime` | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener + shared state + dispatch port | live HTTP/SSE boundary | daemon host | shared transport owner | bound |
| 06 | `handle_adp_socket` / `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP WebSocket command/query/subscribe frames on the daemon host | WebSocket ADP frames + shared protocol state + dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner | bound |
| 06a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge | bound |
| 07 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | load daemon env and exec the configured installed daemon binary on the fixed service bind | `~/.freehand/daemon.env` | daemon process exec | macOS launchd | `FREEHAND_DAEMON_BIN serve` | bound |
| 07a | `default_daemon_bind` / `detect_tailscale_ip` | `scripts/install-launchd.sh` | choose launchd default bind as the local Tailscale IPv4 on the fixed release/S port when Tailscale is available, otherwise fall back to loopback | install/restart profile | `<tailscale-ip>:4041` / `<tailscale-ip>:4042` or loopback fallback | launchd installer | `tailscale ip -4` | bound |
| 07b | `sanitize_launchd_component` | `scripts/install-launchd.sh` | derive deterministic agent-specific Worker label/env/log components | configured Worker agent id | launchd-safe identity component | launchd installer | Worker service path builder | bound |
| 07c | `enable_launchd_service` | `scripts/install-launchd.sh` | enable persistent production LaunchAgents unless an isolated verifier explicitly skips enable overrides | install/restart profile | launchctl enable or no persistent override | launchd installer | `launchctl enable` | bound |
| 07d | `restart_s_profile_relay_if_enabled` | `scripts/install-launchd.sh` | after S-profile master restart, call the relay launchd installer unless explicitly skipped | S-profile restart/install command | synchronized `com.freehand.relayS` restart or explicit failure | launchd installer | `scripts/install-relay-launchd.sh restartS` | bound |
| 07e | `register_relay_host` / `wait_for_registered_host` | `scripts/install-relay-launchd.sh` | start the relay launchd service and publish `studio-host` to the S-profile upstream before accepting mobile proof | relay bind/upstream/account/daemon config | relay health plus registered-host upstream health | relay launchd installer | relay `/relay/hosts` and `/relay/daemon/{host}/health` | bound |
| 08 | `handle_adp_socket` / `RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs` / `crates/freehand-runtime/src/lib.rs` | serve daemon ADP error-center query and initial subscription snapshots from runtime metadata truth | ADP error-center query or subscribe frame | ADP error-center query result or initial subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime metadata projection owner | bound |
| 09 | `run_master_mode` / `monitor_master_lifecycle_runner` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP as the Master host lifetime while monitoring the background Master lifecycle runner stop/error without treating it as a daemon host crash | Master bootstrap + bind + lifecycle runner task | healthy HTTP/ADP host plus explicit stderr lifecycle-runner stop/error evidence | daemon CLI | shared WebUI transport + `ProductionMasterRunner::run_until` | bound |
| 10 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | route configured Slave mode into the production Worker runner without UI transport or app-owned health inference | selected Slave bootstrap | long-running Worker service whose process truth is written by agent.lifecycle | daemon CLI | `run_blocking_worker_service` | bound |
| 11 | `run_blocking_worker_service` | `apps/freehand-daemon/src/main.rs` | isolate the synchronous Worker/provider loop from the daemon async runtime thread | Worker service closure | Worker service result or explicit join failure | `run_worker_mode` | `tokio::task::spawn_blocking` | bound |
| 14 | `run_remote_relay_mode` / `serve_remote_relay_listener` | `apps/freehand-daemon/src/main.rs` / `apps/freehand-server/src/remote_relay.rs` | start standalone relay transport service with its own relay directory registry | relay bind address | live relay transport HTTP/WS boundary | daemon CLI | shared relay transport owner | bound |
| 15 | `RemoteRelayDirectory::publish_host` | `apps/freehand-server/src/remote_relay.rs` | register one account/daemon relay host and normalize endpoint candidates | host registration JSON | relay host record | relay `/relay/hosts` route | relay directory owner | bound |
| 16 | `RemoteRelayDirectory::account_directory` | `apps/freehand-server/src/remote_relay.rs` | return account-scoped relay directory snapshot | account id | sorted relay daemon host records | relay `/relay/directory/{account_id}` route | relay directory owner | bound |
| 17 | `proxy_relay_daemon_http` | `apps/freehand-server/src/remote_relay.rs` | proxy registered daemon HTTP requests to upstream WebUI/health/query routes while preserving query strings and rewriting static HTML/JS daemon-root paths to the relay namespace | relay host id + namespaced HTTP path + query | proxied HTTP response or explicit relay error | relay `/relay/daemon/{relay_host_id}/...` routes | reqwest upstream client | bound |
| 18 | `handle_relay_daemon_adp` / `relay_adp_socket` | `apps/freehand-server/src/remote_relay.rs` | proxy registered daemon ADP WebSocket frames bidirectionally to upstream `/adp` | relay host id plus client WebSocket | proxied ADP response frames or explicit relay error | relay `/relay/daemon/{relay_host_id}/adp` route | tokio-tungstenite upstream client | bound |

## Sync Status Against Code

- daemon bootstrap is bound in code
- Master host-survival monitoring is bound in code: background lifecycle
  runner stop/error no longer terminates the WebUI/ADP host process
- daemon now injects `RuntimeCommandDispatcher` into shared protocol-only HTTP/SSE transport
- daemon now injects `RuntimeCommandDispatcher` into shared protocol-only ADP runtime query transport
- provider-backed submit/query/continuous-SSE restore, provider-failure surfacing, restart resume of turn-id allocation, direct-message HTTP smoke, checkpoint rewind HTTP smoke, missing-checkpoint rewind HTTP failure smoke, and corrupt-checkpoint-bootstrap startup smoke are covered through the daemon app boundary
- ADP WebSocket command/query/subscribe control is covered through the daemon app boundary, including query-as-command rejection
- ADP task list/history query control is covered through the daemon app boundary
- ADP error-center metadata query control is covered through the daemon app boundary
- config-selected bootstrap is now bound in code and uses configured peer topology
- configured Slave startup now binds `runtime.master-worker-loop` instead of failing the app host
- configured Slave runner construction is black-box checked for persisted
  process PID/instance/restart truth under `agent.lifecycle`
- Worker launchd defaults now bind identity as
  `com.freehand.worker[ S].<agent>`, `worker[ S].<agent>.env`, and matching
  agent-specific logs
- launchd online verifier now starts three agent-specific Worker services,
  kills gamma, waits for KeepAlive to produce a new PID, and verifies
  AgentBoard owner truth reports the same task/execution plus `restart_count=1`
- remote relay transport is bound in code: focused tests and `scripts/verify-remote-relay-local-online.sh` register a relay host, query the account directory, proxy upstream namespaced WebUI root/assets/query/health HTTP, proxy upstream `/adp`, and prove missing hosts return explicit `relay_host_not_found`
- relay endpoint `authRequired` is directory/route metadata only in this slice;
  relay HTTP/ADP access authentication is not implemented or claimed, so
  exposure must remain on trusted local/Tailscale routes until a dedicated auth
  owner lands with negative online proof
- S-profile relay launchd management is bound in scripts: `scripts/install-launchd.sh restartS` refreshes `com.freehand.daemonS`, stages runtime-home Android update artifacts when repo `dist/android` is complete, then restarts `com.freehand.relayS`, registers `studio-host`, and makes Android/WebView proof consume the current relay-served asset/update URL instead of a manually started stale relay process
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
