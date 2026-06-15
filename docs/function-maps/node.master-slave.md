# Function Map: `node.master-slave`

- feature_id: `node.master-slave`
- owner crate: `crates/freehand-node`
- owner module: `TBD until implementation lands`
- owner entry symbols:
  - `TBD until implementation lands`

## Request Mainline

- local master accepts user input or task delegation intent
- master may dispatch to the paired slave through WebSocket handshake topology

## Response Mainline

- slave returns progress, status, direct conversation, or turn stream updates
- master may subscribe to slave output while preserving node/source identity

## Error Mainline

- pairing failure, health failure, or unauthorized input to slave return explicit node errors
- slave continues listening after pairing loss

## Shared Multi-Reference Functions

- pending until implementation lands

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TBD` | `TBD` | start agent node runtime | selected agent config | node runtime state | CLI/server | node bootstrap | binding pending |
| 02 | `TBD` | `TBD` | perform websocket pairing | pairing intent | paired or rejected state | node bootstrap | handshake runtime | binding pending |
| 03 | `TBD` | `TBD` | dispatch master task | task request | slave work request | master runtime | slave channel | binding pending |
| 04 | `TBD` | `TBD` | return slave progress/status | slave state | node projection | slave runtime | query/subscription surface | binding pending |

## Sync Status Against Code

- design stub only
- implementation binding pending
