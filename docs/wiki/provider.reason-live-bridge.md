# Wiki: `provider.reason-live-bridge`

Generated from `docs/mainline-calls/provider.reason-live-bridge.json`. Do not edit by hand.

- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- function map: `docs/function-maps/provider.reason-live-bridge.md`
- generated wiki: `docs/wiki/provider.reason-live-bridge.md`
- test design: `docs/testing/provider.reason-live-bridge.md`

## Resource Operation Backlinks

- request_context.build_provider_request

## Request Mainline

- selected agent config enters the runtime-owned live bridge with one bound primary provider and one optional config-validated fallback provider
- live bridge restores or creates the requested session through `ReasonPersistence` before round execution
- the original operator task enters the planner as an `original-task` `TaskContract` segment with a content-derived admission budget instead of a fixed tiny prompt cap
- multi-round carryover retains completion contract, control status contract, runtime tool guidance, and `TaskContract` before volatile previous-visible-output or schema feedback, using content-derived admission budgets instead of arbitrary small runtime caps
- when an existing session is restored, live bridge rebuilds `reason.session-history` base context from effective persisted turns before the next round starts, so same-session follow-up requests include prior user/assistant turn truth
- restored prompt context keeps only the latest round for each repaired logical turn, so superseded failed repair attempts stay in ledgers/UI truth but do not enter the next default prompt context
- runtime emits restore lifecycle debug snapshots through `debug.core` without request text
- live bridge bootstraps one shared metadata ledger for runtime-owned lifecycle facts plus delegated `reason.turn` producer writes
- bridge derives provider descriptor and executor config from the active primary/fallback route without exposing provider wire DTOs; Anthropic output budget stays at the adapter default `DEFAULT_ANTHROPIC_MAX_TOKENS=8192` without a smaller live-bridge cap
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- retryable non-stream HTTP/network failure retries on the primary route; after primary exhaustion, or immediately for failover-eligible non-retryable HTTP status such as 402, the bridge switches once to the configured fallback at the provider-neutral semantic request boundary
- runtime writes provider-request lifecycle metadata without request payload text before executor IO
- runtime emits provider-request lifecycle debug snapshots through `debug.core` without provider payload text
- the first master tool-capable request exposes the Reasonix-aligned master-safe registry subset through provider-neutral request metadata
- the master-safe registry subset omits unrestricted shell scope and exports the matching deterministic schema fingerprint stamped into planner diagnostics
- Anthropic live executor runs the HTTP/SSE request through raw-capable callbacks so runtime can capture debug-only provider raw bodies/events before semantic parsing
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completed provider tool calls are classified by registry execution scope; framework tools remain available to the Master; read-only path tools may inspect readable external paths; writable tools remain locked to the current agent cwd; unrestricted shell is unavailable on Master/Worker provider surfaces and returns a paired failed result if injected; incomplete `tool_use` calls and execution failures become paired failed tool-result exchanges, while writable in-root tool calls first go through runtime checkpoint preview/snapshot/execute gating before success or execution-failure results are written back through `ReasonTurnEngine::apply_provider_output`, persisted, and sent to the next Anthropic request
- runtime writes tool execution lifecycle metadata without tool-result content before tool-result re-entry
- runtime emits tool execution lifecycle debug snapshots through `debug.core` without tool-result content
- completion schema is parsed only after terminal-candidate finish reasons; schema/no-schema mismatch is rejected with field-level feedback as a model response-polishing pattern, not provider failure, or used to schedule the next round
- runtime writes terminal lifecycle metadata before terminal persistence
- runtime emits terminal lifecycle debug snapshots through `debug.core` before terminal persistence
- runtime dispatch callers may consume the same bridge through CLI or daemon command ingress without owning provider DTOs

## Response Mainline

- every provider raw response/error/event body retained in debug mode is written through `ReasonPersistence::record_provider_raw_event` into the debug-only provider ledger
- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- when fallback succeeds, the active turn provider payload and persisted model truth use the fallback model, while the primary error and explicit provider.failover_from/provider.failover_to routing metadata remain queryable
- every applied live semantic output is recorded through `ReasonPersistence::record_provider_output_applied`
- tool-result re-entry is recorded in turn truth and persisted before the next provider request; execution failures remain model-visible failed tool results, not terminal runtime failures
- master workspace-boundary failures and forbidden shell calls remain paired failed tool results so the model can create and assign external work through `task`
- completed/blocked schema writes terminal truth through `ReasonTurnEngine::submit_completion`
- terminal turns are materialized through `ReasonPersistence::record_turn_closed`
- schema retry exhaustion writes blocked terminal truth through `ReasonTurnEngine::block_turn`; provider interruption without a completion-schema candidate writes interrupted terminal truth through `ReasonTurnEngine::interrupt_turn`
- runtime drains both reason-owned and runtime-owned debug snapshots through one shared `DebugHub` hook path
- bridge returns final turn truth, all round turns, captured broadcast events, schema rejection ledger, tool execution count, restore status, and live-output summary without leaking wire DTOs
- runtime callers project the final turn into `UiProtocolState` from one shared runtime owner path

## Error Mainline

- unsupported provider type/protocol is rejected at the bridge boundary
- provider execution failures are classified with concrete error codes and recorded through error.center; retryable non-stream primary failures retry up to ten attempts before one configured fallback switch, failover-eligible non-retryable HTTP failures switch immediately, and fallback exhaustion materializes one failed turn
- adapter, callback, local invalid-config, schema, tool-content, and persistence errors do not activate provider fallback
- stream execution never switches providers because partial semantic output/tool calls cannot be safely replayed without a typed rollback/resume contract
- invalid or missing completion schema is rejected with field-level feedback and retried up to 3 times before blocked terminal truth, not provider failed terminal truth
- master shell or external-workspace attempts are execution-policy failures returned to the model with task/worker instructions; they do not execute, expose external content, or terminalize the turn
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

| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 01 | `run_live_reason_turn` | `crates/freehand-runtime/src/lib.rs` | compose config-selected provider execution with one reason turn | selected agent config plus prompt plus stream mode | turn truth plus broadcast capture plus output summary | CLI/runtime dispatch/tests | live bridge owner |  |  |  | bound |
| 02 | `ReasonPersistence::restore` | `crates/freehand-reason/src/persistence.rs` | restore existing authoritative session truth | runtime home plus agent plus session id | session history plus prior turns or explicit missing truth | live bridge | persistence owner |  |  |  | bound |
| 03 | `MetadataCenter::with_ledger_path` | `crates/freehand-metadata/src/lib.rs` | bootstrap shared runtime metadata ledger before live rounds start | runtime home plus agent plus session id | metadata center with replay-safe prior records or explicit metadata error | live bridge | metadata owner |  |  |  | bound |
| 04 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned restore lifecycle metadata without request text | restore outcome plus stream/provider facts | durable runtime metadata record | live bridge | metadata owner |  |  |  | bound |
| 05 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned restore lifecycle debug snapshot without request text | restore outcome plus stream/provider facts | runtime-owned debug event | live bridge | debug.core |  |  |  | bound |
| 06 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one round turn and provider payload while stamping runtime-owned tool-schema fingerprint into planner diagnostics | session history plus prompt plus optional tool-schema fingerprint | initialized turn record | live bridge | reason owner |  |  |  | bound |
| 07 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | persist live round start | session history plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner |  |  |  | bound |
| 08 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor plus provider payload | provider semantic request | live bridge | provider semantic owner | request_context | provider_request | request_context.build_provider_request | bound |
| 09 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned provider-request lifecycle metadata without payload text | round ordinal plus provider/model/tool-count control facts | durable runtime metadata record | live bridge | metadata owner |  |  |  | bound |
| 10 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned provider-request lifecycle debug snapshot without payload text | round ordinal plus provider/model/tool-count control facts | runtime-owned debug event | live bridge | debug.core |  |  |  | bound |
| 11 | `build_live_provider_driver` | `crates/freehand-runtime/src/lib.rs` | select the provider driver abstraction from config without exposing provider wire DTOs to the live loop | selected provider type/protocol plus auth/base URL | boxed provider-neutral live driver or explicit unsupported provider error | live bridge | provider driver factory |  |  |  | bound |
| 12 | `LiveProviderDriver::execute_stream_with_raw` | `crates/freehand-runtime/src/lib.rs` | execute the provider-selected streaming request through one provider-neutral driver interface while provider crates own protocol-specific wire rendering/parsing | provider semantic request plus provider-neutral raw/output callbacks | incremental raw event bodies plus incremental semantic output batches plus accumulated outputs | live bridge | provider driver abstraction |  |  |  | bound |
| 13 | `record_live_provider_raw` | `crates/freehand-runtime/src/lib.rs` | translate provider-neutral raw captures into runtime-owned provider-raw ledger writes | raw response/error/event body plus session/turn/trace identity | provider raw ledger write or explicit persistence failure | provider driver callback path | live bridge owner |  |  |  | bound |
| 14 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger evidence | provider family plus session/turn/trace identity plus scene provenance plus raw body | durable provider raw debug evidence | live bridge | persistence owner |  |  |  | bound |
| 15 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record plus broadcast or explicit provider-output apply error | live bridge | reason owner |  |  |  | bound |
| 16 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | persist live semantic output application | session history plus active turn plus provider-neutral output | reason ledger row plus active-turn snapshot | live bridge | persistence owner |  |  |  | bound |
| 17 | `BuiltinToolRegistry::reasonix_aligned / execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | export master-safe tool schemas, enforce runtime-home and shell boundaries, keep task delegation available, and route writable in-root calls through checkpoint gating | complete tool call | success, paired policy or execution failure for model continuation, or explicit system/checkpoint error | live bridge | tool registry owner |  |  |  | bound |
| 18 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned tool execution metadata without tool-result content | tool name plus tool call id plus round ordinal | durable runtime metadata record | live bridge | metadata owner |  |  |  | bound |
| 19 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned tool execution lifecycle debug snapshot without tool-result content | tool name plus tool call id plus round ordinal | runtime-owned debug event | live bridge | debug.core |  |  |  | bound |
| 20 | `parse_completion_submission_block` | `crates/freehand-blocks/src/lib.rs` | parse tagged completion schema from model text | model text | typed submission or schema rejection list | live bridge | blocks owner |  |  |  | bound |
| 21 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | persist schema rejection evidence | schema rejection plus active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner |  |  |  | bound |
| 22 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner |  |  |  | bound |
| 23 | `ReasonTurnEngine::block_turn / ReasonTurnEngine::interrupt_turn` | `crates/freehand-reason/src/lib.rs` | write non-failed terminal truth after schema retry exhaustion or provider interruption | retry-exhausted or interrupted summary | blocked or interrupted terminal event | live bridge | reason owner |  |  |  | bound |
| 24 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned terminal lifecycle metadata before terminal persistence | round/tool/schema-rejection counters plus final terminal status | durable runtime metadata record | live bridge | metadata owner |  |  |  | bound |
| 25 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned terminal lifecycle debug snapshot before terminal persistence | round/tool/schema-rejection counters plus final terminal status | runtime-owned debug event | live bridge | debug.core |  |  |  | bound |
| 26 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | materialize terminal live turn | terminal turn truth | closed turn snapshot plus sidecars/index | live bridge | persistence owner |  |  |  | bound |
| 19 | `record_provider_error_metadata` | `crates/freehand-runtime/src/lib.rs` | write provider executor error-code and retry metadata when AnthropicExecutorError is returned | executor error classification plus retry index/cap plus turn identity | durable provider and error-center metadata records | live bridge | metadata owner |  |  |  | bound |
| 20 | `emit_provider_retry_debug` | `crates/freehand-runtime/src/lib.rs` | emit provider executor retry/error debug snapshot when AnthropicExecutorError is returned | executor error code plus retry index/cap plus turn identity | runtime-owned debug event | live bridge | debug.core |  |  |  | bound |
| 27 | `run_live_provider_reason_turn / provider_executor_retry_plan / materialize_provider_executor_failure` | `crates/freehand-runtime/src/lib.rs` | retry recoverable non-stream primary failures up to ten attempts, switch once to the configured fallback after eligible primary exhaustion at the provider-neutral request boundary, and convert inapplicable or exhausted fallback failure into one runtime-classified failed turn | provider executor error plus active route plus optional fallback plus retry plan plus active turn | retry continuation, explicit route-switch metadata plus fallback semantic request, or persisted failed closed turn with concrete provider error code | live bridge executor error path | config, error, reason, and persistence owners |  |  |  | bound |
| 28 | `rebuild_session_history_from_effective_turns / effective_turn_context_segments` | `crates/freehand-runtime/src/lib.rs` | convert effective persisted same-session turns into session-memory base context before the next restored live request starts, keeping only the latest round for each repaired logical turn | restored session history plus effective persisted turns | resume-rebuild session history with prior user/assistant turn memory and without superseded failed repair attempts in prompt context | live bridge restore path | reason.session-history |  |  |  | bound |

## Sync Status Against Mainline Call

- current live path supports Anthropic `messages`, OpenAI-compatible `responses`, and OpenAI-compatible `chat_completions` through one provider-neutral runtime driver abstraction
- runtime owner path preserves incremental stream apply, completion schema loop, persistence, master-safe registry-backed tool loop, matching tool-schema fingerprint wiring, shared metadata-ledger producer wiring, runtime-owned debug snapshot emission, and checkpoint gating without duplicating adapter semantics
- runtime live bridge now bootstraps one shared metadata ledger and writes restore/request/tool/terminal lifecycle metadata without request-text leakage
- runtime live bridge now emits restore/request/tool/terminal lifecycle debug snapshots through `debug.core` without prompt, provider-payload, or tool-result leakage
- runtime live bridge now retains Anthropic raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` without promoting them into authoritative turn/session truth
- restored same-session follow-up requests rebuild `reason.session-history` base context from effective persisted turns before the next round starts, so the next provider request includes prior user/assistant turn truth
- repaired multi-round logical turns keep raw failed attempts in persisted turn/UI truth but admit only the latest repaired round into future prompt context by default
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution and before terminal persistence
- completion/control-status response mismatch is a model response-polishing pattern: nullable optional fields are absent, non-null wrong types remain field-level mismatches, and retry truth must not be projected as provider failure or fail_turn
- tool execution result failures, including missing-file read failures and unknown tool names, are surfaced as `ToolResultStatus::Failed` tool-result re-entry truth, sent to Anthropic with `is_error=true`, and do not materialize runtime error or failed terminal truth by themselves
- master external-workspace and forbidden-shell attempts follow the same paired failed tool-result path with explicit task/worker delegation guidance and no external content leakage
- provider executor and transport failures are distinct from schema mismatch polishing and tool execution result failures: recoverable non-stream primary failures retry up to ten attempts and then switch once to the configured fallback; fallback exhaustion materializes one failed terminal turn with a concrete provider error code and no active turn before dispatch error return
- OpenAI 402 and other failover-eligible non-retryable HTTP status errors switch immediately to fallback without hiding the primary error; primary success and adapter/callback/content failures never activate fallback
- stream failover remains intentionally unsupported until partial-output rollback/resume is a typed contract
- runtime metadata write failures are explicit `RuntimeLiveBridgeError::MetadataFailed` errors and abort the live bridge before fallback or silent continuation
- provider raw ledger write failures are explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed` errors and abort the live bridge before semantic success is reported
- runtime white-box coverage now locks long operator task admission through the live bridge: `original-task` is a `TaskContract`, its budget scales with actual prompt content, and the provider request preserves the prompt tail sentinel
- runtime white-box coverage now proves multi-round provider re-entry retains status schema and runtime tool guidance
- CLI and daemon now both consume the runtime-owned bridge instead of `freehand-testkit`
- generated wiki must be regenerated from `docs/mainline-calls/provider.reason-live-bridge.json` when this function-map truth changes
- provider executor retry/error metadata is now written through `record_provider_error_metadata` at both single-shot and stream error return paths via pipeline node `RuntimeLive05ProviderError`
