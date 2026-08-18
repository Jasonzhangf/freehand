# Wiki: `app.runtime-daemon`

Generated from `docs/mainline-calls/app.runtime-daemon.json`. Do not edit by hand.

- owner crate: `apps/freehand-daemon`
- owner module: `apps/freehand-daemon/src/main.rs`
- function map: `docs/function-maps/app.runtime-daemon.md`
- generated wiki: `docs/wiki/app.runtime-daemon.md`
- test design: `docs/testing/app.runtime-daemon.md`

## Resource Operation Backlinks

- runtime_daemon_host.supervise_launchd_lifetime
- runtime_agent_activity.merge_for_presence
- runtime_daemon_host.host_runtime_transport
- runtime_daemon_host.validate_local_adp_token

## Request Mainline

- daemon process accepts a host command to start the UI transport
- daemon process may be started by macOS launchd through the installed freehand-daemon-launchd wrapper with explicit Android update manifest/APK env paths staged under runtime home; the wrapper rejects an existing blocked guard before spawning another daemon
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

## Response Mainline

- daemon serves runtime-backed dispatch receipts over HTTP command ingress
- daemon can run as a launchd user service with fixed WebUI bind, RunAtLoad, KeepAlive.SuccessfulExit=false, ThrottleInterval, explicit FREEHAND_DAEMON_BIN, explicit Android update manifest/APK paths, stdout/stderr logs under ~/.freehand/logs, and label-scoped guard state under ~/.freehand/state/launchd
- daemon launchd wrapper supervises the explicit FREEHAND_DAEMON_BIN; permanent startup failures and bounded rapid-failure storms return successful wrapper termination so launchd stops retrying
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

## Error Mainline

- invalid daemon CLI input returns explicit startup error
- missing daemon env file, missing launchd wrapper env values, missing executable daemon binary, or incomplete Android update distribution staging returns explicit startup/update-route error instead of silently serving stale APK version truth
- daemon startup, config, or bootstrap failure exits 78; a post-start host failure exits 75; the launchd wrapper blocks on 78, retries bounded nonzero runtime failures, and blocks when the rapid-failure limit is reached
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

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `daemon_exit_code` | `apps/freehand-daemon/src/main.rs` | classify a daemon error after the process entrypoint observes whether the host service started | service-started control truth plus daemon error | EX_CONFIG=78 before host start or EX_TEMPFAIL=75 after host start | main | daemon process exit status | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 02 | `run` | `apps/freehand-daemon/src/main.rs` | parse daemon command and bind address, then start runtime-backed host | daemon CLI input | startup result | daemon process | runtime and bootstrap helpers |  |  |  | bound |
| 02a | `run` | `apps/freehand-daemon/src/main.rs` | route the explicit remote-relay compatibility command into a host-only Relay startup path without taking Relay semantic ownership | remote-relay command plus Relay environment | compatibility host startup result | run | run_remote_relay_mode |  |  |  | bound |
| 02b | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | load the Relay server environment through the concrete Relay config owner | Relay environment and initialized persisted store | validated Relay server config or explicit config error | run_remote_relay_mode | RelayServerConfig::from_env |  |  |  | bound |
| 02c | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | load the initialized Relay store through its concrete persistence owner | validated Relay store path | loaded Relay store or explicit load error | run_remote_relay_mode | RelayStore::load |  |  |  | bound |
| 02d | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | construct the Relay service through its semantic owner | loaded store plus presence lease | Relay service or explicit initialization error | run_remote_relay_mode | RelayService::new |  |  |  | bound |
| 02e | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | bind the compatibility-host listener without owning Relay semantics | validated bind address | TCP listener or explicit bind error | run_remote_relay_mode | TcpListener::bind |  |  |  | bound |
| 02f | `run_remote_relay_mode` | `apps/freehand-daemon/src/main.rs` | hand the bound listener to the Relay service owner | Relay service plus bound listener | serve lifetime or explicit serve error | run_remote_relay_mode | RelayService::serve |  |  |  | bound |
| 03 | `parse_bind_arg` | `apps/freehand-daemon/src/main.rs` | parse CLI bind address and default host and port semantics | bind flag value | socket address | daemon CLI runner | bind parser |  |  |  | bound |
| 04 | `build_runtime_dispatcher_from_default_config` | `apps/freehand-daemon/src/main.rs` | select one agent from default config and create the daemon-owned runtime host dependency set | daemon agent name | runtime dispatcher | daemon startup or tests | `freehand-runtime` |  |  |  | bound |
| 05 | `serve_webui_listener` | `apps/freehand-server/src/lib.rs` | serve protocol-only routes while using injected runtime dispatch and shared state | listener plus shared state plus dispatch port | live HTTP and SSE boundary | daemon host | shared transport owner |  |  |  | bound |
| 06 | `handle_query_checkpoints` | `apps/freehand-server/src/lib.rs` | serve checkpoint summaries from injected protocol state | HTTP checkpoint query | UI checkpoint snapshot JSON | daemon-hosted WebUI transport | protocol state |  |  |  | bound |
| 07 | `handle_adp_socket` | `apps/freehand-server/src/lib.rs` | upgrade daemon-hosted ADP WebSocket connections into protocol-owned command/query/subscribe frame handling | WebSocket ADP frames plus shared protocol state plus dispatch port | ADP response frames and subscription events | WebUI/Android/CLI automation | protocol transport owner |  |  |  | bound |
| 08 | `handle_adp_connection` | `apps/freehand-server/src/lib.rs` | serve protocol-owned ADP command/query/subscribe frames and matching subscription events on one connection | WebSocket ADP connection plus shared protocol state plus dispatch port | ADP response frames and subscription events | ADP socket route | protocol state and runtime dispatch port |  |  |  | bound |
| 08a | `RuntimeCommandDispatcher::query_runtime` | `crates/freehand-runtime/src/lib.rs` | serve daemon-hosted read-only runtime query frames such as task list/history and error-center metadata | ADP query command | ADP query result or failure frame | shared ADP transport | runtime owner query bridge |  |  |  | bound |
| 09 | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | enter the label-scoped launchd wrapper and validate service-control inputs before supervising a daemon child | macOS launchd invocation plus wrapper environment | validated wrapper supervision state or explicit blocked wrapper state | macOS launchd | wrapper validation and guard-state admission | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09d | `run_launchd_wrapper` | `scripts/freehand-daemon-launchd.sh` | spawn and wait for the explicitly configured daemon child, then classify its exit into retry or blocked wrapper control state | validated daemon env plus label-scoped retry state | running daemon child, retryable wrapper exit, or blocked wrapper exit | run_launchd_wrapper | FREEHAND_DAEMON_BIN serve | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09e | `write_launchd_plist` | `scripts/install-launchd.sh` | write the selected label's wrapper-backed LaunchAgent policy and service-control environment | selected launchd profile and wrapper paths | label-scoped LaunchAgent plist | prepare_launchd_service | selected LaunchAgent plist | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09f | `install_launchd_wrapper` | `scripts/install-launchd.sh` | install the current repository wrapper at the selected profile's executable path | repository wrapper plus selected profile wrapper path | installed executable wrapper or explicit install failure | activate_launchd_service | install | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09g | `stop_launchd_service` | `scripts/install-launchd.sh` | stop only the selected LaunchAgent before its guard state may be cleared | selected launchd domain and plist | selected service stopped or already absent | activate_launchd_service | launchctl bootout | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09h | `clear_launchd_guard_after_shutdown` | `scripts/install-launchd.sh` | clear only the selected label's blocked guard after the stop step returns | stopped selected service plus label-scoped guard path | selected guard absent or explicit filesystem failure | activate_launchd_service | label-scoped guard file | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09i | `bootstrap_launchd_service` | `scripts/install-launchd.sh` | bootstrap the prepared wrapper-backed plist for the selected LaunchAgent | prepared selected LaunchAgent plist | bootstrapped selected service or explicit bootstrap failure | activate_launchd_service | launchctl bootstrap | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09j | `launchd_pid_for_agent` | `scripts/verify-master-three-worker-e2e-online.sh` | read the guarded child daemon PID from label-scoped launchd state for the isolated Worker service observer | isolated Worker label | current guarded child daemon PID or explicit missing-state error | wait_for_launchd_worker_pid | ~/.freehand/state/launchd/<label>.json | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.supervise_launchd_lifetime | bound |
| 09a | `sanitize_launchd_component` | `scripts/install-launchd.sh` | derive deterministic agent-specific Worker label, env, and log components | configured Worker agent id | launchd-safe identity component | launchd worker install and restart profiles | Worker service path builder |  |  |  | bound |
| 09b | `enable_launchd_service` | `scripts/install-launchd.sh` | enable persistent production LaunchAgents unless an isolated verifier explicitly skips enable overrides | install or restart launchd profile | launchctl enable or no persistent override | launchd install and restart profiles | launchctl enable |  |  |  | bound |
| 09c | `stage_android_update_dist_if_available` | `scripts/install-launchd.sh` | stage complete Android update artifacts for Master launchd profiles when repo dist/android is present without starting or configuring the independently deployed Relay service | install or restart launchd profile plus repo dist/android artifacts | runtime-home Android update artifacts or explicit staging failure | launchd install and restart profiles | runtime-home Android distribution filesystem |  |  |  | bound |
| 08b | `handle_adp_socket / RuntimeCommandDispatcher::query_runtime` | `apps/freehand-server/src/lib.rs / crates/freehand-runtime/src/lib.rs` | serve daemon ADP task list/error-center query and subscribe surfaces from runtime owner truth | ADP task or error-center query/subscribe frame | ADP task/error-center query result or subscription event | daemon-hosted ADP client | shared WebUI transport plus runtime query/projection owner |  |  |  | bound |
| 10 | `relay_startup_auth` | `apps/freehand-daemon/src/main.rs` | validate configured Relay role plus local ADP auth token before Master binds the host listener, returning a typed pre-bind startup error on failure | Master bootstrap plus configured Relay connection | validated (local_adp_token, role) tuple or explicit pre-bind startup failure | run_master_mode | relay_startup_auth | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.validate_local_adp_token | bound |
| 10a | `run_master_mode` | `apps/freehand-daemon/src/main.rs` | bind the host listener, build the runtime dispatcher, then construct the configured Relay Agent client using the validated local ADP token | validated (local_adp_token, role) tuple plus bind address plus bootstrap | bound listener plus optional RelayAgentClient or explicit host construction failure | daemon CLI | TcpListener::bind plus RuntimeCommandDispatcher::from_selected_agent_with_live plus RelayAgentClient::new_with_presence_source | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.host_runtime_transport | bound |
| 10b | `monitor_master_lifecycle_runner` | `apps/freehand-daemon/src/main.rs` | spawn the background Master lifecycle runner and monitor its stop/error without treating it as a daemon host crash, returning host-observed unexpected stop or panic to the runner activity owner as terminal Error | agent_name plus master_runner plus cancel token plus runner task handle | explicit stderr lifecycle-runner stop/error evidence or typed terminal activity | run_master_mode | ProductionMasterRunner::run_until plus tokio::task::spawn_blocking | runtime_daemon_host | runtime_daemon_host | runtime_daemon_host.host_runtime_transport | bound |
| 10c | `run_master_mode / run_relay_worker_mode / relay_presence_from_runtime` | `apps/freehand-daemon/src/main.rs` | join one configured Agent outbound Relay client to its role-correct host lifetime, merge foreground dispatcher activity with background Master or Worker owner activity, and project only the typed result into the Relay control side-channel | selected Relay connection plus local loopback listener plus foreground and background runtime activity projections | WebUI host result or explicit Relay client terminal error plus typed role/status/count heartbeat | daemon role host | RelayAgentClient::run | runtime_agent_activity | agent_presence | runtime_agent_activity.merge_for_presence | bound |
| 11 | `handle_adp_socket / RuntimeCommandDispatcher::dispatch` | `apps/freehand-server/src/lib.rs / crates/freehand-runtime/src/lib.rs` | serve daemon ADP session CRUD and rollback commands through shared protocol transport and runtime owner dispatch | ADP session management command or session transcript query frame | ADP command receipt plus active/archived/effective transcript query projection | daemon-hosted ADP client | shared WebUI transport plus runtime.ui-command-dispatch |  |  |  | bound |
| 12 | `run_worker_mode` | `apps/freehand-daemon/src/main.rs` | route configured Slave mode into either the production Worker-only lifetime or one Relay-configured loopback UI/ADP, outbound tunnel, and cancellable Worker lifetime without app-owned health inference | selected Slave bootstrap | long-running Worker service whose process truth is written by agent.lifecycle plus optional explicit host or Relay terminal result | daemon CLI | run_blocking_worker_service / serve_webui_listener / RelayAgentClient::run |  |  |  | bound |
| 13 | `run_blocking_worker_service` | `apps/freehand-daemon/src/main.rs` | isolate the non-Relay synchronous Worker and provider loop from the daemon async runtime thread | Worker service closure | Worker service result or explicit blocking-task join failure | run_worker_mode | tokio::task::spawn_blocking |  |  |  | bound |
| 13a | `run_relay_worker_mode / ProductionWorkerRunner::run_until` | `apps/freehand-daemon/src/main.rs / crates/freehand-runtime/src/worker_runner.rs` | spawn the Relay-configured Worker runner with the same explicit cancellation token owned by the loopback host and Relay client lifetime | Relay-configured Worker runner plus owner-supplied cancellation token | cancellable blocking Worker result or explicit join failure | run_relay_worker_mode | ProductionWorkerRunner::run_until |  |  |  | bound |

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
- launchd restart guard is bound by offline and real isolated LaunchAgent verifiers: permanent startup failure plateaus at one run, transient failure restarts, and repeated rapid transient failure plateaus at the configured limit
- the isolated three-Worker verifier's label-scoped child PID read is owned by the app.runtime-daemon.launchd-e2e-observer module and crosses only the declared launchd-control edge
- daemon remote-relay compatibility hosting is bound through run_remote_relay_mode to the relay.transport public API; daemon owns process startup only and no Relay account, token, presence, or proxy semantics
- Relay authentication and account isolation are implemented by relay.transport and exercised by focused tests plus scripts/verify-remote-relay-local-online.sh
- S-profile launchd restart may stage complete runtime-home Android update artifacts but does not start, configure, or restart the independently deployed Relay service
- generated wiki must be regenerated from `docs/mainline-calls/app.runtime-daemon.json` when this function-map truth changes
