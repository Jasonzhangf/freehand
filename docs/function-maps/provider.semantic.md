# Function Map: `provider.semantic`

- feature_id: `provider.semantic`
- owner crate: `crates/freehand-provider-core`
- owner module: `TBD until implementation lands`
- owner entry symbols:
  - `TBD until implementation lands`

## Request Mainline

- normalized provider request enters provider semantic boundary
- provider-specific adapters render wire payloads without leaking adapter DTOs outside adapter crates

## Response Mainline

- provider raw stream or single-shot output becomes unified semantic events
- semantic output carries text, reasoning, tool, usage, terminal, and error semantics

## Error Mainline

- provider errors are classified into unified error contracts
- periodic-recoverable errors preserve recovery windows in seconds
- debug/raw retention stays separate from normal semantic output

## Shared Multi-Reference Functions

- pending until implementation lands

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `TBD` | `TBD` | accept normalized provider request | semantic request | adapter-ready request | reason/orchestrator | provider core boundary | binding pending |
| 02 | `TBD` | `TBD` | render provider wire payload | adapter-ready request | provider payload | provider core | adapter block | binding pending |
| 03 | `TBD` | `TBD` | parse provider event stream | provider raw events | raw adapter events | adapter runtime | adapter parser | binding pending |
| 04 | `TBD` | `TBD` | unify semantic event | raw adapter events | semantic events | adapter parser | semantic mapper | binding pending |
| 05 | `TBD` | `TBD` | classify provider error | provider failure | unified error contract | adapter/runtime | error classifier | binding pending |

## Sync Status Against Code

- design stub only
- implementation binding pending
