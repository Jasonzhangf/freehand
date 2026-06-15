# Function Map: `ui.protocol`

- feature_id: `ui.protocol`
- owner crate: `crates/freehand-ui-protocol`
- owner module: `TBD until implementation lands`
- owner entry symbols:
  - `TBD until implementation lands`

## Request Mainline

- UI commands enter one protocol truth shared by CLI and WebUI
- query and subscribe stay separate
- subscriptions may target latest active turn, specific turn, or node/progress streams

## Response Mainline

- query returns snapshots
- subscribe returns incremental projections
- terminal completion shows only final projected text
- slave turn may surface as WebUI-only separate card while staying in one protocol truth

## Error Mainline

- invalid command, invalid stream selection, or unavailable source projection return explicit protocol errors
- source identity fields remain explicit across success and error paths

## Shared Multi-Reference Functions

- pending until implementation lands

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TBD` | `TBD` | accept UI command | command payload | protocol command object | CLI/WebUI | protocol boundary | binding pending |
| 02 | `TBD` | `TBD` | execute query path | query command | snapshot projection | protocol boundary | query handler | binding pending |
| 03 | `TBD` | `TBD` | execute subscribe path | subscribe command | incremental stream | protocol boundary | stream handler | binding pending |
| 04 | `TBD` | `TBD` | project terminal text | terminal semantic payload | UI terminal text | query/stream handler | projector | binding pending |

## Sync Status Against Code

- design stub only
- implementation binding pending
