# Function Map: `provider.reason-live-bridge`

- feature_id: `provider.reason-live-bridge`
- owner crate: `crates/freehand-runtime`
- owner module: `crates/freehand-runtime/src/lib.rs`
- mainline call source: `docs/mainline-calls/provider.reason-live-bridge.json`
- generated wiki: `docs/wiki/provider.reason-live-bridge.md`
- resource map: `docs/resource-maps/core.json`
- resource operations:
  - `request_context.build_provider_request`
  - `provider_request.select_hosted_search`
- owner entry symbols:
  - `run_live_reason_turn`
  - `LiveReasonExecutionRole::hosted_tool_definitions`
  - `build_live_provider_driver`
  - `CompositeProviderExecutorFactory::build_executor`

## Resource Map Binding

- resource map: `docs/resource-maps/core.json`
- owned resources:
  - `provider_request`
- touched resources:
  - `request_context`
  - `provider_hosted_search`
- resource operations:
  - `request_context.build_provider_request`
  - `provider_request.select_hosted_search` (`provider_request` -> `provider_hosted_search`)
- forbidden shortcuts:
  - Runtime must not patch provider payloads from turn truth without going through typed request context.
  - Instruction manifests must not patch provider requests directly.
  - Runtime must not render OpenAI Responses hosted search wire; it may only select provider-neutral `ProviderHostedToolDefinition` values before adapter IO.

## Request Mainline

- selected agent config enters the runtime-owned live bridge with one bound primary provider and one optional config-validated fallback provider
- live bridge restores or creates the requested session through `ReasonPersistence` before round execution
- when restoring an existing session, live bridge rebuilds future prompt context from effective persisted turns and keeps only the latest round for each repaired logical turn, so superseded failed repair attempts stay in ledgers/UI truth but do not enter the next default prompt context
- the original operator task enters the planner as an `original-task` `TaskContract` segment with a budget derived from actual content size plus margin; runtime must not use a fixed tiny prompt cap that rejects ordinary multi-step master instructions before provider execution
- every Master live round includes a volatile `TaskSpaceSnapshot` segment before
  the original task, sourced from task owner `TaskRuntime::query_task_space_snapshot`
  as a bounded read-only projection over task snapshots, AgentLifecycle, and
  newest master-visible events, so the model can see known tasks, configured
  Worker, valid status filters, blocked/review-ready ids, and recent framework
  events before it calls task query/list/history tools
- every live round also receives a runtime-owned volatile `CurrentTime` segment
  containing UTC RFC3339, local RFC3339, and Unix seconds; search reasoning must
  use this harness clock for relative dates and recency instead of assuming the
  model knows wall-clock time
- multi-round carryover retains completion contract, control status contract, runtime tool guidance, and `TaskContract` before volatile `previous-visible-output` or schema feedback; runtime must not impose arbitrary small input caps such as 512 tokens on model-visible context, while the planner remains responsible for rejecting context that truly exceeds the model/context policy
- live context segment construction is owned by
  `crates/freehand-runtime/src/live_context.rs`; `src/lib.rs` remains the live
  bridge orchestrator and consumes `base_live_context_segments` /
  `base_live_context_segments_with_observer` instead of owning the segment
  builders inline
- path and symlink diagnostics are owned by
  `crates/freehand-runtime/src/path_diagnostics.rs`; the live bridge and Worker
  runner consume the same diagnostic text instead of duplicating path-expansion
  logic inline
- Anthropic request rendering uses the provider adapter default output budget `DEFAULT_ANTHROPIC_MAX_TOKENS=8192`; runtime must not add a smaller ad hoc output cap in the live bridge
- master task creation/dispatch is model-decided from the runtime prompt contract, the exposed `task` tool schema, tool field descriptions, dispatch/no-dispatch conditions, workspace-boundary rule, concurrency/flow-control guidance, and model-visible samples for cross-workspace dispatch, success, execution error, and rejected-review retry; runtime must not infer task creation from prose outside the tool call path
- runtime emits restore lifecycle debug snapshots through `debug.core` without request text
- runtime emits context-planning started/completed lifecycle observations before
  the first provider round starts, without request text or provider payload
  content; first-round live context construction also emits segment-level
  started/completed/failed observations for completion, control, tool guidance,
  instruction capability, task-space snapshot, and original-task segments, so
  UI/debug clients can identify the exact pre-provider segment instead of a
  silent pending submit
- bridge derives provider descriptor and executor config from the active primary/fallback route, then obtains a provider-core `ProviderLiveExecutor` through `freehand-provider-executors` assembly without exposing concrete provider wire DTOs to the live loop
- bridge derives provider-hosted search capability from config/provider descriptor and execution profile; OpenAI Responses and Anthropic Messages providers with `web_search=auto` can declare hosted search, while disabled or protocol-unsupported combinations leave it absent
- `reason.turn` may start multiple rounds under one logical live request when completion schema says `continue` or when schema rejection requires same-task retry
- provider semantic request is built from each round's turn-owned provider payload
- retryable non-stream HTTP/network failure retries on the primary route; after primary exhaustion, or immediately for failover-eligible non-retryable HTTP status such as 402, the bridge switches once to the configured fallback at the provider-neutral semantic request boundary
- actual retry and fallback-switch progress is projected as transient model-request transport substate on the same turn before the bridge sleeps, retries, or enters the fallback route; a later provider semantic response replaces it, while retry/error-center evidence remains outside public conversation truth
- the first master tool-capable request exposes the Reasonix-aligned
  master-safe registry subset through provider-neutral request metadata:
  locked local workspace tools, concrete-URL `web_fetch`, plus `task` and
  `timer`
- the first master request may also declare provider-hosted `web_search` metadata when the selected provider supports hosted search mixed with function tools; this is provider-native search, not a Freehand function tool
- the master-safe registry subset omits unrestricted shell/browser/broad
  `web_search` tools, `todo_write`, and `complete_step`, and exports the
  matching deterministic schema fingerprint stamped into planner diagnostics
- search-only hosted providers must not be mixed into the Master function-tool request; broad/current search on such providers is routed through `TaskExecutionProfile::CleanSearch` Worker turns with zero Freehand function tools
- the first master-task-capable request tells the model to use local workspace
  tools directly when the current selected session cwd is enough, to dispatch
  only for different-cwd/isolated/concurrent/long-running/resumable work, to
  use `web_fetch` directly for known HTTP/HTTPS URLs, and to block only when
  neither Master nor configured Workers/provider-hosted capabilities expose
  the required concrete capability, or when broad search/browser behavior is
  required but neither provider-hosted search nor a `clean_search` Worker is
  available
- the first master-task-capable request includes owner-scoped task orchestration guidance and exact JSON examples such as `task({"op":"create",...})` instead of pseudo-call syntax or standalone semantic-action tool names
- the Master task snapshot guidance tells the model not to call
  `status="all"` and to use the injected current framework truth before
  exploratory task/agent calls, reducing multi-round tool probing
- Worker runtime tool guidance names the exact Worker-safe tool surface from
  `tool.registry`, states that all path tools are locked to the canonical task
  cwd, forbids `shell`/`bash`/`readlink`/`pwd`/`cat`/`find` guesses, and gives
  first-call path patterns such as `ls` before `read_file`; Worker capability
  guidance also names `web_fetch` for known HTTP/HTTPS URLs
- sourced-search hosted/camo/social failure enters exactly one recovery round;
  that round removes hosted search and normal camo tools and exposes only
  concrete-URL `web_fetch`. A successful fetch is discovery evidence, not
  verification, and still requires camo; a failed or unusable recovery writes
  Blocked immediately without another provider round.
- runtime emits provider-request lifecycle debug snapshots through `debug.core` without provider payload text
- provider-core `ProviderLiveExecutor` runs concrete HTTP/SSE executor requests through raw-capable callbacks owned by the adapter crates so runtime can capture debug-only provider raw bodies/events before semantic parsing
- stream mode applies outputs incrementally through the executor callback path before the provider response completes
- completed provider tool calls are classified by registry execution scope;
  Master local workspace read/search/write/edit tools execute only in the
  current selected session cwd under `tool.registry` path and checkpoint
  locks; Master concrete-URL `web_fetch` executes through the tool registry
  network scope; Master `task` and `timer` remain framework tools; injected
  Master shell, browser, broad `web_search`, `todo_write`, or `complete_step`
  calls return a paired failed capability-boundary result with exact
  local-vs-dispatch guidance and no file-content leak; Worker
  read/search/write/mutation path tools remain locked
  to the task cwd after absolute-normalization and symlink/canonical
  resolution, and boundary failures tell the model to use relative in-cwd paths
  or return blocked instead of probing external roots; Worker `web_fetch`
  executes only concrete HTTP/HTTPS URL reads, not broad search; Worker
  shell/unknown-tool failures include the exact Worker tool list and forbid invented
  `shell`/`readlink` style calls; incomplete `tool_use` calls are converted into
  failed tool-result re-entry truth instead of schema retry; writable in-root
  Worker calls first go through runtime checkpoint preview/snapshot/execute
  gating, then success or execution-failure results are written back through
  `ReasonTurnEngine::apply_provider_output`, persisted, and sent to the next
  Anthropic request as a paired tool result exchange
- runtime emits tool execution lifecycle debug snapshots and error-center
  metadata through `debug.core`/`metadata.core` without tool-result content; the
  full model-visible recovery text stays in reason tool-result truth only
- completion schema is parsed only when the provider finish reason is a terminal completion candidate such as `stop` or `end_turn`; it is then validated and either accepted, rejected with field-level feedback plus UI-visible retry waiting projection, or used to schedule the next round; provider terminal truth is accepted as terminal without schema
- runtime emits terminal lifecycle debug snapshots through `debug.core` before terminal persistence
- runtime dispatch callers may consume the same bridge through CLI or daemon command ingress without owning provider DTOs

## Response Mainline

- every provider raw response/error/event body retained in debug mode is written through `ReasonPersistence::record_provider_raw_event` into the debug-only provider ledger
- provider-neutral outputs are applied back into the active round through `ReasonTurnEngine::apply_provider_output`
- when fallback succeeds, the active turn provider payload and persisted model truth use the fallback model, while the primary error and explicit `provider.failover_from`/`provider.failover_to` routing metadata remain queryable
- recovered retry/failover turns contain no provider semantic error event; the temporary provider recovery activity is cleared by the normal response/terminal projection
- every applied live semantic output is recorded through `ReasonPersistence::record_provider_output_applied`
- tool-result re-entry is recorded in turn truth and persisted before the next provider request; execution failures remain model-visible failed tool results, not terminal runtime failures; runtime publishes a model-continuation waiting event after tool results are paired for the next provider request
- master capability-boundary failures for unavailable tools remain paired
  failed tool results, so the model can either continue with local workspace
  tools or create and assign different-cwd/external work through `task`
- completed/blocked schema may write terminal truth through `ReasonTurnEngine::submit_completion`; provider terminal truth is also accepted as terminal without schema
- terminal turns are materialized through `ReasonPersistence::record_turn_closed`
- schema retry exhaustion no longer blocks solely on missing completion/search schema; it closes with provider terminal truth when present
- sourced-search recovery exhaustion no longer blocks solely on missing typed evidence; after bounded recovery attempts it closes with provider terminal truth
- runtime drains both reason-owned and runtime-owned debug snapshots through one shared `DebugHub` hook path
- bridge returns final turn truth, all round turns, captured broadcast events, schema rejection ledger, tool execution count, restore status, and live-output summary without leaking wire DTOs
- runtime callers project the final turn into `UiProtocolState` from one shared runtime owner path

## Error Mainline

- unsupported provider type/protocol is rejected at the bridge boundary
- provider execution failures are classified with concrete error codes and recorded through `error.center`; retryable non-stream primary failures retry up to ten attempts before one configured fallback switch, failover-eligible non-retryable HTTP failures switch immediately, and fallback exhaustion materializes one failed turn
- adapter, callback, local invalid-config, schema, tool-content, and persistence errors do not activate provider fallback
- stream execution never switches providers because partial semantic output/tool calls cannot be safely replayed without a typed rollback/resume contract
- invalid or missing completion/control-status schema is a normal
  response-schema mismatch pattern, not a provider failure: nullable optional
  fields are treated as absent, non-null wrong types receive type-aware
  field-level feedback, and the model may polish the response for up to 3
  consecutive terminal-candidate responses; after bounded retries the bridge
  closes with provider terminal truth when present instead of fabricating a
  blocked/failed outcome
- master shell or external-workspace attempts are execution-policy failures returned to the model with task/worker instructions; they do not execute, expose external content, or terminalize the turn
- non-terminal completion-schema rejection retries publish a waiting projection so UI clients can show that repair feedback was sent to the model
- non-terminal control-status rejection uses the same persisted response-schema
  retry truth and waiting projection; it must not return
  `ProviderRequestBuildFailed`
- incomplete tool calls are not executed as successful side effects
- incomplete `tool_use` responses are paired back to the model as failed tool results; they must not become schema retries or terminal runtime failures
- writable tools without preview/checkpoint support are rejected explicitly
- unknown tool names and registered but unimplemented tool names return explicit
  failed tool results paired to the original tool call with exact role-specific
  tool-surface guidance, so the model can continue the turn without guessing
  schema/tool names
- runtime system errors, including provider transport errors after retry exhaustion, persistence failures, metadata failures, checkpoint infrastructure failures, and provider-output apply failures, remain explicit terminal bridge errors and are not converted into tool results
- provider executor transport failures materialize `ErrorErr01RuntimeClassified` plus failed terminal truth with the concrete provider error code through the active turn before returning dispatch failure, so UI/ADP clients see a closed failed turn instead of a hanging active turn
- provider-output apply failures from `reason.turn` are returned as explicit `RuntimeLiveBridgeError::ProviderOutputApplyFailed`
- provider raw debug-ledger write failures are returned as explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed`
- persistence restore/write failures fail the live bridge explicitly
- provider terminal metadata is final turn truth when no completion schema is available; accepted Freehand completion schema remains the authoritative semantic summary when present

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
  - purpose: convert provider-neutral raw captures into provider-family-tagged `reason.persistence` debug-ledger rows with scene provenance
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
| 05a | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned context-planning and context-segment lifecycle metadata without request text | role, stream mode, cwd binding, configured-worker count, context segment id/status/inclusion/elapsed facts, segment count, estimated token budget | durable runtime metadata record | live bridge | metadata owner | bound |
| 05b | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned context-planning and context-segment lifecycle debug snapshots without request text | context planning started/completed facts plus segment started/completed/failed facts | runtime-owned debug event consumable as model-waiting UI activity | live bridge | `debug.core` | bound |
| 05c | `live_context::base_live_context_segments` / `live_context::base_live_context_segments_with_observer` | `crates/freehand-runtime/src/live_context.rs` | build the typed completion, control, tool-guidance, instruction-capability, bounded task-space, and original-task context segments outside the live bridge orchestrator, optionally emitting owner-safe segment build events | original prompt + role + configured Worker set + runtime home + cwd + agent id | ordered base live context segments plus optional started/completed/failed segment observations, or explicit owner error | `run_live_provider_reason_turn` / `next_round_segments` | context, instruction, and task owners | bound |
| 06 | `ReasonTurnEngine::start_turn` | `crates/freehand-reason/src/lib.rs` | create one round turn and provider payload | session history + prompt | initialized turn record | live bridge | reason owner | bound |
| 07 | `ReasonPersistence::record_turn_started` | `crates/freehand-reason/src/persistence.rs` | persist live round start | session history + active turn | reason ledger row + active-turn snapshot | live bridge | persistence owner | bound |
| 08 | `build_semantic_request` | `crates/freehand-provider-core/src/lib.rs` | build provider-neutral request | provider descriptor + provider payload | provider semantic request | live bridge | provider semantic owner | bound |
| 09 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned provider-request lifecycle metadata without payload text | round ordinal + provider/model/tool-count control facts | durable runtime metadata record | live bridge | metadata owner | bound |
| 10 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned provider-request lifecycle debug snapshot without payload text | round ordinal + provider/model/tool-count control facts | runtime-owned debug event | live bridge | `debug.core` | bound |
| 11 | `build_live_provider_driver` / `CompositeProviderExecutorFactory::build_executor` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-provider-executors/src/lib.rs` | select a provider-core live executor from config through the provider executor assembly without runtime depending on concrete adapter crates | selected provider type/protocol + descriptor/auth/base URL | boxed `ProviderLiveExecutor` or explicit unsupported/build error | live bridge | provider-core executor factory assembly | bound |
| 12 | `ProviderLiveExecutor::execute_once_with_raw` / `ProviderLiveExecutor::execute_stream_with_raw` | `crates/freehand-provider-core/src/lib.rs` | execute the provider-selected request through one provider-core trait while provider adapter crates own protocol-specific wire rendering/parsing | provider semantic request + provider-neutral raw/output callbacks | provider semantic outputs plus callback-visible provider-neutral raw capture | live bridge | provider-core executor trait | bound |
| 13 | `record_live_provider_raw` | `crates/freehand-runtime/src/lib.rs` | translate provider-neutral raw captures into runtime-owned provider-raw ledger writes | raw response/error/event body + session/turn/trace identity | provider raw ledger write or explicit persistence failure | provider driver callback path | live bridge owner | bound |
| 14 | `ReasonPersistence::record_provider_raw_event` | `crates/freehand-reason/src/persistence.rs` | append debug-only provider raw ledger evidence | provider family + session/turn/trace identity + scene provenance + raw body | durable provider raw debug evidence | live bridge | persistence owner | bound |
| 15 | `ReasonTurnEngine::apply_provider_output` | `crates/freehand-reason/src/lib.rs` | write provider-neutral outputs into turn truth | provider semantic output | updated turn record + broadcast or explicit provider-output apply error | live bridge | reason owner | bound |
| 16 | `ReasonPersistence::record_provider_output_applied` | `crates/freehand-reason/src/persistence.rs` | persist live semantic output application | session history + active turn + provider-neutral output | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 17 | `BuiltinToolRegistry::reasonix_aligned` / `pending_tool_calls_for_execution` / `execute_registry_tool_call` | `crates/freehand-runtime/src/lib.rs` | export Reasonix-aligned tool schemas, select the latest unexecuted tool call per id, and route writable tool calls through runtime checkpoint gating before execute | complete or incomplete tool call | success or failed tool-result re-entry, or explicit system/checkpoint error | live bridge | tool registry owner | bound |
| 18 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned tool execution metadata without tool-result content | tool name + tool call id + round ordinal | durable runtime metadata record | live bridge | metadata owner | bound |
| 19 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned tool execution lifecycle debug snapshot without tool-result content | tool name + tool call id + round ordinal | runtime-owned debug event | live bridge | `debug.core` | bound |
| 20 | `turn_has_completion_candidate_finish_reason` / `parse_completion_submission_block` | `crates/freehand-runtime/src/lib.rs` / `crates/freehand-blocks/src/lib.rs` | gate completion parsing on terminal-candidate finish reason before parsing tagged completion schema from model text | provider finish reason + model text | typed submission or schema rejection list | live bridge | runtime + blocks owner | bound |
| 21 | `ReasonPersistence::record_completion_rejected` | `crates/freehand-reason/src/persistence.rs` | persist schema rejection evidence | schema rejection + active turn | reason ledger row plus active-turn snapshot | live bridge | persistence owner | bound |
| 22 | `ReasonTurnEngine::submit_completion` | `crates/freehand-reason/src/lib.rs` | write accepted completed/blocked terminal truth | validated completion submission | terminal event | live bridge | reason owner | bound |
| 23 | `ReasonTurnEngine::block_turn` / `ReasonTurnEngine::interrupt_turn` | `crates/freehand-reason/src/lib.rs` | write non-failed terminal truth for schema retry exhaustion or provider interruption without a completion-schema candidate | retry-exhausted or interrupted summary | blocked or interrupted terminal event | live bridge | reason owner | bound |
| 24 | `write_live_bridge_metadata` | `crates/freehand-runtime/src/lib.rs` | write runtime-owned terminal lifecycle metadata before terminal persistence | round/tool/schema-rejection counters + final terminal status | durable runtime metadata record | live bridge | metadata owner | bound |
| 25 | `emit_live_bridge_debug` | `crates/freehand-runtime/src/lib.rs` | emit runtime-owned terminal lifecycle debug snapshot before terminal persistence | round/tool/schema-rejection counters + final terminal status | runtime-owned debug event | live bridge | `debug.core` | bound |
| 26 | `ReasonPersistence::record_turn_closed` | `crates/freehand-reason/src/persistence.rs` | materialize terminal live turn | terminal turn truth | closed turn snapshot + sidecars/index | live bridge | persistence owner | bound |
| 27 | `run_live_provider_reason_turn` / `record_provider_error_metadata` / `provider_executor_retry_plan` / `sleep_provider_retry` / `materialize_provider_executor_failure` | `crates/freehand-runtime/src/lib.rs` | classify provider executor/transport failures, retry recoverable non-stream primary attempts up to ten times, wait between retries through cancel-token-aware backoff, switch once to a configured fallback at the provider-neutral request boundary after eligible primary exhaustion, and materialize failed truth only when no fallback applies or the fallback fails | provider executor error + active route + optional fallback + active turn + cancel token | retry metadata, interruptible retry wait, explicit route-switch metadata plus fallback semantic request, or persisted failed/cancelled closed turn | live bridge executor error path | config/error/reason/persistence owners | bound |
| 28 | `rebuild_session_history_from_effective_turns` / `effective_turn_context_segments` | `crates/freehand-runtime/src/lib.rs` | convert effective persisted same-session turns into session-memory base context before the next restored live request starts, keeping only the latest round for each repaired logical turn | restored session history + effective persisted turns | resume-rebuild session history with prior user/assistant turn memory and without superseded failed repair attempts in prompt context | live bridge restore path | `reason.session-history` | bound |

## Sync Status Against Code

- current live path supports Anthropic `messages`, OpenAI-compatible `responses`, and OpenAI-compatible `chat_completions` through provider-core `ProviderLiveExecutor`; runtime no longer has direct `freehand-provider-openai` or `freehand-provider-anthropic` dependency edges
- current live path selects provider-hosted search only as provider-neutral hosted tool metadata; OpenAI Responses and Anthropic Messages wire rendering remain adapter-owned
- runtime owner path preserves incremental stream apply, completion schema loop, persistence, registry-backed tool loop, tool-schema fingerprint wiring, shared metadata-ledger producer wiring, runtime-owned debug snapshot emission, checkpoint gating, and shared path/symlink diagnostics without duplicating adapter or path semantics
- runtime live bridge now bootstraps one shared metadata ledger and writes restore/request/tool/terminal lifecycle metadata without request-text leakage
- runtime live bridge now emits restore/request/tool/terminal lifecycle debug snapshots through `debug.core` without prompt, provider-payload, or tool-result leakage
- runtime live bridge now retains provider-neutral raw response/error/event bodies through `ReasonPersistence::record_provider_raw_event` without promoting them into authoritative turn/session truth
- restored same-session follow-up requests now rebuild `reason.session-history` base context from effective persisted turns before the next round starts, so the next provider request includes prior user/assistant turn truth
- repaired multi-round logical turns keep raw failed attempts in persisted turn/UI truth but admit only the latest repaired round into future prompt context by default
- runtime live bridge cancellation checkpoints now have positive and negative coverage before tool execution, before terminal persistence, and during provider retry backoff sleep
- tool execution result failures, including missing-file read failures and unknown tool names, are expected to surface as `ToolResultStatus::Failed` tool-result re-entry truth, be sent to the next Anthropic request with `is_error=true`, and must not materialize runtime error or failed terminal truth by themselves
- provider executor/transport failures are distinct from tool execution result failures and schema mismatch polishing: recoverable non-stream primary errors retry up to ten attempts and then switch once to the configured fallback; fallback exhaustion materializes one failed terminal turn with a concrete code and no active turn before dispatch error return
- OpenAI 402 and other failover-eligible non-retryable HTTP status errors switch immediately to fallback without hiding the primary error; primary success and adapter/callback/content failures never activate fallback
- stream failover remains intentionally unsupported until partial-output rollback/resume is a typed contract
- runtime white-box coverage now explicitly locks failed-tool-result multi-round continuation, incomplete `tool_use` returning a failed tool result with zero schema retries, and keeps provider/metadata/persistence/checkpoint failures as system/runtime errors
- runtime dispatcher failure projection now preserves already-restored other-session transcripts while replacing only the failed session turns after provider/system failure recovery
- runtime white-box coverage now locks the original task as `TaskContract` and proves multi-round re-entry retains status/tool/task contract guidance
- runtime white-box coverage now explicitly locks master-autonomous task tool loops through mock provider responses: success dispatch closes after review approval, execution error becomes blocked without success close, and rejected review enters recovering before resubmission/approval/close
- runtime tool-result text for structured task execution facts is derived from the Task Center event type, so `review_ready`, `blocked`, and `recovering` worker results are paired back to the model with their owner-visible lifecycle semantics
- runtime metadata write failures are explicit `RuntimeLiveBridgeError::MetadataFailed` errors and abort the live bridge before fallback or silent continuation
- provider raw ledger write failures are explicit `RuntimeLiveBridgeError::ReasonPersistenceFailed` errors and abort the live bridge before semantic success is reported
- CLI and daemon now both consume the runtime-owned bridge instead of `freehand-testkit`
- the generated wiki must be regenerated from `docs/mainline-calls/provider.reason-live-bridge.json` when this function-map truth changes
