# Test Design: `node.master-slave`

- feature_id: `node.master-slave`
- owner: `crates/freehand-node`
- resource map: `docs/resource-maps/core.json`
- resource operation coverage:
  - `config.bootstrap_node_pairing`
  - `remote_daemon_registry.project_directory`
  - `node_pairing.project_to_ui`

## Resource Operation Test Coverage

| resource operation | status | white-box | module black-box | project black-box |
| --- | --- | --- | --- | --- |
| `config.bootstrap_node_pairing` | bound | `cargo test -p freehand-node` covers pairing state, config ownership, pair-token/source validation, metadata/debug leak-prevention, and rejection tests | `cargo test -p freehand-node` covers local node runtime handshake, status snapshot, relisten, and permission-boundary smokes | `cargo run -p xtask -- gates check` covers project boundary plus node resource-map/mainline binding; live websocket transport remains a documented gap outside this bound source edge |
| `remote_daemon_registry.project_directory` | bound | `cargo test -p freehand-node remote_daemon_directory -- --nocapture` covers account-scoped multi-daemon directory projection, credential-safe summaries, direct-first route resolution, and relay selection only after direct health failure | `cargo test -p freehand-node -- --nocapture` covers the node-owned directory API together with pairing/projection smokes | `cargo run -p xtask -- gates check` covers project boundary plus remote directory resource-map/mainline binding; live relay tunnel and Tailscale OS probing remain documented gaps |
| `node_pairing.project_to_ui` | bound | `cargo test -p freehand-node` covers delegated progress, slave-turn publication, status projection, permission rejection, and debug observation tests | `cargo test -p freehand-node` covers node boundary status/progress/subscription smokes | `cargo run -p xtask -- gates check` covers project boundary plus UI projection resource-map/mainline binding; live UI transport integration remains a documented gap outside this bound source edge |

- lifecycle path under test:
  - local master/slave pair handshakes
  - config-owned remote daemon registry publishes one node-owned account directory with multiple daemons and route diagnostics
  - slave input permission locks to paired source
  - pairing loss returns slave to listening state
  - status and progress query remain available
  - debug producer snapshots stay observation-only and request-text-free
- white-box plan:
  - pairing state transitions, permission checks, handshake validation, relisten behavior
  - config bootstrap validation for master/slave ownership fields
  - direct-message permission validation
  - metadata bootstrap owner/write-node provenance validation
  - metadata emission for pairing, delegated task, and slave turn publication without request/body leakage
  - debug emission for bootstrap, pairing rejection, and slave-turn publication without pair-token or turn-text leakage
  - debug sink failure observable through `DebugHub::subscribe_failures` without blocking node truth mutation
  - metadata write failure before rejected status or other node-truth materialization
  - pairing rejection for unauthorized source node and unauthorized source ip
  - delegated-task empty-status rejection
  - slave-turn publication permission rejection
  - remote daemon directory credential redaction and relay-after-direct-failure route resolution
- module black-box plan:
  - status snapshot and progress query through node boundary
  - slave turn publication visible through subscription surface
  - shared metadata-center smoke through node boundary
  - shared debug-hub snapshot smoke through node boundary
  - remote daemon directory publish/resolve smoke through node boundary
- project black-box impact:
  - master can delegate work and subscribe to slave turn stream through runtime wiring
  - config-selected live runtime bootstrap shares one metadata ledger path with node bootstrap and pairing writes
  - machine-readable mainline truth remains the only source for generated wiki artifacts
- fixtures / replay inputs / runtime evidence paths:
  - websocket handshake replays
  - pairing ledgers
  - `~/.freehand/state/nodes`
  - `~/.freehand/replays/nodes`
- known gaps:
  - real websocket transport IO is not implemented yet
  - transport heartbeat and reconnect timing policy not yet defined
  - relay signaling/tunnel/pass-through IO, account directory server presence, Tailscale OS auto-connect/probing loop, and true-device QR scan proof are not implemented in this node directory slice
- sync status between design and implementation:
  - `LocalNodeRuntime` baseline implemented
  - `RemoteDaemonDirectory` baseline implemented for local account-scoped directory projection and route resolution only
  - tests cover pairing success/failure, permission lock, relisten, progress query, turn subscription, direct-message guardrails, metadata bootstrap/provenance, metadata leak prevention, debug bootstrap/pairing/slave-turn leak prevention, debug sink failure observation-only delivery, metadata write failure no-truth-materialization, and explicit rejection for unauthorized source node/ip, empty task status, and unauthorized slave-turn publication
  - migrated mainline-call source and generated wiki are kept in sync with this test design
