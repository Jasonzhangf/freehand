# Test Design: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner: `apps/freehand-daemon`
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
  - a configured Relay Agent client is part of the Master host lifetime;
    either the WebUI host or Relay client ending closes `run_master_mode` with
    the original error instead of entering a detached infinite reconnect loop
  - daemon restart restores persisted latest-turn projection before new command ingress
  - provider-backed submit and direct-message commands return runtime-backed receipts
  - latest-turn query reflects runtime-owned terminal projection changes after provider completion
  - latest-turn SSE reflects restored projections and continues streaming later runtime updates on the same connection
  - ADP WebSocket query/command/subscribe uses the same protocol truth without browser UI
  - provider execution failures surface through protocol-mapped HTTP failure payloads
  - slave-mode agent selection starts one configured Worker's production runner
    without UI transport
  - Slave runner construction persists typed process PID/instance health
    through `agent.lifecycle`; daemon and launchd remain non-owner hosts
  - three configured Workers receive unique launchd labels, env files, and log
    files so their processes cannot overwrite one another's service truth
- resource operations under test:
- white-box plan:
  - daemon bootstrap helper coverage
  - config-selected bootstrap coverage
  - Worker service runs through a blocking boundary so provider-owned blocking runtimes are never created or dropped on the daemon async runtime thread
  - Worker blocking-task panic/join failure is returned as an explicit daemon startup/service error
  - launchd wrapper env validation coverage through shell syntax, explicit daemon binary validation, Tailscale bind selection, and runtime smoke
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
