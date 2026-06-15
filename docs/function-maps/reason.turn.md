# Function Map: `reason.turn`

- feature_id: `reason.turn`
- owner crate: `crates/freehand-reason`
- owner module: `TBD until implementation lands`
- owner entry symbols:
  - `TBD until implementation lands`

## Request Mainline

- user input and context material enter the turn orchestration path
- turn orchestration renders provider-ready input and manages tool re-entry

## Response Mainline

- provider semantic events become turn truth updates
- turn truth broadcasts semantic events for reasoning, text, tool, usage, terminal, and error
- terminal result is projected from validated completion schema, not raw provider finish reason

## Error Mainline

- invalid completion schema is rejected and reprompted
- provider `finish_reason=stop` or `finish_reason=end_turn` does not end the turn by itself
- raw provider events go to debug ledger, not session truth

## Shared Multi-Reference Functions

- pending until implementation lands

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TBD` | `TBD` | create turn context | user input + session state | rendered turn input | CLI/server/node | reason orchestrator | binding pending |
| 02 | `TBD` | `TBD` | dispatch provider request | rendered turn input | provider semantic stream | reason orchestrator | provider boundary | binding pending |
| 03 | `TBD` | `TBD` | materialize turn truth | semantic events | persisted turn state | provider boundary | turn state writer | binding pending |
| 04 | `TBD` | `TBD` | validate terminal schema | candidate terminal payload | accepted terminal outcome or rejection | turn state writer | terminal validator | binding pending |
| 05 | `TBD` | `TBD` | broadcast semantic event | turn truth delta | subscriber events | turn state writer | event bus | binding pending |

## Sync Status Against Code

- design stub only
- implementation binding pending
