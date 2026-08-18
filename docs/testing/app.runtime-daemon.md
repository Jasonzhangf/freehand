# Test Design: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner: `apps/freehand-daemon`
- module_registry: `docs/module-registry/app.runtime-daemon.json`
- verification_map: `docs/verification-maps/app.runtime-daemon.json`
- resource map: `docs/resource-maps/core.json`
- lifecycle path under test:
  - daemon bootstrap selects one agent from config and creates a runtime dispatcher
  - launchd wrapper loads `~/.freehand/daemon.env` and starts one configured daemon agent through explicit `FREEHAND_DAEMON_BIN` plus Android update manifest/APK env paths on a fixed port, binding to the local Tailscale IPv4 when available so Android/WebView clients can reach the daemon
  - daemon install/restart owns only daemon services and Android update artifact staging; it must not start, configure, register, or health-check the independently deployed Relay service
  - runtime dispatcher exposes one shared UI state handle
  - daemon injects runtime dispatch into shared HTTP/SSE transport
  - daemon exposes the same shared state and runtime dispatch through ADP WebSocket at `/adp`
  - daemon exposes runtime-backed read-only query port through ADP for owner projections such as task list/history and error-center metadata query/initial subscription snapshots
  - Master mode keeps HTTP/WebUI/ADP host lifetime independent from the
    background Master lifecycle runner; runner stop/error is explicit stderr
    evidence and must not crash the host process
  - Worker mode requires one configured, discoverable loopback `local_web_url`; missing endpoint truth fails startup instead of binding an unprojected ephemeral port
  - non-loopback WebUI clients use the typed Relay Agent URL; the WebUI keeps page, asset, and ADP paths relative so the Relay Agent prefix remains intact
  - a configured Relay Agent client is part of the Master host lifetime;
    either the WebUI host or Relay client ending closes `run_master_mode` with
    the original error instead of entering a detached infinite reconnect loop
  - daemon restart restores persisted latest-turn projection before new command ingress
  - provider-backed submit and direct-message commands return runtime-backed receipts
  - latest-turn query reflects runtime-owned terminal projection changes after provider completion
  - latest-turn SSE reflects restored projections and continues streaming later runtime updates on the same connection
  - ADP WebSocket query/command/subscribe uses the same protocol truth without browser UI
  - provider execution failures surface through protocol-mapped HTTP failure payloads
  - slave-mode agent selection without Relay starts one configured Worker's
    production runner without UI transport
  - Relay-configured Slave mode binds a loopback-only WebUI/ADP host, the
    configured outbound Relay client, and the production Worker runner to one
    cancellable daemon lifetime; any host, tunnel, or runner terminal result is
    explicit and cancels the remaining owned work
  - Slave runner construction persists typed process PID/instance health
    through `agent.lifecycle`; daemon and launchd remain non-owner hosts
  - three configured Workers receive unique launchd labels, env files, and log
    files so their processes cannot overwrite one another's service truth
  - launchd wrapper classifies permanent startup failure separately from
    transient host termination, writes explicit service-control state, retries
    transient failure within a bounded window, and opens a circuit instead of
    producing an unbounded restart/log storm
- resource operations under test:
  - `runtime_daemon_host.host_runtime_transport`
  - `runtime_daemon_host.validate_local_adp_token`
  - `runtime_daemon_host.supervise_launchd_lifetime`
  - `runtime_agent_activity.merge_for_presence`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `runtime_daemon_host.host_runtime_transport` | bound | `cargo test -p freehand-daemon run_master_mode_binds_after_validated_local_adp -- --nocapture` proves the Master listener binds only after `relay_startup_auth` succeeds | `bash scripts/install-launchd.sh restartS` plus `curl http://100.66.1.82:4042/health` proves the installed S daemon reaches the host-binds step | `bash scripts/install-launchd.sh restartS` plus `curl http://100.66.1.82:4042/health` and `freehand-cliS adp-smoke --url ws://100.66.1.82:4042/adp` prove the installed S daemon reaches healthy host |
| `runtime_daemon_host.validate_local_adp_token` | bound | `cargo test -p freehand-daemon relay_master_requires_local_adp_auth_before_host_start -- --nocapture` proves `relay_startup_auth` rejects missing and whitespace tokens before Master bind; `cargo test -p freehand-daemon relay_worker_requires_local_adp_auth_before_host_start -- --nocapture` proves the Worker host fails fast on the same condition | `bash scripts/install-launchd.sh restartS` with `FREEHAND_ADP_AUTH_TOKEN` set propagates the token to `~/.freehand/daemonS.env`; a negative restart with the token removed proves the installed service exits before bind | `freehand-cliS adp-smoke --url ws://100.66.1.82:4042/adp` with the propagated token proves the live ADP bridge accepts the persisted token; a missing-token restart proves the daemon does not bind |
| `runtime_daemon_host.supervise_launchd_lifetime` | bound | `cargo test -p freehand-daemon daemon_exit_code -- --nocapture` proves startup errors map to `78` and post-start host errors map to `75`; `bash scripts/verify-launchd-restart-guard.sh` proves state parsing and bounded circuit behavior; `cargo test -p xtask daemon_launchd_mainline_edges_ -- --nocapture` locks the observer PID edge to its exact file and symbol | `bash scripts/verify-launchd-restart-guard-online.sh` proves a real isolated LaunchAgent plateaus after permanent startup failure, restarts a transient failure, and plateaus after the rapid-failure limit; `bash scripts/verify-master-three-worker-e2e-online.sh` reads the same label-scoped guarded PID during the three-Worker proof | `scripts/install-launchd.sh restartS` plus `launchctl print` and `/health` prove the installed S daemon uses the same guarded wrapper and remains healthy |
| `runtime_agent_activity.merge_for_presence` | bound | `cargo test -p freehand-daemon relay_presence_projection_maps_only_typed_control_activity -- --nocapture` verifies typed foreground/background activity merge and Relay control projection mapping | `cargo test -p freehand-daemon relay_worker_requires_local_adp_auth_before_host_start -- --nocapture` verifies the Relay Worker host does not start without local ADP auth; `cargo test -p freehand-runtime production_master_runner_activity -- --nocapture` verifies the background owner projection | `scripts/verify-remote-relay-local-online.sh` plus authenticated directory observation verifies online Relay presence status/count during real background Master work without ADP payload copying |

- white-box plan:
  - daemon bootstrap helper coverage
  - config-selected bootstrap coverage
  - Worker service runs through a blocking boundary so provider-owned blocking runtimes are never created or dropped on the daemon async runtime thread
  - Worker blocking-task panic/join failure is returned as an explicit daemon startup/service error
  - Relay-configured Worker startup requires local ADP auth, publishes Worker
    role through the typed Relay control channel, and never copies role or
    lifecycle state into HTTP/ADP payloads
  - Worker Relay presence merges direct-session activity from the dispatcher
    with delegated-task activity from the Worker lifecycle owner; either source
    can make the Agent active and neither is reconstructed from HTTP/ADP payloads
    through `RelayAgentPresenceProjection`; source errors terminate the Relay
    client instead of reusing the static bootstrap heartbeat
  - Master Relay presence merges direct-session dispatcher activity with the
    background `ProductionMasterRunner` projection; test-disabled lifecycle
    mode contributes explicit typed Idle, while terminal runner failure remains
    Error and the HTTP/ADP host stays available for diagnosis
  - positive: running/error runtime activity maps to the matching Relay control
    status and exact active-session count
  - negative: missing or empty local ADP authentication rejects Worker Relay
    host startup before the listener is exposed
  - negative: missing or empty local ADP authentication rejects Master Relay
    host startup before its WebUI listener is bound or service-started truth is
    published, so launchd classifies the failure as permanent startup error
  - Worker host startup proves the selected Worker reason/session namespace is
    queryable through ADP while Master-only commands fail explicitly
  - launchd wrapper env validation coverage through shell syntax, explicit daemon binary validation, Tailscale bind selection, and runtime smoke
  - positive: startup-complete host failure returns retryable `75`
  - negative: pre-host config/bootstrap failure returns permanent `78`
  - negative: an existing blocked launchd state prevents another daemon spawn
  - negative: failure to persist running or retry guard state stops automatic
    restart and does not leave the supervised child running
  - positive: `cargo test -p xtask daemon_launchd_mainline_edges_accept_exact_adjacent_bindings -- --nocapture` proves every launchd source edge has one exact step, file, symbol, and resource operation
  - negative: `cargo test -p xtask daemon_launchd_mainline_edges_reject_compound_binding -- --nocapture` proves a slash-combined launchd symbol binding fails the architecture gate
  - positive: explicit install/restart clears only the selected label's blocked state after preflight
  - launchd source audit rejects any call to the removed Relay launchd helper or any `upstreamBaseUrl` registration path
  - bind-arg parsing coverage
  - dependency boundary scan
- module black-box plan:
  - daemon provider-backed submit-user-input HTTP smoke
  - daemon Master host-survival smoke:
    `cargo test -p freehand-daemon master_mode_keeps_host_alive_when_lifecycle_runner_stops -- --nocapture`
    corrupts Master loop state so the background runner stops, then proves
    `/health` remains available
  - daemon latest-turn query after provider-backed submit smoke
  - daemon restart latest-turn query/SSE restore smoke
  - daemon same-connection latest-turn SSE continuation smoke
  - daemon restart next-turn-id continuation smoke
  - daemon provider failure HTTP smoke
  - daemon direct-message dispatch HTTP smoke
  - daemon checkpoint rewind HTTP smoke
  - daemon missing-checkpoint rewind HTTP failure smoke
  - daemon slave-mode production Worker runner bootstrap smoke
  - daemon Worker bind positive/negative tests prove typed `local_web_url`, explicit
    bind override, and legacy configs without `local_web_url` receive an isolated
    loopback ephemeral bind rather than entering a restart loop
  - explicit Worker bind is accepted only when it matches configured advertised
    `local_web_url`; mismatches fail before binding so dashboard discovery cannot
    route to a stale endpoint
  - daemon Relay Worker host smoke proves selected Slave config binds loopback
    WebUI/ADP and stops when the runner, host, or Relay client terminates
  - daemon Relay Worker host negative smoke proves missing/empty ADP auth and
    Master-only dispatch fail explicitly without leaving a detached Worker
    runner
  - daemon Worker bootstrap queries the same agent id and verifies owner-backed
    PID, process-instance identity, `alive=true`, and initial restart count
  - daemon Worker blocking-boundary positive and negative smoke
  - launchd shell fixture proves `worker-alpha`, `worker-beta`, and
    `worker-gamma` resolve to three unique labels/env/log paths through
    `scripts/verify-launchd-worker-naming.sh`
  - launchd online fixture proves three agent-specific Worker services start
    under unique labels in isolated HOME, survive one gamma crash through
    KeepAlive restart, and expose new PID/process instance plus
    `restart_count=1` through AgentBoard owner truth via
    `scripts/verify-launchd-three-worker-services-online.sh`
  - daemon corrupt-checkpoint-bootstrap startup rejection smoke
  - daemon ADP WebSocket command/query/subscribe smoke
  - daemon ADP session CRUD plus rollback command/query smoke
  - daemon ADP task list/history query smoke
  - daemon ADP task list subscription smoke
  - daemon ADP error-center metadata query smoke
  - daemon ADP query-as-command rejection smoke
  - launchd service smoke: `launchctl print`, `/health`, `/`, log file creation, restart wait-until-healthy behavior, and Tailscale-IP `/health` reachability for Android clients when Tailscale is present
  - launchd restart-guard online smoke uses isolated HOME and unique labels to
    prove permanent failure run-count plateau, one transient restart into a
    stable process, and bounded repeated transient failure plateau
  - `make ci` enters both restart-guard verifier targets; the online target runs
    on Darwin, and CI plus release each carry a dedicated macOS job that executes
    the real launchd verifier before completion or publication
  - `scripts/install-launchd.sh restartS` succeeds with daemon health independent of Relay deployment state; Relay deployment is verified only through `relay.transport` gates
- project black-box impact:
  - closes the first real runtime host gap without polluting the protocol-only app boundary
  - machine-readable mainline truth remains the only source for generated wiki artifacts
- fixtures / replay inputs / runtime evidence paths:
  - `~/.freehand/state/ui`
  - `~/.freehand/state/turns`
  - `~/.freehand/ledgers/reason`
  - `~/.freehand/logs/daemon.stdout.log`
  - `~/.freehand/logs/daemon.stderr.log`
  - `~/.freehand/state/launchd/<label>.json`
- known gaps:
  - real node-pairing websocket transport is not wired yet; daemon ADP WebSocket is a UI/control/status transport over existing runtime-owned local node semantics, not the node pairing transport
  - isolated controlled-provider three-process proof and launchd-managed
    three-service KeepAlive restart proof are green; production closure still
    requires real-provider recovery evidence
- sync status between design and implementation:
  - daemon bootstrap helper is landed
  - Master mode host-survival monitoring is landed and focused-test bound; the
    WebUI/ADP host remains observable when the background lifecycle runner
    stops with an owner-truth error
  - runtime-backed submit/query/restart-restore/continuous-SSE/provider-failure/direct-message/checkpoint-rewind HTTP smoke is landed
  - daemon HTTP rewind now also has explicit missing-manifest failure coverage through the same command ingress
  - daemon startup now also has explicit corrupt checkpoint-projection bootstrap failure coverage
  - daemon ADP WebSocket command/query/subscribe and query-as-command rejection coverage is landed
  - daemon ADP session rollback coverage is required when session rollback command surface changes
  - daemon ADP task list/history query coverage is landed
  - daemon ADP task list subscription coverage is landed
  - daemon ADP error-center metadata query coverage is landed
  - config-selected bootstrap smoke is landed and uses configured peer topology
  - configured Slave bootstrap now constructs the production Worker runner
  - configured Slave bootstrap test verifies process identity through
    TaskRuntime/AgentLifecycle owner truth
  - agent-specific Worker launchd naming, its non-mutating executable fixture,
    isolated three-process runtime proof, and live launchd-managed
    three-service KeepAlive proof are landed
  - migrated mainline-call source and generated wiki are kept in sync with this test design
