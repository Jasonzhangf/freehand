# UI Protocol Design

## Status

Confirmed discussion only. Unconfirmed details remain `TBD`.

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
- subscribe node status / progress / turn stream classes

First-version stream classes are locked at semantic level as:

- `turn`
- `progress`
- `node_status`

### Query vs subscribe

- query and subscribe are separate
- query returns snapshot
- subscribe returns incremental updates

First-version query surface includes:

- latest active turn snapshot
- specific `turn_id` snapshot
- node status snapshot
- task progress snapshot

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
