# Function Map: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `runtime_daemon_host`
- resource operations:
  - `runtime_agent_activity.merge_for_presence`
- owner entry symbols:
  - `main`
  - `run`
  - `run_master_mode`
  - `monitor_master_lifecycle_runner`
  - `run_worker_mode`
  - `run_blocking_worker_service`
  - `build_runtime_dispatcher_from_default_config`
  - `parse_bind_arg`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `runtime_daemon_host`
- touched resources:
  - `runtime_command`
  - `ui_projection`
  - `agent`
  - `runtime_agent_activity`
- resource operations:
  - `runtime_agent_activity.merge_for_presence` (`runtime_agent_activity` -> `agent_presence`)
- forbidden shortcuts:
  - runtime daemon may host `RelayService` only through the `relay.transport` public API; it must not own account, presence, or proxy semantics.
  - daemon may map typed activity into Relay control heartbeats only; it must not merge activity into ADP/UI business payloads.

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process accepts `remote-relay` and requires `FREEHAND_RELAY_BIND` to start a standalone account-scoped relay transport service
- daemon process may be started by macOS launchd through the installed `freehand-daemon-launchd` wrapper with explicit Android update manifest/APK env paths staged under runtime home
- each configured Worker process has an agent-specific launchd label, env file,
  stdout log, and stderr log; a shared `workerS` service is not the Worker pool
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- daemon bootstrap routes Master mode to the runtime-backed UI host; Slave mode
  without Relay runs only `runtime.master-worker-loop`, while Relay-configured
  Slave mode also binds a loopback UI/ADP host for that Worker's own session
  namespace
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

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon can run as a launchd user service with fixed WebUI port, Tailscale-IP bind when available, `RunAtLoad`, `KeepAlive`, stdout/stderr logs under `~/.freehand/logs`, and Android update distribution paths under `~/.freehand/dist/android`
- daemon launchd wrapper execs the explicit `FREEHAND_DAEMON_BIN` from `~/.freehand/daemon.env` instead of resolving a possibly stale daemon from `PATH`
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
- each Slave daemon runs one configured Worker's production claim/execute/report
  loop; only an explicitly Relay-configured Slave binds a loopback WebUI/ADP
  transport, and that host uses Worker execution policy rather than Master
  orchestration semantics
- ADP AgentBoard/AgentLifecycle queries expose owner-projected Worker process
  health and restart identity without app-owned PID logic

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

## Shared Multi-Reference Functions

- `serve_webui_listener`
  - owner: `apps/freehand-server/src/lib.rs`
  - purpose: provide one protocol-only HTTP/SSE transport implementation for both smoke and runtime host apps
  - allowed callers: `apps/freehand-server`, `apps/freehand-daemon`
  - related tests: WebUI transport smoke, daemon submit/query smoke
  - why shared: avoids a duplicate second copy of UI transport behavior
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

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `main` | `apps/freehand-daemon/src/main.rs` | launch daemon process entrypoint and forward to CLI runner | process entry | process exit result | operator/service manager | app host entrypoint | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime/bootstrap helpers | bound |
| 02a | `run` | `apps/freehand-daemon/src/main.rs` | route the explicit remote-relay compatibility command into a host-only Relay startup path | remote-relay command + Relay environment | compatibility host startup result | `run` | `run_remote_relay_mode` | bound |
| 02b-02f | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | call each adjacent compatibility-host dependency explicitly while Relay retains all semantics | Relay environment + initialized store | serve lifetime or explicit config/load/init/bind/serve error | `run_remote_relay_mode` | `RelayServerConfig::from_env`, `RelayStore::load`, `RelayService::new`, `TcpListener::bind`, `RelayService::serve` | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host/port semantics | bind flag value | socket address | daemon CLI runner | bind parser | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup/tests | `freehand-runtime` | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener + shared state + dispatch port | live HTTP/SSE boundary | daemon host | shared transport owner | bound |
| 06 | `handle_adp_socket` / `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP WebSocket command/query/subscribe frames on the daemon host | WebSocket ADP frames + shared protocol state + dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner | bound |
| 06a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge | bound |
| 07 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | load daemon env and exec the configured installed daemon binary on the fixed service bind | `~/.freehand/daemon.env` | daemon process exec | macOS launchd | `FREEHAND_DAEMON_BIN serve` | bound |
| 07a | `default_daemon_bind` / `detect_tailscale_ip` | `scripts/install-launchd.sh` | choose launchd default bind as the local Tailscale IPv4 on the fixed release/S port when Tailscale is available, otherwise fall back to loopback | install/restart profile | `<tailscale-ip>:4041` / `<tailscale-ip>:4042` or loopback fallback | launchd installer | `tailscale ip -4` | bound |
| 07b | `sanitize_launchd_component` | `scripts/install-launchd.sh` | derive deterministic agent-specific Worker label/env/log components | configured Worker agent id | launchd-safe identity component | launchd installer | Worker service path builder | bound |
| 07c | `enable_launchd_service` | `scripts/install-launchd.sh` | enable persistent production LaunchAgents unless an isolated verifier explicitly skips enable overrides | install/restart profile | launchctl enable or no persistent override | launchd installer | `launchctl enable` | bound |
| 08 | `handle_adp_socket` / `RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs` / `crates/freehand-runtime/src/lib.rs` | serve daemon ADP error-center query and initial subscription snapshots from runtime metadata truth | ADP error-center query or subscribe frame | ADP error-center query result or initial subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime metadata projection owner | bound |
| 09 | `run_master_mode` / `monitor_master_lifecycle_runner` | `apps/freehand-daemon/src/main.rs` | run WebUI/ADP as the Master host lifetime while monitoring the background Master lifecycle runner stop/error without treating it as a daemon host crash; host-observed unexpected stop/panic returns to the runner activity owner as terminal Error | Master bootstrap + bind + lifecycle runner task + cancellation truth | healthy HTTP/ADP host plus explicit stderr lifecycle-runner stop/error evidence and typed terminal activity | daemon CLI | shared WebUI transport + `ProductionMasterRunner::run_until` | bound |
| 10 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | route configured Slave mode into either the production Worker-only lifetime or one Relay-configured loopback UI/ADP + tunnel + cancellable Worker lifetime without app-owned health inference | selected Slave bootstrap | long-running Worker service whose process truth is written by agent.lifecycle plus optional Relay transport result | daemon CLI | `run_blocking_worker_service` / `serve_webui_listener` / `RelayAgentClient::run` | bound |
| 10a | `run_master_mode` / `run_relay_worker_mode` / `relay_presence_from_runtime` | `apps/freehand-daemon/src/main.rs` | join one configured Agent outbound Relay client to its role-correct host lifetime, merge foreground dispatcher activity with background Master/Worker owner activity, and project only the typed result into the Relay control side-channel | selected Relay connection + loopback listener + foreground/background runtime activity projections | WebUI host result or explicit Relay client terminal error plus typed role/status/count heartbeat | daemon role host | `RelayAgentClient::run` | bound |
| 11 | `run_blocking_worker_service` | `apps/freehand-daemon/src/main.rs` | isolate the non-Relay synchronous Worker/provider loop from the daemon async runtime thread | Worker service closure | Worker service result or explicit join failure | `run_worker_mode` | `tokio::task::spawn_blocking` | bound |
| 11a | `run_relay_worker_mode` | `apps/freehand-daemon/src/main.rs` | spawn `ProductionWorkerRunner::run_until` with the same explicit cancellation token owned by the loopback host and Relay client lifetime | Relay-configured Worker runner + host cancellation token | cancellable blocking Worker result or explicit join failure | `run_worker_mode` | `ProductionWorkerRunner::run_until` | bound |

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
- Relay account, presence, proxy, and deployment truth is owned by `relay.transport`; daemon `remote-relay` mode is a compatibility process host over that public API and owns no Relay semantics. Daemon install/restart no longer starts or configures a Relay service.
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
