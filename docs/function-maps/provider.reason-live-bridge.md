# Function Map: `provider.reason-live-bridge`

- feature_id: `provider.reason-live-bridge`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.reason-live-bridge.json`
- generated wiki: `docs/wiki/provider.reason-live-bridge.md`
- owner entry symbols:
  - `run_live_reason_turn`

## Request Mainline

- selected agent config enters the runtime-owned live bridge with one bound provider
- live bridge restores or creates the requested session through `ReasonPersistence` before round execution
- runtime emits restore lifecycle debug snapshots through `debug.core` without request text
- bridge derives provider descriptor and executor config from selected provider truth
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- the first tool-capable request exposes a Reasonix-aligned runtime tool registry through provider-neutral request metadata
- the same runtime tool registry exports one deterministic implemented-schema fingerprint that is stamped into planner diagnostics before provider request build
- runtime emits provider-request lifecycle debug snapshots through `debug.core` without provider payload text
- Anthropic live executor runs the HTTP/SSE request through raw-capable callbacks so runtime can capture debug-only provider raw bodies/events before semantic parsing
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completed provider tool calls are executed by `freehand-tools`; writable tool calls first go through runtime checkpoint preview/snapshot/execute gating, then are written back through `ReasonTurnEngine::apply_provider_output`, persisted, and sent to the next Anthropic request as a tool result exchange
- runtime emits tool execution lifecycle debug snapshots through `debug.core` without tool-result content
- completion schema is parsed from tagged text, validated, and either accepted, rejected with field-level feedback, or used to schedule the next round
- runtime emits terminal lifecycle debug snapshots through `debug.core` before terminal persistence
- runtime dispatch callers may consume the same bridge through CLI or daemon command ingress without owning provider DTOs

## Response Mainline

- every provider raw response/error/event body retained in debug mode is written through `ReasonPersistence::record_provider_raw_event` into the debug-only provider ledger
- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- every applied live semantic output is recorded through `ReasonPersistence::record_provider_output_applied`
- tool-result re-entry is recorded in turn truth and persisted before the next provider request
- completed/blocked schema writes terminal truth through `ReasonTurnEngine::submit_completion`
- terminal turns are materialized through `ReasonPersistence::record_turn_closed`
- retry exhaustion writes failed terminal truth through `ReasonTurnEngine::fail_turn`
- runtime drains both reason-owned and runtime-owned debug snapshots through one shared `DebugHub` hook path
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
- provider-output apply failures from `reason.turn` are returned as explicit `RuntimeLiveBridgeError::ProviderOutputApplyFailed`
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
| 01 | `run_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | compose config-selected provider execution with one reason turn | selected agent config + prompt + stream mode | turn truth + broadcast capture + output summary | CLI/runtime dispatch/tests | live bridge owner | bound |
| 02 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | restore existing authoritative session truth | runtime home + agent + session id | session history + prior turns or explicit missing truth | live bridge | persistence owner | bound |
| 03 | `MetadataCenter::with_ledger_path` | `crates/freehand-metadata/src/lib.rs` | bootstrap shared runtime metadata ledger before live rounds start | runtime home + agent + session id | metadata center with replay-safe prior records or explicit metadata error | live bridge | metadata owner | bound |
| 04 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned restore lifecycle metadata without request text | restore outcome + stream/provider facts | durable runtime metadata record | live bridge | metadata owner | bound |
| 05 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned restore lifecycle debug snapshot without request text | restore outcome + stream/provider facts | runtime-owned debug event | live bridge | `debug.core` | bound |
| 06 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one round turn and provider payload | session history + prompt | initialized turn record | live bridge | reason owner | bound |
| 07 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | persist live round start | session history + active turn | reason ledger row + active-turn snapshot | live bridge | persistence owner | bound |
| 08 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor + provider payload | provider semantic request | live bridge | provider semantic owner | bound |
| 09 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned provider-request lifecycle metadata without payload text | round ordinal + provider/model/tool-count control facts | durable runtime metadata record | live bridge | metadata owner | bound |
| 10 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned provider-request lifecycle debug snapshot without payload text | round ordinal + provider/model/tool-count control facts | runtime-owned debug event | live bridge | `debug.core` | bound |
| 11 | `AnthropicExecutor::execute_once_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one non-stream Anthropic request and expose raw response/error body before semantic parsing | provider semantic request + auth/base URL + raw callback | provider semantic outputs plus callback-visible raw body/error body | live bridge | anthropic executor | bound |
| 12 | `AnthropicExecutor::execute_stream_with_raw` | `crates/freehand-provider-anthropic/src/lib.rs` | execute one stream Anthropic request and call back per raw event and per semantic batch before completion | provider semantic request + auth/base URL + raw callback + semantic callback | incremental raw event bodies plus incremental semantic output batches plus accumulated outputs | live bridge | anthropic executor | bound |
| 13 | `record_live_provider_raw` | `crates/freehand-runtime/src/lib.rs` | translate Anthropic raw captures into runtime-owned provider-raw ledger writes | raw response/error/event body + session/turn/trace identity | provider raw ledger write or explicit persistence failure | anthropic executor callback path | live bridge owner | bound |
| 14 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger evidence | provider family + session/turn/trace identity + scene provenance + raw body | durable provider raw debug evidence | live bridge | persistence owner | bound |
| 15 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record + broadcast or explicit provider-output apply error | live bridge | reason owner | bound |
| 16 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | persist live semantic output application | session history + active turn + provider-neutral output | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 17 | `BuiltinToolRegistry::reasonix_aligned` / `execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | export Reasonix-aligned tool schemas and route writable tool calls through runtime checkpoint gating before execute | complete tool call | tool execution output or explicit tool error | live bridge | tool registry owner | bound |
| 18 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned tool execution metadata without tool-result content | tool name + tool call id + round ordinal | durable runtime metadata record | live bridge | metadata owner | bound |
| 19 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned tool execution lifecycle debug snapshot without tool-result content | tool name + tool call id + round ordinal | runtime-owned debug event | live bridge | `debug.core` | bound |
| 20 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text | typed submission or schema rejection list | live bridge | blocks owner | bound |
| 21 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | persist schema rejection evidence | schema rejection + active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 22 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner | bound |
| 23 | `ReasonTurnEngine::fail_turn` | `crates/freehand-reason/src/lib.rs` | write failed terminal truth after schema retry exhaustion | retry-exhausted failure summary | terminal event | live bridge | reason owner | bound |
| 24 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned terminal lifecycle metadata before terminal persistence | round/tool/schema-rejection counters + final terminal status | durable runtime metadata record | live bridge | metadata owner | bound |
| 25 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned terminal lifecycle debug snapshot before terminal persistence | round/tool/schema-rejection counters + final terminal status | runtime-owned debug event | live bridge | `debug.core` | bound |
| 26 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | materialize terminal live turn | terminal turn truth | closed turn snapshot + sidecars/index | live bridge | persistence owner | bound |

## Sync Status Against Code

- current live path supports Anthropic `messages` only
- runtime owner path preserves incremental stream apply, completion schema loop, persistence, registry-backed tool loop, tool-schema fingerprint wiring, shared metadata-ledger producer wiring, runtime-owned debug snapshot emission, and checkpoint gating without duplicating adapter semantics
- runtime live bridge now bootstraps one shared metadata ledger and writes restore/request/tool/terminal lifecycle metadata without request-text leakage
- runtime live bridge now emits restore/request/tool/terminal lifecycle debug snapshots through `debug.core` without prompt, provider-payload, or tool-result leakage
- runtime live bridge now retains Anthropic raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` without promoting them into authoritative turn/session truth
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and before terminal persistence
- unknown tool names and registered-but-unimplemented tool names are expected to surface as explicit `RuntimeLiveBridgeError::ToolExecutionFailed(...)` failures from the runtime-owned tool loop and must not materialize tool-result or terminal success truth
- runtime white-box coverage now explicitly locks both unknown-tool and registered-but-unimplemented-tool failure paths at the live bridge boundary
- runtime metadata write failures are explicit `RuntimeLiveBridgeError::MetadataFailed` errors and abort the live bridge before fallback or silent continuation
- provider raw ledger write failures are explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed` errors and abort the live bridge before semantic success is reported
- CLI and daemon now both consume the runtime-owned bridge instead of `freehand-testkit`
- the generated wiki must be regenerated from `docs/mainline-calls/provider.reason-live-bridge.json` when this function-map truth changes
