# Wiki: `ui.protocol`

Generated from `docs/mainline-calls/ui.protocol.json`. Do not edit by hand.

- owner crate: `crates/freehand-ui-protocol`
- owner module: `crates/freehand-ui-protocol/src/lib.rs`
- function map: `docs/function-maps/ui.protocol.md`
- generated wiki: `docs/wiki/ui.protocol.md`
- test design: `docs/testing/ui.protocol.md`

## Request Mainline

- UI commands enter one protocol truth shared by CLI and WebUI
- UI acts as an input ingress only; command submission does not make UI a truth writer
- command ingress acceptance is explicit and route-scoped: only mutation-intent commands may enter the ingress transport path
- accepted command ingress is wrapped into a dispatch envelope that declares the target owner feature/module before leaving the protocol boundary
- runtime-owned mutation commands such as checkpoint rewind stay explicit at the protocol envelope layer and do not become UI-owned semantics
- query and subscribe stay separate
- subscriptions may target latest active turn, specific turn, specific turn debug state, or node/progress streams

## Response Mainline

- query returns snapshots
- checkpoint query returns read-only checkpoint summary projections supplied by runtime owner code
- command ingress returns explicit dispatch receipt without claiming truth mutation success
- subscribe returns an initial snapshot followed by continuous incremental projections through a protocol-owned subscription channel
- projections are read-only views over owner-written truth
- terminal completion shows only final projected text
- public conversation projection strips raw completion schema blocks and excludes reasoning, usage, provider payload, and debug details from the main user-visible stream
- debug state is projected as a read-only per-turn snapshot/stream with summary text plus ordered detail lines
- `ui.protocol` may ingest observation-only debug events from `debug.core` receivers and materialize only the snapshot projection into protocol state
- `ui.protocol` may ingest shared semantic/tool/usage/terminal/error contracts incrementally and update one turn projection without depending on `freehand-reason`
- slave turn may surface as WebUI-only separate card while staying in one protocol truth
- client-specific projection gating stays inside the protocol owner, not in apps
- UI must be able to consume reason-turn state and debug-state projections without owning either truth source
- transport adapters may drain debug receivers and query protocol snapshots, but projection ownership stays in `freehand-ui-protocol`

## Error Mainline

- invalid command, invalid stream selection, or unavailable source projection return explicit protocol errors
- query/subscribe commands sent to command-ingress route are explicit protocol misuse errors
- empty checkpoint rewind ids are rejected at the protocol boundary before runtime dispatch
- checkpoint query misses return an empty read-only snapshot, not an implicit recovery or filesystem fallback
- source identity fields remain explicit across success and error paths
- UI-side commands may request mutations, but mutation success/failure is decided by owner modules and reflected back as projections or errors

## Shared Multi-Reference Functions

- `terminal_text_projection`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: collapse terminal event to final user-visible text
  - allowed callers: query handlers, stream handlers, CLI/WebUI adapters
  - related tests: terminal result projection smoke
  - why shared: ensures CLI and WebUI project the same terminal text truth
- `public_conversation_items`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: derive user-visible conversation items from a full turn projection without exposing internal reasoning/debug/raw schema data
  - allowed callers: CLI/WebUI renderers, transport adapters
  - related tests: public conversation projection smoke
  - why shared: all UI clients need one public projection rule instead of per-client filtering
- `turn_projection_for_client`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: gate slave substream visibility by UI client kind without changing turn truth
  - allowed callers: CLI/WebUI adapters, query handlers
  - related tests: slave turn subscription smoke
  - why shared: keeps client-specific projection rules centralized and protocol-owned
- `DebugStateSnapshot::new`
  - owner: `crates/freehand-debug/src/lib.rs`
  - purpose: construct reusable debug snapshots consumed by UI protocol without making UI the debug owner
  - allowed callers: reason/provider/node/testkit/UI protocol adapters
  - related tests: debug state query/subscription smoke
  - why shared: keeps debug projection shape in `debug.core` instead of duplicating it inside UI protocol
- `accept_command_ingress`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: validate mutation-intent command ingress and return explicit acknowledgement without mutating truth
  - allowed callers: CLI/WebUI transport adapters
  - related tests: command ingress acceptance/rejection smoke
  - why shared: keeps ingress-route semantics inside the protocol owner instead of duplicating them in apps
- `build_command_dispatch_envelope`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: route accepted ingress command to the declared owner feature/module before dispatch
  - allowed callers: CLI/WebUI transport adapters, runtime owner adapters
  - related tests: command dispatch envelope owner-routing smoke
  - why shared: keeps command-to-owner routing out of app transport glue
- `checkpoint_projection_from_runtime_summary`
  - owner: `crates/freehand-ui-protocol/src/lib.rs`
  - purpose: convert runtime-owned checkpoint summaries into UI-safe read-only projection rows
  - allowed callers: runtime dispatcher bridge, app query handlers through protocol state
  - related tests: checkpoint summary query smoke
  - why shared: keeps checkpoint UI projection single-sourced without letting UI parse runtime manifests

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `validate_command` | `crates/freehand-ui-protocol/src/lib.rs` | accept and validate UI command payload | UI command | validated command | CLI/WebUI | protocol boundary | bound |
| 02 | `accept_command_ingress` | `crates/freehand-ui-protocol/src/lib.rs` | accept only mutation-intent ingress commands and return explicit ack | UI command | ingress ack | CLI/WebUI transport adapters | protocol boundary | bound |
| 03 | `protocol_rejection` | `crates/freehand-ui-protocol/src/lib.rs` | convert protocol error into transport-safe rejection payload | protocol error | rejection payload | CLI/WebUI transport adapters | protocol boundary | bound |
| 04 | `build_command_dispatch_envelope` | `crates/freehand-ui-protocol/src/lib.rs` | wrap accepted ingress command with declared owner routing | UI command | dispatch envelope | CLI/WebUI transport adapters | protocol boundary | bound |
| 05 | `UiProtocolState::query` | `crates/freehand-ui-protocol/src/lib.rs` | execute read-only query path | query command | snapshot projection | protocol boundary | query handler | bound |
| 06 | `UiProtocolState::subscribe` | `crates/freehand-ui-protocol/src/lib.rs` | expose the protocol-owned continuous subscription channel for app transports | none | UiSubscriptionEvent receiver | app/transport adapters | protocol state | bound |
| 07 | `subscription_selector` | `crates/freehand-ui-protocol/src/lib.rs` | build read-only subscribe selector | subscribe command | subscription selector | protocol boundary | stream handler | bound |
| 08 | `subscription_matches` | `crates/freehand-ui-protocol/src/lib.rs` | route incremental projection to matching subscription | subscription selector plus projection | delivery decision | stream handler | selector matcher | bound |
| 09 | `turn_projection_from_events` | `crates/freehand-ui-protocol/src/lib.rs` | project whole-turn state into UI snapshot | semantic/tool/usage/terminal/error inputs | UI turn projection | query/stream handler | projector | bound |
| 10 | `terminal_text_projection` | `crates/freehand-ui-protocol/src/lib.rs` | project terminal text | terminal semantic payload | UI terminal text | query/stream handler | projector | bound |
| 10a | `public_conversation_items / public_turn_projection` | `crates/freehand-ui-protocol/src/lib.rs` | derive public user-visible conversation stream and strip raw completion schema | full turn projection | public turn projection | app transports/renderers | projector | bound |
| 11 | `UiProtocolState::apply_semantic_event / apply_tool_call / apply_usage_event / apply_terminal_event / apply_error_event` | `crates/freehand-ui-protocol/src/lib.rs` | incrementally update one turn projection from shared contract events and publish subscription updates | shared reason/error contracts | updated queryable/subscribable turn projection | runtime/debug bridges | protocol state | bound |
| 12 | `turn_projection_for_client` | `crates/freehand-ui-protocol/src/lib.rs` | gate client-specific slave substream visibility | turn projection plus client kind | client-specific turn projection | CLI/WebUI adapter | projector | bound |
| 13 | `UiProtocolState::set_debug_state` | `crates/freehand-ui-protocol/src/lib.rs` | store per-turn read-only debug projection for UI consumption and publish subscription updates | freehand-debug snapshot | queryable/subscribable debug state | reason/node/debug bridge | protocol state | bound |
| 14 | `UiProtocolState::apply_debug_event` | `crates/freehand-ui-protocol/src/lib.rs` | ingest one observation-only debug event into UI protocol state when a snapshot is present | freehand-debug event | updated per-turn debug state or ignored event | reason/node/debug bridge | protocol state | bound |
| 15 | `UiProtocolState::drain_debug_receiver` | `crates/freehand-ui-protocol/src/lib.rs` | drain a debug.core receiver without making UI a truth writer | debug receiver | applied snapshot count | protocol transport/app adapters | protocol state | bound |
| 16 | `debug_projection_from_event` | `crates/freehand-ui-protocol/src/lib.rs` | map one debug event to read-only UI debug projection when snapshot exists | freehand-debug event | UiProjection::Debug | protocol tests/transport adapters | projector | bound |
| 17 | `UiProtocolState::set_checkpoint_snapshot / checkpoint_projection_from_runtime_summary` | `crates/freehand-ui-protocol/src/lib.rs` | store and query read-only checkpoint summaries supplied by runtime owner | runtime checkpoint summary DTO | checkpoint query result | runtime dispatcher / app query handlers | protocol state | bound |

## Sync Status Against Mainline Call

- command validation, query selection, subscription routing, turn projection, and debug-state projection are bound in code
- command ingress acceptance, dispatch-envelope routing, and rejection payload mapping are now bound in code
- checkpoint rewind is now a protocol-owned mutation-intent command routed to `runtime.checkpoint-rewind`
- checkpoint summary projection/query is read-only protocol state and code-bound
- client-specific projection gating is now also bound in code
- UiProtocolState now owns a continuous subscription channel plus incremental shared-contract turn projection updates
- debug-state projection consumes freehand-debug::DebugStateSnapshot instead of a UI-owned duplicate DTO
- UI ingress versus truth-writer separation is now locked in the function map
- minimal per-turn debug-state query/subscribe plus receiver-drain bridge are now bound in UiProtocolState
- public turn projection is protocol-owned
