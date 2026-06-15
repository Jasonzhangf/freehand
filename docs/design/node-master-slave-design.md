# Node Master Slave Design

## Status

First baseline is now implemented as a local in-memory runtime model.

Important boundary:

- implemented: node lifecycle semantics, pairing validation, permission lock, status/progress/turn projection
- not implemented yet: real socket IO adapter and finalized wire schema

This keeps first-version truth explicit without inventing transport details that are still `TBD`.

## Confirmed

### First-version scope

Current first version scope is local only:

- one local `master`
- one local `slave`

Remote multi-slave topology is not part of current first version.

### Pairing transport

- `master` and `slave` pair through WebSocket handshake semantics
- current code models this as `PairingTransport::WebSocket`
- real websocket server/client wiring is a later transport-layer step

### Pairing source rule

- `slave` allows only one fixed pairing source
- changing the pairing source requires config change and slave restart

### Lost-pair behavior

- when slave loses its pairing, it continues listening
- continued listening is for later re-pairing

### Task delegation model

`master` can delegate work to `slave`.

Confirmed interaction modes:

- master sends a task and waits for execution progress
- master can actively query progress
- master can directly talk to slave
- master can subscribe to slave turn stream
- slave turn subscription should be UI-like, so master can stream slave turn output directly inside master framework

### Delegation granularity

Current code baseline:

- master sends delegated task intent with `session_id`, `turn_id`, and progress text
- slave records progress snapshot keyed by `turn_id`
- exact task payload schema beyond that remains `TBD`

### Node state visibility

First version must explicitly record node state.

At minimum, node state must support:

- health check
- pairing success
- pairing failure
- status query

Suggested state family is not final yet, but status must cover those concerns.

## Implemented Runtime Binding

- owner crate: `crates/freehand-node`
- owner type: `LocalNodeRuntime`
- bound symbols:
  - `LocalNodeRuntime::new`
  - `LocalNodeRuntime::pair_slave`
  - `LocalNodeRuntime::lose_slave_pairing`
  - `LocalNodeRuntime::delegate_task`
  - `LocalNodeRuntime::send_direct_message`
  - `LocalNodeRuntime::publish_slave_turn`
  - `LocalNodeRuntime::query_node_status`
  - `LocalNodeRuntime::query_task_progress`

## Open Questions / TBD

- exact real WebSocket wire schema
- exact delegated task payload beyond current baseline
- exact reconnect/heartbeat timing policy
- exact remote multi-node topology after local first version

## Update trigger

Update this doc when:

- local-only scope changes
- pairing transport changes
- pairing source rules change
- node state model changes
- task delegation model changes
