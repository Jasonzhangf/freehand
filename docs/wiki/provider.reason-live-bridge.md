# Wiki: `provider.reason-live-bridge`

Generated from `docs/mainline-calls/provider.reason-live-bridge.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/provider.reason-live-bridge.md`
- generated wiki: `docs/wiki/provider.reason-live-bridge.md`
- test design: `docs/testing/provider.reason-live-bridge.md`

## Request Mainline

- selected agent config enters the runtime-owned live bridge with one bound provider
- live bridge restores or creates the requested session through `ReasonPersistence` before round execution
- bridge derives provider descriptor and executor config from selected provider truth
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- the first tool-capable request exposes a Reasonix-aligned runtime tool registry through provider-neutral request metadata
- Anthropic live executor runs the HTTP/SSE request and returns provider-neutral semantic outputs for each round
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completed provider tool calls are executed by `freehand-tools`; writable tool calls first go through runtime checkpoint preview/snapshot/execute gating, then are written back through `ReasonTurnEngine::apply_provider_output`, persisted, and sent to the next Anthropic request as a tool result exchange
- completion schema is parsed from tagged text, validated, and either accepted, rejected with field-level feedback, or used to schedule the next round
- runtime dispatch callers may consume the same bridge through CLI or daemon command ingress without owning provider DTOs

## Response Mainline

- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- every applied live semantic output is recorded through `ReasonPersistence::record_provider_output_applied`
- tool-result re-entry is recorded in turn truth and persisted before the next provider request
- completed/blocked schema writes terminal truth through `ReasonTurnEngine::submit_completion`
- terminal turns are materialized through `ReasonPersistence::record_turn_closed`
- retry exhaustion writes failed terminal truth through `ReasonTurnEngine::fail_turn`
- bridge returns final turn truth, all round turns, captured broadcast events, schema rejection ledger, tool execution count, restore status, and live-output summary without leaking wire DTOs
- runtime callers project the final turn into `UiProtocolState` from one shared runtime owner path

## Error Mainline

- unsupported provider type/protocol is rejected at the bridge boundary
- provider execution failures are returned explicitly
- invalid or missing completion schema is rejected with field-level feedback and retried up to 3 times
- incomplete tool calls are not executed and do not become tool-result truth
- writable tools without preview/checkpoint support are rejected explicitly
- unknown tool names fail explicitly instead of being ignored
- registered but unimplemented tool names fail explicitly instead of being treated as successful fallback
- persistence restore/write failures fail the live bridge explicitly
- provider terminal metadata does not become final completion truth without accepted Freehand completion schema

## Shared Multi-Reference Functions

- `build_semantic_request`
  - owner: `crates/freehand-provider-core/src/lib.rs`
  - purpose: convert turn-owned provider payload plus provider descriptor into provider-neutral request truth
  - allowed callers: runtime bridges, tests
  - related tests: provider semantic request tests, live bridge request build tests
  - why shared: keeps provider-neutral request ownership centralized
- `ReasonPersistence::restore`
  - owner: `crates/freehand-reason/src/persistence.rs`
  - purpose: recover authoritative session truth before live execution
  - allowed callers: reason runtime/harness owners only
  - related tests: reason persistence restore tests, live bridge restore tests
  - why shared: live and smoke recovery must use the same authoritative truth path

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | compose config-selected provider execution with one reason turn | selected agent config plus prompt plus stream mode | turn truth plus broadcast capture plus output summary | CLI/runtime dispatch/tests | live bridge owner | bound |
| 02 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | restore existing authoritative session truth | runtime home plus agent plus session id | session history plus prior turns or explicit missing truth | live bridge | persistence owner | bound |
| 03 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one round turn and provider payload | session history plus prompt | initialized turn record | live bridge | reason owner | bound |
| 04 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | persist live round start | session history plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 05 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor plus provider payload | provider semantic request | live bridge | provider semantic owner | bound |
| 06 | `AnthropicExecutor::execute_once` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one non-stream Anthropic request | provider semantic request plus auth/base URL | provider semantic outputs | live bridge | anthropic executor | bound |
| 07 | `AnthropicExecutor::execute_stream_with` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one stream Anthropic request and call back per semantic batch before completion | provider semantic request plus auth/base URL plus callback | incremental semantic output batches plus accumulated outputs | live bridge | anthropic executor | bound |
| 08 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record plus broadcast | live bridge | reason owner | bound |
| 09 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | persist live semantic output application | session history plus active turn plus provider-neutral output | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 10 | `BuiltinToolRegistry::reasonix_aligned / execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | export Reasonix-aligned tool schemas and route writable tool calls through runtime checkpoint gating before execute | complete tool call | tool execution output or explicit tool error | live bridge | tool registry owner | bound |
| 11 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text | typed submission or schema rejection list | live bridge | blocks owner | bound |
| 12 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | persist schema rejection evidence | schema rejection plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 13 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner | bound |
| 14 | `ReasonTurnEngine::fail_turn` | `crates/freehand-reason/src/lib.rs` | write failed terminal truth after schema retry exhaustion | retry-exhausted failure summary | terminal event | live bridge | reason owner | bound |
| 15 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | materialize terminal live turn | terminal turn truth | closed turn snapshot plus sidecars/index | live bridge | persistence owner | bound |

## Sync Status Against Mainline Call

- current live path supports Anthropic `messages` only
- runtime owner path preserves incremental stream apply, completion schema loop, persistence, registry-backed tool loop, and checkpoint gating without duplicating adapter semantics
- CLI and daemon now both consume the runtime-owned bridge instead of `freehand-testkit`
- generated wiki must be regenerated from `docs/mainline-calls/provider.reason-live-bridge.json` when this function-map truth changes
