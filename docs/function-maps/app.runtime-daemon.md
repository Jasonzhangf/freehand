# Function Map: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- module_registry: `docs/module-registry/app.runtime-daemon.json`
- verification_map: `docs/verification-maps/app.runtime-daemon.json`
- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `runtime_daemon_host`
- resource operations:
  - `runtime_daemon_host.supervise_launchd_lifetime`
  - `runtime_agent_activity.merge_for_presence`
- owner entry symbols:
  - `main`
  - `daemon_exit_code`
  - `run`
  - `run_master_mode`
  - `relay_startup_auth`
  - `monitor_master_lifecycle_runner`
  - `run_worker_mode`
  - `parse_worker_bind_arg`
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
  - `runtime_daemon_host.host_runtime_transport` (`runtime_daemon_host` -> `runtime_daemon_host`)
  - `runtime_daemon_host.validate_local_adp_token` (`runtime_daemon_host` -> `runtime_daemon_host`)
  - `runtime_daemon_host.supervise_launchd_lifetime` (`runtime_daemon_host` -> `runtime_daemon_host`)
  - `runtime_agent_activity.merge_for_presence` (`runtime_agent_activity` -> `agent_presence`)
- forbidden shortcuts:
  - runtime daemon may host `RelayService` only through the `relay.transport` public API; it must not own account, presence, or proxy semantics.
  - daemon may map typed activity into Relay control heartbeats only; it must not merge activity into ADP/UI business payloads.

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process accepts `remote-relay` and requires `FREEHAND_RELAY_BIND` to start a standalone account-scoped relay transport service
- daemon process may be started by macOS launchd through the installed `freehand-daemon-launchd` wrapper with explicit Android update manifest/APK env paths staged under runtime home; the wrapper rejects an existing blocked guard before spawning another daemon
- each configured Worker process has an agent-specific launchd label, env file,
  stdout log, and stderr log; a shared `workerS` service is not the Worker pool
- daemon bootstrap selects one agent from default config and creates one runtime dispatcher
- daemon bootstrap routes Master mode to the runtime-backed UI host; every Slave
  binds its configured `local_web_url` as a WebUI/ADP host while its Worker loop
  runs in the same process, preserving one process and session namespace per Agent
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
- daemon can run as a launchd user service with fixed WebUI port, Tailscale-IP bind when available, `RunAtLoad`, `KeepAlive.SuccessfulExit=false`, `ThrottleInterval`, stdout/stderr logs under `~/.freehand/logs`, guard state under `~/.freehand/state/launchd`, and Android update distribution paths under `~/.freehand/dist/android`
- daemon launchd wrapper supervises the explicit `FREEHAND_DAEMON_BIN` from `~/.freehand/daemon.env` instead of resolving a possibly stale daemon from `PATH`; a stable service remains running while permanent startup failures and bounded rapid-failure storms return successful wrapper termination so launchd stops retrying
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
- daemon startup/config/bootstrap failure exits `78`; a post-start host failure exits `75`; the launchd wrapper blocks on `78`, retries bounded nonzero runtime failures, and blocks when the rapid-failure limit is reached
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
- `run_launchd_wrapper`
  - owner: `scripts/freehand-daemon-launchd.sh`
  - purpose: supervise only daemon process/service lifetime and persist bounded retry or blocked control state
  - allowed callers: launchd-managed Master and Worker profiles
  - related tests: `scripts/verify-launchd-restart-guard.sh`, `scripts/verify-launchd-restart-guard-online.sh`
  - why shared: one launchd lifetime policy prevents Master/Worker profile drift without owning AgentLifecycle or business truth
- `launchd_pid_for_agent`
  - owner: `scripts/verify-master-three-worker-e2e-online.sh`
  - purpose: read the label-scoped guarded child PID used by the isolated launchd Worker observer
  - allowed callers: `wait_for_launchd_worker_pid`
  - related tests: `scripts/verify-master-three-worker-e2e-online.sh`
  - why shared: keeps launchd readiness observation on the daemon host control projection instead of runtime Worker truth

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `daemon_exit_code` | `apps/freehand-daemon/src/main.rs` | classify a daemon error after the process entrypoint observes whether the host service started | service-started control truth plus daemon error | `EX_CONFIG=78` before host start or `EX_TEMPFAIL=75` after host start | `main` | daemon process exit status | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime/bootstrap helpers | bound |
| 02a | `run` | `apps/freehand-daemon/src/main.rs` | route the explicit remote-relay compatibility command into a host-only Relay startup path | remote-relay command + Relay environment | compatibility host startup result | `run` | `run_remote_relay_mode` | bound |
| 02b-02f | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | call each adjacent compatibility-host dependency explicitly while Relay retains all semantics | Relay environment + initialized store | serve lifetime or explicit config/load/init/bind/serve error | `run_remote_relay_mode` | `RelayServerConfig::from_env`, `RelayStore::load`, `RelayService::new`, `TcpListener::bind`, `RelayService::serve` | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host/port semantics | bind flag value | socket address | daemon CLI runner | bind parser | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup/tests | `freehand-runtime` | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener + shared state + dispatch port | live HTTP/SSE boundary | daemon host | shared transport owner | bound |
| 06 | `handle_adp_socket` / `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP WebSocket command/query/subscribe frames on the daemon host | WebSocket ADP frames + shared protocol state + dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner | bound |
| 06a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge | bound |
| 09 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | enter the label-scoped launchd wrapper and validate service-control inputs before supervising a daemon child | macOS launchd invocation plus wrapper environment | validated wrapper supervision state or explicit blocked wrapper state | macOS launchd | wrapper validation and guard-state admission | bound |
| 09a | `sanitize_launchd_component` | `scripts/install-launchd.sh` | derive deterministic agent-specific Worker label/env/log components | configured Worker agent id | launchd-safe identity component | launchd installer | Worker service path builder | bound |
| 09b | `enable_launchd_service` | `scripts/install-launchd.sh` | enable persistent production LaunchAgents unless an isolated verifier explicitly skips enable overrides | install/restart profile | launchctl enable or no persistent override | launchd installer | `launchctl enable` | bound |
| 09c | `stage_android_update_dist_if_available` | `scripts/install-launchd.sh` | stage complete Android update artifacts without taking Relay deployment ownership | install/restart profile plus complete Android artifacts | runtime-home Android distribution or explicit failure | launchd installer | runtime-home Android distribution filesystem | bound |
| 09d | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | spawn and wait for the explicitly configured daemon child, then classify its exit into retry or blocked wrapper control state | validated daemon env plus label-scoped retry state | running daemon child, retryable wrapper exit, or blocked wrapper exit | `run_launchd_wrapper` | `FREEHAND_DAEMON_BIN serve` | bound |
| 09e | `write_launchd_plist` | `scripts/install-launchd.sh` | write the selected label's wrapper-backed LaunchAgent policy and service-control environment | selected launchd profile plus wrapper paths | label-scoped LaunchAgent plist | `prepare_launchd_service` | selected LaunchAgent plist | bound |
| 09f | `install_launchd_wrapper` | `scripts/install-launchd.sh` | install the current repository wrapper at the selected profile's executable path | repository wrapper plus selected profile wrapper path | installed executable wrapper or explicit install failure | `activate_launchd_service` | `install` | bound |
| 09g | `stop_launchd_service` | `scripts/install-launchd.sh` | stop only the selected LaunchAgent before its guard state may be cleared | selected launchd domain and plist | selected service stopped or already absent | `activate_launchd_service` | `launchctl bootout` | bound |
| 09h | `clear_launchd_guard_after_shutdown` | `scripts/install-launchd.sh` | clear only the selected label's blocked guard after the stop step returns | stopped selected service plus label-scoped guard path | selected guard absent or explicit filesystem failure | `activate_launchd_service` | label-scoped guard file | bound |
| 09i | `bootstrap_launchd_service` | `scripts/install-launchd.sh` | bootstrap the prepared wrapper-backed plist for the selected LaunchAgent | prepared selected LaunchAgent plist | bootstrapped selected service or explicit bootstrap failure | `activate_launchd_service` | `launchctl bootstrap` | bound |
| 09j | `launchd_pid_for_agent` | `scripts/verify-master-three-worker-e2e-online.sh` | read the guarded child daemon PID from label-scoped launchd state for the isolated Worker service observer | isolated Worker label | current guarded child daemon PID or explicit missing-state error | `wait_for_launchd_worker_pid` | `~/.freehand/state/launchd/<label>.json` | bound |
| 08b | `handle_adp_socket` / `RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs` / `crates/freehand-runtime/src/lib.rs` | serve daemon ADP error-center query and initial subscription snapshots from runtime metadata truth | ADP error-center query or subscribe frame | ADP error-center query result or initial subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime metadata projection owner | bound |
| 10 | `relay_startup_auth` | `apps/freehand-daemon/src/main.rs` | validate configured Relay role plus local ADP auth token before Master binds the host listener, returning a typed pre-bind startup error on failure | Master bootstrap + configured Relay connection | validated (local_adp_token, role) tuple or explicit pre-bind startup failure | `run_master_mode` | `relay_startup_auth` | bound |
| 10a | `run_master_mode` | `apps/freehand-daemon/src/main.rs` | bind the host listener, build the runtime dispatcher, then construct the configured Relay Agent client using the validated local ADP token | validated (local_adp_token, role) tuple + bind address + bootstrap | bound listener + optional RelayAgentClient or explicit host construction failure | daemon CLI | `TcpListener::bind` + `RuntimeCommandDispatcher::from_selected_agent_with_live` + `RelayAgentClient::new_with_presence_source` | bound |
| 10b | `monitor_master_lifecycle_runner` | `apps/freehand-daemon/src/main.rs` | spawn the background Master lifecycle runner and monitor its stop/error without treating it as a daemon host crash; host-observed unexpected stop/panic returns to the runner activity owner as terminal Error | agent_name + master_runner + cancel token + runner task handle | explicit stderr lifecycle-runner stop/error evidence or typed terminal activity | `run_master_mode` | `ProductionMasterRunner::run_until` + `tokio::task::spawn_blocking` | bound |
| 10c | `run_master_mode` / `run_relay_worker_mode` / `relay_presence_from_runtime` | `apps/freehand-daemon/src/main.rs` | join one configured Agent outbound Relay client to its role-correct host lifetime, merge foreground dispatcher activity with background Master/Worker owner activity, and project only the typed result into the Relay control side-channel | selected Relay connection + loopback listener + foreground/background runtime activity projections | WebUI host result or explicit Relay client terminal error plus typed role/status/count heartbeat | daemon role host | `RelayAgentClient::run` | bound |
| 11 | `run_blocking_worker_service` | `apps/freehand-daemon/src/main.rs` | isolate the non-Relay synchronous Worker/provider loop from the daemon async runtime thread | Worker service closure | Worker service result or explicit join failure | `run_worker_mode` | `tokio::task::spawn_blocking` | bound |
| 11a | `run_relay_worker_mode` | `apps/freehand-daemon/src/main.rs` | spawn `ProductionWorkerRunner::run_until` with the same explicit cancellation token owned by the loopback host and Relay client lifetime | Relay-configured Worker runner + host cancellation token | cancellable blocking Worker result or explicit join failure | `run_worker_mode` | `ProductionWorkerRunner::run_until` | bound |

## Sync Status Against Code

- daemon bootstrap is bound in code
- Master host-survival monitoring is bound in code: background lifecycle
  runner stop/error no longer terminates the WebUI/ADP host process
- Master host pre-bind validation now binds `runtime_daemon_host.validate_local_adp_token` and refuses to construct `RelayAgentClient` without `FREEHAND_ADP_AUTH_TOKEN`
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
- launchd restart guard is bound by offline and real isolated LaunchAgent
  verifiers: permanent startup failure plateaus at one run, transient failure
  restarts, and repeated rapid transient failure plateaus at the configured limit
- Relay account, presence, proxy, and deployment truth is owned by `relay.transport`; daemon `remote-relay` mode is a compatibility process host over that public API and owns no Relay semantics. Daemon install/restart no longer starts or configures a Relay service.
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
