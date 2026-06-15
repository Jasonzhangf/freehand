# UI And Runtime Topology

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

## Confirmed

### UI relationship

- UI and reasoning are separate
- multiple UIs should access one truth source
- UI should use `freehand-ui-protocol`
- UI should not directly depend on provider crates
- UI should bind to feature/function truth through function map driven ownership, not ad hoc logic
- first version UI scope is CLI and WebUI
- CLI and WebUI share one protocol truth, but may render different views

### Runtime topology

- runtime home is `~/.freehand`
- master/slave topology is part of target system
- runtime and scene evidence should live in known runtime directories
- reasoning module and runtime orchestration are not the same responsibility
  - `freehand-reason`: reasoning turn semantics
  - `freehand-node`: master/slave and runtime topology

### Confirmed master/slave semantics

- master/slave is an input-permission configuration
- local multiple agents are managed by `config.toml`
- one `config.toml` may define multiple local agents
- one `config.toml` may define multiple providers
- config source path is `~/.freehand/config.toml`
- multi-agent layout uses `[agents.<name>]`
- provider layout uses `[providers.<id>]`
- each agent has a startup configuration entry
- startup configuration decides runtime mode
- each agent binds to one configured provider id
- whichever side is configured as `master` accepts user input
- configured `master` may dispatch tasks to:
  - local sub-agents
  - remote slave agents
- `slave` is configured into task-receiving mode
- when configured into `slave` mode, startup config includes:
  - `name`
  - `mode`
  - pairing token
- `allowed_pair_ip` is optional, and omission means no IP filtering
- once paired successfully, `slave` enters healthy slave mode
- in healthy slave mode, `slave` executes input from its paired source only
- paired source may be:
  - a user
  - a `master`
- in healthy slave mode, `slave` must not accept other unrelated direct inputs
- current first version topology is local one-master one-slave
- current first version pairing transport is WebSocket handshake
- slave uses one fixed pairing source and changing that source requires config change plus restart
- if pairing is lost, slave keeps listening for re-pairing
- master can subscribe to slave turn stream and surface it like UI streaming

### Operational workflow

- feature work starts from owner lookup
- debug starts from function map and runtime paths
- if runtime truth changes, docs and workflow truth must be updated
- slave turn may appear as a separate sub-stream in WebUI while being omitted from CLI rendering

## Open Questions / TBD

- whether first multi-UI transport is SSE, WebSocket, HTTP polling, or mixed
- exact command flow from UI to truth source
- exact master/slave registration and heartbeat design
- exact access control model beyond source IP + pairing token
- exact session/topic isolation model for multiple UIs

## Update trigger

Update this doc when:

- UI transport changes
- UI protocol boundary changes
- master/slave runtime topology changes
- runtime home usage model changes
