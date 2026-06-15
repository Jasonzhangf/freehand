# Function Map: `ui.protocol`

- feature_id: `ui.protocol`
- owner crate: `crates/freehand-ui-protocol`
- owner module: `crates/freehand-ui-protocol/src/lib.rs`
- owner entry symbols:
  - `validate_command`
  - `subscription_selector`
  - `subscription_matches`
  - `turn_projection_from_events`
  - `terminal_text_projection`
  - `UiProtocolState::query`
  - `turn_projection_for_client`

## Request Mainline

- UI commands enter one protocol truth shared by CLI and WebUI
- query and subscribe stay separate
- subscriptions may target latest active turn, specific turn, or node/progress streams

## Response Mainline

- query returns snapshots
- subscribe returns incremental projections
- terminal completion shows only final projected text
- slave turn may surface as WebUI-only separate card while staying in one protocol truth
- client-specific projection gating stays inside the protocol owner, not in apps

## Error Mainline

- invalid command, invalid stream selection, or unavailable source projection return explicit protocol errors
- source identity fields remain explicit across success and error paths

## Shared Multi-Reference Functions

- `terminal_text_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: collapse terminal event to final user-visible text
  - allowed callers: query handlers, stream handlers, CLI/WebUI adapters
  - related tests: terminal result projection smoke
  - why shared: ensures CLI and WebUI project the same terminal text truth
- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: gate slave substream visibility by UI client kind without changing turn truth
  - allowed callers: CLI/WebUI adapters, query handlers
  - related tests: slave turn subscription smoke
  - why shared: keeps client-specific projection rules centralized and protocol-owned

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | accept and validate UI command payload | UI command | validated command | CLI/WebUI | protocol boundary | bound |
| 02 | `UiProtocolState::query` | `crates/freehand-ui-protocol/src/lib.rs` | execute query path | query command | snapshot projection | protocol boundary | query handler | bound |
| 03 | `subscription_selector` | `crates/freehand-ui-protocol/src/lib.rs` | build subscribe selector | subscribe command | subscription selector | protocol boundary | stream handler | bound |
| 04 | `subscription_matches` | `crates/freehand-ui-protocol/src/lib.rs` | route incremental projection to matching subscription | subscription selector + projection | delivery decision | stream handler | selector matcher | bound |
| 05 | `turn_projection_from_events` | `crates/freehand-ui-protocol/src/lib.rs` | project turn state into UI snapshot | semantic/tool/usage/terminal/error inputs | UI turn projection | query/stream handler | projector | bound |
| 06 | `terminal_text_projection` | `crates/freehand-ui-protocol/src/lib.rs` | project terminal text | terminal semantic payload | UI terminal text | query/stream handler | projector | bound |
| 07 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate client-specific slave substream visibility | turn projection + client kind | client-specific turn projection | CLI/WebUI adapter | projector | bound |

## Sync Status Against Code

- command validation, query selection, subscription routing, and turn projection are bound in code
- client-specific projection gating is now also bound in code
