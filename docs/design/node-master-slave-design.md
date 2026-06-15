# Node Master Slave Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### First-version scope

Current first version scope is local only:

- one local `master`
- one local `slave`

Remote multi-slave topology is not part of current first version.

### Pairing transport

- `master` and `slave` pair through WebSocket handshake

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

Confirmed so far:

- master sends delegated tasks to slave
- progress is visible during execution

Exact task granularity boundary remains `TBD`.

### Node state visibility

First version must explicitly record node state.

At minimum, node state must support:

- health check
- pairing success
- pairing failure
- status query

Suggested state family is not final yet, but status must cover those concerns.

## Open Questions / TBD

- exact WebSocket handshake schema
- exact delegated task granularity
- exact state enum names
- exact progress query protocol
- exact turn-subscription wire shape between master and slave
- exact reconnect semantics after pairing loss

## Update trigger

Update this doc when:

- local-only scope changes
- pairing transport changes
- pairing source rules change
- node state model changes
- task delegation model changes

