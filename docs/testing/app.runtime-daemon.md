# Test Design: `app.runtime-daemon`

- feature_id: `app.runtime-daemon`
- owner: `apps/freehand-daemon`
- resource map: `docs/resource-maps/core.json`
- lifecycle path under test:
  - daemon bootstrap selects one agent from config and creates a runtime dispatcher
  - launchd wrapper loads `~/.freehand/daemon.env` and starts one configured daemon agent through explicit `FREEHAND_DAEMON_BIN` plus Android update manifest/APK env paths on a fixed port, binding to the local Tailscale IPv4 when available so Android/WebView clients can reach the daemon
  - runtime dispatcher exposes one shared UI state handle
  - daemon injects runtime dispatch into shared HTTP/SSE transport
  - daemon exposes the same shared state and runtime dispatch through ADP WebSocket at `/adp`
  - daemon exposes runtime-backed read-only query port through ADP for owner projections such as task list/history and error-center metadata query/initial subscription snapshots
  - Master mode keeps HTTP/WebUI/ADP host lifetime independent from the
    background Master lifecycle runner; runner stop/error is explicit stderr
    evidence and must not crash the host process
  - standalone remote relay mode hosts account-scoped relay directory registration plus namespaced WebUI HTTP/ADP pass-through for registered daemon hosts
  - S-profile launchd restart keeps the Android relay and APK update distribution in the same lifecycle by staging runtime-home Android update artifacts, restarting `com.freehand.relayS`, registering `studio-host`, and proving relay-served WebUI assets/update routes before true-device acceptance
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
  - `remote_relay_transport.register_host`
  - `remote_relay_transport.query_account_directory`
  - `remote_relay_transport.proxy_http`
  - `remote_relay_transport.proxy_adp`

## Resource Operation Test Coverage

| resource operation | status | white-box coverage | module black-box coverage | project black-box coverage |
| --- | --- | --- | --- | --- |
| `remote_relay_transport.register_host` | bound | `cargo test -p freehand-server --lib remote_relay -- --nocapture` covers `RemoteRelayDirectory::publish_host` normalization and stored host record truth | `cargo test -p freehand-server --lib remote_relay -- --nocapture` posts `/relay/hosts` and asserts the accepted host record | `scripts/verify-remote-relay-local-online.sh` starts real upstream/relay processes and proves registration through the relay HTTP API; `cargo run -p xtask -- gates check` enforces resource-map/mainline/function/test binding |
| `remote_relay_transport.query_account_directory` | bound | `cargo test -p freehand-server --lib remote_relay -- --nocapture` covers `RemoteRelayDirectory::account_directory` sorted account snapshot projection | `cargo test -p freehand-server --lib remote_relay -- --nocapture` queries `/relay/directory/jason` and asserts schema/account/daemon host truth | `scripts/verify-remote-relay-local-online.sh` queries the real relay account directory; `cargo run -p xtask -- gates check` enforces resource-map/mainline/function/test binding |
| `remote_relay_transport.proxy_http` | bound | `cargo test -p freehand-server --lib remote_relay -- --nocapture` covers registered-host lookup, query preservation, namespaced path forwarding, and explicit missing-host rejection through `handle_relay_daemon_http` | `cargo test -p freehand-server --lib remote_relay -- --nocapture` proves `/relay/daemon/studio-host/`, namespaced CSS/JS, `/health`, and HTTP owner routes proxy the same upstream daemon while missing hosts return `relay_host_not_found` | `scripts/verify-remote-relay-local-online.sh` proves real-process namespaced WebUI HTML/CSS/JS/health pass-through plus missing-host rejection; Android true-device closeout installs the APK with an app-owned `remote_registry` relay endpoint and requires canonical WebUI layout probe, foreground activity, screenshot, and config readback; `cargo run -p xtask -- gates check` enforces resource-map/mainline/function/test binding |
| `remote_relay_transport.proxy_adp` | bound | `cargo test -p freehand-server --lib remote_relay -- --nocapture` covers registered-host upstream ADP URL conversion and bidirectional relay socket path | `cargo test -p freehand-server --lib remote_relay -- --nocapture` proves `/relay/daemon/studio-host/adp` returns an upstream ADP `QueryLatestActiveTurn` result | `scripts/verify-remote-relay-local-online.sh` proves real process ADP pass-through with `freehand-cli adp-smoke` over the relay URL; `cargo run -p xtask -- gates check` enforces resource-map/mainline/function/test binding |
- white-box plan:
  - daemon bootstrap helper coverage
  - config-selected bootstrap coverage
  - Worker service runs through a blocking boundary so provider-owned blocking runtimes are never created or dropped on the daemon async runtime thread
  - Worker blocking-task panic/join failure is returned as an explicit daemon startup/service error
  - launchd wrapper env validation coverage through shell syntax, explicit daemon binary validation, Tailscale bind selection, and runtime smoke
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
  - remote relay transport smoke:
    `cargo test -p freehand-server --lib remote_relay -- --nocapture` registers a host, queries the account directory, proves `/relay/daemon/{relay_host_id}/` plus namespaced assets and HTTP owner routes proxy the upstream WebUI, proves `/relay/daemon/{relay_host_id}/health` proxies upstream `/health`, proves `/relay/daemon/{relay_host_id}/adp` proxies an upstream ADP `QueryLatestActiveTurn`, and proves an unregistered host returns `relay_host_not_found`
  - remote relay local online smoke:
    `scripts/verify-remote-relay-local-online.sh` starts real `freehand-server webui-serve-smoke`, `freehand-daemon remote-relay`, and `freehand-cli adp-smoke` processes, then proves relay registration, directory query, namespaced WebUI HTML/CSS/JS and health pass-through, ADP pass-through, and missing-host rejection
  - S-profile relay launchd smoke:
    `scripts/install-launchd.sh restartS` must stage complete Android update artifacts when repo `dist/android` is present, restart `com.freehand.daemonS` and `com.freehand.relayS`, register `studio-host` to `http://127.0.0.1:4042`, prove `http://100.66.1.82:44042/relay/daemon/studio-host/` advertises the current asset version, prove `/relay/daemon/studio-host/android/update.json` exposes the staged version truth, and prove relay ADP smoke succeeds before Android WebView proof is accepted
  - Android relay true-device smoke:
    build and install the current debug APK, expose one fixed relay port through device routing, persist a `remote_registry` config whose active endpoint `webUrl` is `/relay/daemon/{relay_host_id}/`, launch `com.freehand.android/.ui.MainActivity`, and require app-owned config readback, relay-served current asset version, `FreehandWebUiLayout` canonical shell/CSS/JS/mobile evidence, foreground activity, no stale live rows for terminal sessions, no fatal logcat, and screenshot review
  - launchd service smoke: `launchctl print`, `/health`, `/`, log file creation, restart wait-until-healthy behavior, and Tailscale-IP `/health` reachability for Android clients when Tailscale is present
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
  - remote relay transport focused and local-online coverage is required for host registration, account directory, namespaced WebUI HTTP pass-through, ADP WebSocket pass-through, and missing-host rejection
  - relay endpoint `authRequired` is directory/route metadata only in this
    slice; relay HTTP/ADP access authentication is not implemented or claimed,
    so relay exposure must remain behind the trusted local/Tailscale route
    until a dedicated auth owner and online negative proof land
  - S-profile Android relay coverage is required after WebUI asset, live-state, or APK update distribution changes: relay asset version, relay APK update manifest, relay ADP smoke, true-device CDP DOM state, and screenshot must all come from the Tailscale relay endpoint
  - config-selected bootstrap smoke is landed and uses configured peer topology
  - configured Slave bootstrap now constructs the production Worker runner
  - configured Slave bootstrap test verifies process identity through
    TaskRuntime/AgentLifecycle owner truth
  - agent-specific Worker launchd naming, its non-mutating executable fixture,
    isolated three-process runtime proof, and live launchd-managed
    three-service KeepAlive proof are landed
  - migrated mainline-call source and generated wiki are kept in sync with this test design
