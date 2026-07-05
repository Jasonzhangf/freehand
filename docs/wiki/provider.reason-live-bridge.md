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
- runtime emits restore lifecycle debug snapshots through `debug.core` without request text
- live bridge bootstraps one shared metadata ledger for runtime-owned lifecycle facts plus delegated `reason.turn` producer writes
- bridge derives provider descriptor and executor config from selected provider truth
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- runtime writes provider-request lifecycle metadata without request payload text before executor IO
- runtime emits provider-request lifecycle debug snapshots through `debug.core` without provider payload text
- the first tool-capable request exposes a Reasonix-aligned runtime tool registry through provider-neutral request metadata
- the same runtime tool registry exports one deterministic implemented-schema fingerprint that is stamped into planner diagnostics before provider request build
- Anthropic live executor runs the HTTP/SSE request through raw-capable callbacks so runtime can capture debug-only provider raw bodies/events before semantic parsing
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completed provider tool calls are executed by `freehand-tools`; incomplete `tool_use` calls and execution failures become paired failed tool-result exchanges, while writable tool calls first go through runtime checkpoint preview/snapshot/execute gating before success or execution-failure results are written back through `ReasonTurnEngine::apply_provider_output`, persisted, and sent to the next Anthropic request
- runtime writes tool execution lifecycle metadata without tool-result content before tool-result re-entry
- runtime emits tool execution lifecycle debug snapshots through `debug.core` without tool-result content
- completion schema is parsed only after terminal-candidate finish reasons; schema/no-schema mismatch is rejected with field-level feedback as a model response-polishing pattern, not provider failure, or used to schedule the next round
- runtime writes terminal lifecycle metadata before terminal persistence
- runtime emits terminal lifecycle debug snapshots through `debug.core` before terminal persistence
- runtime dispatch callers may consume the same bridge through CLI or daemon command ingress without owning provider DTOs

## Response Mainline

- every provider raw response/error/event body retained in debug mode is written through `ReasonPersistence::record_provider_raw_event` into the debug-only provider ledger
- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- every applied live semantic output is recorded through `ReasonPersistence::record_provider_output_applied`
- tool-result re-entry is recorded in turn truth and persisted before the next provider request; execution failures remain model-visible failed tool results, not terminal runtime failures
- completed/blocked schema writes terminal truth through `ReasonTurnEngine::submit_completion`
- terminal turns are materialized through `ReasonPersistence::record_turn_closed`
- schema retry exhaustion writes blocked terminal truth through `ReasonTurnEngine::block_turn`; provider interruption without a completion-schema candidate writes interrupted terminal truth through `ReasonTurnEngine::interrupt_turn`
- runtime drains both reason-owned and runtime-owned debug snapshots through one shared `DebugHub` hook path
- bridge returns final turn truth, all round turns, captured broadcast events, schema rejection ledger, tool execution count, restore status, and live-output summary without leaking wire DTOs
- runtime callers project the final turn into `UiProtocolState` from one shared runtime owner path

## Error Mainline

- unsupported provider type/protocol is rejected at the bridge boundary
- provider execution failures are classified with concrete error codes and retried up to five non-stream attempts with exponential backoff starting at 1 second before failed terminal truth is materialized
- invalid or missing completion schema is rejected with field-level feedback and retried up to 3 times before blocked terminal truth, not provider failed terminal truth
- incomplete tool calls are not executed and do not become tool-result truth
- writable tools without preview/checkpoint support are rejected explicitly
- unknown tool names and registered but unimplemented tool names return explicit failed tool results paired to the original tool call so the model can continue the turn
- runtime system errors, including provider transport errors, persistence failures, metadata failures, checkpoint infrastructure failures, and provider-output apply failures, remain explicit terminal bridge errors and are not converted into tool results
- provider executor transport failures materialize ErrorErr01RuntimeClassified plus failed terminal truth with the concrete provider error code through the active turn before returning dispatch failure, so UI/ADP clients see a closed failed turn instead of a hanging active turn
- provider-output apply failures from `reason.turn` are returned as explicit `RuntimeLiveBridgeError::ProviderOutputApplyFailed`
- metadata ledger bootstrap and metadata write failures are returned as explicit `RuntimeLiveBridgeError::MetadataFailed` errors
- provider raw debug-ledger write failures are returned as explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed`
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
- `MetadataCenter::with_ledger_path`
  - owner: `crates/freehand-metadata/src/lib.rs`
  - purpose: bootstrap one shared durable metadata center for runtime-owned bridge lifecycle facts plus delegated `reason.turn` producer writes
  - allowed callers: runtime live bridge, tests
  - related tests: live bridge metadata ledger smoke, live bridge metadata write failure smoke
  - why shared: keeps metadata ledger bootstrap and replay inside `metadata.core` instead of runtime-local file maps
- `emit_live_bridge_debug`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: publish runtime-owned restore/request/tool/terminal lifecycle debug snapshots through `debug.core` without leaking request payload or tool-result content
  - allowed callers: runtime live bridge, tests
  - related tests: live bridge runtime debug hook smoke, live bridge tool debug smoke
  - why shared: keeps runtime-owned provider-boundary observation formatting in one owner instead of duplicating per-stage debug event assembly
- `record_live_provider_raw`
  - owner: `crates/freehand-runtime/src/lib.rs`
  - purpose: convert Anthropic executor raw captures into provider-family-tagged `reason.persistence` debug-ledger rows with scene provenance
  - allowed callers: runtime live bridge, tests
  - related tests: live bridge provider raw ledger smoke, live bridge provider raw ledger failure smoke
  - why shared: keeps provider raw retention mapping in one runtime-owned bridge helper instead of duplicating body/event-to-ledger translation per call site

## Function Call Table

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | compose config-selected provider execution with one reason turn | selected agent config plus prompt plus stream mode | turn truth plus broadcast capture plus output summary | CLI/runtime dispatch/tests | live bridge owner | bound |
| 02 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | restore existing authoritative session truth | runtime home plus agent plus session id | session history plus prior turns or explicit missing truth | live bridge | persistence owner | bound |
| 03 | `MetadataCenter::with_ledger_path` | `crates/freehand-metadata/src/lib.rs` | bootstrap shared runtime metadata ledger before live rounds start | runtime home plus agent plus session id | metadata center with replay-safe prior records or explicit metadata error | live bridge | metadata owner | bound |
| 04 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned restore lifecycle metadata without request text | restore outcome plus stream/provider facts | durable runtime metadata record | live bridge | metadata owner | bound |
| 05 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned restore lifecycle debug snapshot without request text | restore outcome plus stream/provider facts | runtime-owned debug event | live bridge | debug.core | bound |
| 06 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one round turn and provider payload while stamping runtime-owned tool-schema fingerprint into planner diagnostics | session history plus prompt plus optional tool-schema fingerprint | initialized turn record | live bridge | reason owner | bound |
| 07 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | persist live round start | session history plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 08 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor plus provider payload | provider semantic request | live bridge | provider semantic owner | bound |
| 09 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned provider-request lifecycle metadata without payload text | round ordinal plus provider/model/tool-count control facts | durable runtime metadata record | live bridge | metadata owner | bound |
| 10 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned provider-request lifecycle debug snapshot without payload text | round ordinal plus provider/model/tool-count control facts | runtime-owned debug event | live bridge | debug.core | bound |
| 11 | `AnthropicExecutor::execute_once_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one non-stream Anthropic request and expose raw response/error body before semantic parsing | provider semantic request plus auth/base URL plus raw callback | provider semantic outputs plus callback-visible raw body/error body | live bridge | anthropic executor | bound |
| 12 | `AnthropicExecutor::execute_stream_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one stream Anthropic request and call back per raw event and per semantic batch before completion | provider semantic request plus auth/base URL plus raw callback plus semantic callback | incremental raw event bodies plus incremental semantic output batches plus accumulated outputs | live bridge | anthropic executor | bound |
| 13 | `record_live_provider_raw` | `crates/freehand-runtime/src/lib.rs` | translate Anthropic raw captures into runtime-owned provider-raw ledger writes | raw response/error/event body plus session/turn/trace identity | provider raw ledger write or explicit persistence failure | anthropic executor callback path | live bridge owner | bound |
| 14 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger evidence | provider family plus session/turn/trace identity plus scene provenance plus raw body | durable provider raw debug evidence | live bridge | persistence owner | bound |
| 15 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record plus broadcast or explicit provider-output apply error | live bridge | reason owner | bound |
| 16 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | persist live semantic output application | session history plus active turn plus provider-neutral output | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 17 | `BuiltinToolRegistry::reasonix_aligned / execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | export Reasonix-aligned tool schemas and route writable tool calls through runtime checkpoint gating before execute | complete tool call | success or failed tool-result re-entry, or explicit system/checkpoint error | live bridge | tool registry owner | bound |
| 18 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned tool execution metadata without tool-result content | tool name plus tool call id plus round ordinal | durable runtime metadata record | live bridge | metadata owner | bound |
| 19 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned tool execution lifecycle debug snapshot without tool-result content | tool name plus tool call id plus round ordinal | runtime-owned debug event | live bridge | debug.core | bound |
| 20 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text | typed submission or schema rejection list | live bridge | blocks owner | bound |
| 21 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | persist schema rejection evidence | schema rejection plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 22 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner | bound |
| 23 | `ReasonTurnEngine::block_turn / ReasonTurnEngine::interrupt_turn` | `crates/freehand-reason/src/lib.rs` | write non-failed terminal truth after schema retry exhaustion or provider interruption | retry-exhausted or interrupted summary | blocked or interrupted terminal event | live bridge | reason owner | bound |
| 24 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned terminal lifecycle metadata before terminal persistence | round/tool/schema-rejection counters plus final terminal status | durable runtime metadata record | live bridge | metadata owner | bound |
| 25 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned terminal lifecycle debug snapshot before terminal persistence | round/tool/schema-rejection counters plus final terminal status | runtime-owned debug event | live bridge | debug.core | bound |
| 26 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | materialize terminal live turn | terminal turn truth | closed turn snapshot plus sidecars/index | live bridge | persistence owner | bound |
| 19 | `record_provider_error_metadata` | `crates/freehand-runtime/src/lib.rs` | write provider executor error-code and retry metadata when AnthropicExecutorError is returned | executor error classification plus retry index/cap plus turn identity | durable provider and error-center metadata records | live bridge | metadata owner | bound |
| 20 | `emit_provider_retry_debug` | `crates/freehand-runtime/src/lib.rs` | emit provider executor retry/error debug snapshot when AnthropicExecutorError is returned | executor error code plus retry index/cap plus turn identity | runtime-owned debug event | live bridge | debug.core | bound |
| 27 | `provider_executor_retry_plan / materialize_provider_executor_failure` | `crates/freehand-runtime/src/lib.rs` | retry recoverable non-stream provider failures up to five attempts and convert exhausted/non-retryable provider executor failure into runtime-classified error truth plus failed terminal truth before returning dispatch failure | provider executor error plus retry plan plus active turn | retry continuation or persisted failed closed turn with concrete provider error code | live bridge executor error path | reason and persistence owners | bound |

## Sync Status Against Mainline Call

- current live path supports Anthropic `messages` only
- runtime owner path preserves incremental stream apply, completion schema loop, persistence, registry-backed tool loop, tool-schema fingerprint wiring, shared metadata-ledger producer wiring, runtime-owned debug snapshot emission, and checkpoint gating without duplicating adapter semantics
- runtime live bridge now bootstraps one shared metadata ledger and writes restore/request/tool/terminal lifecycle metadata without request-text leakage
- runtime live bridge now emits restore/request/tool/terminal lifecycle debug snapshots through `debug.core` without prompt, provider-payload, or tool-result leakage
- runtime live bridge now retains Anthropic raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` without promoting them into authoritative turn/session truth
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and before terminal persistence
- schema/no-schema response mismatch is a model response-polishing pattern: it remains CompletionSchemaRejected plus error.center repair_schema recovery action and must not be projected as provider failure or fail_turn
- tool execution result failures, including missing-file read failures and unknown tool names, are surfaced as `ToolResultStatus::Failed` tool-result re-entry truth, sent to Anthropic with `is_error=true`, and do not materialize runtime error or failed terminal truth by themselves
- provider executor and transport failures are distinct from schema mismatch polishing and tool execution result failures: recoverable non-stream failures retry up to five attempts, then materialize a failed terminal turn with a concrete provider error code and no active turn before the dispatch error is returned
- runtime metadata write failures are explicit `RuntimeLiveBridgeError::MetadataFailed` errors and abort the live bridge before fallback or silent continuation
- provider raw ledger write failures are explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed` errors and abort the live bridge before semantic success is reported
- CLI and daemon now both consume the runtime-owned bridge instead of `freehand-testkit`
- generated wiki must be regenerated from `docs/mainline-calls/provider.reason-live-bridge.json` when this function-map truth changes
- provider executor retry/error metadata is now written through `record_provider_error_metadata` at both single-shot and stream error return paths via pipeline node `RuntimeLive05ProviderError`
