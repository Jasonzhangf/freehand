# UI Protocol Design

## Status

Confirmed discussion with first transport baseline partly landed. Remaining open items stay `TBD`.

## Confirmed

### First-version UI scope

First version supports:

- CLI
- WebUI

### UI protocol responsibility

`ui.protocol` owns:

- UI commands
- UI events
- UI projections
- subscription contracts
- query contracts

It does not own concrete rendering components.

`ui.protocol` is an input ingress plus read-only projection boundary:

- UI may submit commands into the system
- UI may consume turn-state and debug-state projections
- UI must not directly mutate reason truth
- UI must not directly mutate debug truth

### First-version commands

First version command surface includes:

- submit user input
- subscribe turn stream
- query turn status
- query node status
- query task progress
- send direct message to slave
- cancel turn
- resume / retry

These commands are ingress only:

- UI submits intent
- owner modules decide whether and how system truth changes
- UI does not become a turn/debug/session truth writer by sending a command

### First-version displayed projections

First version projection surface includes all of:

- reasoning
- text
- tool
- usage
- terminal
- error
- node status
- pairing status
- task progress
- slave turn stream

UI also needs debug-state consumption at the architecture level, but the exact first-version debug projection contract remains `TBD`.

### Completion result display

- UI shows only final projected text for completion-schema result
- structured completion schema is not exposed as first-version UI truth

### Slave turn presentation

- slave turn appears as an independent sub-stream
- CLI does not render slave turn sub-stream
- WebUI renders slave turn as a separate card

### Subscription model

First version supports all of:

- subscribe latest active turn
- subscribe specific `turn_id`
- subscribe specific `turn_id` debug state
- subscribe node status / progress / turn stream classes

First-version stream classes are locked at semantic level as:

- `turn`
- `progress`
- `node_status`
- `debug`

### Query vs subscribe

- query and subscribe are separate
- query returns snapshot
- subscribe returns incremental updates
- both are read-only from the UI side
- neither gives UI direct mutation authority over reason/debug truth
- first WebUI transport baseline maps this to:
  - HTTP query endpoints
  - SSE subscribe endpoints

First-version query surface includes:

- latest active turn snapshot
- specific `turn_id` snapshot
- specific `turn_id` debug snapshot
- node status snapshot
- task progress snapshot

### First-version debug-state contract

First version locks a minimal debug-state UI contract:

- debug state is queryable by `turn_id`
- debug state is subscribable by `turn_id`
- debug state is a read-only projection
- debug state carries source identity plus `turn_id`
- debug state carries one summary `status_text`
- debug state carries ordered `detail_lines`

This is a projection contract only:

- raw provider payloads stay in debug ledgers and replay artifacts
- authoritative reason/debug/session truth stays outside UI
- UI consumes debug-state snapshots and streams without becoming a writer

### Source identity fields

UI protocol must carry explicit source fields:

- `source_agent_id`
- `source_node_id`
- `source_turn_id`
- `stream_kind`

These fields exist so UI can preserve both:

- source semantic position
- source node position

### Black-box targets for UI protocol

First version black-box targets include:

- command -> projection smoke
- slave turn subscription smoke
- node status query smoke
- terminal result projection smoke
- debug state query/subscription smoke

UI black-box validation is user-behavior oriented:

- module black-box covers protocol-visible behavior of `freehand-ui-protocol`
- project black-box covers end-to-end command/query/subscribe behavior across runtime wiring

## Open Questions / TBD

- exact command schema
- exact projection schema
- exact WebUI card structure for slave turn
- exact CLI omission rule for slave turn in mixed sessions

## Update trigger

Update this doc when:

- UI scope changes
- command surface changes
- projection surface changes
- slave turn presentation changes
- query/subscribe boundary changes
- black-box targets change
